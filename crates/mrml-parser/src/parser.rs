//! The recursive-descent parser core.
//!
//! [`Parser`] walks the source as a byte cursor and builds an arena AST through an
//! [`AstBuilder`]. It never bails on the first error: malformed input produces a
//! [`Diagnostic`] and parsing continues with a best-effort recovery, so a (possibly
//! partial) [`Document`] is always returned.
//!
//! # Byte-cursor safety
//!
//! Scanning is byte-based, but every stopping delimiter (`<`, `>`, `"`, `'`, `=`,
//! `/`, `-`, ASCII whitespace) is ASCII. In UTF-8, ASCII bytes never appear inside a
//! multi-byte sequence (continuation bytes are all `>= 0x80`), so a scan that stops
//! at an ASCII delimiter always stops on a `char` boundary. Every `&str` slice is
//! therefore taken between two boundaries and is valid.

use mrml_allocator::Vec as ArenaVec;
use mrml_ast::{AstBuilder, Document, Node};
use mrml_diagnostics::Diagnostic;
use mrml_span::Span;
use mrml_syntax::MjmlTag;

/// A closing tag (`</name>`) that a nested [`Parser::parse_content`] surfaced to its
/// caller without consuming, because `name` matches an open element on the stack.
struct CloseInfo<'a> {
    /// The tag name found inside the closing tag.
    name: &'a str,
    /// Byte offset of the `<` that begins the closing tag.
    start: usize,
}

/// A recursive-descent parser over a UTF-8 source buffer.
pub struct Parser<'a> {
    /// The full source text.
    source: &'a str,
    /// `source` as raw bytes, for cheap cursor arithmetic.
    bytes: &'a [u8],
    /// The current byte offset into `source`.
    pos: usize,
    /// The AST builder threading the arena through the whole parse.
    builder: AstBuilder<'a>,
    /// Diagnostics accumulated during parsing (never fatal on their own).
    pub errors: std::vec::Vec<Diagnostic>,
    /// Set only on an internal invariant violation forcing a full stop.
    pub panicked: bool,
    /// Names of the elements currently open, outermost first.
    open_tags: std::vec::Vec<&'a str>,
}

impl<'a> Parser<'a> {
    /// Create a parser for `source`, building into `builder`'s arena.
    #[must_use]
    pub fn new(source: &'a str, builder: AstBuilder<'a>) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            pos: 0,
            builder,
            errors: std::vec::Vec::new(),
            panicked: false,
            open_tags: std::vec::Vec::new(),
        }
    }

    // ---- cursor helpers -------------------------------------------------

    /// The byte at the cursor, if any.
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    /// The byte `n` positions ahead of the cursor, if any.
    fn peek_at(&self, n: usize) -> Option<u8> {
        self.bytes.get(self.pos + n).copied()
    }

    /// Advance the cursor by one byte (no-op at EOF).
    fn bump(&mut self) {
        if self.pos < self.bytes.len() {
            self.pos += 1;
        }
    }

    /// Whether the remaining input starts with `s`.
    fn at_str(&self, s: &str) -> bool {
        self.bytes[self.pos..].starts_with(s.as_bytes())
    }

    /// Skip ASCII whitespace.
    fn skip_whitespace(&mut self) {
        while let Some(b) = self.peek() {
            if b.is_ascii_whitespace() {
                self.bump();
            } else {
                break;
            }
        }
    }

    /// Advance until the input is at `s` (consuming `s`), or to EOF if absent.
    fn skip_past_str(&mut self, s: &str) {
        while self.pos < self.bytes.len() {
            if self.at_str(s) {
                self.pos += s.len();
                return;
            }
            self.bump();
        }
    }

    /// Advance past the next `b` (consuming it), or to EOF if absent.
    fn skip_past_byte(&mut self, b: u8) {
        while let Some(c) = self.peek() {
            self.bump();
            if c == b {
                return;
            }
        }
    }

    /// Copy of the field so slices carry the `'a` source lifetime, not `self`'s.
    fn slice(&self, start: usize, end: usize) -> &'a str {
        let src: &'a str = self.source;
        &src[start..end]
    }

    /// Whether `b` may start a tag or attribute name.
    fn is_name_start(b: u8) -> bool {
        b.is_ascii_alphabetic() || b == b'_' || b == b':'
    }

    /// Whether `b` may continue a tag or attribute name.
    fn is_name_byte(b: u8) -> bool {
        b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':' | b'.')
    }

    /// Read a run of name bytes from the cursor, advancing past it.
    fn read_name(&mut self) -> &'a str {
        let start = self.pos;
        while let Some(b) = self.peek() {
            if Self::is_name_byte(b) {
                self.bump();
            } else {
                break;
            }
        }
        self.slice(start, self.pos)
    }

    // ---- entry ----------------------------------------------------------

    /// Parse the whole source into a [`Document`].
    pub fn parse_document(&mut self) -> Document<'a> {
        let mut children: ArenaVec<'a, Node<'a>> = self.builder.vec();
        loop {
            match self.parse_content(&mut children) {
                None => break,
                Some(close) => {
                    // No open element owns this closing tag: it is stray. (In
                    // practice `parse_content` already consumes stray closes itself,
                    // so this is a defensive belt-and-braces branch.)
                    self.pos = close.start;
                    let end = self.consume_close_tag();
                    self.errors.push(
                        Diagnostic::error(format!("unexpected closing tag `</{}>`", close.name))
                            .with_label(
                                Span::new(close.start as u32, end as u32),
                                "no matching open element",
                            ),
                    );
                }
            }
        }
        let span = Span::new(0, self.source.len() as u32);
        self.builder.document(children, span)
    }

    // ---- content loop ---------------------------------------------------

    /// Parse a run of sibling nodes into `out`.
    ///
    /// Returns `None` at end of input. Returns `Some(close)` when a closing tag whose
    /// name matches an element still open on the stack is reached; the closing tag is
    /// left unconsumed so the owning [`Parser::parse_element`] frame can handle it
    /// (either as its own close or as an implicit close that bubbles further up).
    fn parse_content(&mut self, out: &mut ArenaVec<'a, Node<'a>>) -> Option<CloseInfo<'a>> {
        loop {
            let c = self.peek()?;
            if c != b'<' {
                let node = self.parse_text();
                out.push(node);
                continue;
            }

            // Comment: `<!-- ... -->`.
            if self.at_str("<!--") {
                let node = self.parse_comment();
                out.push(node);
                continue;
            }

            // XML / processing-instruction prolog: `<?xml ... ?>`. Skipped, no node.
            if self.at_str("<?") {
                self.skip_past_str("?>");
                continue;
            }

            // Any other declaration, e.g. `<!doctype html>` (case-insensitive).
            // Skipped, no node.
            if self.at_str("<!") {
                self.skip_past_byte(b'>');
                continue;
            }

            // Closing tag: `</name>`.
            if self.at_str("</") {
                let start = self.pos;
                let name = self.peek_close_name();
                if self.open_tags.contains(&name) {
                    // Owned by this or an ancestor element: surface it unconsumed.
                    return Some(CloseInfo { name, start });
                }
                // Stray closing tag: report and skip it, then keep going.
                let end = self.consume_close_tag();
                self.errors.push(
                    Diagnostic::error(format!("unexpected closing tag `</{name}>`")).with_label(
                        Span::new(start as u32, end as u32),
                        "no matching open element",
                    ),
                );
                continue;
            }

            // Opening tag: `<name ...>`.
            if self.peek_at(1).is_some_and(Self::is_name_start) {
                let el = self.parse_element();
                out.push(self.builder.element_node(el));
                continue;
            }

            // A lone `<` that starts no tag: treat it as text.
            let node = self.parse_text();
            out.push(node);
        }
    }

    // ---- elements -------------------------------------------------------

    /// Parse an element starting at the cursor (which must be on its `<`).
    fn parse_element(&mut self) -> &'a mrml_ast::Element<'a> {
        let start = self.pos;
        self.bump(); // `<`
        let raw_tag = self.read_name();
        let tag = MjmlTag::from_str(raw_tag).unwrap_or(MjmlTag::Unknown);
        let attributes = self.parse_attributes();
        self.skip_whitespace();

        // Self-closing: `<mj-image ... />`.
        if self.at_str("/>") {
            self.pos += 2;
            let end = self.pos;
            return self.finish_element(tag, raw_tag, attributes, self.builder.vec(), start, end);
        }

        // Open-tag terminator `>`. If it is missing we are at EOF or malformed input;
        // recover by closing the element immediately.
        if self.peek() == Some(b'>') {
            self.bump();
        } else {
            self.errors.push(
                Diagnostic::error(format!("unclosed start tag `<{raw_tag}>`"))
                    .with_label(
                        Span::new(start as u32, self.pos as u32),
                        "start tag begins here",
                    )
                    .with_help("expected `>` or `/>`"),
            );
            let end = self.pos;
            return self.finish_element(tag, raw_tag, attributes, self.builder.vec(), start, end);
        }

        // Raw-content ("ending") elements: capture the body verbatim as one Text node
        // rather than parsing MJML children.
        if tag.is_ending_tag() {
            return self.parse_raw_element(tag, raw_tag, attributes, start);
        }

        // Normal container element: parse children until our closing tag.
        self.open_tags.push(raw_tag);
        let mut children: ArenaVec<'a, Node<'a>> = self.builder.vec();
        let end;
        match self.parse_content(&mut children) {
            None => {
                // EOF before a matching close: unclosed element.
                end = self.pos;
                self.push_unclosed(raw_tag, start);
                self.open_tags.pop();
            }
            Some(close) => {
                if close.name == raw_tag {
                    // Our own closing tag: consume it.
                    self.consume_close_tag();
                    end = self.pos;
                    self.open_tags.pop();
                } else {
                    // A closing tag for an ancestor: close here implicitly and leave
                    // the tag unconsumed for the ancestor frame.
                    end = close.start;
                    self.push_unclosed(raw_tag, start);
                    self.open_tags.pop();
                }
            }
        }
        self.finish_element(tag, raw_tag, attributes, children, start, end)
    }

    /// Parse the verbatim body of a raw-content element and its closing tag.
    fn parse_raw_element(
        &mut self,
        tag: MjmlTag,
        raw_tag: &'a str,
        attributes: ArenaVec<'a, mrml_ast::Attribute<'a>>,
        start: usize,
    ) -> &'a mrml_ast::Element<'a> {
        self.open_tags.push(raw_tag);
        let content_start = self.pos;

        // Scan verbatim until our own `</raw_tag>`, an ancestor's closing tag, or EOF.
        let content_end;
        let close;
        loop {
            match self.peek() {
                None => {
                    content_end = self.pos;
                    close = Close::Eof;
                    break;
                }
                Some(b'<') if self.at_str("</") => {
                    let name = self.peek_close_name();
                    if name == raw_tag {
                        content_end = self.pos;
                        close = Close::Own;
                        break;
                    }
                    if self.open_tags.contains(&name) {
                        content_end = self.pos;
                        close = Close::Ancestor;
                        break;
                    }
                    // A stray/unknown closing tag inside raw content is literal text.
                    self.bump();
                }
                Some(_) => self.bump(),
            }
        }

        let mut children: ArenaVec<'a, Node<'a>> = self.builder.vec();
        if content_end > content_start {
            let raw = self.slice(content_start, content_end);
            let text = self.builder.text(
                self.builder.atom(raw),
                Span::new(content_start as u32, content_end as u32),
            );
            children.push(self.builder.text_node(text));
        }

        let end = match close {
            Close::Own => {
                self.consume_close_tag();
                self.pos
            }
            Close::Ancestor | Close::Eof => {
                self.push_unclosed(raw_tag, start);
                content_end
            }
        };
        self.open_tags.pop();
        self.finish_element(tag, raw_tag, attributes, children, start, end)
    }

    /// Allocate an [`Element`](mrml_ast::Element) with the given fields.
    fn finish_element(
        &self,
        tag: MjmlTag,
        raw_tag: &'a str,
        attributes: ArenaVec<'a, mrml_ast::Attribute<'a>>,
        children: ArenaVec<'a, Node<'a>>,
        start: usize,
        end: usize,
    ) -> &'a mrml_ast::Element<'a> {
        self.builder.element(
            tag,
            self.builder.atom(raw_tag),
            attributes,
            children,
            Span::new(start as u32, end as u32),
        )
    }

    /// Push an "unclosed element" diagnostic labeled at the open tag.
    fn push_unclosed(&mut self, raw_tag: &str, start: usize) {
        self.errors.push(
            Diagnostic::error(format!("unclosed element `<{raw_tag}>`"))
                .with_label(Span::new(start as u32, (start + 1) as u32), "opened here")
                .with_help(format!("expected a matching `</{raw_tag}>`")),
        );
    }

    // ---- attributes -----------------------------------------------------

    /// Parse zero or more attributes up to the tag terminator.
    fn parse_attributes(&mut self) -> ArenaVec<'a, mrml_ast::Attribute<'a>> {
        let mut attrs = self.builder.vec();
        loop {
            self.skip_whitespace();
            match self.peek() {
                None => break,
                Some(b'>') => break,
                Some(b'/') if self.peek_at(1) == Some(b'>') => break,
                Some(b) if Self::is_name_start(b) => {
                    let attr = self.parse_attribute();
                    attrs.push(attr);
                }
                Some(_) => {
                    // Skip an unexpected byte so we always make progress.
                    self.bump();
                }
            }
        }
        attrs
    }

    /// Parse a single attribute: `name`, `name="v"`, `name='v'`, or `name=v`.
    fn parse_attribute(&mut self) -> mrml_ast::Attribute<'a> {
        let start = self.pos;
        let name = self.read_name();
        self.skip_whitespace();

        let value = if self.peek() == Some(b'=') {
            self.bump(); // `=`
            self.skip_whitespace();
            match self.peek() {
                Some(q @ (b'"' | b'\'')) => {
                    self.bump(); // opening quote
                    let v_start = self.pos;
                    while let Some(b) = self.peek() {
                        if b == q {
                            break;
                        }
                        self.bump();
                    }
                    let v_end = self.pos;
                    if self.peek() == Some(q) {
                        self.bump(); // closing quote
                    }
                    Some(self.slice(v_start, v_end))
                }
                _ => {
                    // Unquoted value: up to whitespace, `>`, or `/`.
                    let v_start = self.pos;
                    while let Some(b) = self.peek() {
                        if b.is_ascii_whitespace() || b == b'>' || b == b'/' {
                            break;
                        }
                        self.bump();
                    }
                    Some(self.slice(v_start, self.pos))
                }
            }
        } else {
            None // valueless (boolean) attribute
        };

        let span = Span::new(start as u32, self.pos as u32);
        // NOTE: the value is stored raw/unescaped; entity decoding (`&amp;` etc.) is
        // deferred to a later semantic pass. TODO(entities).
        self.builder.attribute(
            self.builder.atom(name),
            value.map(|v| self.builder.atom(v)),
            span,
        )
    }

    // ---- comments & text ------------------------------------------------

    /// Parse a `<!-- ... -->` comment. The value is the inner text, delimiters kept
    /// only in the span. An unterminated comment runs to EOF (best effort).
    fn parse_comment(&mut self) -> Node<'a> {
        let start = self.pos;
        self.pos += 4; // `<!--`
        let inner_start = self.pos;
        let mut inner_end = self.pos;
        let mut terminated = false;
        while self.pos < self.bytes.len() {
            if self.at_str("-->") {
                inner_end = self.pos;
                self.pos += 3;
                terminated = true;
                break;
            }
            self.bump();
        }
        if !terminated {
            inner_end = self.pos;
        }
        let inner = self.slice(inner_start, inner_end);
        let comment = self.builder.comment(
            self.builder.atom(inner),
            Span::new(start as u32, self.pos as u32),
        );
        self.builder.comment_node(comment)
    }

    /// Parse a text run. Whitespace is preserved verbatim (no trimming). Consumes at
    /// least one byte to guarantee progress, then runs to the next `<` or EOF.
    fn parse_text(&mut self) -> Node<'a> {
        let start = self.pos;
        self.bump(); // consume at least one byte (e.g. a lone `<`)
        while let Some(b) = self.peek() {
            if b == b'<' {
                break;
            }
            self.bump();
        }
        let value = self.slice(start, self.pos);
        let text = self.builder.text(
            self.builder.atom(value),
            Span::new(start as u32, self.pos as u32),
        );
        self.builder.text_node(text)
    }

    // ---- closing-tag helpers -------------------------------------------

    /// Read the name inside the closing tag at the cursor (`</name>`) without moving.
    fn peek_close_name(&self) -> &'a str {
        let mut i = self.pos + 2; // skip `</`
        let start = i;
        while let Some(&b) = self.bytes.get(i) {
            if Self::is_name_byte(b) {
                i += 1;
            } else {
                break;
            }
        }
        self.slice(start, i)
    }

    /// Consume a closing tag at the cursor (`</name ...>`) up to and including `>`.
    /// Returns the byte offset just past the consumed tag.
    fn consume_close_tag(&mut self) -> usize {
        self.pos += 2; // `</`
        while let Some(b) = self.peek() {
            if Self::is_name_byte(b) {
                self.bump();
            } else {
                break;
            }
        }
        self.skip_whitespace();
        if self.peek() == Some(b'>') {
            self.bump();
        }
        self.pos
    }
}

/// How a raw-content scan terminated.
enum Close {
    /// Found the element's own `</tag>`.
    Own,
    /// Hit a closing tag belonging to an ancestor (implicit close).
    Ancestor,
    /// Reached end of input.
    Eof,
}
