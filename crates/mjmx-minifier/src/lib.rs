//! Whitespace-only HTML minifier and beautifier for mjmx's rendered output.
//!
//! These are *post-processors*: they take a fully rendered HTML string and
//! rewrite only its insignificant whitespace. Neither builds a DOM — both
//! tokenize the input into a verbatim tag/text/comment/raw-text/doctype stream
//! (see [`tokenizer`]) and re-serialize it. The invariant is **semantic
//! invariance**: for any rendered document `x`, both `minify(x)` and
//! `beautify(x)` are [`mjmx_html_compare`]-equal to `x`.
//!
//! ## `minify`
//! Shrinks a document by removing insignificant whitespace:
//! - Pure inter-tag whitespace runs (text nodes that are only whitespace) are
//!   **dropped** entirely.
//! - Ordinary text with real content has each internal whitespace run collapsed
//!   to a single space; a single leading/trailing space is preserved so inline
//!   spacing between text and elements is not lost.
//! - Preserved **verbatim**: the doctype; every comment (especially MSO
//!   conditional comments, whose internal structure is significant); the inner
//!   content of raw-text elements (`<style>`, `<script>`, `<title>`,
//!   `<textarea>`, `<pre>`); and the exact source of every tag.
//!
//! ## `beautify`
//! Re-indents with a **2-space** step, one token per line. Each element opens a
//! new indentation level for its children and its close tag dedents to match.
//! Void and self-closing tags do not nest. Raw-text elements keep their inner
//! content byte-for-byte (emitted verbatim between the open and close tag lines),
//! and comments are emitted intact. Whitespace-only text is dropped; text with
//! content is collapsed and placed on its own line. Exact indentation style is
//! this crate's own — byte-parity with mjml's `js-beautify` is a non-goal.

mod tokenizer;

use tokenizer::{Token, tokenize};

/// Minify `html`: strip insignificant inter-tag and intra-text whitespace while
/// preserving the doctype, comments (incl. MSO conditionals), and raw-text
/// element bodies verbatim. See the [crate] docs for the exact rules.
///
/// The result is [`mjmx_html_compare`]-equal to the input and, for a typical
/// multi-line document, strictly shorter.
#[must_use]
pub fn minify(html: &str) -> String {
    let tokens = tokenize(html);
    let mut out = String::with_capacity(html.len());
    for token in &tokens {
        match token {
            Token::Doctype(s) | Token::Comment(s) | Token::RawText(s) => out.push_str(s),
            Token::Open { text, .. } | Token::Close { text, .. } => out.push_str(text),
            Token::Text(s) => {
                if !s.trim().is_empty() {
                    out.push_str(&collapse_ws(s));
                }
            }
        }
    }
    out
}

/// Beautify `html`: re-indent with a 2-space step, one token per line, keeping
/// raw-text blocks and comments intact. See the [crate] docs for the style.
///
/// The result is [`mjmx_html_compare`]-equal to the input.
#[must_use]
pub fn beautify(html: &str) -> String {
    let tokens = tokenize(html);
    let mut out = String::with_capacity(html.len() * 2);
    let mut indent: usize = 0;
    // Tracks, for each currently-open element, whether it is a raw-text element
    // (whose open tag did *not* increase the indent, so its close must not
    // decrease it).
    let mut stack: Vec<bool> = Vec::new();

    for token in &tokens {
        match token {
            // The doctype always sits flush-left and never affects indentation.
            Token::Doctype(s) => push_line(&mut out, 0, s),
            Token::Comment(s) => push_line(&mut out, indent, s),
            Token::RawText(s) => {
                // Verbatim inner content of a raw-text element, kept byte-for-byte.
                out.push_str(s);
                if !s.ends_with('\n') {
                    out.push('\n');
                }
            }
            Token::Open {
                text,
                tag,
                void,
                self_closing,
            } => {
                push_line(&mut out, indent, text);
                let raw = is_raw_text_tag(tag);
                if !void && !self_closing {
                    stack.push(raw);
                    // Raw-text content is emitted verbatim at the current level;
                    // only ordinary elements open a new indentation level.
                    if !raw {
                        indent += 1;
                    }
                }
            }
            Token::Close { text, .. } => {
                // Dedent for ordinary elements; raw-text elements never indented.
                match stack.pop() {
                    Some(false) => indent = indent.saturating_sub(1),
                    Some(true) => {}
                    None => indent = indent.saturating_sub(1),
                }
                push_line(&mut out, indent, text);
            }
            Token::Text(s) => {
                let collapsed = collapse_ws(s);
                let trimmed = collapsed.trim();
                if !trimmed.is_empty() {
                    push_line(&mut out, indent, trimmed);
                }
            }
        }
    }

    out
}

/// Whether `tag` (already lower-cased) is a raw-text element handled verbatim.
fn is_raw_text_tag(tag: &str) -> bool {
    matches!(tag, "style" | "script" | "title" | "textarea" | "pre")
}

/// Collapse every internal whitespace run to a single space, keeping at most one
/// leading and one trailing space so inline spacing is preserved. Does not trim.
fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_ws {
                out.push(' ');
                prev_ws = true;
            }
        } else {
            out.push(c);
            prev_ws = false;
        }
    }
    out
}

/// Push `2 * indent` spaces, then `content`, then a newline.
fn push_line(out: &mut String, indent: usize, content: &str) {
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push_str(content);
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_inter_tag_whitespace() {
        let input = "<div>\n  <span>a</span>\n</div>";
        let out = minify(input);
        assert_eq!(out, "<div><span>a</span></div>");
    }

    #[test]
    fn collapses_text_runs_to_single_space() {
        let input = "<p>hello    world\n\tagain</p>";
        assert_eq!(minify(input), "<p>hello world again</p>");
    }

    #[test]
    fn preserves_inline_trailing_space_in_mixed_text() {
        // A content-bearing text node keeps a single trailing space, so the gap
        // to the following inline element is not lost.
        let input = "<p>foo   <b>bar</b></p>";
        assert_eq!(minify(input), "<p>foo <b>bar</b></p>");
    }

    #[test]
    fn drops_pure_whitespace_between_inline_tags() {
        // A text node that is *only* whitespace is insignificant inter-tag layout
        // and is dropped, matching mjml's collapse-whitespace behavior.
        let input = "<b>a</b>\n  <i>b</i>";
        assert_eq!(minify(input), "<b>a</b><i>b</i>");
    }

    #[test]
    fn preserves_style_block_verbatim() {
        let input = "<style type=\"text/css\">\n  #outlook a {\n    padding: 0;\n  }\n</style>";
        // Inner whitespace of a <style> must survive untouched.
        assert_eq!(minify(input), input);
    }

    #[test]
    fn preserves_pre_block_verbatim() {
        let input = "<pre>  line1\n    line2  </pre>";
        assert_eq!(minify(input), input);
    }

    #[test]
    fn preserves_mso_conditional_comment() {
        let input = "<div>\n  <!--[if mso | IE]><table><tr><td><![endif]-->\n  <p>x</p>\n</div>";
        let out = minify(input);
        assert!(out.contains("<!--[if mso | IE]><table><tr><td><![endif]-->"));
        // Surrounding inter-tag whitespace is gone, comment is intact.
        assert_eq!(
            out,
            "<div><!--[if mso | IE]><table><tr><td><![endif]--><p>x</p></div>"
        );
    }

    #[test]
    fn preserves_downlevel_revealed_comment() {
        let input = "<!--[if !mso]><!--><meta charset=\"utf-8\"><!--<![endif]-->";
        // The two comments each end at their first `-->`; both survive verbatim.
        assert_eq!(minify(input), input);
    }

    #[test]
    fn keeps_doctype() {
        let input = "<!doctype html>\n<html>\n  <body></body>\n</html>";
        let out = minify(input);
        assert!(out.starts_with("<!doctype html>"));
        assert_eq!(out, "<!doctype html><html><body></body></html>");
    }

    #[test]
    fn minify_shrinks_indented_document() {
        let input = "<ul>\n  <li>one</li>\n  <li>two</li>\n  <li>three</li>\n</ul>\n";
        assert!(minify(input).len() < input.len());
    }

    #[test]
    fn void_elements_do_not_nest_in_beautify() {
        let input = "<div><meta charset=\"utf-8\"><br><img src=\"x\"></div>";
        let out = beautify(input);
        // The children sit one level in; the </div> returns to column 0.
        assert!(out.contains("\n  <meta charset=\"utf-8\">\n"));
        assert!(out.contains("\n  <br>\n"));
        assert!(out.contains("\n  <img src=\"x\">\n"));
        assert!(out.contains("\n</div>\n"));
        assert!(out.starts_with("<div>\n"));
    }

    #[test]
    fn beautify_indents_nested_elements() {
        let input = "<a><b><c>x</c></b></a>";
        let out = beautify(input);
        let expected = "<a>\n  <b>\n    <c>\n      x\n    </c>\n  </b>\n</a>\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn beautify_keeps_raw_text_intact() {
        let input = "<div><style>a{b:c}\n  d{e:f}</style></div>";
        let out = beautify(input);
        assert!(out.contains("a{b:c}\n  d{e:f}"));
        // The <style>/<style> pair share an indentation level (no nesting bump).
        assert!(out.contains("\n  <style>\n"));
        assert!(out.contains("\n  </style>\n"));
    }
}
