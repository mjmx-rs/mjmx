//! Canonical MJML **source** formatter for `mjmx` (the `mjmx fmt` tool).
//!
//! Unlike [`mjmx_minifier`](https://docs.rs/mjmx-minifier), which rewrites *rendered
//! HTML*, this crate pretty-prints an [`mjmx_ast`] tree back into **MJML source**.
//! mjml has no formatter of its own, so this is `mjmx`'s own canonical style; its
//! correctness is defined by **self-consistency**, not by an mjml oracle:
//!
//! 1. **Idempotence** — `format(parse(format(parse(src))))` equals `format(parse(src))`.
//! 2. **Reparse-equivalence** — parsing the formatted output yields a tree
//!    structurally equal to parsing the original (same tag sequence, same ordered
//!    attribute name→value pairs, byte-identical raw-content bodies, equal comments,
//!    and equal text once insignificant whitespace is normalized).
//! 3. **Render-equivalence** — rendering the formatted source produces HTML that is
//!    [`mjmx_html_compare`](https://docs.rs/mjmx-html-compare)-equal to rendering the
//!    original.
//!
//! # Style
//!
//! - **Indentation** — [`FormatOptions::indent_width`] spaces per level (default 2),
//!   one element per line.
//! - **Attributes** — printed in **source order** (never sorted), on the element's
//!   opening-tag line: `name="value"`, or a bare `name` for a valueless attribute.
//!   Values are wrapped in double quotes, or single quotes when the value itself
//!   contains a `"` (but no `'`), so the value round-trips.
//! - **Empty elements self-close** — an element with no meaningful children is
//!   emitted as `<mj-image ... />`.
//! - **Containers** — an element with children opens a block: the opening tag, its
//!   children each indented one level deeper, then the closing tag dedented to match.
//! - **Raw-content elements** — for
//!   [`is_ending_tag`](mjmx_syntax::MjmlTag::is_ending_tag) elements (mj-text,
//!   mj-button, mj-raw, mj-table, …) the captured body is emitted **verbatim**
//!   between the opening and closing tags, exactly as the user wrote it — user HTML
//!   is never re-indented. Because the body is emitted inline right after `>`,
//!   whether the element reads as a single line or a multi-line block is determined
//!   solely by whether that body itself contains newlines: `<mj-text>Hi</mj-text>`
//!   stays inline, while an mj-raw block whose body starts with a newline naturally
//!   spans several lines.
//! - **Comments** — preserved verbatim on their own line (`<!--…-->`).
//! - **Text** — a text node between elements is trimmed and placed on its own line;
//!   a text node that is only whitespace (insignificant inter-element layout) is
//!   dropped.
//! - **Trailing newline** — non-empty output always ends with exactly one `\n`.

use mjmx_ast::{Document, Element, Node};

/// Options controlling [`format`].
#[derive(Debug, Clone)]
pub struct FormatOptions {
    /// Number of spaces per indentation level. Defaults to 2.
    pub indent_width: usize,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self { indent_width: 2 }
    }
}

/// Format `document` back into canonical MJML source text.
///
/// See the [crate] docs for the exact style. The output is idempotent under
/// re-formatting, reparse-equivalent to the original tree, and render-equivalent to
/// the original source.
#[must_use]
pub fn format(document: &Document, options: &FormatOptions) -> String {
    let formatter = Formatter {
        indent_width: options.indent_width,
    };
    let mut out = String::new();
    for node in &document.children {
        formatter.write_node(&mut out, *node, 0);
    }
    out
}

/// Internal, stateless printer parameterized by the indentation step.
struct Formatter {
    indent_width: usize,
}

impl Formatter {
    /// Push `level` indentation steps of spaces.
    fn write_indent(&self, out: &mut String, level: usize) {
        for _ in 0..(level * self.indent_width) {
            out.push(' ');
        }
    }

    /// Write a single node (and its subtree) at `level`, terminated by a newline.
    fn write_node(&self, out: &mut String, node: Node, level: usize) {
        match node {
            Node::Element(el) => self.write_element(out, el, level),
            Node::Comment(comment) => {
                self.write_indent(out, level);
                out.push_str("<!--");
                out.push_str(comment.value.as_str());
                out.push_str("-->\n");
            }
            Node::Text(text) => {
                let trimmed = text.value.as_str().trim();
                if !trimmed.is_empty() {
                    self.write_indent(out, level);
                    out.push_str(trimmed);
                    out.push('\n');
                }
            }
        }
    }

    /// Write an element at `level`.
    ///
    /// Three shapes are possible:
    /// - **verbatim inline** — for raw-content (ending) elements *and* text-only
    ///   elements such as `mj-title`/`mj-preview`/`mj-style`, whose body is
    ///   semantically significant and is therefore emitted byte-for-byte between the
    ///   tags rather than re-indented;
    /// - **self-close** — for an element with no meaningful children;
    /// - **block** — for a container that holds element or comment children.
    fn write_element(&self, out: &mut String, el: &Element, level: usize) {
        let has_child_elements = el
            .children
            .iter()
            .any(|c| matches!(c, Node::Element(_) | Node::Comment(_)));

        // Raw-content elements always preserve their body verbatim (even when it is
        // only whitespace); an empty one self-closes.
        if el.tag.is_ending_tag() {
            match text_body(el) {
                Some(body) => self.write_inline(out, el, level, &body),
                None => self.write_self_close(out, el, level),
            }
            return;
        }

        // A non-ending element with no element/comment children but with
        // significant text (mj-title, mj-preview, mj-style, ...) is a "text
        // container": emit its text verbatim inline so it survives byte-for-byte.
        if !has_child_elements {
            match text_body(el) {
                Some(body) if !body.trim().is_empty() => self.write_inline(out, el, level, &body),
                _ => self.write_self_close(out, el, level),
            }
            return;
        }

        // Container with element/comment children: block form.
        self.write_indent(out, level);
        write_open_tag(out, el, false);
        out.push('\n');
        for child in &el.children {
            self.write_node(out, *child, level + 1);
        }
        self.write_indent(out, level);
        out.push_str("</");
        out.push_str(el.raw_tag.as_str());
        out.push_str(">\n");
    }

    /// Emit `<tag ...>body</tag>` on the current line, `body` verbatim.
    fn write_inline(&self, out: &mut String, el: &Element, level: usize, body: &str) {
        self.write_indent(out, level);
        write_open_tag(out, el, false);
        out.push_str(body);
        out.push_str("</");
        out.push_str(el.raw_tag.as_str());
        out.push_str(">\n");
    }

    /// Emit `<tag ... />` on the current line.
    fn write_self_close(&self, out: &mut String, el: &Element, level: usize) {
        self.write_indent(out, level);
        write_open_tag(out, el, true);
        out.push('\n');
    }
}

/// Write an element's opening tag (name + attributes) without a trailing newline.
///
/// When `self_close` is set the tag is closed with ` />`; otherwise with `>`.
fn write_open_tag(out: &mut String, el: &Element, self_close: bool) {
    out.push('<');
    out.push_str(el.raw_tag.as_str());
    for attr in &el.attributes {
        out.push(' ');
        out.push_str(attr.name.as_str());
        if let Some(value) = attr.value {
            let value = value.as_str();
            // Pick a quote that does not occur in the value so it round-trips.
            let quote = if value.contains('"') && !value.contains('\'') {
                '\''
            } else {
                '"'
            };
            out.push('=');
            out.push(quote);
            out.push_str(value);
            out.push(quote);
        }
    }
    if self_close {
        out.push_str(" />");
    } else {
        out.push('>');
    }
}

/// The concatenated verbatim text body of an element (its [`Text`](mjmx_ast::Text)
/// children joined in order), or `None` when the element has no text child at all.
///
/// Raw-content elements carry exactly one such child; the join is a robustness
/// measure for any element that happens to hold several text runs.
fn text_body(el: &Element) -> Option<String> {
    let mut body = String::new();
    let mut found = false;
    for child in &el.children {
        if let Node::Text(text) = child {
            body.push_str(text.value.as_str());
            found = true;
        }
    }
    found.then_some(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mjmx_allocator::Allocator;
    use mjmx_parser::parse;

    /// Parse then format `src` with the default options.
    fn fmt(src: &str) -> String {
        let alloc = Allocator::new();
        let parsed = parse(src, &alloc);
        assert!(
            parsed.errors.is_empty(),
            "parse errors: {:?}",
            parsed.errors
        );
        format(&parsed.document, &FormatOptions::default())
    }

    #[test]
    fn indents_nested_containers_two_spaces() {
        let out =
            fmt("<mjml><mj-body><mj-section><mj-column></mj-column></mj-section></mj-body></mjml>");
        let expected = "\
<mjml>
  <mj-body>
    <mj-section>
      <mj-column />
    </mj-section>
  </mj-body>
</mjml>
";
        assert_eq!(out, expected);
    }

    #[test]
    fn keeps_attributes_in_source_order() {
        let out = fmt("<mj-image src=\"a.png\" alt=\"x\" width=\"100px\" />");
        assert_eq!(
            out,
            "<mj-image src=\"a.png\" alt=\"x\" width=\"100px\" />\n"
        );
    }

    #[test]
    fn valueless_attribute_prints_bare() {
        let out = fmt("<mj-image src=\"a.png\" data-flag />");
        // A valueless attribute is emitted as a bare name, in source order.
        assert_eq!(out, "<mj-image src=\"a.png\" data-flag />\n");
    }

    #[test]
    fn empty_container_self_closes() {
        assert_eq!(fmt("<mj-body></mj-body>"), "<mj-body />\n");
    }

    #[test]
    fn raw_text_element_stays_inline() {
        let out = fmt("<mj-text>Hello <b>world</b></mj-text>");
        assert_eq!(out, "<mj-text>Hello <b>world</b></mj-text>\n");
    }

    #[test]
    fn raw_body_is_emitted_verbatim() {
        // The inner HTML (including its own indentation) must survive byte-for-byte.
        let src = "<mj-raw>\n      <div class=\"c\">hi</div>\n    </mj-raw>";
        let out = fmt(src);
        assert_eq!(
            out,
            "<mj-raw>\n      <div class=\"c\">hi</div>\n    </mj-raw>\n"
        );
    }

    #[test]
    fn drops_whitespace_only_text_between_elements() {
        let out = fmt("<mj-section>\n   \n  <mj-column></mj-column>\n</mj-section>");
        assert_eq!(out, "<mj-section>\n  <mj-column />\n</mj-section>\n");
    }

    #[test]
    fn comment_kept_on_own_line() {
        let out = fmt("<mjml><!-- hi --><mj-body></mj-body></mjml>");
        assert_eq!(out, "<mjml>\n  <!-- hi -->\n  <mj-body />\n</mjml>\n");
    }

    #[test]
    fn value_with_double_quote_uses_single_quotes() {
        let out = fmt("<mj-image alt='say \"hi\"' />");
        assert_eq!(out, "<mj-image alt='say \"hi\"' />\n");
    }
}
