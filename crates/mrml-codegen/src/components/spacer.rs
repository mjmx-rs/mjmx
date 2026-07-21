//! `<mj-spacer>` — vertical whitespace: a fixed-height `<div>` holding a hair space.

use mrml_ast::Element;

use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

// mjml default: 20px tall, no padding.
const HEIGHT: &str = "20px";

/// Renderer for `<mj-spacer>`.
pub struct Spacer<'a> {
    element: &'a Element<'a>,
}

impl<'a> Spacer<'a> {
    /// Wrap an `<mj-spacer>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }
}

impl<'a> Render<'a> for Spacer<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        let height = ctx
            .semantic
            .attribute(self.element, "height", Some(HEIGHT))
            .unwrap_or(HEIGHT);
        // Padding is unset by default; a resolved value (inline / mj-attributes) is
        // added to the cell.
        let padding = ctx.semantic.attribute(self.element, "padding", None);

        match padding {
            Some(padding) => buf.push_str(&format!(
                "<td style=\"font-size:0px;padding:{padding};word-break:break-word;\">"
            )),
            None => buf.push_str("<td style=\"font-size:0px;word-break:break-word;\">"),
        }
        buf.push_str(&format!(
            "<div style=\"height:{height};line-height:{height};\">&#8202;</div>"
        ));
        buf.push_str("</td>");
        Ok(())
    }
}
