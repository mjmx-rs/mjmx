//! `<mj-button>` — a link styled as a button: a presentation table wrapping an
//! `<a>` (with `href`) or a `<p>` (without).

use mrml_ast::{Element, Node};

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

        let background_color = attr("background-color", BACKGROUND_COLOR);
        let color = attr("color", COLOR);
        let font_family = attr("font-family", FONT_FAMILY);
        let font_size = attr("font-size", FONT_SIZE);
        let font_weight = attr("font-weight", FONT_WEIGHT);
        let border = attr("border", BORDER);
        let border_radius = attr("border-radius", BORDER_RADIUS);
        let padding = attr("padding", PADDING);
        let inner_padding = attr("inner-padding", INNER_PADDING);
        let align = attr("align", ALIGN);
        let vertical_align = attr("vertical-align", VERTICAL_ALIGN);
        let line_height = attr("line-height", LINE_HEIGHT);
        let text_decoration = attr("text-decoration", TEXT_DECORATION);
        let text_transform = attr("text-transform", TEXT_TRANSFORM);
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

        buf.push_str(&format!(
            "<td align=\"{align}\" style=\"font-size:0px;padding:{padding};word-break:break-word;\">"
        ));
        buf.push_str(
            "<table border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"border-collapse:separate;line-height:100%;\">",
        );
        buf.push_str("<tbody><tr>");
        buf.push_str(&format!(
            "<td align=\"{align}\" bgcolor=\"{background_color}\" role=\"presentation\" style=\"border:{border};border-radius:{border_radius};cursor:auto;mso-padding-alt:{inner_padding};background:{background_color};\" valign=\"{vertical_align}\">"
        ));

        let a_style = format!(
            "display:inline-block;background:{background_color};color:{color};font-family:{font_family};font-size:{font_size};font-weight:{font_weight};line-height:{line_height};margin:0;text-decoration:{text_decoration};text-transform:{text_transform};padding:{inner_padding};mso-padding-alt:0px;border-radius:{border_radius};"
        );
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
