//! Merging matched declarations into `style` attributes, and the attribute
//! mirrors that go with them.
//!
//! ## The cascade, as observed in mjml 4.18.0's output
//!
//! A rendered mjml document already carries a `style` attribute on nearly every
//! element, and an inline style beats any selector. So, per element:
//!
//! - declarations from matched rules are ranked by `(!important, specificity,
//!   source order)` and applied in that order, last write winning;
//! - a winner whose property the element's own `style` attribute already sets is
//!   **discarded** — the pre-existing declaration stands;
//! - a winner for a property the attribute does not set is **prepended**;
//! - an `!important` winner deletes the pre-existing declaration from its
//!   position and is **appended** at the end, *without* the `!important` keyword
//!   (there is nothing left for it to out-rank).
//!
//! The whole attribute of a touched element is then rewritten in `prop: value; `
//! form. Elements no rule matched are not rewritten at all, which is why one
//! document ends up with both spacings — mjml's own output does too.

use std::fmt::Write as _;
use std::ops::Range;

use mjmx_html_tokenizer::Attr;

use crate::css::{Declaration, Rule};
use crate::dom::{Element, Tree};
use crate::options::InlineOptions;

/// `style` declarations that also have to be mirrored onto an HTML attribute of
/// a `<table>`, in the order the attributes are appended. The declaration is
/// *kept* as well — the mirror is additive.
const TABLE_ATTRIBUTES: &[(&str, &str)] = &[
    ("background-color", "bgcolor"),
    ("background-image", "background"),
    ("text-align", "align"),
    ("vertical-align", "valign"),
];

/// Elements whose `width`/`height` declarations are mirrored onto the matching
/// presentational attribute — the ones where Outlook honours the attribute and
/// ignores the CSS.
fn takes_dimension_attributes(tag: &str) -> bool {
    matches!(tag, "table" | "td" | "th" | "img")
}

/// One splice into the document: replace `range` with `text`.
pub(crate) struct Edit {
    pub(crate) range: Range<usize>,
    pub(crate) text: String,
}

/// A declaration on its way into a `style` attribute.
struct Merged {
    property: String,
    value: String,
}

impl Merged {
    fn write(&self, out: &mut String) {
        if !out.is_empty() {
            out.push(' ');
        }
        let _ = write!(out, "{}: {};", self.property, self.value);
    }
}

/// Apply `rules` to `html`, returning the rewritten document.
///
/// `tree` was built from the *masked* document (see [`mask`](crate::mask)), whose
/// byte offsets are the same as `html`'s — so the edits it yields splice into the
/// original, which is what restores every masked templating construct. Attribute
/// *values* still have to be read back out of `html`, since the tree's borrow
/// exposes the masked bytes; that is what [`attr_value`] is for.
pub(crate) fn apply(
    html: &str,
    rules: &[Rule],
    tree: &Tree<'_>,
    options: &InlineOptions,
) -> String {
    let mut edits: Vec<Edit> = Vec::new();
    for (index, element) in tree.elements.iter().enumerate() {
        if element.inlinable {
            rewrite(html, rules, tree, index, element, options, &mut edits);
        }
    }
    splice(html, edits)
}

/// Queue the edits for one element, if any rule matched it.
fn rewrite(
    html: &str,
    rules: &[Rule],
    tree: &Tree<'_>,
    index: usize,
    element: &Element<'_>,
    options: &InlineOptions,
    edits: &mut Vec<Edit>,
) {
    let winners = winners(rules, tree, index);
    if winners.is_empty() {
        return;
    }
    let merged = merge(&winners, element, html, options);

    // Attributes that do not exist yet are collected into one insertion at the
    // end of the attribute list, so they land in a stable order.
    let mut appended = String::new();
    let mut style = String::new();
    for declaration in &merged {
        declaration.write(&mut style);
    }
    if !style.is_empty() {
        set_attribute(
            element,
            &options.style_attribute_name,
            &style,
            edits,
            &mut appended,
        );
    }
    for (value, attribute) in mirrored_attributes(element, &merged, options) {
        set_attribute(element, attribute, &value, edits, &mut appended);
    }
    if !appended.is_empty() {
        edits.push(Edit {
            range: element.attrs_end..element.attrs_end,
            text: appended,
        });
    }
}

/// The declarations that win the cascade for one element, in the order they are
/// first set — which is the order they are written back out in.
fn winners(rules: &[Rule], tree: &Tree<'_>, index: usize) -> Vec<Declaration> {
    // `(important, specificity, rule order)` ascending, so applying in order and
    // letting the last write win reproduces the cascade, source order included.
    let mut ranked: Vec<(bool, _, usize, &Declaration)> = Vec::new();
    for (order, rule) in rules.iter().enumerate() {
        if rule.selector.matches(tree, index) {
            for declaration in &rule.declarations {
                ranked.push((
                    declaration.important,
                    rule.specificity(),
                    order,
                    declaration,
                ));
            }
        }
    }
    if ranked.is_empty() {
        return Vec::new();
    }
    // Stable, so two declarations of the same property in one rule keep their
    // order and the later still wins.
    ranked.sort_by_key(|(important, specificity, order, _)| (*important, *specificity, *order));

    let mut winners: Vec<Declaration> = Vec::new();
    for (.., declaration) in ranked {
        match winners
            .iter_mut()
            .find(|w| w.property == declaration.property)
        {
            // Overwritten in place: a property keeps the position it was first
            // given, however many rules go on to set it.
            Some(existing) => existing.clone_from(declaration),
            None => winners.push(declaration.clone()),
        }
    }
    winners
}

/// Fold the cascade winners into the element's existing `style` attribute.
fn merge(
    winners: &[Declaration],
    element: &Element<'_>,
    html: &str,
    options: &InlineOptions,
) -> Vec<Merged> {
    let mut existing: Vec<Merged> = element
        .attr(&options.style_attribute_name)
        .map(|attr| parse_style_attribute(attr_value(html, attr)))
        .unwrap_or_default();

    let mut prepended: Vec<Merged> = Vec::new();
    let mut appended: Vec<Merged> = Vec::new();
    for winner in winners {
        let merged = Merged {
            property: winner.property.clone(),
            value: if winner.important && options.preserve_important {
                format!("{} !important", winner.value)
            } else {
                winner.value.clone()
            },
        };
        let held = existing.iter().any(|d| d.property == winner.property);
        if winner.important {
            // The `!important` keyword itself does not survive: it has already
            // done its job by evicting the inline declaration.
            existing.retain(|d| d.property != winner.property);
            appended.push(merged);
        } else if !held {
            prepended.push(merged);
        }
    }

    prepended.extend(existing);
    prepended.extend(appended);
    prepended
}

/// The attribute's value as it stands in `source`.
///
/// [`Attr::value`] borrows whichever string the tree was built from, which is the
/// *masked* document; the span is valid in both, so reading through it recovers
/// the real bytes. The value ends just before the closing quote (when there is
/// one) and is [`Attr::value`]'s own length long.
fn attr_value<'a>(source: &'a str, attr: &Attr<'_>) -> &'a str {
    let raw = &source[attr.span.clone()];
    let end = attr.span.end - usize::from(raw.ends_with(['"', '\'']));
    &source[end - attr.value.len()..end]
}

/// Split a `style` attribute value into declarations.
///
/// The split is on top-level `;` only: a `;` inside `url(…)`, a quoted string or
/// a `(…)` group belongs to the value.
fn parse_style_attribute(value: &str) -> Vec<Merged> {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut quote: Option<u8> = None;
    let push = |slice: &str, out: &mut Vec<Merged>| {
        if let Some((property, value)) = slice.split_once(':') {
            let property = property.trim();
            if !property.is_empty() {
                out.push(Merged {
                    property: property.to_ascii_lowercase(),
                    value: value.trim().to_owned(),
                });
            }
        }
    };
    for i in 0..bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None => match b {
                b'"' | b'\'' => quote = Some(b),
                b'(' => depth += 1,
                b')' => depth = depth.saturating_sub(1),
                b';' if depth == 0 => {
                    push(&value[start..i], &mut out);
                    start = i + 1;
                }
                _ => {}
            },
        }
    }
    push(&value[start..], &mut out);
    out
}

/// The presentational attributes to mirror the merged declarations onto, as
/// `(value, attribute name)` pairs.
///
/// Two independent mappings live here. `width`/`height` become **unitless**
/// attributes on tables and images, and only from a plain pixel or unitless
/// length — `width: 100%` and `height: auto` have no attribute spelling and are
/// left alone. The `<table>`-only mirrors are [`TABLE_ATTRIBUTES`].
///
/// Each mapping is individually switchable, under juice's names:
/// `applyWidthAttributes`, `applyHeightAttributes` and
/// `applyAttributesTableElements`. All three default on, as they do in juice, so
/// this is what mjml does unless a caller says otherwise.
fn mirrored_attributes(
    element: &Element<'_>,
    merged: &[Merged],
    options: &InlineOptions,
) -> Vec<(String, &'static str)> {
    let lookup = |property: &str| {
        merged
            .iter()
            .find(|d| d.property == property)
            // An attribute mirror never carries the keyword, whatever
            // `preserve_important` says — juice strips it here too.
            .map(|d| without_important(&d.value))
    };
    let mut out = Vec::new();
    if takes_dimension_attributes(&element.tag) {
        let dimensions = [
            ("width", options.apply_width_attributes),
            ("height", options.apply_height_attributes),
        ];
        for (dimension, enabled) in dimensions {
            if !enabled {
                continue;
            }
            if let Some(value) = lookup(dimension).and_then(as_pixel_length) {
                out.push((value.to_owned(), dimension));
            }
        }
    }
    if options.apply_attributes_table_elements && element.tag == "table" {
        for (property, attribute) in TABLE_ATTRIBUTES {
            if let Some(value) = lookup(property) {
                out.push((unwrap_url(value).to_owned(), *attribute));
            }
        }
    }
    out
}

/// A declaration value without its trailing `!important`.
fn without_important(value: &str) -> &str {
    value
        .strip_suffix("!important")
        .map_or(value, str::trim_end)
}

/// A length that can be written as an HTML dimension attribute: an integer
/// number of pixels, with or without the unit, returned without it.
fn as_pixel_length(value: &str) -> Option<&str> {
    let digits = value.strip_suffix("px").unwrap_or(value);
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())).then_some(digits)
}

/// Unwrap `url(…)`, quotes included, so `background-image` can become the
/// `background` attribute's bare URL.
fn unwrap_url(value: &str) -> &str {
    let Some(inner) = value
        .strip_prefix("url(")
        .or_else(|| value.strip_prefix("URL("))
        .and_then(|rest| rest.strip_suffix(')'))
    else {
        return value;
    };
    let inner = inner.trim();
    inner
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| inner.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(inner)
}

/// Set `name="value"` on `element`: in place when the attribute already exists,
/// otherwise queued onto `appended` for insertion after the last attribute.
pub(crate) fn set_attribute(
    element: &Element<'_>,
    name: &str,
    value: &str,
    edits: &mut Vec<Edit>,
    appended: &mut String,
) {
    let text = format!("{name}=\"{}\"", escape_attribute(value));
    match element.attr(name) {
        Some(attr) => edits.push(Edit {
            range: attr.span.clone(),
            text,
        }),
        None => {
            appended.push(' ');
            appended.push_str(&text);
        }
    }
}

/// Escape what a double-quoted attribute value cannot hold verbatim. CSS this
/// pass emits is almost always quote-free, but `content: "\""` and
/// `url("…")` are legal and must not break the tag.
fn escape_attribute(value: &str) -> std::borrow::Cow<'_, str> {
    if value.contains('"') {
        std::borrow::Cow::Owned(value.replace('"', "&quot;"))
    } else {
        std::borrow::Cow::Borrowed(value)
    }
}

/// Apply every queued edit to `html`. Ranges never overlap: each one is either a
/// single attribute's span or an empty insertion point past all of them.
pub(crate) fn splice(html: &str, mut edits: Vec<Edit>) -> String {
    if edits.is_empty() {
        return html.to_owned();
    }
    edits.sort_by_key(|edit| (edit.range.start, edit.range.end));
    let mut out = String::with_capacity(html.len() + edits.len() * 24);
    let mut cursor = 0;
    for edit in edits {
        // Defensive: a malformed document could in principle produce overlapping
        // spans, and dropping the later edit is better than panicking on a slice.
        if edit.range.start < cursor {
            continue;
        }
        out.push_str(&html[cursor..edit.range.start]);
        out.push_str(&edit.text);
        cursor = edit.range.end;
    }
    out.push_str(&html[cursor..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style_of(value: &str) -> Vec<(String, String)> {
        parse_style_attribute(value)
            .into_iter()
            .map(|d| (d.property, d.value))
            .collect()
    }

    #[test]
    fn style_attributes_split_on_top_level_semicolons() {
        assert_eq!(
            style_of("font-size:0px;padding:10px 25px;"),
            [
                ("font-size".to_owned(), "0px".to_owned()),
                ("padding".to_owned(), "10px 25px".to_owned())
            ]
        );
        assert_eq!(
            style_of("background:url(a;b.png)"),
            [("background".to_owned(), "url(a;b.png)".to_owned())]
        );
    }

    #[test]
    fn only_integral_pixel_lengths_become_attributes() {
        assert_eq!(as_pixel_length("321px"), Some("321"));
        assert_eq!(as_pixel_length("321"), Some("321"));
        assert_eq!(as_pixel_length("100%"), None);
        assert_eq!(as_pixel_length("auto"), None);
        assert_eq!(as_pixel_length("12.5px"), None);
    }

    #[test]
    fn background_image_urls_are_unwrapped() {
        assert_eq!(unwrap_url("url(a.png)"), "a.png");
        assert_eq!(unwrap_url("url( \"a.png\" )"), "a.png");
        assert_eq!(unwrap_url("url('a.png')"), "a.png");
        assert_eq!(unwrap_url("#123456"), "#123456");
    }
}
