//! `<mj-section>` — a full-width row with an mso/IE ghost-table layout.

use mrml_ast::Element;
use mrml_syntax::MjmlTag;

use crate::components::Column;
use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

// mjml default: sections are 600px wide (the mj-body default width).
const CONTAINER_WIDTH: u32 = 600;

/// Renderer for `<mj-section>`.
pub struct Section<'a> {
    element: &'a Element<'a>,
}

impl<'a> Section<'a> {
    /// Wrap an `<mj-section>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }
}

impl Render for Section<'_> {
    fn render(&self, ctx: &mut RenderContext, buf: &mut String) -> RenderResult {
        let width = CONTAINER_WIDTH;

        let columns: Vec<&Element> = self
            .element
            .element_children()
            .filter(|el| el.tag == MjmlTag::MjColumn)
            .collect();
        // Equal-split widths; single column => 100%. Integer math is exact for the
        // divisors mjml emits (1 => 100, 2 => 50, ...).
        let count = columns.len().max(1) as u32;
        let pct = 100 / count;
        let col_px = width * pct / 100;

        // Outer mso/IE ghost table (fixed 600px) that Outlook honors.
        buf.push_str(&format!(
            "<!--[if mso | IE]><table align=\"center\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" class=\"\" role=\"presentation\" style=\"width:{width}px;\" width=\"{width}\" ><tr><td style=\"line-height:0px;font-size:0px;mso-line-height-rule:exactly;\"><![endif]-->"
        ));

        // Real centered container.
        buf.push_str(&format!(
            "<div style=\"margin:0px auto;max-width:{width}px;\">"
        ));
        buf.push_str(
            "<table align=\"center\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"width:100%;\">",
        );
        buf.push_str("<tbody><tr>");
        // mjml default: section padding 20px 0.
        buf.push_str(
            "<td style=\"direction:ltr;font-size:0px;padding:20px 0;text-align:center;\">",
        );

        // Column ghost row: <table><tr>, then a ghost <td> per column interleaved
        // with the real column markup.
        buf.push_str(
            "<!--[if mso | IE]><table role=\"presentation\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\"><tr>",
        );
        for col in &columns {
            buf.push_str(&format!(
                "<td class=\"\" style=\"vertical-align:top;width:{col_px}px;\" ><![endif]-->"
            ));
            Column::new(col, pct).render(ctx, buf)?;
            buf.push_str("<!--[if mso | IE]></td>");
        }
        buf.push_str("</tr></table><![endif]-->");

        buf.push_str("</td></tr></tbody></table></div>");
        buf.push_str("<!--[if mso | IE]></td></tr></table><![endif]-->");
        Ok(())
    }
}
