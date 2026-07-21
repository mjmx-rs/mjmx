//! `<mj-wrapper>` — a section-shaped container that wraps whole `<mj-section>`s so a
//! background can span the full row.
//!
//! Structurally it renders exactly like a section (the centered `max-width` container,
//! the `20px 0` padding cell, the presentation ghost table), but its ghost row wraps
//! each *section* child in `<tr><td width=…>` instead of laying out columns. A
//! `full-width="full-width"` wrapper adds an outer 100%-width table around that same
//! inner markup.
//!
//! The wrapper emits its Outlook conditional comments as independent blocks; the
//! document-level `mergeOutlookConditionnals` pass fuses the adjacent
//! `<![endif]--><!--[if mso | IE]>` seams (e.g. the wrapper cell flowing straight into
//! a nested section's ghost table), reproducing mjml byte-for-byte.

use mrml_ast::Element;
use mrml_syntax::MjmlTag;

use crate::components::Section;
use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

/// Renderer for `<mj-wrapper>`.
pub struct Wrapper<'a> {
    element: &'a Element<'a>,
}

impl<'a> Wrapper<'a> {
    /// Wrap an `<mj-wrapper>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }

    /// Whether this is a `full-width="full-width"` wrapper.
    fn is_full_width(&self) -> bool {
        self.element.attr("full-width") == Some("full-width")
    }

    /// The centered-container body shared by both variants (renderBefore + the section
    /// container + the wrapped section children + renderAfter).
    fn render_inner(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        let width = ctx.container_width;
        // mjml default: wrapper padding 20px 0 — horizontal padding 0, so the content
        // box handed to nested sections equals the wrapper width.
        let box_width = width;

        // renderBefore: the section-shaped ghost table.
        buf.push_str(&format!(
            "<!--[if mso | IE]><table align=\"center\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" class=\"\" role=\"presentation\" style=\"width:{width}px;\" width=\"{width}\" ><tr><td style=\"line-height:0px;font-size:0px;mso-line-height-rule:exactly;\"><![endif]-->"
        ));

        // The centered container.
        buf.push_str(&format!(
            "<div style=\"margin:0px auto;max-width:{width}px;\">"
        ));
        buf.push_str(
            "<table align=\"center\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"width:100%;\">",
        );
        buf.push_str("<tbody><tr>");
        buf.push_str(
            "<td style=\"direction:ltr;font-size:0px;padding:20px 0;text-align:center;\">",
        );

        // Presentation ghost table wrapping each section child in its own <tr><td>.
        buf.push_str(
            "<!--[if mso | IE]><table role=\"presentation\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\"><![endif]-->",
        );

        let saved = ctx.container_width;
        ctx.container_width = box_width;
        for child in self
            .element
            .element_children()
            .filter(|el| el.tag == MjmlTag::MjSection)
        {
            buf.push_str(&format!(
                "<!--[if mso | IE]><tr><td class=\"\" width=\"{width}px\" ><![endif]-->"
            ));
            Section::new(child).render(ctx, buf)?;
            buf.push_str("<!--[if mso | IE]></td></tr><![endif]-->");
        }
        ctx.container_width = saved;

        buf.push_str("<!--[if mso | IE]></table><![endif]-->");
        buf.push_str("</td></tr></tbody></table></div>");
        // renderAfter.
        buf.push_str("<!--[if mso | IE]></td></tr></table><![endif]-->");
        Ok(())
    }
}

impl<'a> Render<'a> for Wrapper<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        if self.is_full_width() {
            // Outer full-width table, then the identical centered-container body.
            buf.push_str(
                "<table align=\"center\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"width:100%;\"><tbody><tr><td>",
            );
            self.render_inner(ctx, buf)?;
            buf.push_str("</td></tr></tbody></table>");
            Ok(())
        } else {
            self.render_inner(ctx, buf)
        }
    }
}
