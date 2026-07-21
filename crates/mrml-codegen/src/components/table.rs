//! `<mj-table>` — a `<table>` wrapping the user's raw inner rows verbatim.
//!
//! `mj-table` is an "ending" tag: the parser captured its inner markup (`<tr>`/`<td>`
//! …) as a single verbatim [`Text`](mrml_ast::Node::Text) run, which is emitted inside
//! the table untouched. The default font-family references the Ubuntu stack, so — like
//! mj-text — referencing it triggers the head font import (see `head.rs`).

use mrml_ast::{Element, Node};

use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

// mjml defaults.
const ALIGN: &str = "left";
const PADDING: &str = "10px 25px";
const COLOR: &str = "#000000";
const FONT_FAMILY: &str = "Ubuntu, Helvetica, Arial, sans-serif";
const FONT_SIZE: &str = "13px";
const LINE_HEIGHT: &str = "22px";
const TABLE_LAYOUT: &str = "auto";
const WIDTH: &str = "100%";
const BORDER: &str = "none";
const CELLPADDING: &str = "0";
const CELLSPACING: &str = "0";

/// Renderer for `<mj-table>`.
pub struct Table<'a> {
    element: &'a Element<'a>,
}

impl<'a> Table<'a> {
    /// Wrap an `<mj-table>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }
}

impl<'a> Render<'a> for Table<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        let attr = |name: &str, default: &'a str| {
            ctx.semantic
                .attribute(self.element, name, Some(default))
                .unwrap_or(default)
        };

        let align = attr("align", ALIGN);
        let padding = attr("padding", PADDING);
        let color = attr("color", COLOR);
        let font_family = attr("font-family", FONT_FAMILY);
        let font_size = attr("font-size", FONT_SIZE);
        let line_height = attr("line-height", LINE_HEIGHT);
        let table_layout = attr("table-layout", TABLE_LAYOUT);
        let width = attr("width", WIDTH);
        let border = attr("border", BORDER);
        let cellpadding = attr("cellpadding", CELLPADDING);
        let cellspacing = attr("cellspacing", CELLSPACING);
        // mjml only emits `role` when set (no default); it is omitted otherwise.
        let role = self.element.attr("role");

        // The inner rows: captured verbatim as one text run, emitted as-is.
        let mut content = String::new();
        for child in self.element.children() {
            if let Node::Text(t) = child {
                content.push_str(t.value.as_str());
            }
        }

        buf.push_str(&format!(
            "<td align=\"{align}\" style=\"font-size:0px;padding:{padding};word-break:break-word;\">"
        ));
        buf.push_str(&format!(
            "<table cellpadding=\"{cellpadding}\" cellspacing=\"{cellspacing}\""
        ));
        if let Some(role) = role {
            buf.push_str(&format!(" role=\"{role}\""));
        }
        buf.push_str(&format!(
            " width=\"{width}\" border=\"0\" style=\"color:{color};font-family:{font_family};font-size:{font_size};line-height:{line_height};table-layout:{table_layout};width:{width};border:{border};\">"
        ));
        buf.push_str(&content);
        buf.push_str("</table></td>");
        Ok(())
    }
}
