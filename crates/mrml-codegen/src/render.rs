//! The [`Render`] trait and the tag dispatcher.
//!
//! Every renderable component implements [`Render`]: it writes its HTML fragment
//! into a shared buffer and pushes any head requirements (fonts, media queries,
//! styles) into the [`RenderContext`]. [`render_element`] maps an [`Element`] to its
//! component renderer by [`MjmlTag`]; tags outside the current slice produce a clear
//! [`Diagnostic`] so later milestones know exactly what is missing.

use mrml_ast::Element;
use mrml_diagnostics::Diagnostic;
use mrml_span::GetSpan;
use mrml_syntax::MjmlTag;

use crate::components::social_element::SocialInherited;
use crate::components::{
    Body, Button, Column, Divider, Group, Image, Navbar, NavbarLink, Raw, Section, Social,
    SocialElement, Spacer, Table, Text, Wrapper,
};
use crate::context::RenderContext;

/// The result of rendering: unit on success, a [`Diagnostic`] on failure.
pub type RenderResult = Result<(), Diagnostic>;

/// A component that can render itself to HTML, side-effecting the [`RenderContext`].
///
/// The lifetime `'a` is the document arena: a component borrows its element and the
/// context's [`Semantic`](mrml_semantic::Semantic) view for the same `'a`.
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
    match element.tag {
        MjmlTag::MjBody => Body::new(element).render(ctx, buf),
        MjmlTag::MjWrapper => Wrapper::new(element).render(ctx, buf),
        MjmlTag::MjSection => Section::new(element).render(ctx, buf),
        MjmlTag::MjGroup => Group::new(element, 1).render(ctx, buf),
        // A bare column outside a section defaults to full width.
        MjmlTag::MjColumn => Column::new(element, 100).render(ctx, buf),
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
        _ => Err(unsupported(element)),
    }
}

/// Build the diagnostic for a component not yet supported by the renderer.
pub(crate) fn unsupported(element: &Element<'_>) -> Diagnostic {
    Diagnostic::error(format!(
        "rendering is not implemented for <{}> yet",
        element.raw_tag.as_str()
    ))
    .with_label(element.span(), "unsupported component in this build")
    .with_help(
        "this build renders mj-body, mj-wrapper, mj-section, mj-group, mj-column, mj-text, mj-image, mj-divider, mj-spacer, mj-button, mj-raw, mj-table, mj-social, mj-social-element, mj-navbar and mj-navbar-link",
    )
}
