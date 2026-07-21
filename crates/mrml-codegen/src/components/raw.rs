//! `<mj-raw>` — a passthrough that emits its inner HTML verbatim.
//!
//! `mj-raw` is an "ending" tag, so the parser captured its inner markup as raw text
//! children; rendering simply concatenates them with no wrapping element. Only the body
//! case is handled here — a `<mj-raw>` inside `<mj-head>` (a head passthrough,
//! typically `position="file-start"`) is not yet emitted and is deferred.

use mrml_ast::{Element, Node};

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
