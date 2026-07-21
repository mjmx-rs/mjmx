//! `<mj-text>` — a text block; body captured verbatim, renders inside a `<td>`.

use mrml_ast::{Element, Node};

use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

// mjml default: Ubuntu font stack, imported from Google Fonts.
const FONT_FAMILY: &str = "Ubuntu, Helvetica, Arial, sans-serif";
const UBUNTU_HREF: &str = "https://fonts.googleapis.com/css?family=Ubuntu:300,400,500,700";

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

impl Render for Text<'_> {
    fn render(&self, ctx: &mut RenderContext, buf: &mut String) -> RenderResult {
        // Using the Ubuntu family requires importing it in the head.
        ctx.add_font("Ubuntu", UBUNTU_HREF);

        // mj-text is an "ending" tag: its body is a single verbatim text run.
        let mut content = String::new();
        for child in self.element.children() {
            if let Node::Text(t) = child {
                content.push_str(t.value.as_str());
            }
        }

        // mjml default: padding 10px 25px; font-size 13px; line-height 1;
        // text-align left; color #000000.
        buf.push_str(
            "<td align=\"left\" style=\"font-size:0px;padding:10px 25px;word-break:break-word;\">",
        );
        buf.push_str(&format!(
            "<div style=\"font-family:{FONT_FAMILY};font-size:13px;line-height:1;text-align:left;color:#000000;\">"
        ));
        buf.push_str(&content);
        buf.push_str("</div></td>");
        Ok(())
    }
}
