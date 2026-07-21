//! The universal `css-class` attribute.
//!
//! Every mjml `BodyComponent` accepts `css-class`; the value is appended to the
//! element's rendered `class` attribute. mjml never handles it centrally — each
//! component decides *where* the class lands (the column's responsive `<div>`, the
//! section's outer `<div>`, the `<td>` a content component renders itself into,
//! …) — so these helpers only provide the two formatting rules the components
//! share, and each renderer places the result exactly where mjml does.
//!
//! The two rules, straight from mjml 4.18.0:
//!
//! * `htmlAttributes` (`mjml-core/lib/createComponent.js`) drops attributes whose
//!   value is `null`/`undefined`, but keeps the empty string. An unset `css-class`
//!   therefore *omits* `class` entirely, while `suffixCssClasses` — which returns
//!   `''` rather than `undefined` — leaves the `class=""` that our ghost markup
//!   already reproduces.
//! * `suffixCssClasses(classes, suffix)` (`mjml-core/lib/helpers/suffixCssClasses.js`)
//!   is `classes ? classes.split(' ').map(c => `${c}-${suffix}`).join(' ') : ''`.

use std::borrow::Cow;

use mjmx_ast::Element;

use crate::context::RenderContext;

/// Resolve `css-class` for `el` through the semantic layer, so `<mj-attributes>`
/// and `mj-class` can set it just like an inline attribute.
///
/// An empty value is treated as unset, matching JS truthiness in every mjml
/// `css-class` site (`this.getAttribute('css-class') ? … : …`).
///
/// The [`Cow`] is `Owned` only in mjml's multi-`mj-class` concatenation case (see
/// [`mjmx_semantic::Semantic::css_class`]); everything else still borrows the arena.
/// Call sites hand `.as_deref()` to the formatting helpers below.
pub(crate) fn css_class<'a>(ctx: &RenderContext<'a>, el: &Element<'a>) -> Option<Cow<'a, str>> {
    ctx.semantic.css_class(el).filter(|value| !value.is_empty())
}

/// mjml's `htmlAttributes({ class })`: `class="…" ` when set, nothing when unset.
///
/// The trailing space keeps the caller's format string readable — the attribute is
/// always followed by another one in mjml's key order.
pub(crate) fn class_attr(class: Option<&str>) -> String {
    class.map_or_else(String::new, |class| format!("class=\"{class}\" "))
}

/// Same as [`class_attr`], but with the space *before* the attribute — for tags
/// whose `class` is the only attribute (`<tr>`, `<td>` in `mj-accordion`), which
/// mjml renders as a bare `<tr>` when unset.
pub(crate) fn leading_class_attr(class: Option<&str>) -> String {
    class.map_or_else(String::new, |class| format!(" class=\"{class}\""))
}

/// mjml's `suffixCssClasses`: `"a b"` + `outlook` → `"a-outlook b-outlook"`.
///
/// Returns the empty string when unset — that is why mjml's ghost tables carry a
/// bare `class=""`.
pub(crate) fn suffix_css_classes(classes: Option<&str>, suffix: &str) -> String {
    classes.map_or_else(String::new, |classes| {
        classes
            .split(' ')
            .map(|class| format!("{class}-{suffix}"))
            .collect::<Vec<_>>()
            .join(" ")
    })
}

/// Append `css-class` to a component-generated class list (mjml's
/// `classesName += ` ${this.getAttribute('css-class')}`` in `mj-column` / `mj-group`).
pub(crate) fn append_css_class(generated: &str, class: Option<&str>) -> String {
    match class {
        Some(class) => format!("{generated} {class}"),
        None => generated.to_string(),
    }
}
