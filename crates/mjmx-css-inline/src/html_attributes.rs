//! The `<mj-html-attributes>` pass: add HTML attributes to every element a CSS
//! selector matches.
//!
//! This is mjmx's stand-in for the cheerio step mjml runs on the assembled body,
//! read off mjml 4.18.0's own output (`mjml-core/src/index.js`, the
//! `globalData.htmlAttributes` loop):
//!
//! ```js
//! const $ = cheerio.load(content, { xmlMode: true, decodeEntities: false })
//! each(globalData.htmlAttributes, (data, selector) => {
//!   each(data, (value, attrName) => {
//!     $(selector).each(function () { $(this).attr(attrName, value || '') })
//!   })
//! })
//! ```
//!
//! Behaviour matched against real mjml output:
//!
//! - a matched element's attribute is **overwritten** when it already exists
//!   (`.attr()` is a set), and added at the end of the attribute list otherwise;
//! - a selector that matches nothing is a silent no-op;
//! - the pass runs on the body content only, so a selector never reaches head
//!   metadata — mjmx runs it on the whole document and gates on the same
//!   `inlinable` flag the inliner uses (plus the structural `html`/`body` tags,
//!   which are not part of mjml's cheerio `content`);
//! - **conditional comments are opaque.** The ghost tables inside
//!   `<!--[if mso | IE]>…<![endif]-->` are comment text to the tokenizer, so a
//!   `table` selector reaches only the real tables — exactly what mjml's output
//!   shows (cheerio, fed the same commented markup, never sees them either). This
//!   is why running the pass *after* mjmx's `mergeOutlookConditionnals` is
//!   equivalent to running it before, as mjml does. See [`crate::dom`].
//!
//! Order: mjml runs this **before** juice, so mjmx runs it before CSS inlining —
//! an added attribute can be a selector target for the inliner that follows.
//!
//! Unlike the inliner, templating syntax is **not** masked here: mjml's cheerio
//! pass does not mask either (only juice's `codeBlocks` do), so matching that
//! behaviour means tokenizing the document as-is.
//!
//! A selector mjmx cannot parse (a pseudo-class, say) is dropped rather than
//! mis-applied, the same known gap the selector engine has for the inliner.

use crate::apply::{Edit, set_attribute, splice};
use crate::dom;
use crate::selector::Selector;

/// One `(selector, name, value)` triple to apply, in source order.
pub struct HtmlAttribute<'a> {
    /// The CSS selector (mjml's `<mj-selector path>`).
    pub selector: &'a str,
    /// The attribute name to set.
    pub name: &'a str,
    /// The attribute value (mjml's `value || ''`, so an empty body sets `name=""`).
    pub value: &'a str,
}

/// Apply `attributes` to `html`, returning the rewritten document.
///
/// With an empty list — or one whose every selector is unparseable — `html` is
/// returned untouched, so the default render path is unchanged byte for byte.
#[must_use]
pub fn apply_html_attributes(html: &str, attributes: &[HtmlAttribute<'_>]) -> String {
    if attributes.is_empty() {
        return html.to_owned();
    }
    // Parse each selector once. A selector that does not parse is dropped (`None`).
    let parsed: Vec<(Option<Selector>, &str, &str)> = attributes
        .iter()
        .map(|a| (Selector::parse(a.selector), a.name, a.value))
        .collect();
    if parsed.iter().all(|(sel, ..)| sel.is_none()) {
        return html.to_owned();
    }

    let tree = dom::build(html);
    let mut edits: Vec<Edit> = Vec::new();
    for (index, element) in tree.elements.iter().enumerate() {
        // Restrict to the body content mjml's cheerio pass sees: everything the
        // inliner would touch, minus the structural document tags that only exist
        // in mjmx's fully-assembled output (mjml's `content` is body-inner).
        if !element.inlinable || matches!(element.tag.as_ref(), "html" | "body") {
            continue;
        }
        // Resolve the winning value per attribute name across every matching triple,
        // the last matching triple winning (cheerio applies each in order, `.attr()`
        // overwriting). First-seen order is kept so appended attributes are stable.
        let mut resolved: Vec<(&str, &str)> = Vec::new();
        for (selector, name, value) in &parsed {
            let Some(selector) = selector else { continue };
            if selector.matches(&tree, index) {
                match resolved
                    .iter_mut()
                    .find(|(n, _)| n.eq_ignore_ascii_case(name))
                {
                    Some(entry) => entry.1 = value,
                    None => resolved.push((name, value)),
                }
            }
        }
        if resolved.is_empty() {
            continue;
        }
        let mut appended = String::new();
        for (name, value) in resolved {
            set_attribute(element, name, value, &mut edits, &mut appended);
        }
        if !appended.is_empty() {
            edits.push(Edit {
                range: element.attrs_end..element.attrs_end,
                text: appended,
            });
        }
    }
    splice(html, edits)
}

#[cfg(test)]
mod tests {
    use super::{HtmlAttribute, apply_html_attributes};

    fn document(body: &str) -> String {
        format!("<!doctype html><html><head></head><body>{body}</body></html>")
    }

    #[test]
    fn a_matched_element_gains_the_attribute() {
        let out = apply_html_attributes(
            &document("<div class=\"a\"><p>x</p></div>"),
            &[HtmlAttribute {
                selector: ".a p",
                name: "data-track",
                value: "hello",
            }],
        );
        assert!(out.contains(r#"<p data-track="hello">x</p>"#), "{out}");
    }

    #[test]
    fn an_existing_attribute_is_overwritten_in_place() {
        let out = apply_html_attributes(
            &document("<table border=\"0\"><tr><td>c</td></tr></table>"),
            &[HtmlAttribute {
                selector: "table",
                name: "border",
                value: "9",
            }],
        );
        assert!(out.contains(r#"<table border="9">"#), "{out}");
        assert!(!out.contains(r#"border="0""#), "{out}");
    }

    #[test]
    fn a_selector_matching_nothing_is_a_no_op() {
        let html = document("<div class=\"a\">x</div>");
        let out = apply_html_attributes(
            &html,
            &[HtmlAttribute {
                selector: ".none",
                name: "data-x",
                value: "y",
            }],
        );
        assert_eq!(out, html);
    }

    #[test]
    fn conditional_comment_contents_are_never_touched() {
        let out = apply_html_attributes(
            &document(
                "<!--[if mso | IE]><table><tr><td><![endif]--><table class=\"real\"><tr><td>c</td></tr></table>",
            ),
            &[HtmlAttribute {
                selector: "table",
                name: "border",
                value: "9",
            }],
        );
        // Only the real table (outside the conditional) is rewritten.
        assert_eq!(out.matches(r#"border="9""#).count(), 1, "{out}");
        assert!(
            out.contains("<!--[if mso | IE]><table><tr><td><![endif]-->"),
            "{out}"
        );
    }

    #[test]
    fn head_metadata_is_out_of_reach() {
        let out = apply_html_attributes(
            &document("<div class=\"a\">x</div>"),
            &[HtmlAttribute {
                selector: "html",
                name: "data-x",
                value: "y",
            }],
        );
        assert!(!out.contains("data-x"), "{out}");
    }
}
