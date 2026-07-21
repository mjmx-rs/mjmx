//! `<mj-navbar>` — a horizontal link bar with an optional CSS-checkbox mobile menu.
//!
//! The bar is a `<div class="mj-inline-links">` wrapping the links, each interleaved
//! with mso conditional `<td>`s that carry its padding (the adjacent conditionals mjml
//! emits are pre-merged here). When `hamburger="hamburger"`, a checkbox/label toggle is
//! prepended and the `.mj-menu-*` responsive stylesheet is registered in the head —
//! which mjml registers whenever the component is used, hamburger or not.

use mrml_ast::Element;
use mrml_syntax::MjmlTag;

use crate::components::navbar_link::NavbarLink;
use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

// mjml defaults.
const ALIGN: &str = "center";
const ICO_ALIGN: &str = "center";
const ICO_OPEN: &str = "&#9776;";
const ICO_CLOSE: &str = "&#8855;";
const ICO_COLOR: &str = "#000000";
const ICO_FONT_SIZE: &str = "30px";
const ICO_FONT_FAMILY: &str = "Ubuntu, Helvetica, Arial, sans-serif";
const ICO_TEXT_TRANSFORM: &str = "uppercase";
const ICO_PADDING: &str = "10px";
const ICO_TEXT_DECORATION: &str = "none";
const ICO_LINE_HEIGHT: &str = "30px";

/// Renderer for `<mj-navbar>`.
pub struct Navbar<'a> {
    element: &'a Element<'a>,
}

impl<'a> Navbar<'a> {
    /// Wrap an `<mj-navbar>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }

    fn resolve(&self, ctx: &RenderContext<'a>, name: &str, default: &'a str) -> &'a str {
        ctx.semantic
            .attribute(self.element, name, Some(default))
            .unwrap_or(default)
    }
}

impl<'a> Render<'a> for Navbar<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        let align = self.resolve(ctx, "align", ALIGN);
        let base_url = self.element.attr("base-url");
        let hamburger = self.element.attr("hamburger") == Some("hamburger");

        // mjml registers the mobile-menu stylesheet whenever mj-navbar is used.
        let max = ctx.breakpoint.saturating_sub(1);
        ctx.add_style(menu_style(max));

        let links: Vec<&Element> = self
            .element
            .element_children()
            .filter(|el| el.tag == MjmlTag::MjNavbarLink)
            .collect();

        // Outer cell: navbar has no padding default, so it is omitted unless set.
        let padding = self.element.attr("padding");
        buf.push_str(&format!("<td align=\"{align}\" style=\"font-size:0px;"));
        if let Some(p) = padding {
            buf.push_str(&format!("padding:{p};"));
        }
        buf.push_str("word-break:break-word;\">");

        if hamburger {
            let id = ctx.next_hamburger_id();
            let ico_color = self.resolve(ctx, "ico-color", ICO_COLOR);
            let ico_font_size = self.resolve(ctx, "ico-font-size", ICO_FONT_SIZE);
            let ico_font_family = self.resolve(ctx, "ico-font-family", ICO_FONT_FAMILY);
            let ico_text_transform = self.resolve(ctx, "ico-text-transform", ICO_TEXT_TRANSFORM);
            let ico_text_decoration = self.resolve(ctx, "ico-text-decoration", ICO_TEXT_DECORATION);
            let ico_line_height = self.resolve(ctx, "ico-line-height", ICO_LINE_HEIGHT);
            let ico_padding = self.resolve(ctx, "ico-padding", ICO_PADDING);
            let ico_align = self.resolve(ctx, "ico-align", ICO_ALIGN);
            let ico_open = self.resolve(ctx, "ico-open", ICO_OPEN);
            let ico_close = self.resolve(ctx, "ico-close", ICO_CLOSE);

            buf.push_str(&format!(
                "<!--[if !mso]><!--><input type=\"checkbox\" id=\"{id}\" class=\"mj-menu-checkbox\" style=\"display:none !important; max-height:0; visibility:hidden;\" /><!--<![endif]-->"
            ));
            buf.push_str(
                "<div class=\"mj-menu-trigger\" style=\"display:none;max-height:0px;max-width:0px;font-size:0px;overflow:hidden;\">",
            );
            buf.push_str(&format!(
                "<label for=\"{id}\" class=\"mj-menu-label\" style=\"display:block;cursor:pointer;mso-hide:all;-moz-user-select:none;user-select:none;color:{ico_color};font-size:{ico_font_size};font-family:{ico_font_family};text-transform:{ico_text_transform};text-decoration:{ico_text_decoration};line-height:{ico_line_height};padding:{ico_padding};\" align=\"{ico_align}\">"
            ));
            buf.push_str(&format!(
                "<span class=\"mj-menu-icon-open\" style=\"mso-hide:all;\"> {ico_open} </span>"
            ));
            buf.push_str(&format!(
                "<span class=\"mj-menu-icon-close\" style=\"display:none;mso-hide:all;\"> {ico_close} </span>"
            ));
            buf.push_str("</label></div>");
        }

        buf.push_str("<div class=\"mj-inline-links\" style=\"\">");
        for (i, el) in links.iter().enumerate() {
            let link = NavbarLink::new(el, base_url);
            if i == 0 {
                buf.push_str(&format!(
                    "<!--[if mso | IE]><table role=\"presentation\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" align=\"{align}\"><tr><td style=\"{}\" class=\"{}\" ><![endif]-->",
                    link.td_style(ctx),
                    link.outlook_class()
                ));
            } else {
                buf.push_str(&format!(
                    "<!--[if mso | IE]></td><td style=\"{}\" class=\"{}\" ><![endif]-->",
                    link.td_style(ctx),
                    link.outlook_class()
                ));
            }
            link.render_anchor(ctx, buf);
        }
        buf.push_str("<!--[if mso | IE]></td></tr></table><![endif]-->");
        buf.push_str("</div>");

        buf.push_str("</td>");
        Ok(())
    }
}

/// The `.mj-menu-*` mobile-menu stylesheet mjml registers (beautified exactly), keyed at
/// the lower breakpoint `max`.
fn menu_style(max: u32) -> String {
    format!(
        "\n    noinput.mj-menu-checkbox {{\n      display: block !important;\n      max-height: none !important;\n      visibility: visible !important;\n    }}\n\n    @media only screen and (max-width:{max}px) {{\n      .mj-menu-checkbox[type=\"checkbox\"]~.mj-inline-links {{\n        display: none !important;\n      }}\n\n      .mj-menu-checkbox[type=\"checkbox\"]:checked~.mj-inline-links,\n      .mj-menu-checkbox[type=\"checkbox\"]~.mj-menu-trigger {{\n        display: block !important;\n        max-width: none !important;\n        max-height: none !important;\n        font-size: inherit !important;\n      }}\n\n      .mj-menu-checkbox[type=\"checkbox\"]~.mj-inline-links>a {{\n        display: block !important;\n      }}\n\n      .mj-menu-checkbox[type=\"checkbox\"]:checked~.mj-menu-trigger .mj-menu-icon-close {{\n        display: block !important;\n      }}\n\n      .mj-menu-checkbox[type=\"checkbox\"]:checked~.mj-menu-trigger .mj-menu-icon-open {{\n        display: none !important;\n      }}\n    }}\n\n  "
    )
}
