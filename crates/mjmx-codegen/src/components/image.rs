//! `<mj-image>` — an `<img>` inside an aligned `<td>` and inner presentation table.

use mjmx_ast::Element;

use crate::components::css_class::{class_attr, css_class};
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

        // Pixel width used to size the outer <td> (mjml renders the <img> at width:100%
        // of this cell). mjml's `getContentWidth` is
        // `min(getBoxWidths().box, width ?? Infinity)`, i.e. an explicit `width` still
        // gets clamped to the container's content box.
        let paddings = padding_side(padding, opt("padding-left"), Side::Left)
            + padding_side(padding, opt("padding-right"), Side::Right);
        let borders = border_side(opt("border-left").unwrap_or(border))
            + border_side(opt("border-right").unwrap_or(border));
        let box_width = ctx.container_width.saturating_sub(paddings + borders);
        let px = width
            .and_then(parse_px)
            .map_or(box_width, |w| w.min(box_width));

        // Register mj-image's responsive head style (fluid width on mobile). mjml keys
        // the media query at breakpoint - 1 px.
        let max = ctx.breakpoint.saturating_sub(1);
        ctx.add_style(format!(
            "\n    @media only screen and (max-width:{max}px) {{\n      table.mj-full-width-mobile {{\n        width: 100% !important;\n      }}\n\n      td.mj-full-width-mobile {{\n        width: auto !important;\n      }}\n    }}\n\n  "
        ));

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
        // mjml puts the child's `css-class` on the wrapping `<td>` (mj-column /
        // mj-hero `renderChildren`), between `align` and `style`.
        let class = class_attr(css_class(ctx, self.element).as_deref());
        buf.push_str(&format!(
            "<td align=\"{align}\" {class}style=\"{td_style}\">"
        ));

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

/// A horizontal side of a CSS box shorthand.
enum Side {
    Left,
    Right,
}

/// mjml's `getShorthandAttrValue('padding', side)`: the longhand wins, otherwise the
/// matching slot of the `padding` shorthand.
fn padding_side(shorthand: &str, longhand: Option<&str>, side: Side) -> u32 {
    if let Some(value) = longhand {
        return parse_px(value).unwrap_or(0);
    }
    let parts: Vec<&str> = shorthand.split_whitespace().collect();
    let value = match (parts.len(), &side) {
        (1, _) => parts.first(),
        (2 | 3, _) => parts.get(1),
        (4, Side::Right) => parts.get(1),
        (4, Side::Left) => parts.get(3),
        _ => None,
    };
    value.copied().and_then(parse_px).unwrap_or(0)
}

/// mjml's `borderParser`: the pixel count inside a `border` shorthand
/// (`"solid 4px #000"` → `4`, `"none"` → `0`).
fn border_side(border: &str) -> u32 {
    border.split_whitespace().find_map(parse_px).unwrap_or(0)
}

/// Parse a leading pixel count out of a CSS length like `"300px"` or `"300"`.
fn parse_px(value: &str) -> Option<u32> {
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}
