//! `<mj-image>` — an `<img>` inside an aligned `<td>` and inner presentation table.

use mjmx_ast::Element;

use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

// mjml defaults.
const ALIGN: &str = "center";
const PADDING: &str = "10px 25px";
const HEIGHT: &str = "auto";
const BORDER: &str = "0";
const FONT_SIZE: &str = "13px";
const TARGET: &str = "_blank";
const ALT: &str = "";

/// Renderer for `<mj-image>`.
pub struct Image<'a> {
    element: &'a Element<'a>,
}

impl<'a> Image<'a> {
    /// Wrap an `<mj-image>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }
}

impl<'a> Render<'a> for Image<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        let attr = |name: &str, default: &'a str| {
            ctx.semantic
                .attribute(self.element, name, Some(default))
                .unwrap_or(default)
        };
        let opt = |name: &str| ctx.semantic.attribute(self.element, name, None);

        let align = attr("align", ALIGN);
        let padding = attr("padding", PADDING);
        let alt = attr("alt", ALT);
        let src = self.element.attr("src").unwrap_or("");
        let height = attr("height", HEIGHT);
        let border = attr("border", BORDER);
        let border_radius = opt("border-radius");
        let font_size = attr("font-size", FONT_SIZE);
        let container_background_color = opt("container-background-color");
        let width = opt("width");
        let href = self.element.attr("href");
        let target = attr("target", TARGET);

        // Register mj-image's responsive head style (fluid width on mobile). mjml keys
        // the media query at breakpoint - 1 px.
        let max = ctx.breakpoint.saturating_sub(1);
        ctx.add_style(format!(
            "\n    @media only screen and (max-width:{max}px) {{\n      table.mj-full-width-mobile {{\n        width: 100% !important;\n      }}\n\n      td.mj-full-width-mobile {{\n        width: auto !important;\n      }}\n    }}\n\n  "
        ));

        // Pixel width used to size the outer <td> (mjml renders the <img> at width:100%
        // of this cell). Falls back to the container content width when unset.
        let px = width.and_then(parse_px).unwrap_or(550);
        // The <img> `height` HTML attribute: numeric px, or the literal `auto`.
        let height_attr = if height == "auto" {
            "auto".to_string()
        } else {
            parse_px(height).map_or_else(|| height.to_string(), |h| h.to_string())
        };

        // Outer <td>: container-background-color prepends a `background:` declaration.
        let mut td_style = String::new();
        if let Some(cbg) = container_background_color {
            td_style.push_str(&format!("background:{cbg};"));
        }
        td_style.push_str(&format!(
            "font-size:0px;padding:{padding};word-break:break-word;"
        ));
        buf.push_str(&format!("<td align=\"{align}\" style=\"{td_style}\">"));

        buf.push_str(
            "<table border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"border-collapse:collapse;border-spacing:0px;\">",
        );
        buf.push_str("<tbody><tr>");
        buf.push_str(&format!("<td style=\"width:{px}px;\">"));

        if let Some(href) = href {
            buf.push_str(&format!("<a href=\"{href}\" target=\"{target}\">"));
        }

        // <img> style: border, optional border-radius, then the fixed run.
        let mut img_style = format!("border:{border};");
        if let Some(br) = border_radius {
            img_style.push_str(&format!("border-radius:{br};"));
        }
        img_style.push_str(&format!(
            "display:block;outline:none;text-decoration:none;height:{height};width:100%;font-size:{font_size};"
        ));
        buf.push_str(&format!(
            "<img alt=\"{alt}\" src=\"{src}\" style=\"{img_style}\" width=\"{px}\" height=\"{height_attr}\" />"
        ));

        if href.is_some() {
            buf.push_str("</a>");
        }

        buf.push_str("</td></tr></tbody></table></td>");
        Ok(())
    }
}

/// Parse a leading pixel count out of a CSS length like `"300px"` or `"300"`.
fn parse_px(value: &str) -> Option<u32> {
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}
