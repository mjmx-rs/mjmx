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

        // Themeable attributes.
        let attr = |name: &str, default: &'a str| {
            ctx.semantic
                .attribute(self.element, name, Some(default))
                .unwrap_or(default)
        };
        let opt = |name: &str| ctx.semantic.attribute(self.element, name, None);
        // mjml default: section padding 20px 0, direction ltr, text-align center.
        let padding = attr("padding", "20px 0");
        let direction = attr("direction", "ltr");
        let text_align = attr("text-align", "center");
        let background_color = opt("background-color");
        let border = opt("border");
        let border_radius = opt("border-radius");

        // Content box handed to children: section width minus horizontal padding and
        // borders (mjml's getBoxWidths).
        let box_width = width - horizontal_padding(padding) - border.map_or(0, horizontal_border);

        // Non-raw layout children (columns and groups) share the row equally; a single
        // child spans 100%. Integer math is exact for the divisors mjml emits.
        let children: Vec<&Element> = self
            .element
            .element_children()
            .filter(|el| matches!(el.tag, MjmlTag::MjColumn | MjmlTag::MjGroup))
            .collect();
        let count = children.len().max(1) as u32;
        let pct = 100 / count;

        // `background:x;background-color:x;` prefix shared by the div and inner table.
        let bg_prefix = background_color
            .map(|bg| format!("background:{bg};background-color:{bg};"))
            .unwrap_or_default();

        // Outer mso/IE ghost table (fixed to the section width) that Outlook honors;
        // carries the section background as a `bgcolor`.
        let mso_bgcolor = background_color
            .map(|bg| format!("bgcolor=\"{bg}\" "))
            .unwrap_or_default();
        buf.push_str(&format!(
            "<!--[if mso | IE]><table align=\"center\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" class=\"\" role=\"presentation\" style=\"width:{width}px;\" width=\"{width}\" {mso_bgcolor}><tr><td style=\"line-height:0px;font-size:0px;mso-line-height-rule:exactly;\"><![endif]-->"
        ));

        // Real centered container. Border-radius adds a rounded-clip on the div and
        // forces `border-collapse:separate` on the inner table.
        let radius_div = border_radius
            .map(|br| format!("border-radius:{br};overflow:hidden;"))
            .unwrap_or_default();
        buf.push_str(&format!(
            "<div style=\"{bg_prefix}margin:0px auto;max-width:{width}px;{radius_div}\">"
        ));
        let table_suffix = if border_radius.is_some() {
            "border-collapse:separate;"
        } else {
            ""
        };
        buf.push_str(&format!(
            "<table align=\"center\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"{bg_prefix}width:100%;{table_suffix}\">"
        ));
        buf.push_str("<tbody><tr>");
        // Section content <td>: border and border-radius only when set; direction,
        // padding and text-align always (component defaults).
        let mut td_style = String::new();
        if let Some(b) = border {
            td_style.push_str(&format!("border:{b};"));
        }
        if let Some(br) = border_radius {
            td_style.push_str(&format!("border-radius:{br};"));
        }
        td_style.push_str(&format!(
            "direction:{direction};font-size:0px;padding:{padding};text-align:{text_align};"
        ));
        buf.push_str(&format!("<td style=\"{td_style}\">"));

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
                    // The ghost cell carries the group's vertical-align when it is set.
                    let mut style = String::new();
                    if let Some(va) = ctx.semantic.attribute(child, "vertical-align", None) {
                        style.push_str(&format!("vertical-align:{va};"));
                    }
                    buf.push_str(&format!(
                        "<td class=\"\" style=\"{style}width:{child_px}px;\" ><![endif]-->"
                    ));
                    Group::new(child, count).render(ctx, buf)?;
                }
                _ => {
                    // The ghost cell mirrors the column's own vertical-align.
                    let va = ctx
                        .semantic
                        .attribute(child, "vertical-align", Some("top"))
                        .unwrap_or("top");
                    buf.push_str(&format!(
                        "<td class=\"\" style=\"vertical-align:{va};width:{child_px}px;\" ><![endif]-->"
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

/// Combined left + right border width from a `border` shorthand (`"2px solid #000"`).
/// mjml reads the same `border` value for both sides.
fn horizontal_border(border: &str) -> u32 {
    let one = border
        .split_whitespace()
        .find_map(|v| {
            v.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse::<u32>()
                .ok()
        })
        .unwrap_or(0);
    one * 2
}
