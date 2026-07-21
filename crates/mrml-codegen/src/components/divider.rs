//! `<mj-divider>` — a horizontal rule: a `<p>` for clients plus an mso/IE ghost table.

use mrml_ast::Element;

use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

// mjml defaults.
const BORDER_COLOR: &str = "#000000";
const BORDER_WIDTH: &str = "4px";
const BORDER_STYLE: &str = "solid";
const WIDTH: &str = "100%";
const PADDING: &str = "10px 25px";
const ALIGN: &str = "center";

// mjml default: the section/body container is 600px wide in this slice.
const CONTAINER_WIDTH: u32 = 600;

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

        let border_top = format!("{border_style} {border_width} {border_color}");

        // Ghost-table width: the fluid width resolved against the content box
        // (container minus horizontal padding).
        let content_width = CONTAINER_WIDTH - horizontal_padding(padding);
        let outlook_width = resolve_width(width, content_width);

        buf.push_str(&format!(
            "<td align=\"{align}\" style=\"font-size:0px;padding:{padding};word-break:break-word;\">"
        ));
        buf.push_str(&format!(
            "<p style=\"border-top:{border_top};font-size:1px;margin:0px auto;width:{width};\"></p>"
        ));
        buf.push_str(&format!(
            "<!--[if mso | IE]><table align=\"{align}\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" style=\"border-top:{border_top};font-size:1px;margin:0px auto;width:{outlook_width}px;\" role=\"presentation\" width=\"{outlook_width}px\" ><tr><td style=\"height:0;line-height:0;\"> &nbsp;\n</td></tr></table><![endif]-->"
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
