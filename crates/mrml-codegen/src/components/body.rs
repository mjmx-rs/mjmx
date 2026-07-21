//! `<mj-body>` — the outer email wrapper `<div>`.

use mrml_ast::Element;

use crate::context::RenderContext;
use crate::render::{Render, RenderResult, render_element};

/// Renderer for `<mj-body>`.
pub struct Body<'a> {
    element: &'a Element<'a>,
}

impl<'a> Body<'a> {
    /// Wrap an `<mj-body>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }
}

impl Render for Body<'_> {
    fn render(&self, ctx: &mut RenderContext, buf: &mut String) -> RenderResult {
        // The body carries the ARIA email wrapper. `style=""` because this slice has
        // no background-color; lang/dir are inherited from the <mjml> root.
        buf.push_str(&format!(
            "<div aria-roledescription=\"email\" style=\"\" role=\"article\" lang=\"{lang}\" dir=\"{dir}\">",
            lang = ctx.lang,
            dir = ctx.dir,
        ));

        // Only element children (sections) render; whitespace text between
        // body-level components is not significant and is skipped.
        for child in self.element.element_children() {
            render_element(child, ctx, buf)?;
        }

        buf.push_str("</div>");
        Ok(())
    }
}
