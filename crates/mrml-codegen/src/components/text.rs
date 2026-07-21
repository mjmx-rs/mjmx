//! `<mj-text>` — a text block; body captured verbatim, renders inside a `<td>`.

use mrml_ast::{Element, Node};

use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

// mjml default: Ubuntu font stack; color #000000. Whether the Ubuntu face is actually
// imported in the head is decided later by scanning the rendered body for the family
// name (see head.rs), so a resolved non-Ubuntu family emits no import.
const FONT_FAMILY: &str = "Ubuntu, Helvetica, Arial, sans-serif";
const COLOR: &str = "#000000";

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
        // Resolve the font-family and color through the semantic layer so that
        // <mj-attributes> / mj-class / inline overrides win over the component
        // default, in that precedence order.
        let font_family = ctx
            .semantic
            .attribute(self.element, "font-family", Some(FONT_FAMILY))
            .unwrap_or(FONT_FAMILY);
        let color = ctx
            .semantic
            .attribute(self.element, "color", Some(COLOR))
            .unwrap_or(COLOR);

        // mj-text is an "ending" tag: its body is a single verbatim text run.
        let mut content = String::new();
        for child in self.element.children() {
            if let Node::Text(t) = child {
                content.push_str(t.value.as_str());
            }
        }

        // mjml default: padding 10px 25px; font-size 13px; line-height 1;
        // text-align left.
        buf.push_str(
            "<td align=\"left\" style=\"font-size:0px;padding:10px 25px;word-break:break-word;\">",
        );
        buf.push_str(&format!(
            "<div style=\"font-family:{font_family};font-size:13px;line-height:1;text-align:left;color:{color};\">"
        ));
        buf.push_str(&content);
        buf.push_str("</div></td>");
        Ok(())
    }
}
