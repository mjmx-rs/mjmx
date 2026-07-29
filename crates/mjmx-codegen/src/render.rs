//! The [`Render`] trait and the tag dispatcher.
//!
//! Every renderable component implements [`Render`]: it writes its HTML fragment
//! into a shared buffer and pushes any head requirements (fonts, media queries,
//! styles) into the [`RenderContext`]. [`render_element`] maps an [`Element`] to its
//! component renderer by [`MjmlTag`]; tags outside the current slice produce a clear
//! [`Diagnostic`] so later milestones know exactly what is missing.

use mjmx_ast::{Element, Node};
use mjmx_diagnostics::Diagnostic;
use mjmx_span::GetSpan;
use mjmx_syntax::MjmlTag;

use crate::components::social_element::SocialInherited;
use crate::components::{
    Accordion, Body, Button, Carousel, Column, Divider, Group, Hero, Image, Navbar, NavbarLink,
    Raw, Section, Social, SocialElement, Spacer, Table, Text, Wrapper,
};
use crate::context::RenderContext;
use crate::custom::ComponentCx;

/// The result of rendering: unit on success, a [`Diagnostic`] on failure.
pub type RenderResult = Result<(), Diagnostic>;

/// Concatenate an element's verbatim text children — the body of an "ending" tag.
///
/// `mj-text`, `mj-button`, `mj-table`, `mj-navbar-link`, `mj-social-element` and the
/// accordion cells all take their content this way, and a custom component reads it as
/// [`ComponentCx::raw_inner`](crate::ComponentCx::raw_inner).
///
/// The text runs are substrings of the element's own source extent, so its span length is
/// a tight upper bound on the result: one allocation, never a resize.
pub(crate) fn inner_text(element: &Element<'_>) -> String {
    let span = element.span();
    let mut out = String::with_capacity(span.end.saturating_sub(span.start) as usize);
    for child in element.children() {
        if let Node::Text(text) = child {
            out.push_str(text.value.as_str());
        }
    }
    out
}

/// [`inner_text`], with leading and trailing whitespace removed.
///
/// Trimmed in place rather than through `trim().to_string()`, which would allocate a
/// second buffer to hold what the first already contains.
pub(crate) fn inner_text_trimmed(element: &Element<'_>) -> String {
    let mut out = inner_text(element);
    out.truncate(out.trim_end().len());
    let leading = out.len() - out.trim_start().len();
    out.drain(..leading);
    out
}

/// Append one CSS declaration — `property:value;` — to a style buffer.
///
/// Building a declaration list is by far the most common thing a component does, and the
/// pieces are already strings: pushing them straight into the buffer skips the whole
/// formatting machinery (`fmt::Arguments` dispatch and a `Formatter::pad` per value) that
/// `write!(style, "property:{value};")` walks through to reach the same four writes.
pub(crate) fn push_decl(style: &mut String, property: &str, value: &str) {
    style.push_str(property);
    style.push(':');
    style.push_str(value);
    style.push(';');
}

/// Starting capacity for the inline `style="…"` values components accumulate.
///
/// Measured over the rendered fixtures, a style attribute is 36 bytes at the median and
/// 108 at the 90th percentile, so this holds nine in ten of them in a single allocation.
/// A `String` growing from zero instead reallocates and copies three or four times before
/// reaching that size — the same declarations, paid for repeatedly.
pub(crate) const STYLE_CAPACITY: usize = 128;

/// A component that can render itself to HTML, side-effecting the [`RenderContext`].
///
/// The lifetime `'a` is the document arena: a component borrows its element and the
/// context's [`Semantic`](mjmx_semantic::Semantic) view for the same `'a`.
pub trait Render<'a> {
    /// Render this component into `buf`, registering head requirements in `ctx`.
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult;
}

/// Dispatch an element to its component renderer by tag.
///
/// The four M4 slice components (`mj-body`, `mj-section`, `mj-column`, `mj-text`)
/// render fully. Any other tag yields a "not yet implemented" error carrying the
/// element's span, so M6 can fill them in.
pub fn render_element<'a>(
    element: &'a Element<'a>,
    ctx: &mut RenderContext<'a>,
    buf: &mut String,
) -> RenderResult {
    traced(element, ctx, buf, |ctx, buf| dispatch(element, ctx, buf))
}

/// Record the output range an element writes, pairing it with the element's source span.
///
/// This is the single instrumentation point for source maps. It brackets a component's
/// write with `buf.len()` before and after and hands the pair to the
/// [`SourceMapBuilder`](mjmx_sourcemap::SourceMapBuilder) in the context — it never
/// touches `buf`, so the produced HTML is byte-identical whether collection is on or off.
///
/// When collection is off (the default) this is a plain call behind one `Option` check.
///
/// Most components are reached through [`render_element`], which is wrapped here; the few
/// parents that construct a child renderer directly (a wrapper's sections, a section's
/// columns, a group's columns, a social element's row) call this themselves, so the map
/// still has a mapping for every element that produced output.
pub(crate) fn traced<'a, R>(
    element: &Element<'_>,
    ctx: &mut RenderContext<'a>,
    buf: &mut String,
    f: impl FnOnce(&mut RenderContext<'a>, &mut String) -> R,
) -> R {
    let Some(sm) = ctx.sourcemap.as_mut() else {
        return f(ctx, buf);
    };
    let depth = sm.enter();
    let (span, tag) = (element.span(), element.tag);
    let start = buf.len();
    let result = f(ctx, buf);
    let end = buf.len();
    if let Some(sm) = ctx.sourcemap.as_mut() {
        sm.record(span, tag, start..end, depth);
    }
    result
}

/// Render a container's children, giving comments mjml's treatment.
///
/// mjml's parser rewrites every comment into an implicit `<mj-raw>` child of the
/// enclosing element (`oncomment` in `mjml-parser-xml`), and `mj-raw` is a *raw*
/// element — so a comment is emitted verbatim at its own position, outside the row/cell
/// scaffolding the container wraps its real children in. `element` children are handed
/// to `f`, which supplies exactly that scaffolding; comments bypass it, and text nodes
/// (whitespace between tags) are dropped as they always have been.
///
/// # Callers must keep Outlook conditionals closed
///
/// A comment lands wherever the buffer currently is, so a caller must not have an
/// `<!--[if …]>` block open around the loop: HTML comments do not nest, and the
/// comment's own `-->` would terminate the conditional early and leak the Outlook-only
/// markup that follows into every other client. Emit each ghost block opened *and*
/// closed — as mjml's `renderWrappedChildren` does — and let
/// [`merge_outlook_conditionals`](crate::render_document) fuse the adjacent ones back
/// together afterwards; the output is byte-identical when no comment intervenes.
///
/// Honors [`RenderOptions::keep_comments`](crate::RenderOptions::keep_comments): when it
/// is off, a comment renders to nothing.
pub(crate) fn render_children<'a>(
    element: &'a Element<'a>,
    ctx: &mut RenderContext<'a>,
    buf: &mut String,
    mut f: impl FnMut(&'a Element<'a>, &mut RenderContext<'a>, &mut String) -> RenderResult,
) -> RenderResult {
    for child in element.children() {
        match child {
            Node::Element(el) => f(el, ctx, buf)?,
            Node::Comment(comment) => {
                if ctx.options.keep_comments {
                    buf.push_str("<!--");
                    buf.push_str(comment.value.as_str());
                    buf.push_str("-->");
                }
            }
            Node::Text(_) => {}
        }
    }
    Ok(())
}

/// Whether an element is a *raw* element in mjml's sense (`isRawElement()`), i.e. one
/// that renders its content verbatim and is exempt from a parent's row/cell scaffolding.
///
/// Only `<mj-raw>` sets `rawElement = true` in mjml 4.18.0, so a section/column/group/hero
/// emits a raw child directly at its own position instead of wrapping it in a `<tr>` (or a
/// ghost `<td>`) — see each container's `renderChildren`/`renderWrappedChildren`.
pub(crate) fn is_raw_element(element: &Element<'_>) -> bool {
    element.tag == MjmlTag::MjRaw
}

/// The tag → component-renderer table, wrapped by [`render_element`].
fn dispatch<'a>(
    element: &'a Element<'a>,
    ctx: &mut RenderContext<'a>,
    buf: &mut String,
) -> RenderResult {
    match element.tag {
        MjmlTag::MjBody => Body::new(element).render(ctx, buf),
        MjmlTag::MjWrapper => Wrapper::new(element).render(ctx, buf),
        MjmlTag::MjSection => Section::new(element).render(ctx, buf),
        // A bare group/column outside a section has no gutter context.
        MjmlTag::MjGroup => Group::new(element, 1, None).render(ctx, buf),
        // A bare column outside a section defaults to full width.
        MjmlTag::MjColumn => {
            let raw = element.attr("width");
            Column::new(
                element,
                crate::components::ColumnWidth::resolve(raw, 1),
                raw,
                1,
                crate::components::Gutter::none(),
            )
            .render(ctx, buf)
        }
        MjmlTag::MjText => Text::new(element).render(ctx, buf),
        MjmlTag::MjImage => Image::new(element).render(ctx, buf),
        MjmlTag::MjDivider => Divider::new(element).render(ctx, buf),
        MjmlTag::MjSpacer => Spacer::new(element).render(ctx, buf),
        MjmlTag::MjButton => Button::new(element).render(ctx, buf),
        MjmlTag::MjRaw => Raw::new(element).render(ctx, buf),
        MjmlTag::MjTable => Table::new(element).render(ctx, buf),
        MjmlTag::MjSocial => Social::new(element).render(ctx, buf),
        MjmlTag::MjSocialElement => {
            SocialElement::new(element, SocialInherited::default()).render(ctx, buf)
        }
        MjmlTag::MjNavbar => Navbar::new(element).render(ctx, buf),
        MjmlTag::MjNavbarLink => NavbarLink::new(element, None).render(ctx, buf),
        MjmlTag::MjHero => Hero::new(element).render(ctx, buf),
        MjmlTag::MjCarousel => Carousel::new(element).render(ctx, buf),
        MjmlTag::MjAccordion => Accordion::new(element).render(ctx, buf),
        // An unrecognized tag may be a host-registered custom component (Tier 1). The
        // registry ref is `Copy` (an `Option<&_>`), so reading it here releases the
        // context borrow before `ComponentCx` takes `ctx` mutably; `comp` borrows the
        // registry (`'a`), independent of that mutable borrow.
        MjmlTag::Unknown => {
            let registered = ctx.registry();
            if let Some(comp) = registered.and_then(|reg| reg.get(element.raw_tag.as_str())) {
                // A Tier 2 container drives its own markup + children (no cell wrap); a
                // Tier 1 leaf is wrapped in the standard `<td>` cell.
                return if comp.is_container() {
                    render_custom_container(element, comp, ctx, buf)
                } else {
                    render_custom_in_cell(element, comp, ctx, buf)
                };
            }
            Err(unsupported(element))
        }
        _ => Err(unsupported(element)),
    }
}

/// Render a registered Tier 1 custom component wrapped in the standard leaf `<td>` cell.
///
/// mjml's column/hero emit the cell around every non-raw child (`mjml-column`'s
/// `renderChildren`), reading the cell's attributes from that *child*: `align`, `css-class`
/// → `class`, and a style block of `background` (from `container-background-color`),
/// `font-size:0px`, the `padding` family, and `word-break:break-word`. mjmx's built-in
/// leaves emit this cell themselves (see `mj-text`), so a custom component gets the same
/// treatment here — the author returns the cell *content* (a `<span>`, `<div>`, …) and mjmx
/// supplies the cell, keeping the table markup valid (`<tr><td>…</td></tr>`).
///
/// Unlike a built-in, a custom component has no mjml-declared attribute defaults, so every
/// cell attribute is emitted **only when the cascade resolves one** — mjml's `htmlAttributes`
/// drops `null`/`undefined` values. A component with no cell attributes therefore yields
/// `<td style="font-size:0px;word-break:break-word;">`.
fn render_custom_in_cell<'a>(
    element: &'a Element<'a>,
    comp: &dyn crate::custom::CustomComponent,
    ctx: &mut RenderContext<'a>,
    buf: &mut String,
) -> RenderResult {
    use crate::components::css_class::css_class;

    // Resolve the cell attributes from the component's own attributes (no built-in default;
    // mjml omits an undefined value). `css_class` also folds any `mj-class` css-class in.
    let align = ctx.semantic.attribute(element, "align", None);
    let class = css_class(ctx, element);
    let container_bg = ctx
        .semantic
        .attribute(element, "container-background-color", None);
    let padding = ctx.semantic.attribute(element, "padding", None);
    let padding_top = ctx.semantic.attribute(element, "padding-top", None);
    let padding_right = ctx.semantic.attribute(element, "padding-right", None);
    let padding_bottom = ctx.semantic.attribute(element, "padding-bottom", None);
    let padding_left = ctx.semantic.attribute(element, "padding-left", None);

    // Style block, in mjml's key order: background, font-size, padding (+ longhands),
    // word-break. `font-size:0px` and `word-break:break-word` are always present.
    let mut style = String::new();
    if let Some(bg) = container_bg {
        push_decl(&mut style, "background", bg);
    }
    style.push_str("font-size:0px;");
    for (prop, value) in [
        ("padding", padding),
        ("padding-top", padding_top),
        ("padding-right", padding_right),
        ("padding-bottom", padding_bottom),
        ("padding-left", padding_left),
    ] {
        if let Some(v) = value {
            style.push_str(prop);
            style.push(':');
            style.push_str(v);
            style.push(';');
        }
    }
    style.push_str("word-break:break-word;");

    // Attributes in mjml's `htmlAttributes` order: align, class, style — undefined omitted.
    buf.push_str("<td");
    if let Some(align) = align {
        buf.push_str(" align=\"");
        buf.push_str(align);
        buf.push('"');
    }
    if let Some(class) = class.as_deref() {
        buf.push_str(" class=\"");
        buf.push_str(class);
        buf.push('"');
    }
    buf.push_str(" style=\"");
    buf.push_str(&style);
    buf.push_str("\">");

    let mut cx = ComponentCx::new(element, ctx, buf);
    comp.render(&mut cx)?;

    buf.push_str("</td>");
    Ok(())
}

/// Render a registered Tier 2 **container** custom component.
///
/// Unlike a Tier 1 leaf, no wrapping `<td>` cell is emitted: the component drives its own
/// markup and calls [`ComponentCx::render_children`] to render its MJML children through
/// normal dispatch. mjmx only brackets the call to save and restore the parent's
/// container width, so a width the container sets for its children (via
/// [`ComponentCx::set_container_width`]) does not leak to a sibling.
fn render_custom_container<'a>(
    element: &'a Element<'a>,
    comp: &dyn crate::custom::CustomComponent,
    ctx: &mut RenderContext<'a>,
    buf: &mut String,
) -> RenderResult {
    let saved_width = ctx.container_width;
    let result = {
        let mut cx = ComponentCx::new(element, ctx, buf);
        comp.render(&mut cx)
    };
    ctx.container_width = saved_width;
    result
}

/// Build the diagnostic for a tag the body renderer cannot emit. Two distinct cases,
/// worded so neither reads as "a feature mjmx has not built yet": an unrecognized tag
/// does not exist in MJML at all (matching mjml's "Element X doesn't exist or is not
/// registered"), while a recognized tag reaching this arm is one with no body renderer
/// (a head-only element misplaced in the body).
pub(crate) fn unsupported(element: &Element<'_>) -> Diagnostic {
    if element.tag == MjmlTag::Unknown {
        return Diagnostic::error(format!(
            "<{}> is not a recognized MJML component",
            element.raw_tag.as_str()
        ))
        .with_label(element.span(), "unknown component")
        .with_help("check the spelling; MJML does not define this element");
    }
    Diagnostic::error(format!(
        "<{}> is not valid inside the body",
        element.raw_tag.as_str()
    ))
    .with_label(element.span(), "not a body component")
    .with_help("this element is only valid inside <mj-head>")
}
