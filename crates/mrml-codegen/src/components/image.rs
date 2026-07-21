//! `<mj-image>` — an `<img>` inside an aligned `<td>` and inner presentation table.

use mrml_ast::Element;

use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

// mjml defaults.
const ALIGN: &str = "center";
const PADDING: &str = "10px 25px";
const HEIGHT: &str = "auto";
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

        let align = attr("align", ALIGN);
        let padding = attr("padding", PADDING);
        let alt = attr("alt", ALT);
        let src = self.element.attr("src").unwrap_or("");
        let height = attr("height", HEIGHT);
        let width = self.element.attr("width");
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

        buf.push_str(&format!(
            "<td align=\"{align}\" style=\"font-size:0px;padding:{padding};word-break:break-word;\">"
        ));
        buf.push_str(
            "<table border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"border-collapse:collapse;border-spacing:0px;\">",
        );
        buf.push_str("<tbody><tr>");
        buf.push_str(&format!("<td style=\"width:{px}px;\">"));

        if let Some(href) = href {
            buf.push_str(&format!("<a href=\"{href}\" target=\"{target}\">"));
        }

        buf.push_str(&format!(
            "<img alt=\"{alt}\" src=\"{src}\" style=\"border:0;display:block;outline:none;text-decoration:none;height:auto;width:100%;font-size:13px;\" width=\"{px}\" height=\"{height}\" />"
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
