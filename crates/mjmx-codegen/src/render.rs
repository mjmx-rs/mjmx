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

/// The result of rendering: unit on success, a [`Diagnostic`] on failure.
pub type RenderResult = Result<(), Diagnostic>;

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
        MjmlTag::MjGroup => Group::new(element, 1).render(ctx, buf),
        // A bare column outside a section defaults to full width.
        MjmlTag::MjColumn => {
            let raw = element.attr("width");
            Column::new(
                element,
                crate::components::ColumnWidth::resolve(raw, 1),
                raw,
                1,
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
        _ => Err(unsupported(element)),
    }
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
