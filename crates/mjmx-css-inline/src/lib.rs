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
//! - **Templating syntax is opaque too.** EJS (`<% %>`) and Handlebars
//!   (`{{ }}`) are masked out before the document is tokenized, so a construct
//!   that happens to contain markup characters is neither parsed as markup nor
//!   rewritten. This is unconditional, as it is in juice; `<mj-style inline>` in
//!   a Handlebars template is a normal thing to have. See [`mask`](mod@mask), and
//!   [`PreserveTag`] to register more delimiters.
//!
//! ## Options
//!
//! [`InlineOptions`] is mjmx's [`juiceOptions`](options), and its
//! [`preserve_tags`](InlineOptions::preserve_tags) field is mjml's
//! `juicePreserveTags`. Because there is no juice here to forward an object to,
//! each juice option was classified individually rather than accepted wholesale —
//! the [`options`](mod@options) module documents the full triage, including the
//! ones mjmx deliberately refuses.
//!
//! ## Known gaps
//!
//! Functional and structural pseudo-classes (`:nth-child`, `:not`, `:first-child`)
//! are decidable against a static document but are not implemented; a selector
//! using one is dropped rather than mis-applied. Likewise CSS Nesting. Neither
//! appears in mjml's own output or in the fixture corpus.
//!
//! `resolveCSSVariables` (juice's default `true`, always on here) resolves `var(--x)`
//! / `var(--x, fallback)` against the custom-property cascade and drops the `--x`
//! declarations — see [`apply`]. `inlinePseudoElements` (juice's default `false`,
//! opt-in via [`options::InlineOptions::inline_pseudo_elements`]) synthesizes
//! `::before`/`::after` from their `content` (strings, `url()`→img, `attr()`,
//! `var()`, `counter()` with a `counter-reset`/`counter-increment` subsystem) — see
//! [`pseudo`]. Both are ports of juice 10.0.1.

mod apply;
mod css;
mod dom;
mod html_attributes;
mod mask;
mod options;
mod pseudo;
mod selector;

pub use html_attributes::{HtmlAttribute, apply_html_attributes};
pub use options::{InlineOptions, PreserveTag, SUPPORTED_OPTIONS, refusal, unsupported_option};

/// Inline `css` into `html` with mjml's own settings, returning the rewritten
/// document.
///
/// `css` is the concatenated body of every `<mj-style inline="inline">`, in
/// source order. Rules are applied in the order given, so later stylesheets win
/// ties exactly as they would in a single sheet.
///
/// Inlining is skipped entirely when `css` holds no inlinable rule, in which case
/// `html` is returned unchanged.
///
/// Equivalent to [`inline_with`] under [`InlineOptions::default`].
#[must_use]
pub fn inline(html: &str, css: &str) -> String {
    inline_with(html, css, &InlineOptions::default())
}

/// [`inline`], under explicit [`InlineOptions`].
#[must_use]
pub fn inline_with(html: &str, css: &str, options: &InlineOptions) -> String {
    let rules = css::parse_stylesheet(css);
    if rules.is_empty() {
        return html.to_owned();
    }
    // The tree is built from the masked copy so templating syntax is not read as
    // markup, but every edit is spliced back into the original — which restores
    // the masked bytes without a decode pass.
    match mask::mask(html, &options.preserve_tags) {
        Some(masked) => apply::apply(html, &rules, &dom::build(&masked), options),
        None => apply::apply(html, &rules, &dom::build(html), options),
    }
}

#[cfg(test)]
mod tests {
    use super::{InlineOptions, PreserveTag, inline, inline_with};

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
    fn resolves_css_variables_like_juice() {
        // Verified against mjml 4.18/5.4 output: `--brand` is dropped, `var(--brand)`
        // resolves from the same element, `var(--missing, #00ff00)` takes its fallback,
        // and `var(--nope)` with no fallback is left verbatim.
        let out = inline(
            &document(r#"<td class="v" style="font-size:0px;">x</td>"#),
            ".v { --brand: #ff0000; color: var(--brand); background-color: var(--missing, #00ff00); border-color: var(--nope); }",
        );
        assert!(out.contains("color: #ff0000;"), "{out}");
        assert!(out.contains("background-color: #00ff00;"), "{out}");
        assert!(out.contains("border-color: var(--nope);"), "{out}");
        assert!(!out.contains("--brand"), "custom property must be dropped: {out}");
        // A resolved value reaches the attribute mirror too: mirroring reads the merged
        // declarations *after* substitution (on a `<table>`, the tag mjmx mirrors).
        let table = inline(
            &document(r#"<table class="v" style="font-size:0px;">x</table>"#),
            ".v { background-color: var(--x, #00ff00); }",
        );
        assert!(table.contains(r##"bgcolor="#00ff00""##), "{table}");
    }

    #[test]
    fn resolves_css_variables_up_the_ancestor_chain() {
        // A `--brand` defined on an ancestor is visible to a descendant `var(--brand)`,
        // nearest definition winning.
        let out = inline(
            &document(
                r#"<div class="outer"><td class="v" style="font-size:0px;">x</td></div>"#,
            ),
            ".outer { --brand: #123456; } .v { color: var(--brand); }",
        );
        assert!(out.contains("color: #123456;"), "{out}");
    }

    #[test]
    fn resolves_nested_variable_references() {
        // `--a: var(--b)` then `color: var(--a)` collapses to `--b`'s value.
        let out = inline(
            &document(r#"<td class="v" style="font-size:0px;">x</td>"#),
            ".v { --b: green; --a: var(--b); color: var(--a); }",
        );
        assert!(out.contains("color: green;"), "{out}");
        assert!(!out.contains("--a"), "{out}");
        assert!(!out.contains("--b"), "{out}");
    }

    fn pseudo_opts() -> InlineOptions {
        InlineOptions {
            inline_pseudo_elements: true,
            ..InlineOptions::default()
        }
    }

    #[test]
    fn pseudo_elements_are_dropped_by_default() {
        // Off by default (juice's default; mjml never enables it): no span is synthesized.
        let out = inline(
            &document("<p class=\"p\">x</p>"),
            ".p::before{content:\"NEW\"}",
        );
        assert!(!out.contains("NEW"), "{out}");
        assert!(!out.contains("<span"), "{out}");
    }

    #[test]
    fn pseudo_element_string_content_becomes_a_prepended_span() {
        let out = inline_with(
            &document("<p class=\"p\">x</p>"),
            ".p::before{content:\"NEW\";color:red}",
            &pseudo_opts(),
        );
        // A `<span>` with the rule's other declarations, prepended to the content.
        assert!(
            out.contains("<span style=\"color: red;\">NEW</span>x"),
            "{out}"
        );
    }

    #[test]
    fn pseudo_element_after_is_appended_and_url_is_an_img() {
        let out = inline_with(
            &document("<p class=\"p\">x</p>"),
            ".p::after{content:\"!\"} .p::before{content:url(dot.png)}",
            &pseudo_opts(),
        );
        // before = <img>, then the text, then after = <span>.
        assert!(out.contains("<img src=\"dot.png\">x<span>!</span>"), "{out}");
    }

    #[test]
    fn pseudo_element_counter_and_attr_and_full_juice_parity() {
        // Verified against juice 10.0.1 (inlinePseudoElements: true) output.
        let out = inline_with(
            &document(
                "<ol><li class=\"i\" data-x=\"Q\">a</li>\
                 <li class=\"i\" data-x=\"R\">b</li></ol><p class=\"p\">y</p>",
            ),
            "ol{counter-reset:item} li{counter-increment:item} \
             li.i::before{content:counter(item) \". \";color:red} \
             .p::after{content:\" [\" attr(data-x) \"]\"}",
            &pseudo_opts(),
        );
        // Counter resolves per element in document order.
        assert!(out.contains("<span style=\"color: red;\">1. </span>a"), "{out}");
        assert!(out.contains("<span style=\"color: red;\">2. </span>b"), "{out}");
        // counter-reset / counter-increment stay inlined on the real elements.
        assert!(out.contains("counter-reset: item;"), "{out}");
        assert!(out.contains("counter-increment: item;"), "{out}");
        // `.p` has no data-x, so attr() contributes nothing: " []".
        assert!(out.contains("y<span> []</span>"), "{out}");
    }

    #[test]
    fn pseudo_element_counter_styles() {
        let out = inline_with(
            &document("<ol><li class=\"i\">a</li><li class=\"i\">b</li><li class=\"i\">c</li></ol>"),
            "ol{counter-reset:c} li{counter-increment:c} li.i::before{content:counter(c, upper-latin)}",
            &pseudo_opts(),
        );
        assert!(out.contains("<span>A</span>a"), "{out}");
        assert!(out.contains("<span>B</span>b"), "{out}");
        assert!(out.contains("<span>C</span>c"), "{out}");
    }

    #[test]
    fn pseudo_element_roman_counter_matches_juice() {
        // juice 10.0.1: 4→iv, 40→xl, 1990→mcmxc (exercises the thousands + subtractive path).
        for (value, roman) in [(4, "iv"), (40, "xl"), (1990, "mcmxc")] {
            let css = format!(
                "ol{{counter-reset:c {}}} li{{counter-increment:c}} li.i::before{{content:counter(c,lower-roman)}}",
                value - 1
            );
            let out = inline_with(
                &document("<ol><li class=\"i\">a</li></ol>"),
                &css,
                &pseudo_opts(),
            );
            assert!(out.contains(&format!("<span>{roman}</span>")), "{value}: {out}");
        }
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
    fn handlebars_holding_markup_is_neither_parsed_nor_rewritten() {
        // Without masking the tokenizer reads a real `<div class="a">` inside the
        // string literal, and the inliner splices a `style` attribute into the
        // middle of the Handlebars expression. juice masks `{{ }}` on every call,
        // so mjml does not have this bug and neither may mjmx.
        let out = inline(
            &document("<span class=\"a\">{{ \"<div class='a'>\" }}</span>"),
            ".a{color:red}",
        );
        assert!(
            out.contains(r#"<span class="a" style="color: red;">{{ "<div class='a'>" }}</span>"#),
            "{out}"
        );
        assert_eq!(out.matches("color: red").count(), 1, "{out}");
    }

    #[test]
    fn a_masked_element_does_not_reparent_what_follows() {
        // The unclosed `<div>` hiding in the literal must not become an ancestor
        // of the `<p>`, or a descendant selector would match it.
        let out = inline(
            &document("<span>{{ \"<div>\" }}</span><p>x</p>"),
            "div p{color:red}",
        );
        assert!(!out.contains("color: red"), "{out}");
    }

    #[test]
    fn ejs_is_masked_too() {
        let out = inline(
            &document("<b class=\"a\"><%= \"<i class='a'>\" %></b>"),
            ".a{color:red}",
        );
        assert!(out.contains(r#"<%= "<i class='a'>" %>"#), "{out}");
        assert_eq!(out.matches("color: red").count(), 1, "{out}");
    }

    #[test]
    fn a_registered_tag_is_masked_on_top_of_the_defaults() {
        let html = document("<span class=\"a\"><# \"<div class='a'>\" </#</span>");
        // Unregistered, the markup inside the custom delimiters is real markup.
        assert_eq!(
            inline(&html, ".a{color:red}").matches("color: red").count(),
            2
        );

        let options =
            InlineOptions::default().with_preserve_tags([PreserveTag::new("myTag", "<#", "</#")]);
        let out = inline_with(&html, ".a{color:red}", &options);
        assert!(out.contains(r#"<# "<div class='a'>" </#"#), "{out}");
        assert_eq!(out.matches("color: red").count(), 1, "{out}");
        // The defaults are still in force alongside it.
        assert!(
            inline_with(
                &document("<p class=\"a\">{{ \"<i class='a'>\" }}</p>"),
                ".a{color:red}",
                &options
            )
            .matches("color: red")
            .count()
                == 1
        );
    }

    #[test]
    fn a_style_attribute_holding_a_code_block_survives_being_rewritten() {
        // The element *is* rewritten, so its existing declarations are re-emitted
        // — they must be the original bytes, not the mask.
        let out = inline(
            &document("<p class=\"a\" style=\"color: {{ c }};\">x</p>"),
            ".a{margin:0}",
        );
        assert!(
            out.contains(r#"style="margin: 0; color: {{ c }};""#),
            "{out}"
        );
    }

    #[test]
    fn the_attribute_mirrors_are_individually_switchable() {
        let html = document("<table><tr><td>c</td></tr></table>");
        let css = "table { width: 321px; height: 44px; background-color: #123456; }";

        let all_off = inline_with(
            &html,
            css,
            &InlineOptions {
                apply_width_attributes: false,
                apply_height_attributes: false,
                apply_attributes_table_elements: false,
                ..InlineOptions::default()
            },
        );
        assert!(!all_off.contains("width=\""), "{all_off}");
        assert!(!all_off.contains("height=\""), "{all_off}");
        assert!(!all_off.contains("bgcolor"), "{all_off}");
        // The declarations themselves are untouched by these switches.
        assert!(all_off.contains("width: 321px;"), "{all_off}");

        let height_only = inline_with(
            &html,
            css,
            &InlineOptions {
                apply_width_attributes: false,
                ..InlineOptions::default()
            },
        );
        assert!(!height_only.contains("width=\""), "{height_only}");
        assert!(height_only.contains(r#"height="44""#), "{height_only}");
        assert!(height_only.contains("bgcolor"), "{height_only}");
    }

    #[test]
    fn preserve_important_keeps_the_keyword_in_the_value_only() {
        let html = document("<table style=\"font-size:0px;\"><tr><td>c</td></tr></table>");
        let css = "table { width: 321px !important; }";
        let options = InlineOptions {
            preserve_important: true,
            ..InlineOptions::default()
        };
        let out = inline_with(&html, css, &options);
        assert!(out.contains("width: 321px !important;"), "{out}");
        // The mirror never carries the keyword.
        assert!(out.contains(r#"width="321""#), "{out}");
        // Default is unchanged: the keyword is dropped.
        assert!(
            inline(&html, css).contains("width: 321px;"),
            "{}",
            inline(&html, css)
        );
    }

    #[test]
    fn the_style_attribute_can_be_renamed() {
        let options = InlineOptions {
            style_attribute_name: "data-style".to_owned(),
            ..InlineOptions::default()
        };
        let out = inline_with(
            &document("<p class=\"a\" data-style=\"margin:0;\">x</p>"),
            ".a{color:red}",
            &options,
        );
        assert!(
            out.contains(r#"data-style="color: red; margin: 0;""#),
            "{out}"
        );
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
