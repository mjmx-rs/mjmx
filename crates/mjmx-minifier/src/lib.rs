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

mod css;
mod tag;
mod tokenizer;

use tokenizer::{Token, tokenize};

/// Knobs on [`minify_with`], mirroring mjml's `minifyOptions`.
///
/// mjml spreads `minifyOptions` over the four html-minifier switches it hardcodes
/// (`mjml-core/src/index.js:419-425`), so those four — and only those four — are what
/// a `minifyOptions` object can meaningfully change in an mjml pipeline. The defaults
/// here are mjml's, with one deliberate exception noted on
/// [`remove_empty_attributes`](Self::remove_empty_attributes); [`minify`] and
/// `minify_with(_, &MinifyOptions::default())` always produce identical bytes.
///
/// Any other html-minifier switch (`removeComments`, `collapseBooleanAttributes`, …)
/// is deliberately absent: mjml never sets it, and honoring it would mean claiming
/// html-minifier parity this crate does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MinifyOptions {
    /// Collapse insignificant whitespace. `// html-minifier default: false;
    /// mjml sets true`.
    ///
    /// This is the crate's historical behavior: whitespace-only text nodes are
    /// dropped and content-bearing ones have their internal runs collapsed. With
    /// `false`, text nodes are emitted byte-for-byte and `minify` only ever removes
    /// what the other switches remove.
    pub collapse_whitespace: bool,
    /// Minify the CSS inside `<style>` elements. `// html-minifier default: false;
    /// mjml also sets false`.
    ///
    /// **Divergence, deliberate:** html-minifier delegates this to `clean-css`, a full
    /// CSS *optimizer* — it shortens colors, merges shorthands and reorders
    /// declarations. This crate instead parses the stylesheet with `cssparser` and
    /// re-serializes it with only its insignificant whitespace and comments removed
    /// ([`css`]). Because it is a real parse it knows a selector from a declaration,
    /// so `color: red` collapses to `color:red` while the descendant combinator in
    /// `a :hover` is preserved. Every emitted byte comes from the source: values,
    /// selectors, strings, `url()`s, custom properties and `!important` are never
    /// rewritten, and unparseable CSS is passed through verbatim.
    ///
    /// Since mjml defaults this off, the default output is unaffected either way — and
    /// only the default path carries this crate's semantic-invariance guarantee, since
    /// rewriting a `<style>` body necessarily changes the text
    /// [`mjmx_html_compare`] sees there.
    pub minify_css: bool,
    /// Treat attribute names case-sensitively. `// html-minifier default: false;
    /// mjml sets true`.
    ///
    /// `true` — the default — leaves every tag byte-for-byte. `false` lower-cases
    /// attribute *names* (html-minifier's behavior for non-custom markup). Tag names
    /// are never rewritten: mjmx only ever emits lower-case ones, and the mixed-case
    /// markup in an MJML document (`<o:OfficeDocumentSettings>`) lives inside MSO
    /// conditional comments, which this crate preserves verbatim by design.
    pub case_sensitive: bool,
    /// Drop attributes whose value is empty. `// html-minifier default: false;
    /// mjml sets true`.
    ///
    /// Follows html-minifier's `canDeleteEmptyAttribute` and its `reEmptyAttribute`
    /// allow-list exactly: empty `class`/`id`/`style`/`title`/`lang`/`dir`, a specific
    /// set of `on*` handlers, and `value=""` on `<input>`. Every other empty attribute
    /// (`alt=""`, `href=""`) is meaningful and is kept. A tag with nothing to drop
    /// keeps its exact source bytes.
    ///
    /// **Divergence, deliberate:** this defaults to `false`, where mjml passes `true`.
    /// mjml's own rendered output carries `<div … style="" role="article">`, so mjml's
    /// default silently *removes an attribute* during minification — which would break
    /// this crate's semantic-invariance guarantee (a minified document is
    /// [`mjmx_html_compare`]-equal to its input) and would shift the bytes of every
    /// existing `minify` caller. Pass `remove_empty_attributes: true` — or
    /// `--minify-options '{"removeEmptyAttributes": true}'`, or `minifyOptions:
    /// { removeEmptyAttributes: true }` from a binding — for byte parity with mjml.
    pub remove_empty_attributes: bool,
    /// Which upstream CSS minifier's output shape to reproduce, when
    /// [`minify_css`](Self::minify_css) is on. `// mjmx extension`.
    ///
    /// mjml has no such option: 4.x minifies CSS with html-minifier's `clean-css` and
    /// 5.x with htmlnano's `cssnano`, and the two disagree in ways that are visible in
    /// the output. Neither is "more correct", so the release being reproduced picks one
    /// — see [`CssDialect`].
    pub css_dialect: CssDialect,
}

/// Which upstream CSS minifier's output shape [`minify_with`] reproduces.
///
/// Both dialects are whitespace-and-comments-only passes over a real parse: mjmx never
/// adopts either optimizer's *rewriting* (shortening colours, merging shorthands,
/// reordering declarations), so `0px` stays `0px` under both. They differ only in which
/// insignificant bytes each upstream chooses to drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CssDialect {
    /// html-minifier's `clean-css`, as mjml 4.18.0 uses it. Collapses selector
    /// preludes (`.a, .b` → `.a,.b`) and keeps the space before `!important`.
    #[default]
    CleanCss,
    /// htmlnano's `cssnano`, as mjml 5.4.0 uses it. Leaves selector preludes
    /// byte-for-byte — `.c   >   .d` keeps all three spaces — and closes the gap
    /// before `!important`.
    Cssnano,
}

impl Default for MinifyOptions {
    fn default() -> Self {
        Self {
            // mjml passes collapseWhitespace: true.
            collapse_whitespace: true,
            // mjml passes minifyCSS: false.
            minify_css: false,
            // mjml passes caseSensitive: true.
            case_sensitive: true,
            // mjml passes removeEmptyAttributes: true; we default to false to keep
            // minify semantics-preserving. See the field docs.
            remove_empty_attributes: false,
            // mjml 4.18.0's minifier. `for_mjml5` selects the other one.
            css_dialect: CssDialect::CleanCss,
        }
    }
}

impl MinifyOptions {
    /// The defaults mjml **5.x** minifies with, rather than 4.18.0's.
    ///
    /// Two differ from [`Default`]: 5.x minifies CSS by default (`minifyCSS` still
    /// exists there and still turns it off), and it does so with cssnano rather than
    /// clean-css. The other two switches are unchanged.
    ///
    /// Callers that layer a user's `minifyOptions` over a base pick the base from the
    /// mjml release being reproduced, so an explicit switch still wins.
    #[must_use]
    pub fn for_mjml5() -> Self {
        Self {
            minify_css: true,
            css_dialect: CssDialect::Cssnano,
            ..Self::default()
        }
    }
}

/// Minify `html`: strip insignificant inter-tag and intra-text whitespace while
/// preserving the doctype, comments (incl. MSO conditionals), and raw-text
/// element bodies verbatim. See the [crate] docs for the exact rules.
///
/// The result is [`mjmx_html_compare`]-equal to the input and, for a typical
/// multi-line document, strictly shorter.
///
/// Equivalent to [`minify_with`] under [`MinifyOptions::default`].
#[must_use]
pub fn minify(html: &str) -> String {
    minify_with(html, &MinifyOptions::default())
}

/// [`minify`] under caller-chosen [`MinifyOptions`] — mjml's `minifyOptions`.
#[must_use]
pub fn minify_with(html: &str, options: &MinifyOptions) -> String {
    let tokens = tokenize(html);
    let mut out = String::with_capacity(html.len());
    for token in &tokens {
        match token {
            Token::Doctype(s) | Token::Comment(s) => out.push_str(s),
            Token::RawText { text, tag } => {
                if options.minify_css && tag == "style" {
                    out.push_str(&css::minify_css(text, options.css_dialect));
                } else {
                    out.push_str(text);
                }
            }
            Token::Open { text, tag, .. } => {
                out.push_str(tag::rewrite(text, tag, options).as_deref().unwrap_or(text));
            }
            Token::Close { text, .. } => out.push_str(text),
            Token::Text(s) => {
                if !options.collapse_whitespace {
                    out.push_str(s);
                } else if !s.trim().is_empty() {
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
            Token::RawText { text: s, .. } => {
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

    /// [`MinifyOptions`] with one switch flipped away from mjml's defaults.
    fn with(f: impl FnOnce(&mut MinifyOptions)) -> MinifyOptions {
        let mut options = MinifyOptions::default();
        f(&mut options);
        options
    }

    #[test]
    fn default_options_are_mjmls_except_remove_empty_attributes() {
        let options = MinifyOptions::default();
        assert!(options.collapse_whitespace);
        assert!(!options.minify_css);
        assert!(options.case_sensitive);
        // Deliberately `false` where mjml passes `true`: dropping `style=""` is an
        // attribute removal, and `minify` promises semantic invariance.
        assert!(!options.remove_empty_attributes);
    }

    #[test]
    fn minify_with_defaults_equals_minify() {
        let input = "<div>\n  <span>a  b</span>\n  <style>.x { color: red }</style>\n</div>";
        assert_eq!(minify_with(input, &MinifyOptions::default()), minify(input));
    }

    #[test]
    fn collapse_whitespace_off_keeps_text_verbatim() {
        let input = "<div>\n  <span>a    b</span>\n</div>";
        let out = minify_with(input, &with(|o| o.collapse_whitespace = false));
        assert_eq!(out, input);
    }

    #[test]
    fn minify_css_on_rewrites_only_style_bodies() {
        let input = "<style>\n  .a {\n    color: red;\n  }\n</style><pre>\n  keep  me\n</pre>";
        let out = minify_with(input, &with(|o| o.minify_css = true));
        assert!(out.starts_with("<style>.a{color:red}</style>"), "{out}");
        // <pre> is a raw-text element too, and is never touched.
        assert!(out.ends_with("<pre>\n  keep  me\n</pre>"), "{out}");
    }

    /// The semantic-invariance guarantee is a property of the *default* options,
    /// and `minifyCSS` is off there — a `<style>` body is raw text to
    /// [`mjmx_html_compare`], so any rewrite of it would be a visible difference.
    #[test]
    fn default_minify_of_a_document_with_css_is_semantically_invariant() {
        let input = concat!(
            "<html>\n  <head>\n    <style type=\"text/css\">\n",
            "      /* c */\n      .a :hover { color : red; }\n",
            "      @media only screen and (min-width:480px) { .b { width: 50% } }\n",
            "    </style>\n  </head>\n  <body>\n    <p>hi   there</p>\n  </body>\n</html>\n"
        );
        let out = minify(input);
        assert!(
            mjmx_html_compare::compare(input, &out).is_ok(),
            "minify changed the document: {out}"
        );
        assert!(out.len() < input.len());
    }

    #[test]
    fn minify_css_off_keeps_style_bodies_verbatim() {
        let input = "<style>\n  .a {\n    color: red;\n  }\n</style>";
        assert_eq!(minify_with(input, &MinifyOptions::default()), input);
    }

    #[test]
    fn remove_empty_attributes_drops_empty_class_and_style() {
        let input = r#"<div class="" style="" align="center">x</div>"#;
        // Off by default (mjmx divergence): the tag survives byte-for-byte.
        assert_eq!(minify_with(input, &MinifyOptions::default()), input);
        assert_eq!(
            minify_with(input, &with(|o| o.remove_empty_attributes = true)),
            r#"<div align="center">x</div>"#
        );
    }

    #[test]
    fn case_sensitive_off_lower_cases_attribute_names() {
        let input = r#"<td vAlign="top">x</td>"#;
        assert_eq!(minify_with(input, &MinifyOptions::default()), input);
        assert_eq!(
            minify_with(input, &with(|o| o.case_sensitive = false)),
            r#"<td valign="top">x</td>"#
        );
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
