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
/// a table element (see [`is_table_element`]), in the order the attributes are
/// appended. The declaration is *kept* as well — the mirror is additive.
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

/// The elements juice's `applyAttributesTableElements` mirrors [`TABLE_ATTRIBUTES`]
/// onto — juice's `tableElements` set, not `<table>` alone. Tags reach here
/// already lower-cased by the tokenizer, so no fold is needed.
fn is_table_element(tag: &str) -> bool {
    matches!(
        tag,
        "table" | "th" | "tr" | "td" | "caption" | "colgroup" | "col" | "thead" | "tbody" | "tfoot"
    )
}

/// A value juice will not mirror onto a presentational attribute: a CSS gradient
/// has no attribute spelling, so juice's `/(linear|radial)-gradient\(/i` guard
/// drops the mirror while keeping the `style` declaration.
fn is_gradient(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("linear-gradient(") || lower.contains("radial-gradient(")
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
    // The custom-property (`--x`) cascade for `var()` resolution: each element's
    // final `--*` declarations (matched rules folded into its own `style`), so a
    // lookup can walk ancestors like juice's `findVariableValue`. Built only when
    // resolution is on, and empty for elements a stylesheet cannot touch.
    //
    // Building it runs the whole cascade a second time per element, so it is skipped
    // when the document provably has no `var()` to resolve — see [`uses_var`]. That is
    // an equivalence, not an approximation: with no `var()` anywhere,
    // [`substitute_vars`] is the identity, so the maps could never be read.
    let substitute = options.resolve_css_variables && uses_var(html, rules);
    let custom_maps: Vec<Vec<(String, String)>> = if substitute {
        tree.elements
            .iter()
            .enumerate()
            .map(|(index, element)| custom_properties(html, rules, tree, index, element, options))
            .collect()
    } else {
        Vec::new()
    };

    let mut edits: Vec<Edit> = Vec::new();
    for (index, element) in tree.elements.iter().enumerate() {
        if element.inlinable {
            rewrite(
                html,
                rules,
                tree,
                index,
                element,
                options,
                substitute,
                &custom_maps,
                &mut edits,
            );
        }
    }
    // juice's `inlinePseudoElements`: synthesize `::before`/`::after` and splice them
    // in with the same edit list (so byte offsets stay valid against `html`).
    if options.inline_pseudo_elements {
        edits.extend(crate::pseudo::pseudo_edits(html, rules, tree, options));
    }
    splice(html, edits)
}

/// Queue the edits for one element, if any rule matched it.
#[allow(clippy::too_many_arguments)]
fn rewrite(
    html: &str,
    rules: &[Rule],
    tree: &Tree<'_>,
    index: usize,
    element: &Element<'_>,
    options: &InlineOptions,
    substitute: bool,
    custom_maps: &[Vec<(String, String)>],
    edits: &mut Vec<Edit>,
) {
    let winners = winners(rules, tree, index);
    if winners.is_empty() {
        return;
    }
    let mut merged = merge(&winners, element, html, options);

    // juice's `resolveCSSVariables`: substitute every `var()` against the cascade
    // (before mirroring, so a resolved colour still reaches `bgcolor`) and drop the
    // `--x` declarations from the output.
    if options.resolve_css_variables {
        // The substitution itself is skipped when the document has no `var()` at all
        // (it would be the identity), but the `--x` declarations are still dropped —
        // juice removes them from the output whether or not anything referenced them.
        if substitute {
            for declaration in &mut merged {
                declaration.value =
                    substitute_vars(&declaration.value, index, tree, custom_maps, 0);
            }
        }
        merged.retain(|declaration| !declaration.property.starts_with("--"));
    }

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

/// Whether any `var()` reference can reach the output, and so whether the custom-property
/// cascade is worth building.
///
/// Every value the inliner writes comes from either a stylesheet declaration or an
/// existing `style` attribute in the document, so if neither mentions `var(` then no
/// merged value can, and [`substitute_vars`] would return each value unchanged. Emails
/// with custom properties are the exception rather than the rule, and the map costs a
/// second full cascade over every element to build.
fn uses_var(html: &str, rules: &[Rule]) -> bool {
    rules
        .iter()
        .any(|rule| rule.declarations.iter().any(|d| d.value.contains("var(")))
        || html.contains("var(")
}

/// The declarations that win the cascade for one element, in the order they are
/// first set — which is the order they are written back out in.
pub(crate) fn winners(rules: &[Rule], tree: &Tree<'_>, index: usize) -> Vec<Declaration> {
    // `(important, specificity, rule order)` ascending, so applying in order and
    // letting the last write win reproduces the cascade, source order included.
    let mut ranked: Vec<(bool, _, usize, &Declaration)> = Vec::new();
    for (order, rule) in rules.iter().enumerate() {
        // A `::before`/`::after` rule never inlines onto a real element — it is the
        // pseudo pass's input.
        if rule.pseudo.is_some() {
            continue;
        }
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
    if options.apply_attributes_table_elements && is_table_element(&element.tag) {
        for (property, attribute) in TABLE_ATTRIBUTES {
            if let Some(value) = lookup(property) {
                // juice refuses a gradient: it has no attribute spelling, so the
                // mirror is skipped (the declaration is still kept in `style`).
                if is_gradient(value) {
                    continue;
                }
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

// ---- juice's `resolveCSSVariables`: custom-property cascade + `var()` substitution ----

/// One element's final custom properties (`--name → value`), as juice's
/// `styleProps` would hold them: its matched-rule winners folded into its own
/// `style` attribute, filtered to `--*`. Empty for an element no stylesheet may
/// touch. This is what an ancestor walk reads during resolution.
pub(crate) fn custom_properties(
    html: &str,
    rules: &[Rule],
    tree: &Tree<'_>,
    index: usize,
    element: &Element<'_>,
    options: &InlineOptions,
) -> Vec<(String, String)> {
    if !element.inlinable {
        return Vec::new();
    }
    let winners = winners(rules, tree, index);
    merge(&winners, element, html, options)
        .into_iter()
        .filter(|declaration| declaration.property.starts_with("--"))
        .map(|declaration| (declaration.property, declaration.value))
        .collect()
}

/// juice's `findVariableValue`: the value of custom property `name`, taken from
/// the nearest element up the ancestor chain (the element itself first) that
/// defines it. Names are matched against the lower-cased declarations mjmx keeps.
pub(crate) fn resolve_var(
    index: usize,
    name: &str,
    tree: &Tree<'_>,
    custom_maps: &[Vec<(String, String)>],
) -> Option<String> {
    let name = name.trim().to_ascii_lowercase();
    let mut current = Some(index);
    while let Some(i) = current {
        if let Some(map) = custom_maps.get(i) {
            if let Some((_, value)) = map.iter().find(|(n, _)| *n == name) {
                return Some(value.clone());
            }
        }
        current = tree.elements[i].parent;
    }
    None
}

/// juice's `replaceVariables`: rewrite every `var(--name)` / `var(--name,
/// fallback)` in `value`. A resolved value is itself re-scanned (so `var()`s that
/// appear from a substitution resolve too); an undefined variable with no
/// fallback is left verbatim. `depth` guards against a custom-property cycle.
pub(crate) fn substitute_vars(
    value: &str,
    index: usize,
    tree: &Tree<'_>,
    custom_maps: &[Vec<(String, String)>],
    depth: usize,
) -> String {
    if depth > 32 {
        return value.to_owned();
    }
    let Some((start, open)) = find_var_call(value) else {
        return value.to_owned();
    };
    let Some(close) = matching_paren(value, open) else {
        return value.to_owned();
    };
    let (name, fallback) = split_var_args(&value[open + 1..close]);
    let before = &value[..start];
    let after = &value[close + 1..];
    let replacement = match resolve_var(index, name, tree, custom_maps) {
        // A found value can contain further `var()`s, resolved from the *consuming*
        // element (juice re-scans against the same element).
        Some(resolved) => substitute_vars(&resolved, index, tree, custom_maps, depth + 1),
        None => match fallback {
            Some(fallback) => substitute_vars(fallback.trim(), index, tree, custom_maps, depth + 1),
            // Undefined and no fallback: keep the `var(…)` literal, skip past it.
            None => value[start..=close].to_owned(),
        },
    };
    format!(
        "{before}{replacement}{}",
        substitute_vars(after, index, tree, custom_maps, depth + 1)
    )
}

/// Locate the first `var(` function call: `(start, open)` where `start` is the
/// `v` and `open` is the `(`. The identifier immediately before the `(` (allowing
/// whitespace) must be exactly `var` (case-insensitive), so `svar(` is not one.
fn find_var_call(value: &str) -> Option<(usize, usize)> {
    let bytes = value.as_bytes();
    for open in 0..bytes.len() {
        if bytes[open] != b'(' {
            continue;
        }
        let mut ident_end = open;
        while ident_end > 0 && bytes[ident_end - 1].is_ascii_whitespace() {
            ident_end -= 1;
        }
        let mut ident_start = ident_end;
        while ident_start > 0
            && (bytes[ident_start - 1].is_ascii_alphabetic() || bytes[ident_start - 1] == b'-')
        {
            ident_start -= 1;
        }
        if value[ident_start..ident_end].eq_ignore_ascii_case("var") {
            return Some((ident_start, open));
        }
    }
    None
}

/// The index of the `)` that closes the `(` at `open`, respecting nesting and
/// quotes.
fn matching_paren(value: &str, open: usize) -> Option<usize> {
    let bytes = value.as_bytes();
    let mut depth = 0i32;
    let mut quote: Option<u8> = None;
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        match quote {
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None => match b {
                b'"' | b'\'' => quote = Some(b),
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            },
        }
    }
    None
}

/// Split a `var()`'s arguments on the first top-level comma into the custom
/// property name and the optional fallback (`var(--x)` → `("--x", None)`,
/// `var(--x, y)` → `("--x", Some(" y"))`).
fn split_var_args(inner: &str) -> (&str, Option<&str>) {
    let bytes = inner.as_bytes();
    let mut depth = 0i32;
    let mut quote: Option<u8> = None;
    for (i, &b) in bytes.iter().enumerate() {
        match quote {
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None => match b {
                b'"' | b'\'' => quote = Some(b),
                b'(' => depth += 1,
                b')' => depth -= 1,
                b',' if depth == 0 => return (&inner[..i], Some(&inner[i + 1..])),
                _ => {}
            },
        }
    }
    (inner, None)
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
