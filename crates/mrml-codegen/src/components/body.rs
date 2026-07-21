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

impl<'a> Render<'a> for Body<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        // A <mj-preview> emits a hidden preheader <div> as the first body child,
        // before the email wrapper.
        if let Some(preview) = ctx.semantic.head().preview.clone() {
            buf.push_str(&format!(
                "<div style=\"display:none;font-size:1px;color:#ffffff;line-height:1px;max-height:0px;max-width:0px;opacity:0;overflow:hidden;\">{preview}</div>"
            ));
        }

        // The body carries the ARIA email wrapper. `style=""` because this slice has
        // no background-color; lang/dir are inherited from the <mjml> root. A
        // <mj-title> adds an `aria-label` before `aria-roledescription`.
        let aria_label = match &ctx.semantic.head().title {
            Some(title) => format!("aria-label=\"{title}\" "),
            None => String::new(),
        };
        buf.push_str(&format!(
            "<div {aria_label}aria-roledescription=\"email\" style=\"\" role=\"article\" lang=\"{lang}\" dir=\"{dir}\">",
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
