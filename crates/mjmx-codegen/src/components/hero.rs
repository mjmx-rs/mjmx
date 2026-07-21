//! `<mj-hero>` — a full-width hero banner with a background image/color.
//!
//! Structurally a centered `max-width` container (like a section) whose single row
//! cell carries the CSS `background` shorthand; Outlook gets a `<v:image>` painted
//! behind an mso ghost table. The hero's body children (mj-text, mj-button, …) render
//! inside a nested `mj-hero-content` table, each in its own `<tr>` exactly as a column
//! lays out its children. Only `mode="fixed-height"` is implemented (the fixture mode);
//! the cell height is `height` minus the vertical padding.

use mjmx_ast::Element;

use crate::context::RenderContext;
use crate::render::{Render, RenderResult, render_element};

// mjml defaults.
const MODE: &str = "fixed-height";
const HEIGHT: &str = "0px";
const BACKGROUND_COLOR: &str = "#ffffff";
const BACKGROUND_POSITION: &str = "center center";
const PADDING: &str = "0px";
const VERTICAL_ALIGN: &str = "top";

/// Renderer for `<mj-hero>`.
pub struct Hero<'a> {
    element: &'a Element<'a>,
}

impl<'a> Hero<'a> {
    /// Wrap an `<mj-hero>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }
}

impl<'a> Render<'a> for Hero<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        let attr = |name: &str, default: &'a str| {
            ctx.semantic
                .attribute(self.element, name, Some(default))
                .unwrap_or(default)
        };
        let opt = |name: &str| ctx.semantic.attribute(self.element, name, None);

        let _mode = attr("mode", MODE);
        let height = attr("height", HEIGHT);
        let background_color = attr("background-color", BACKGROUND_COLOR);
        let background_url = opt("background-url");
        let background_position = attr("background-position", BACKGROUND_POSITION);
        let background_height = opt("background-height");
        let padding = attr("padding", PADDING);
        let vertical_align = attr("vertical-align", VERTICAL_ALIGN);

        let container_width = ctx.container_width;
        // v:image width defaults to background-width, else the container width.
        let image_width = opt("background-width")
            .map(str::to_string)
            .unwrap_or_else(|| format!("{container_width}px"));

        // fixed-height: the cell height is `height` minus the vertical padding.
        let cell_height = parse_px(height)
            .saturating_sub(shorthand_px(padding, Side::Top) + shorthand_px(padding, Side::Bottom));

        // The CSS `background` shorthand: color, then (when a url is set) the image.
        let background = match background_url {
            Some(url) => {
                format!("{background_color} url('{url}') no-repeat {background_position} / cover")
            }
            None => background_color.to_string(),
        };

        // Outlook ghost table + the VML background image behind it.
        buf.push_str(&format!(
            "<!--[if mso | IE]><table align=\"center\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"width:{container_width}px;\" width=\"{container_width}\" ><tr><td style=\"line-height:0;font-size:0;mso-line-height-rule:exactly;\">"
        ));
        buf.push_str("<v:image style=\"border:0;");
        if let Some(h) = background_height {
            buf.push_str(&format!("height:{h};"));
        }
        buf.push_str(&format!(
            "mso-position-horizontal:center;position:absolute;top:0;width:{image_width};z-index:-3;\""
        ));
        if let Some(url) = background_url {
            buf.push_str(&format!(" src=\"{url}\""));
        }
        buf.push_str(" xmlns:v=\"urn:schemas-microsoft-com:vml\" /><![endif]-->");

        // Centered container.
        buf.push_str(&format!(
            "<div style=\"margin:0 auto;max-width:{container_width}px;\">"
        ));
        buf.push_str(
            "<table border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"width:100%;\">",
        );
        buf.push_str("<tbody><tr style=\"vertical-align:top;\">");

        // The background cell.
        buf.push_str("<td");
        if let Some(url) = background_url {
            buf.push_str(&format!(" background=\"{url}\""));
        }
        buf.push_str(&format!(
            " style=\"background:{background};background-position:{background_position};background-repeat:no-repeat;padding:{padding};vertical-align:{vertical_align};height:{cell_height}px;\" height=\"{cell_height}\">"
        ));

        // Inner Outlook table sized to the container width.
        buf.push_str(&format!(
            "<!--[if mso | IE]><table border=\"0\" cellpadding=\"0\" cellspacing=\"0\" style=\"width:{container_width}px;\" width=\"{container_width}\" ><tr><td style=\"\"><![endif]-->"
        ));

        // mj-hero-content: the nested content table; each child sits in its own <tr>.
        buf.push_str("<div class=\"mj-hero-content\" style=\"margin:0px auto;\">");
        buf.push_str(
            "<table border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"width:100%;margin:0px;\">",
        );
        buf.push_str("<tbody><tr><td style=\"\">");
        buf.push_str(
            "<table border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"width:100%;margin:0px;\">",
        );
        buf.push_str("<tbody>");
        for child in self.element.element_children() {
            buf.push_str("<tr>");
            render_element(child, ctx, buf)?;
            buf.push_str("</tr>");
        }
        buf.push_str("</tbody></table></td></tr></tbody></table></div>");

        buf.push_str("<!--[if mso | IE]></td></tr></table><![endif]-->");

        buf.push_str("</td></tr></tbody></table></div>");
        buf.push_str("<!--[if mso | IE]></td></tr></table><![endif]-->");
        Ok(())
    }
}

/// A padding shorthand side.
#[derive(Clone, Copy)]
enum Side {
    Top,
    Bottom,
}

/// Parse a leading pixel count out of a CSS length like `"200px"` or `"200"`.
fn parse_px(value: &str) -> u32 {
    value
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

/// The pixel value of one side of a CSS padding shorthand.
fn shorthand_px(padding: &str, side: Side) -> u32 {
    let parts: Vec<u32> = padding.split_whitespace().map(parse_px).collect();
    let (top, bottom) = match parts.as_slice() {
        [all] => (*all, *all),
        [tb, _lr] => (*tb, *tb),
        [t, _lr, b] => (*t, *b),
        [t, _r, b, _l] => (*t, *b),
        _ => (0, 0),
    };
    match side {
        Side::Top => top,
        Side::Bottom => bottom,
    }
}
