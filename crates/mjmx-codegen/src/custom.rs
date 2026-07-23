//! Native custom components — mjmx's Tier 1 extension point.
//!
//! mjml lets a document use third-party components loaded from `.mjmlconfig` as JS
//! modules extending `mjml-core`'s base classes. mjmx cannot run those (no JS runtime,
//! and it is a clean-room compiler, not a `mjml-core` reimplementation). This module is
//! the honest alternative: a small, versioned contract mjmx owns, so a host can register
//! its own renderer for a tag and have it participate in the normal render — attribute
//! cascade, head contributions, and the strict "registered → renders, else the clean
//! unknown-component error" rule.
//!
//! Scope is **Tier 1 (leaf / ending tags)**: a component receives its resolved
//! attributes and its inner source text and returns an HTML fragment. It does not recurse
//! into mjmx's dispatch and does not drive the responsive column / ghost-table layout
//! machinery — that is Tier 2, deliberately deferred (see `docs/design/custom-components.md`).
//!
//! The ABI is [`ComponentCx`]: the entire surface a component author sees. It is versioned
//! by the crate's semver — there is no embedded ABI-version integer.

use std::collections::BTreeMap;

use mjmx_ast::{Element, Node};
use mjmx_diagnostics::Diagnostic;

use crate::context::RenderContext;

/// A host-supplied renderer for a single MJML tag.
///
/// Register implementors on a [`ComponentRegistry`] and pass it to
/// [`render_with_components`](../mjmx/fn.render_with_components.html). When codegen reaches
/// a tag the parser did not recognize, it consults the registry: a match renders through
/// [`render`](CustomComponent::render); no match is the usual "not a recognized MJML
/// component" error.
pub trait CustomComponent {
    /// The tag this component renders, without the angle brackets (e.g. `"mj-badge"`).
    fn tag(&self) -> &str;

    /// Render this component into the output, via the [`ComponentCx`] ABI.
    ///
    /// # Errors
    ///
    /// Return a [`Diagnostic`] to abort the render with a span-labelled message, exactly
    /// as a built-in component does.
    fn render(&self, cx: &mut ComponentCx<'_, '_>) -> Result<(), Diagnostic>;
}

/// A set of [`CustomComponent`]s, keyed by tag.
///
/// Built once by the host and passed by reference into the render. A [`BTreeMap`] keeps
/// the tag order deterministic (it feeds the linter's registered-names set), so a
/// document's diagnostics never depend on registration order.
#[derive(Default)]
pub struct ComponentRegistry {
    components: BTreeMap<String, Box<dyn CustomComponent>>,
}

impl std::fmt::Debug for ComponentRegistry {
    /// The trait objects are opaque, so a registry prints as its registered tags.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentRegistry")
            .field("components", &self.names().collect::<Vec<_>>())
            .finish()
    }
}

impl ComponentRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder form of [`register`](Self::register): register `component` and return `self`.
    #[must_use]
    pub fn with(mut self, component: impl CustomComponent + 'static) -> Self {
        self.register(component);
        self
    }

    /// Register `component` under its [`tag`](CustomComponent::tag). A second component
    /// with the same tag replaces the first.
    pub fn register(&mut self, component: impl CustomComponent + 'static) {
        self.components
            .insert(component.tag().to_string(), Box::new(component));
    }

    /// The component registered for `tag`, if any.
    #[must_use]
    pub fn get(&self, tag: &str) -> Option<&dyn CustomComponent> {
        self.components.get(tag).map(Box::as_ref)
    }

    /// Whether a component is registered for `tag`.
    #[must_use]
    pub fn contains(&self, tag: &str) -> bool {
        self.components.contains_key(tag)
    }

    /// The registered tags, in sorted order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.components.keys().map(String::as_str)
    }

    /// Whether no components are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}

/// The read-only view of a component set that codegen and the linter consult.
///
/// [`ComponentRegistry`] is the native implementation, but a binding (napi, wasm) supplies
/// its own — one whose components call out to a JS/host callback — without re-implementing
/// the registry. Codegen holds a `&dyn ComponentLookup`, so every implementation reaches
/// dispatch through the same door and the "registered → renders, else the clean
/// unknown-component error" rule stays identical across surfaces.
///
/// [`Debug`](std::fmt::Debug) is a supertrait because [`RenderContext`](crate::RenderContext)
/// derives `Debug` and holds a `&dyn ComponentLookup`; an implementation may print however
/// it likes (the registry prints its tag names).
pub trait ComponentLookup: std::fmt::Debug {
    /// The component registered for `tag`, if any.
    fn get(&self, tag: &str) -> Option<&dyn CustomComponent>;

    /// The registered tag names (for the linter's known-tag set). Order is unspecified.
    fn names(&self) -> Vec<&str>;
}

impl ComponentLookup for ComponentRegistry {
    fn get(&self, tag: &str) -> Option<&dyn CustomComponent> {
        ComponentRegistry::get(self, tag)
    }

    fn names(&self) -> Vec<&str> {
        ComponentRegistry::names(self).collect()
    }
}

/// The render context a [`CustomComponent`] sees — the entire, versioned ABI.
///
/// A thin, deliberately narrow wrapper over the internal [`RenderContext`]: attributes in
/// (resolved through the `mj-attributes` / `mj-class` / defaults cascade, exactly as a
/// built-in reads them), HTML out, and optional `<head>` contributions. Nothing of
/// `mjml-core`'s plugin surface — no base class, no `renderChildren`, no component
/// registry, no global context — is exposed.
pub struct ComponentCx<'a, 'buf> {
    element: &'a Element<'a>,
    ctx: &'buf mut RenderContext<'a>,
    buf: &'buf mut String,
}

impl<'a, 'buf> ComponentCx<'a, 'buf> {
    /// Wrap the element, context and output buffer for a custom render.
    pub(crate) fn new(
        element: &'a Element<'a>,
        ctx: &'buf mut RenderContext<'a>,
        buf: &'buf mut String,
    ) -> Self {
        Self { element, ctx, buf }
    }

    /// The cascade-resolved value of attribute `name`, or `None`.
    ///
    /// Resolution is identical to a built-in component's: inline attribute, then
    /// `mj-class`, then per-tag `mj-attributes`, then `mj-all`. Raw (pre-cascade) access
    /// is intentionally not offered — it would bypass `mj-attributes`, a footgun.
    #[must_use]
    pub fn attr(&self, name: &str) -> Option<&'a str> {
        self.ctx.semantic.attribute(self.element, name, None)
    }

    /// [`attr`](Self::attr) with a fallback when the attribute is unset.
    #[must_use]
    pub fn attr_or<'d>(&'d self, name: &str, default: &'d str) -> &'d str {
        self.attr(name).unwrap_or(default)
    }

    /// Every attribute that resolves for this element, as `(name, value)` pairs in sorted
    /// name order — each value cascade-resolved exactly as [`attr`](Self::attr) resolves it.
    ///
    /// This is the faithful snapshot a host binding hands to a JS/other-language callback:
    /// the union of the element's inline attributes and every `mj-attributes` / `mj-class`
    /// / `mj-all` default that applies to its tag, so the callback sees the same values a
    /// built-in component would — without a live call back into the renderer.
    #[must_use]
    pub fn resolved_attrs(&self) -> Vec<(&'a str, &'a str)> {
        self.ctx.semantic.resolved_attributes(self.element)
    }

    /// The content-box width, in pixels, available to this element (set by the parent).
    #[must_use]
    pub fn container_width(&self) -> u32 {
        self.ctx.container_width
    }

    /// The active responsive breakpoint, in pixels.
    #[must_use]
    pub fn breakpoint(&self) -> u32 {
        self.ctx.breakpoint
    }

    /// The verbatim source text between the tag's open and close (its raw inner markup).
    #[must_use]
    pub fn raw_inner(&self) -> String {
        let mut inner = String::new();
        for child in self.element.children() {
            if let Node::Text(text) = child {
                inner.push_str(text.value.as_str());
            }
        }
        inner
    }

    /// Append `html` to the output buffer.
    pub fn write(&mut self, html: &str) {
        self.buf.push_str(html);
    }

    /// Contribute a `<style>` block to the document `<head>` (deduped, insertion order
    /// kept), the same channel a built-in component's raw styles use.
    pub fn add_style(&mut self, css: impl Into<String>) {
        self.ctx.add_style(css);
    }

    /// Contribute a web font to the `<head>`: a `<link>` + `@import` for `href`, emitted
    /// only if the family `name` is referenced in the rendered body — mjml's own rule.
    pub fn add_font(&mut self, name: impl Into<String>, href: impl Into<String>) {
        self.ctx.add_custom_font(name.into(), href.into());
    }
}
