//! `<mj-section>` — a full-width row with an mso/IE ghost-table layout.

use mrml_ast::Element;
use mrml_syntax::MjmlTag;

use crate::components::{Column, Group};
use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

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

impl<'a> Render<'a> for Section<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        // The section's own width is the container width handed down to it (600px at
        // the body root, or whatever a wrapper threads in).
        let width = ctx.container_width;
        // mjml default: section padding 20px 0 — its horizontal padding is 0, so the
        // content box handed to children equals the section width.
        let box_width = width - horizontal_padding("20px 0");

        // Non-raw layout children (columns and groups) share the row equally; a single
        // child spans 100%. Integer math is exact for the divisors mjml emits.
        let children: Vec<&Element> = self
            .element
            .element_children()
            .filter(|el| matches!(el.tag, MjmlTag::MjColumn | MjmlTag::MjGroup))
            .collect();
        let count = children.len().max(1) as u32;
        let pct = 100 / count;

        // Outer mso/IE ghost table (fixed to the section width) that Outlook honors.
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

        // Children get the content box as their container width.
        let saved = ctx.container_width;
        ctx.container_width = box_width;

        // Column ghost row: <table><tr>, then a ghost <td> per child interleaved with
        // the real child markup. Each ghost <td>'s style comes from the child: a column
        // carries `vertical-align:top`, a group carries only its pixel width.
        buf.push_str(
            "<!--[if mso | IE]><table role=\"presentation\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\"><tr>",
        );
        for child in &children {
            let child_px = box_width * pct / 100;
            match child.tag {
                MjmlTag::MjGroup => {
                    buf.push_str(&format!(
                        "<td class=\"\" style=\"width:{child_px}px;\" ><![endif]-->"
                    ));
                    Group::new(child, count).render(ctx, buf)?;
                }
                _ => {
                    buf.push_str(&format!(
                        "<td class=\"\" style=\"vertical-align:top;width:{child_px}px;\" ><![endif]-->"
                    ));
                    Column::new(child, pct).render(ctx, buf)?;
                }
            }
            buf.push_str("<!--[if mso | IE]></td>");
        }
        buf.push_str("</tr></table><![endif]-->");

        ctx.container_width = saved;

        buf.push_str("</td></tr></tbody></table></div>");
        buf.push_str("<!--[if mso | IE]></td></tr></table><![endif]-->");
        Ok(())
    }
}

/// Total left + right padding, in pixels, from a CSS padding shorthand.
fn horizontal_padding(padding: &str) -> u32 {
    let parts: Vec<u32> = padding
        .split_whitespace()
        .map(|v| {
            v.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect();
    match parts.as_slice() {
        [all] => all * 2,
        [_tb, lr] => lr * 2,
        [_t, lr, _b] => lr * 2,
        [_t, r, _b, l] => r + l,
        _ => 0,
    }
}
