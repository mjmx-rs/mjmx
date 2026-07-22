//! CSS inlining for mjmx: `<mj-style inline="inline">` applied to the rendered
//! HTML as `style` attributes.
//!
//! This is mjmx's stand-in for the [juice](https://github.com/Automattic/juice)
//! pass mjml runs at the end of `mjml2html`. It is a port of the *behaviour* —
//! read off mjml 4.18.0's own output, not off juice's source — of that one call:
//! `juice(html, { inlinePseudoElements: false, insertPreservedExtraCss: false,
//! extraCss: inlineStyle, applyStyleTags: false })`.
//!
//! ```
//! let html = mjmx_css_inline::inline(
//!     "<html><body><p class=\"a\" style=\"margin:0;\">hi</p></body></html>",
//!     ".a { color: red; }",
//! );
//! assert!(html.contains(r#"style="color: red; margin: 0;""#));
//! ```
//!
//! ## What it does, and does not, touch
//!
//! - **Only the inline bucket.** `applyStyleTags: false` means the document's own
//!   `<style>` tags are never inlined, and `insertPreservedExtraCss: false` means
//!   the CSS passed here is never emitted as a `<style>` tag. mjmx keeps the two
//!   `<mj-style>` buckets disjoint upstream, in `mjmx_semantic`; this function
//!   only ever sees the inline one.
//! - **Nothing that cannot become a `style` attribute is kept.** At-rules and
//!   state-dependent pseudo-selectors are dropped outright — see
//!   [`css`](mod@css) and [`selector`](mod@selector) for exactly what and why.
//! - **Untouched elements keep their bytes.** Rewriting is splice-based: an
//!   element no rule matched is not reserialized, so its attribute order,
//!   quoting and `font-size:0px;` spacing survive verbatim. Only matched
//!   elements get the `prop: value; ` rewrite, which is why mjml's output mixes
//!   the two spellings.
//! - **Comments are opaque.** The contents of `<!--[if mso | IE]>…<![endif]-->`
//!   are not markup here, which is what makes running this *after* mjmx's
//!   `mergeOutlookConditionnals` equivalent to running it before, as mjml does.
//!   See [`dom`](mod@dom).
//!
//! ## Known gaps
//!
//! Functional and structural pseudo-classes (`:nth-child`, `:not`, `:first-child`)
//! are decidable against a static document but are not implemented; a selector
//! using one is dropped rather than mis-applied. Likewise CSS Nesting. Neither
//! appears in mjml's own output or in the fixture corpus.

mod apply;
mod css;
mod dom;
mod selector;

/// Inline `css` into `html`, returning the rewritten document.
///
/// `css` is the concatenated body of every `<mj-style inline="inline">`, in
/// source order. Rules are applied in the order given, so later stylesheets win
/// ties exactly as they would in a single sheet.
///
/// Inlining is skipped entirely when `css` holds no inlinable rule, in which case
/// `html` is returned unchanged.
#[must_use]
pub fn inline(html: &str, css: &str) -> String {
    let rules = css::parse_stylesheet(css);
    if rules.is_empty() {
        return html.to_owned();
    }
    let tree = dom::build(html);
    apply::apply(html, &rules, &tree)
}

#[cfg(test)]
mod tests {
    use super::inline;

    /// Wrap a body fragment in the minimum document the inliner needs.
    fn document(body: &str) -> String {
        format!(
            "<!doctype html><html><head><style>.a{{color:blue}}</style></head><body>{body}</body></html>"
        )
    }

    #[test]
    fn an_empty_stylesheet_returns_the_input_unchanged() {
        let html = document("<p class=\"a\">x</p>");
        assert_eq!(inline(&html, ""), html);
        assert_eq!(inline(&html, "@media print { .a { color: red } }"), html);
    }

    #[test]
    fn unmatched_elements_keep_their_bytes() {
        let html = document(
            "<p class=\"a\" style=\"font-size:0px;\">x</p><p style=\"font-size:0px;\">y</p>",
        );
        let out = inline(&html, ".a{color:red}");
        assert!(
            out.contains(r#"<p class="a" style="color: red; font-size: 0px;">"#),
            "{out}"
        );
        assert!(out.contains(r#"<p style="font-size:0px;">y</p>"#), "{out}");
    }

    #[test]
    fn the_document_style_tag_is_neither_inlined_nor_disturbed() {
        let out = inline(&document("<p class=\"a\">x</p>"), ".a{color:red}");
        assert!(out.contains("<style>.a{color:blue}</style>"));
        assert!(
            out.contains(r#"<p class="a" style="color: red;">"#),
            "{out}"
        );
    }

    #[test]
    fn important_evicts_and_reappends_the_inline_declaration() {
        let out = inline(
            &document(
                "<p class=\"a\" style=\"font-size:0px;padding:10px;word-break:normal;\">x</p>",
            ),
            ".a{padding:0 !important}",
        );
        assert!(
            out.contains(r#"style="font-size: 0px; word-break: normal; padding: 0;""#),
            "{out}"
        );
    }

    #[test]
    fn table_and_dimension_attributes_are_mirrored() {
        let out = inline(
            &document("<table style=\"border-collapse:collapse;\"><tr><td>c</td></tr></table>"),
            "table { width: 321px; background-color: #123456; text-align: right; }",
        );
        assert!(out.contains(r#"width="321""#), "{out}");
        assert!(out.contains(r##"bgcolor="#123456""##), "{out}");
        assert!(out.contains(r#"align="right""#), "{out}");
        // Additive: the declarations stay in the style attribute too.
        assert!(out.contains("background-color: #123456;"), "{out}");
    }

    #[test]
    fn conditional_comment_contents_are_never_touched() {
        let html = document(
            "<!--[if mso | IE]><table class=\"a\"><tr><td class=\"a\"><![endif]--><p class=\"a\">x</p>",
        );
        let out = inline(&html, ".a{color:red}");
        assert!(
            out.contains("<!--[if mso | IE]><table class=\"a\"><tr><td class=\"a\"><![endif]-->"),
            "{out}"
        );
        assert_eq!(out.matches("color: red").count(), 1, "{out}");
    }
}
