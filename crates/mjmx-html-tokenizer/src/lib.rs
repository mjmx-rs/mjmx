//! A small, forgiving HTML tokenizer producing a flat, span-carrying event stream.
//!
//! It understands the exact HTML shapes emitted by mjml 4.18.0 (and this
//! project's renderer): a leading `<!doctype html>`, void/unclosed elements,
//! raw-text elements (`<style>`/`<script>`/`<title>`/`<textarea>`), MSO
//! conditional comments, normal comments, and single/double-quoted or valueless
//! attributes. It does not build a tree.
//!
//! Two crates consume it, and between them they pin down the API:
//!
//! - [`mjmx_html_compare`](../mjmx_html_compare/index.html) normalizes the stream
//!   and walks two of them in lock-step, recovering nesting structurally;
//! - [`mjmx_css_inline`](../mjmx_css_inline/index.html) builds a tree from it and
//!   *rewrites* start tags in place, which is why every event — and every
//!   individual attribute — carries the byte range it came from. Splicing over
//!   those ranges is what lets the inliner touch the elements a stylesheet
//!   matched and leave every other byte of the document alone.
//!
//! Scanning is byte-oriented. Every delimiter HTML cares about (`<`, `>`, `/`,
//! `=`, `"`, `'`) is ASCII, and no UTF-8 continuation byte can be mistaken for
//! one, so multi-byte text passes through untouched and every recorded offset
//! lands on a character boundary.

use std::ops::Range;

/// One attribute of a start tag, with the byte range of the whole `name="value"`
/// (or bare `name`) source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr<'a> {
    /// The attribute name, exactly as written (not lower-cased).
    pub name: &'a str,
    /// The attribute value with its quotes removed; `""` for a valueless attribute.
    pub value: &'a str,
    /// Byte range of the attribute in the source, from the first byte of `name`
    /// through the closing quote (or the last byte of an unquoted value).
    pub span: Range<usize>,
}

/// One lexical event, tagged with the byte range it occupies in the input.
///
/// Void and self-closing elements produce a single [`Event::Open`] with no
/// matching [`Event::Close`] (there is no close tag in the source).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event<'a> {
    /// `<!doctype html>` — `text` is the inner content with the leading `!`
    /// stripped, e.g. `doctype html`.
    DocType { text: &'a str, span: Range<usize> },
    /// A start tag with its raw (un-normalized) attribute list, in source order.
    Open {
        /// The tag name, exactly as written (not lower-cased).
        tag: &'a str,
        attrs: Vec<Attr<'a>>,
        /// Whether the tag was written self-closing (`<img … />`).
        self_closing: bool,
        /// Byte range of the whole start tag, `<` through `>`.
        span: Range<usize>,
        /// Byte offset just past the last attribute — the insertion point for a
        /// new attribute, which is *before* any trailing `/` or `>`.
        ///
        /// With no attributes this is the end of the tag name, so an insertion
        /// of `" name=\"value\""` is always well-formed.
        attrs_end: usize,
    },
    /// An end tag `</tag>`.
    Close {
        /// The tag name, exactly as written (not lower-cased).
        tag: &'a str,
        /// Byte range of the whole end tag, `<` through `>`.
        span: Range<usize>,
    },
    /// Character data between tags (raw, un-normalized).
    Text { text: &'a str, span: Range<usize> },
    /// A comment's inner content (between `<!--` and `-->`), incl. MSO
    /// conditionals. `span` covers the delimiters as well as `text`.
    Comment { text: &'a str, span: Range<usize> },
}

/// Elements whose content is captured verbatim as a single [`Event::Text`],
/// rather than being parsed as markup.
fn is_raw_text(tag: &str) -> bool {
    tag.len() <= 8
        && matches!(
            tag.to_ascii_lowercase().as_str(),
            "style" | "script" | "title" | "textarea"
        )
}

/// Whether `tag` names a void element — one that never has children or an end
/// tag, so a tree builder must not push it onto the open-element stack.
#[must_use]
pub fn is_void(tag: &str) -> bool {
    tag.len() <= 6
        && matches!(
            tag.to_ascii_lowercase().as_str(),
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

/// Byte offset of the start (`<`) of the matching close tag `</tag>` for a
/// raw-text element, matched case-insensitively.
fn find_raw_close(input: &str, from: usize, tag: &str) -> Option<usize> {
    let bytes = input.as_bytes();
    let needle = tag.as_bytes();
    let n = bytes.len();
    let mut i = from;
    while i < n {
        if bytes[i] == b'<' && i + 1 < n && bytes[i + 1] == b'/' {
            let end = i + 2 + needle.len();
            if end <= n
                && bytes[i + 2..end].eq_ignore_ascii_case(needle)
                && (end >= n || bytes[end].is_ascii_whitespace() || bytes[end] == b'>')
            {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn is_name_end(b: u8) -> bool {
    b.is_ascii_whitespace() || b == b'/' || b == b'>'
}

/// Tokenize `input` into a flat [`Event`] stream.
#[must_use]
pub fn tokenize(input: &str) -> Vec<Event<'_>> {
    let bytes = input.as_bytes();
    let n = bytes.len();
    let mut events = Vec::new();
    let mut i = 0;

    while i < n {
        if bytes[i] != b'<' {
            // Text run up to the next tag.
            let start = i;
            while i < n && bytes[i] != b'<' {
                i += 1;
            }
            events.push(Event::Text {
                text: &input[start..i],
                span: start..i,
            });
            continue;
        }

        if input[i..].starts_with("<!--") {
            // Comment — ends at the first `-->`. MSO conditionals such as
            // `<!--[if mso | IE]>...<![endif]-->` are a single comment.
            let start = i + 4;
            let (content_end, next) = match input[start..].find("-->") {
                Some(offset) => (start + offset, start + offset + 3),
                None => (n, n),
            };
            events.push(Event::Comment {
                text: &input[start..content_end],
                span: i..next,
            });
            i = next;
            continue;
        }

        if i + 1 < n && bytes[i + 1] == b'!' {
            // Declaration such as `<!doctype html>`.
            let end = input[i..].find('>').map_or(n, |offset| i + offset);
            let next = if end < n { end + 1 } else { n };
            events.push(Event::DocType {
                text: &input[(i + 2).min(end)..end],
                span: i..next,
            });
            i = next;
            continue;
        }

        if i + 1 < n && bytes[i + 1] == b'/' {
            // End tag.
            let start = i + 2;
            let end = input[start..].find('>').map_or(n, |offset| start + offset);
            let next = if end < n { end + 1 } else { n };
            events.push(Event::Close {
                tag: input[start..end].trim(),
                span: i..next,
            });
            i = next;
            continue;
        }

        // Start tag.
        let mut j = i + 1;
        let name_start = j;
        while j < n && !is_name_end(bytes[j]) {
            j += 1;
        }
        let tag = &input[name_start..j];
        let mut attrs: Vec<Attr<'_>> = Vec::new();
        let mut attrs_end = j;
        let mut self_closing = false;

        loop {
            while j < n && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j >= n {
                break;
            }
            if bytes[j] == b'>' {
                j += 1;
                break;
            }
            if bytes[j] == b'/' {
                self_closing = true;
                j += 1;
                continue;
            }
            // Attribute name.
            let an_start = j;
            while j < n
                && !bytes[j].is_ascii_whitespace()
                && bytes[j] != b'='
                && bytes[j] != b'>'
                && bytes[j] != b'/'
            {
                j += 1;
            }
            let name = &input[an_start..j];
            let mut span_end = j;
            while j < n && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            let mut value = "";
            if j < n && bytes[j] == b'=' {
                j += 1;
                while j < n && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < n && (bytes[j] == b'"' || bytes[j] == b'\'') {
                    let quote = bytes[j];
                    j += 1;
                    let vs = j;
                    while j < n && bytes[j] != quote {
                        j += 1;
                    }
                    value = &input[vs..j];
                    if j < n {
                        j += 1; // closing quote
                    }
                } else {
                    let vs = j;
                    while j < n && !bytes[j].is_ascii_whitespace() && bytes[j] != b'>' {
                        j += 1;
                    }
                    value = &input[vs..j];
                }
                span_end = j;
            }
            if !name.is_empty() {
                attrs.push(Attr {
                    name,
                    value,
                    span: an_start..span_end,
                });
                attrs_end = span_end;
            }
        }

        events.push(Event::Open {
            tag,
            attrs,
            self_closing,
            span: i..j,
            attrs_end,
        });
        i = j;

        // Raw-text element: swallow inner content verbatim as one Text node; the
        // close tag is then emitted by the normal loop.
        if is_raw_text(tag) && !self_closing {
            let close = find_raw_close(input, i, tag).unwrap_or(n);
            events.push(Event::Text {
                text: &input[i..close],
                span: i..close,
            });
            i = close;
        }
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every event's span slices back to the bytes it was made from, and the
    /// spans tile the input with no gaps — the property the inliner's
    /// splice-based rewriting depends on.
    #[test]
    fn spans_tile_the_input() {
        let input = "<!doctype html>\n<html><head><style>a{b:c}</style></head>\
                     <body><!--[if mso]><td><![endif]--><p class=\"x\">é</p><br/></body></html>";
        let mut cursor = 0;
        for event in tokenize(input) {
            let span = match &event {
                Event::DocType { span, .. }
                | Event::Open { span, .. }
                | Event::Close { span, .. }
                | Event::Text { span, .. }
                | Event::Comment { span, .. } => span.clone(),
            };
            assert_eq!(span.start, cursor, "gap before {event:?}");
            cursor = span.end;
        }
        assert_eq!(cursor, input.len());
    }

    #[test]
    fn attribute_spans_and_insertion_point() {
        let input = "<img alt=\"a\" src='b' hidden />";
        let events = tokenize(input);
        let Event::Open {
            tag,
            attrs,
            self_closing,
            attrs_end,
            ..
        } = &events[0]
        else {
            panic!("expected a start tag, got {:?}", events[0]);
        };
        assert_eq!(*tag, "img");
        assert!(self_closing);
        assert_eq!(attrs.len(), 3);
        assert_eq!((attrs[0].name, attrs[0].value), ("alt", "a"));
        assert_eq!(&input[attrs[0].span.clone()], "alt=\"a\"");
        assert_eq!((attrs[1].name, attrs[1].value), ("src", "b"));
        assert_eq!(&input[attrs[1].span.clone()], "src='b'");
        assert_eq!((attrs[2].name, attrs[2].value), ("hidden", ""));
        assert_eq!(&input[attrs[2].span.clone()], "hidden");
        // The insertion point is before the ` />`, so splicing stays well-formed.
        assert_eq!(&input[..*attrs_end], "<img alt=\"a\" src='b' hidden");
    }

    #[test]
    fn a_bare_tag_offers_the_name_end_as_its_insertion_point() {
        let input = "<td>";
        let events = tokenize(input);
        let Event::Open { attrs_end, .. } = &events[0] else {
            panic!("expected a start tag");
        };
        assert_eq!(*attrs_end, 3);
    }

    #[test]
    fn raw_text_and_comments_are_opaque() {
        let events = tokenize("<style>p{a:'</p>'}</style><!--<b>--><p>");
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Open { .. }))
                .count(),
            2,
            "the <b> inside the comment and the <p> inside <style> are not elements"
        );
        assert!(matches!(&events[1], Event::Text { text, .. } if *text == "p{a:'</p>'}"));
    }
}
