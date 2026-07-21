//! `<mj-button>` — a link styled as a button: a presentation table wrapping an
//! `<a>` (with `href`) or a `<p>` (without).

use mjmx_ast::{Element, Node};

use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

// mjml defaults.
const BACKGROUND_COLOR: &str = "#414141";
const COLOR: &str = "#ffffff";
// The Ubuntu stack; referencing it triggers the head font import (see head.rs), exactly
// like mj-text.
const FONT_FAMILY: &str = "Ubuntu, Helvetica, Arial, sans-serif";
const FONT_SIZE: &str = "13px";
const FONT_WEIGHT: &str = "normal";
const BORDER: &str = "none";
const BORDER_RADIUS: &str = "3px";
const PADDING: &str = "10px 25px";
const INNER_PADDING: &str = "10px 25px";
const ALIGN: &str = "center";
const VERTICAL_ALIGN: &str = "middle";
const LINE_HEIGHT: &str = "120%";
const TEXT_DECORATION: &str = "none";
const TEXT_TRANSFORM: &str = "none";
const TARGET: &str = "_blank";

/// Renderer for `<mj-button>`.
pub struct Button<'a> {
    element: &'a Element<'a>,
}

impl<'a> Button<'a> {
    /// Wrap an `<mj-button>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }
}

impl<'a> Render<'a> for Button<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        let attr = |name: &str, default: &'a str| {
            ctx.semantic
                .attribute(self.element, name, Some(default))
                .unwrap_or(default)
        };
        let opt = |name: &str| ctx.semantic.attribute(self.element, name, None);

        let background_color = attr("background-color", BACKGROUND_COLOR);
        let color = attr("color", COLOR);
        let font_family = attr("font-family", FONT_FAMILY);
        let font_size = attr("font-size", FONT_SIZE);
        let font_weight = attr("font-weight", FONT_WEIGHT);
        let font_style = opt("font-style");
        let border = attr("border", BORDER);
        let border_radius = attr("border-radius", BORDER_RADIUS);
        let padding = attr("padding", PADDING);
        let inner_padding = attr("inner-padding", INNER_PADDING);
        let align = attr("align", ALIGN);
        let vertical_align = attr("vertical-align", VERTICAL_ALIGN);
        let line_height = attr("line-height", LINE_HEIGHT);
        let letter_spacing = opt("letter-spacing");
        let text_decoration = attr("text-decoration", TEXT_DECORATION);
        let text_transform = attr("text-transform", TEXT_TRANSFORM);
        let width = opt("width");
        let height = opt("height");
        let container_background_color = opt("container-background-color");
        let target = attr("target", TARGET);
        let href = self.element.attr("href");

        // Button body is verbatim text; mjml trims it and re-pads with surrounding
        // whitespace inside the anchor.
        let mut content = String::new();
        for child in self.element.children() {
            if let Node::Text(t) = child {
                content.push_str(t.value.as_str());
            }
        }
        let content = content.trim();

        // Outer <td>: container-background-color prepends a `background:` declaration.
        let mut outer_td_style = String::new();
        if let Some(cbg) = container_background_color {
            outer_td_style.push_str(&format!("background:{cbg};"));
        }
        outer_td_style.push_str(&format!(
            "font-size:0px;padding:{padding};word-break:break-word;"
        ));
        buf.push_str(&format!(
            "<td align=\"{align}\" style=\"{outer_td_style}\">"
        ));

        // Inner presentation table: a fixed width lands here verbatim.
        let mut table_style = String::from("border-collapse:separate;");
        if let Some(w) = width {
            table_style.push_str(&format!("width:{w};"));
        }
        table_style.push_str("line-height:100%;");
        buf.push_str(&format!(
            "<table border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"{table_style}\">"
        ));
        buf.push_str("<tbody><tr>");

        // Button cell: align is fixed `center` (independent of the outer `align`).
        let mut cell_style = format!("border:{border};border-radius:{border_radius};cursor:auto;");
        if let Some(fs) = font_style {
            cell_style.push_str(&format!("font-style:{fs};"));
        }
        if let Some(h) = height {
            cell_style.push_str(&format!("height:{h};"));
        }
        cell_style.push_str(&format!(
            "mso-padding-alt:{inner_padding};background:{background_color};"
        ));
        buf.push_str(&format!(
            "<td align=\"center\" bgcolor=\"{background_color}\" role=\"presentation\" style=\"{cell_style}\" valign=\"{vertical_align}\">"
        ));

        // <a>/<p> width: the button width minus horizontal inner-padding and borders
        // (mjml's calculateAWidth); only for pixel widths.
        let a_width = width.and_then(parse_px).map(|w| {
            w.saturating_sub(shorthand_horizontal(inner_padding))
                .saturating_sub(border_horizontal(border))
        });

        let mut a_style = String::from("display:inline-block;");
        if let Some(w) = a_width {
            a_style.push_str(&format!("width:{w}px;"));
        }
        a_style.push_str(&format!(
            "background:{background_color};color:{color};font-family:{font_family};font-size:{font_size};"
        ));
        if let Some(fs) = font_style {
            a_style.push_str(&format!("font-style:{fs};"));
        }
        a_style.push_str(&format!(
            "font-weight:{font_weight};line-height:{line_height};"
        ));
        if let Some(ls) = letter_spacing {
            a_style.push_str(&format!("letter-spacing:{ls};"));
        }
        a_style.push_str(&format!(
            "margin:0;text-decoration:{text_decoration};text-transform:{text_transform};padding:{inner_padding};mso-padding-alt:0px;border-radius:{border_radius};"
        ));

        match href {
            Some(href) => buf.push_str(&format!(
                "<a href=\"{href}\" style=\"{a_style}\" target=\"{target}\"> {content} </a>"
            )),
            None => buf.push_str(&format!("<p style=\"{a_style}\"> {content} </p>")),
        }

        buf.push_str("</td></tr></tbody></table></td>");
        Ok(())
    }
}

/// Parse a leading pixel count out of a CSS length like `"200px"` or `"200"`.
fn parse_px(value: &str) -> Option<u32> {
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// The combined left + right value of a CSS shorthand like padding (`"12px 20px"`).
fn shorthand_horizontal(value: &str) -> u32 {
    let parts: Vec<u32> = value
        .split_whitespace()
        .map(|p| parse_px(p).unwrap_or(0))
        .collect();
    match parts.as_slice() {
        [all] => all + all,
        [_top, side] => side + side,
        [_top, side, _bottom] => side + side,
        [_top, right, _bottom, left] => right + left,
        _ => 0,
    }
}

/// The combined left + right border width of a `border` shorthand (`"2px solid #000"`).
/// mjml reads the same `border` value for both sides.
fn border_horizontal(value: &str) -> u32 {
    let one = value.split_whitespace().find_map(parse_px).unwrap_or(0);
    one + one
}
