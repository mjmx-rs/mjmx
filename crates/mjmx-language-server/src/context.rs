//! What the cursor is sitting on.
//!
//! Completion and hover both need to answer "is the caret in a tag name, in an
//! attribute name, in an attribute value, or in text — and which element encloses
//! it?". The AST cannot answer that on its own: while the user is mid-keystroke the
//! buffer is usually *incomplete* (`<mj-sec`), which is exactly the moment
//! completion is requested. So this module runs a small, deliberately shallow
//! left-to-right scan of the text **before** the caret and reports the resulting
//! lexical state plus the stack of still-open tags.
//!
//! This is not a second parser: it builds no tree, resolves nothing, keeps no
//! attribute values, and is only ever asked about the caret. Everything semantic —
//! which children are legal, which attributes exist, what a component means — comes
//! from [`mjmx_linter::rules`] and [`crate::docs`].

use mjmx_syntax::MjmlTag;

/// The lexical position of the caret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorContext<'s> {
    /// The caret is typing a tag name, as in `<mj-sec|`.
    TagName {
        /// The partial name typed so far (may be empty, right after `<`).
        prefix: &'s str,
        /// Byte offset where the partial name starts (just after `<` or `</`).
        start: usize,
        /// The innermost element still open around the caret, if any.
        parent: Option<OpenTag<'s>>,
        /// Whether this is a closing tag (`</mj-…`).
        closing: bool,
    },
    /// The caret is in the attribute area of an opening tag, as in `<mj-image sr|`.
    AttributeName {
        /// The element whose opening tag the caret is inside.
        tag: OpenTag<'s>,
        /// The partial attribute name typed so far (may be empty).
        prefix: &'s str,
        /// Byte offset where the partial attribute name starts.
        start: usize,
    },
    /// The caret is inside a quoted attribute value, as in `<mj-image src="…|"`.
    AttributeValue {
        /// The element whose opening tag the caret is inside.
        tag: OpenTag<'s>,
        /// The attribute whose value is being typed, if it could be determined.
        attribute: Option<&'s str>,
    },
    /// The caret is in character data between tags.
    Text {
        /// The innermost element still open around the caret, if any.
        parent: Option<OpenTag<'s>>,
    },
    /// The caret is inside a comment, a doctype, or an XML prolog.
    Ignored,
}

/// A tag name as written in the source, plus its resolved [`MjmlTag`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenTag<'s> {
    /// The name exactly as spelled in the source.
    pub raw: &'s str,
    /// The resolved MJML tag, or [`MjmlTag::Unknown`] for non-MJML markup.
    pub tag: MjmlTag,
}

impl<'s> OpenTag<'s> {
    fn new(raw: &'s str) -> Self {
        Self {
            raw,
            tag: MjmlTag::from_str(raw).unwrap_or(MjmlTag::Unknown),
        }
    }
}

/// Determine the [`CursorContext`] at byte `offset` in `source`.
///
/// `offset` is clamped to the document and rounded down to a character boundary.
#[must_use]
pub fn cursor_context(source: &str, offset: usize) -> CursorContext<'_> {
    let mut offset = offset.min(source.len());
    while offset > 0 && !source.is_char_boundary(offset) {
        offset -= 1;
    }
    let prefix = &source[..offset];
    let bytes = prefix.as_bytes();

    let mut stack: Vec<OpenTag<'_>> = Vec::new();
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        // `<!-- … -->`
        if prefix[i..].starts_with("<!--") {
            match prefix[i + 4..].find("-->") {
                Some(rel) => i = i + 4 + rel + 3,
                None => return CursorContext::Ignored,
            }
            continue;
        }
        // `<!doctype …>` / `<?xml … ?>`
        if matches!(bytes.get(i + 1), Some(b'!' | b'?')) {
            match prefix[i..].find('>') {
                Some(rel) => i = i + rel + 1,
                None => return CursorContext::Ignored,
            }
            continue;
        }
        // `</name>`
        if bytes.get(i + 1) == Some(&b'/') {
            let name_start = i + 2;
            let Some(rel) = prefix[i..].find('>') else {
                let name = take_name(&prefix[name_start..]);
                return CursorContext::TagName {
                    prefix: name,
                    start: name_start,
                    parent: stack.last().copied(),
                    closing: true,
                };
            };
            let name = take_name(&prefix[name_start..i + rel]);
            // Pop back to the matching open tag, tolerating unbalanced markup.
            if let Some(at) = stack.iter().rposition(|open| open.raw == name) {
                stack.truncate(at);
            }
            i = i + rel + 1;
            continue;
        }
        // `<name …>` — an opening tag. Find its terminating `>`, honouring quotes.
        let name_start = i + 1;
        let name = take_name(&prefix[name_start..]);
        if name.is_empty() && !prefix[name_start..].is_empty() {
            // `<` followed by something that cannot start a name: plain text.
            i += 1;
            continue;
        }
        match find_tag_end(&prefix[i..]) {
            Some((rel, self_closing)) => {
                if !self_closing && !is_void_html(name) {
                    stack.push(OpenTag::new(name));
                }
                i = i + rel + 1;
            }
            None => return in_open_tag(prefix, name_start, name, stack.last().copied()),
        }
    }

    CursorContext::Text {
        parent: stack.last().copied(),
    }
}

/// The caret is inside an unterminated opening tag starting at `name_start - 1`.
fn in_open_tag<'s>(
    prefix: &'s str,
    name_start: usize,
    name: &'s str,
    parent: Option<OpenTag<'s>>,
) -> CursorContext<'s> {
    let rest = &prefix[name_start + name.len()..];
    if rest.is_empty() {
        // Still typing the name itself.
        return CursorContext::TagName {
            prefix: name,
            start: name_start,
            parent,
            closing: false,
        };
    }

    let tag = OpenTag::new(name);
    // Walk the attribute area tracking quoting, remembering the last attribute name.
    let bytes = rest.as_bytes();
    let mut quote: Option<u8> = None;
    let mut last_attribute: Option<&str> = None;
    let mut token_start: Option<usize> = None;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'"' | b'\'' => {
                quote = Some(b);
                token_start = None;
            }
            b if b.is_ascii_whitespace() || b == b'=' || b == b'/' => {
                if let Some(start) = token_start.take() {
                    last_attribute = Some(&rest[start..i]);
                }
            }
            _ => {
                if token_start.is_none() {
                    token_start = Some(i);
                }
            }
        }
        i += 1;
    }

    if quote.is_some() {
        return CursorContext::AttributeValue {
            tag,
            attribute: last_attribute,
        };
    }
    // An unterminated `name=` (no quote yet) is still a value position.
    if token_start.is_none() && rest.trim_end().ends_with('=') {
        return CursorContext::AttributeValue {
            tag,
            attribute: last_attribute,
        };
    }

    let (start, partial) = match token_start {
        Some(start) => (name_start + name.len() + start, &rest[start..]),
        None => (name_start + name.len() + rest.len(), ""),
    };
    CursorContext::AttributeName {
        tag,
        prefix: partial,
        start,
    }
}

/// Read a tag/attribute name off the front of `s`.
fn take_name(s: &str) -> &str {
    let end = s
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':'))
        .unwrap_or(s.len());
    &s[..end]
}

/// Find the `>` closing the tag that `s` starts with, honouring quoted values.
///
/// Returns `(offset_of_gt, is_self_closing)`.
fn find_tag_end(s: &str) -> Option<(usize, bool)> {
    let bytes = s.as_bytes();
    let mut quote: Option<u8> = None;
    let mut prev_non_space = 0u8;
    for (i, &b) in bytes.iter().enumerate() {
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
            continue;
        }
        match b {
            b'"' | b'\'' => quote = Some(b),
            b'>' => return Some((i, prev_non_space == b'/')),
            _ => {}
        }
        if !b.is_ascii_whitespace() {
            prev_non_space = b;
        }
    }
    None
}

/// HTML void elements, which never appear on the open-tag stack.
///
/// Only relevant inside raw-content components (`mj-text`, `mj-raw`, …) where users
/// write plain HTML; without this a stray `<br>` would shadow the real MJML parent.
fn is_void_html(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caret_after_an_angle_bracket_is_a_tag_name_with_a_parent() {
        let source = "<mjml><mj-body><mj-section><";
        match cursor_context(source, source.len()) {
            CursorContext::TagName {
                prefix,
                parent,
                closing,
                ..
            } => {
                assert_eq!(prefix, "");
                assert!(!closing);
                assert_eq!(parent.unwrap().tag, MjmlTag::MjSection);
            }
            other => panic!("expected TagName, got {other:?}"),
        }
    }

    #[test]
    fn a_partial_tag_name_is_reported_with_its_start() {
        let source = "<mjml><mj-body><mj-sec";
        match cursor_context(source, source.len()) {
            CursorContext::TagName {
                prefix,
                start,
                parent,
                ..
            } => {
                assert_eq!(prefix, "mj-sec");
                assert_eq!(start, source.len() - "mj-sec".len());
                assert_eq!(parent.unwrap().tag, MjmlTag::MjBody);
            }
            other => panic!("expected TagName, got {other:?}"),
        }
    }

    #[test]
    fn closing_tags_pop_the_stack() {
        let source = "<mjml><mj-body><mj-section></mj-section><";
        match cursor_context(source, source.len()) {
            CursorContext::TagName { parent, .. } => {
                assert_eq!(parent.unwrap().tag, MjmlTag::MjBody);
            }
            other => panic!("expected TagName, got {other:?}"),
        }
    }

    #[test]
    fn self_closing_tags_are_not_pushed() {
        let source = "<mjml><mj-body><mj-section><mj-column><mj-image src=\"a.png\" /><";
        match cursor_context(source, source.len()) {
            CursorContext::TagName { parent, .. } => {
                assert_eq!(parent.unwrap().tag, MjmlTag::MjColumn);
            }
            other => panic!("expected TagName, got {other:?}"),
        }
    }

    #[test]
    fn a_partial_closing_tag_is_flagged() {
        let source = "<mjml><mj-body></mj-b";
        match cursor_context(source, source.len()) {
            CursorContext::TagName {
                prefix, closing, ..
            } => {
                assert_eq!(prefix, "mj-b");
                assert!(closing);
            }
            other => panic!("expected TagName, got {other:?}"),
        }
    }

    #[test]
    fn whitespace_after_the_name_switches_to_attributes() {
        let source = "<mjml><mj-body><mj-section><mj-column><mj-image ";
        match cursor_context(source, source.len()) {
            CursorContext::AttributeName { tag, prefix, .. } => {
                assert_eq!(tag.tag, MjmlTag::MjImage);
                assert_eq!(prefix, "");
            }
            other => panic!("expected AttributeName, got {other:?}"),
        }
    }

    #[test]
    fn a_partial_attribute_name_is_reported() {
        let source = "<mj-image width=\"10px\" sr";
        match cursor_context(source, source.len()) {
            CursorContext::AttributeName { tag, prefix, start } => {
                assert_eq!(tag.tag, MjmlTag::MjImage);
                assert_eq!(prefix, "sr");
                assert_eq!(&source[start..], "sr");
            }
            other => panic!("expected AttributeName, got {other:?}"),
        }
    }

    #[test]
    fn inside_quotes_is_an_attribute_value() {
        let source = "<mj-image src=\"http://exam";
        match cursor_context(source, source.len()) {
            CursorContext::AttributeValue { tag, attribute } => {
                assert_eq!(tag.tag, MjmlTag::MjImage);
                assert_eq!(attribute, Some("src"));
            }
            other => panic!("expected AttributeValue, got {other:?}"),
        }
    }

    #[test]
    fn comments_and_prologs_are_skipped_and_reported() {
        let source = "<?xml version=\"1.0\"?><!-- hi --><mjml><";
        match cursor_context(source, source.len()) {
            CursorContext::TagName { parent, .. } => {
                assert_eq!(parent.unwrap().tag, MjmlTag::Mjml);
            }
            other => panic!("expected TagName, got {other:?}"),
        }
        let unterminated = "<mjml><!-- still typing";
        assert_eq!(
            cursor_context(unterminated, unterminated.len()),
            CursorContext::Ignored
        );
    }

    #[test]
    fn text_between_elements_reports_its_parent() {
        let source = "<mjml><mj-body><mj-section><mj-column><mj-text>hello ";
        match cursor_context(source, source.len()) {
            CursorContext::Text { parent } => {
                assert_eq!(parent.unwrap().tag, MjmlTag::MjText);
            }
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn void_html_inside_raw_content_does_not_shadow_the_parent() {
        let source = "<mj-column><mj-text>a<br>b<";
        match cursor_context(source, source.len()) {
            CursorContext::TagName { parent, .. } => {
                assert_eq!(parent.unwrap().tag, MjmlTag::MjText);
            }
            other => panic!("expected TagName, got {other:?}"),
        }
    }

    #[test]
    fn the_caret_can_sit_before_the_end_of_the_buffer() {
        let source =
            "<mjml><mj-body><mj-section><mj-column></mj-column></mj-section></mj-body></mjml>";
        let offset = source.find("<mj-column>").unwrap() + 1;
        match cursor_context(source, offset) {
            CursorContext::TagName { prefix, parent, .. } => {
                assert_eq!(prefix, "");
                assert_eq!(parent.unwrap().tag, MjmlTag::MjSection);
            }
            other => panic!("expected TagName, got {other:?}"),
        }
    }

    #[test]
    fn multibyte_text_does_not_break_the_scan() {
        let source = "<mj-column><mj-text>café 🎉</mj-text><";
        match cursor_context(source, source.len()) {
            CursorContext::TagName { parent, .. } => {
                assert_eq!(parent.unwrap().tag, MjmlTag::MjColumn);
            }
            other => panic!("expected TagName, got {other:?}"),
        }
        // A caret landing mid-character rounds down instead of panicking.
        let mid = source.find('🎉').unwrap() + 2;
        let _ = cursor_context(source, mid);
    }
}
