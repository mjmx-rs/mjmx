//! `<mj-raw>` — a passthrough that emits its inner HTML verbatim.
//!
//! `mj-raw` is an "ending" tag, so the parser captured its inner markup as raw text
//! children; rendering simply concatenates them with no wrapping element. This covers
//! the *body* case. The two other placements are handled outside this renderer, from
//! data collected by `mjmx-semantic`: a `<mj-raw>` under `<mj-head>` is injected at the
//! end of the document `<head>` (`head::render_head`), and a
//! `<mj-raw position="file-start">` sibling of `<mj-body>` is emitted before the doctype
//! (`document::render_document`).

use mjmx_ast::{Element, Node};

use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

/// Renderer for `<mj-raw>`.
pub struct Raw<'a> {
    element: &'a Element<'a>,
}

impl<'a> Raw<'a> {
    /// Wrap an `<mj-raw>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }
}

impl<'a> Render<'a> for Raw<'a> {
    fn render(&self, _ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        for child in self.element.children() {
            if let Node::Text(t) = child {
                buf.push_str(t.value.as_str());
            }
        }
        Ok(())
    }
}
