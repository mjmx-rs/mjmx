//! `<mj-column>` — an inline-block column that registers its responsive rule.

use mrml_ast::Element;

use crate::context::RenderContext;
use crate::render::{Render, RenderResult, render_element};

/// Renderer for `<mj-column>`.
pub struct Column<'a> {
    element: &'a Element<'a>,
    /// This column's width as a whole-number percentage of its container. Always the
    /// share that keys the `mj-column-per-{share}` class and its media query.
    pct: u32,
    /// Whether the column sits inside an `<mj-group>` (no mobile stacking). A group
    /// column keeps its share as the inline outlook-div width; a section column uses
    /// `100%` inline and lets the media query apply the share on desktop.
    in_group: bool,
}

impl<'a> Column<'a> {
    /// Wrap a section (or top-level) `<mj-column>` — mobile-stacking, so the inline
    /// outlook-div width is `100%` and the share lives only in the media query.
    #[must_use]
    pub fn new(element: &'a Element<'a>, pct: u32) -> Self {
        Self {
            element,
            pct,
            in_group: false,
        }
    }

    /// Wrap an `<mj-group>` child `<mj-column>` — no mobile stacking, so the share is
    /// also the inline outlook-div width.
    #[must_use]
    pub fn new_grouped(element: &'a Element<'a>, pct: u32) -> Self {
        Self {
            element,
            pct,
            in_group: true,
        }
    }
}

impl<'a> Render<'a> for Column<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        let pct = self.pct;
        let class = format!("mj-column-per-{pct}");

        // Register the responsive rule so the head emits both the @media block and
        // the .moz-text-html <style media> block for this class.
        ctx.add_media_query(
            &class,
            format!("width: {pct}% !important;\n        max-width: {pct}%;"),
        );

        // Inline outlook-div width: a section column stacks on mobile (100%, share via
        // the media query); a group column holds its share inline (no stacking).
        let div_width = if self.in_group { pct } else { 100 };
        buf.push_str(&format!(
            "<div class=\"{class} mj-outlook-group-fix\" style=\"font-size:0px;text-align:left;direction:ltr;display:inline-block;vertical-align:top;width:{div_width}%;\">"
        ));
        buf.push_str(
            "<table border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"vertical-align:top;\" width=\"100%\">",
        );
        buf.push_str("<tbody>");

        // Each content child sits in its own <tr>; the child renders its own <td>.
        for child in self.element.element_children() {
            buf.push_str("<tr>");
            render_element(child, ctx, buf)?;
            buf.push_str("</tr>");
        }

        buf.push_str("</tbody></table></div>");
        Ok(())
    }
}
