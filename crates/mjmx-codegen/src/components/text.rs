//! `<mj-text>` — a text block; body captured verbatim, renders inside a `<td>`.

use mjmx_ast::{Element, Node};

use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

// mjml default: Ubuntu font stack; color #000000. Whether the Ubuntu face is actually
// imported in the head is decided later by scanning the rendered body for the family
// name (see head.rs), so a resolved non-Ubuntu family emits no import.
const FONT_FAMILY: &str = "Ubuntu, Helvetica, Arial, sans-serif";
const COLOR: &str = "#000000";
const FONT_SIZE: &str = "13px";
const LINE_HEIGHT: &str = "1";
const ALIGN: &str = "left";
const PADDING: &str = "10px 25px";

/// Renderer for `<mj-text>`.
pub struct Text<'a> {
    element: &'a Element<'a>,
}

impl<'a> Text<'a> {
    /// Wrap an `<mj-text>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }
}

impl<'a> Render<'a> for Text<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        // Every themeable attribute resolves through the semantic layer so that
        // <mj-attributes> / mj-class / inline overrides win over the component
        // default, in that precedence order. Attributes with no mjml default are
        // resolved with `None` and only emitted when a value is present.
        let attr = |name: &str, default: Option<&'a str>| {
            ctx.semantic.attribute(self.element, name, default)
        };

        let font_family = attr("font-family", Some(FONT_FAMILY)).unwrap_or(FONT_FAMILY);
        let font_size = attr("font-size", Some(FONT_SIZE)).unwrap_or(FONT_SIZE);
        let line_height = attr("line-height", Some(LINE_HEIGHT)).unwrap_or(LINE_HEIGHT);
        let align = attr("align", Some(ALIGN)).unwrap_or(ALIGN);
        let color = attr("color", Some(COLOR)).unwrap_or(COLOR);
        let padding = attr("padding", Some(PADDING)).unwrap_or(PADDING);
        // No mjml default — only emitted when set.
        let font_style = attr("font-style", None);
        let font_weight = attr("font-weight", None);
        let letter_spacing = attr("letter-spacing", None);
        let text_decoration = attr("text-decoration", None);
        let text_transform = attr("text-transform", None);
        let height = attr("height", None);
        let container_background_color = attr("container-background-color", None);

        // mj-text is an "ending" tag: its body is a single verbatim text run.
        let mut content = String::new();
        for child in self.element.children() {
            if let Node::Text(t) = child {
                content.push_str(t.value.as_str());
            }
        }

        // Inner <div> style, in mjml's declaration order. font-family/size, line-height,
        // text-align and color always appear (component defaults); the rest only when
        // resolved to a value.
        let mut div_style = format!("font-family:{font_family};font-size:{font_size};");
        if let Some(v) = font_style {
            div_style.push_str(&format!("font-style:{v};"));
        }
        if let Some(v) = font_weight {
            div_style.push_str(&format!("font-weight:{v};"));
        }
        if let Some(v) = letter_spacing {
            div_style.push_str(&format!("letter-spacing:{v};"));
        }
        div_style.push_str(&format!("line-height:{line_height};text-align:{align};"));
        if let Some(v) = text_decoration {
            div_style.push_str(&format!("text-decoration:{v};"));
        }
        if let Some(v) = text_transform {
            div_style.push_str(&format!("text-transform:{v};"));
        }
        div_style.push_str(&format!("color:{color};"));
        if let Some(v) = height {
            div_style.push_str(&format!("height:{v};"));
        }

        // Outer <td>: container-background-color prepends a `background:` declaration.
        let mut td_style = String::new();
        if let Some(cbg) = container_background_color {
            td_style.push_str(&format!("background:{cbg};"));
        }
        td_style.push_str(&format!(
            "font-size:0px;padding:{padding};word-break:break-word;"
        ));

        buf.push_str(&format!("<td align=\"{align}\" style=\"{td_style}\">"));
        // A fixed height wraps the div in an Outlook ghost cell.
        if let Some(h) = height {
            buf.push_str(&format!(
                "<!--[if mso | IE]><table role=\"presentation\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\"><tr><td height=\"{h}\" style=\"vertical-align:top;height:{h};\"><![endif]-->"
            ));
        }
        buf.push_str(&format!("<div style=\"{div_style}\">"));
        buf.push_str(&content);
        buf.push_str("</div>");
        if height.is_some() {
            buf.push_str("<!--[if mso | IE]></td></tr></table><![endif]-->");
        }
        buf.push_str("</td>");
        Ok(())
    }
}
