//! `<mj-divider>` — a horizontal rule: a `<p>` for clients plus an mso/IE ghost table.

use mjmx_ast::Element;

use crate::components::css_class::{class_attr, css_class};
use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

// mjml defaults.
const BORDER_COLOR: &str = "#000000";
const BORDER_WIDTH: &str = "4px";
const BORDER_STYLE: &str = "solid";
const WIDTH: &str = "100%";
const PADDING: &str = "10px 25px";
const ALIGN: &str = "center";

/// Renderer for `<mj-divider>`.
pub struct Divider<'a> {
    element: &'a Element<'a>,
}

impl<'a> Divider<'a> {
    /// Wrap an `<mj-divider>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }
}

impl<'a> Render<'a> for Divider<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        let attr = |name: &str, default: &'a str| {
            ctx.semantic
                .attribute(self.element, name, Some(default))
                .unwrap_or(default)
        };

        let border_color = attr("border-color", BORDER_COLOR);
        let border_width = attr("border-width", BORDER_WIDTH);
        let border_style = attr("border-style", BORDER_STYLE);
        let width = attr("width", WIDTH);
        let padding = attr("padding", PADDING);
        let align = attr("align", ALIGN);
        let container_background_color =
            ctx.semantic
                .attribute(self.element, "container-background-color", None);

        let border_top = format!("{border_style} {border_width} {border_color}");

        // The rule's horizontal margin follows the alignment.
        let margin = match align {
            "left" => "0px",
            "right" => "0px 0px 0px auto",
            _ => "0px auto",
        };

        // Ghost-table width: the fluid width resolved against the content box
        // (container minus horizontal padding).
        let content_width = ctx.container_width - horizontal_padding(padding);
        let outlook_width = resolve_width(width, content_width);

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
        buf.push_str(&format!(
            "<p style=\"border-top:{border_top};font-size:1px;margin:{margin};width:{width};\"></p>"
        ));
        buf.push_str(&format!(
            "<!--[if mso | IE]><table align=\"{align}\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" style=\"border-top:{border_top};font-size:1px;margin:{margin};width:{outlook_width}px;\" role=\"presentation\" width=\"{outlook_width}px\" ><tr><td style=\"height:0;line-height:0;\"> &nbsp;\n</td></tr></table><![endif]-->"
        ));
        buf.push_str("</td>");
        Ok(())
    }
}

/// Total left + right padding, in pixels, from a CSS padding shorthand.
fn horizontal_padding(padding: &str) -> u32 {
    let parts: Vec<u32> = padding.split_whitespace().map(px_or_zero).collect();
    match parts.as_slice() {
        [all] => all * 2,
        [_tb, lr] => lr * 2,
        [_t, lr, _b] => lr * 2,
        [_t, r, _b, l] => r + l,
        _ => 0,
    }
}

/// Resolve a width value against a content-box width: a percentage scales it, an
/// explicit length is taken as pixels.
fn resolve_width(width: &str, content_width: u32) -> u32 {
    if let Some(pct) = width.strip_suffix('%') {
        pct.trim()
            .parse::<u32>()
            .map_or(content_width, |p| content_width * p / 100)
    } else {
        px_or_zero(width)
    }
}

/// Parse a leading pixel count, defaulting to zero.
fn px_or_zero(value: &str) -> u32 {
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().unwrap_or(0)
}
