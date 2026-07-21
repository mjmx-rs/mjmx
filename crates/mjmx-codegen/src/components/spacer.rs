//! `<mj-spacer>` — vertical whitespace: a fixed-height `<div>` holding a hair space.

use mjmx_ast::Element;

use crate::components::css_class::{class_attr, css_class};
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
        // Padding and container-background-color are unset by default; a resolved value
        // (inline / mj-attributes) is added to the cell.
        let padding = ctx.semantic.attribute(self.element, "padding", None);
        let container_background_color =
            ctx.semantic
                .attribute(self.element, "container-background-color", None);

        let mut td_style = String::new();
        if let Some(cbg) = container_background_color {
            td_style.push_str(&format!("background:{cbg};"));
        }
        td_style.push_str("font-size:0px;");
        if let Some(padding) = padding {
            td_style.push_str(&format!("padding:{padding};"));
        }
        td_style.push_str("word-break:break-word;");
        // mj-spacer has no `align`, so the wrapping `<td>` is `class` then `style`.
        let class = class_attr(css_class(ctx, self.element).as_deref());
        buf.push_str(&format!("<td {class}style=\"{td_style}\">"));
        buf.push_str(&format!(
            "<div style=\"height:{height};line-height:{height};\">&#8202;</div>"
        ));
        buf.push_str("</td>");
        Ok(())
    }
}
