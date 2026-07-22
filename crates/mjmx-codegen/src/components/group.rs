//! `<mj-group>` — columns kept side-by-side (no mobile stacking).
//!
//! A group is one cell of its section's ghost row, and manages its own child columns'
//! Outlook ghost table internally. Its child columns render as normal column `<div>`s
//! (their percentage is taken against the *group* width, not the section), while the
//! group's own class/width is a percentage of the section.

use mjmx_ast::Element;
use mjmx_syntax::MjmlTag;

use crate::components::css_class::{append_css_class, css_class};
use crate::components::{Column, ColumnWidth};
use crate::context::RenderContext;
use crate::render::{Render, RenderResult, render_children};

/// Renderer for `<mj-group>`.
pub struct Group<'a> {
    element: &'a Element<'a>,
    /// The number of non-raw layout siblings in the parent section (the group's share
    /// of the section is `100 / siblings` percent).
    siblings: u32,
}

impl<'a> Group<'a> {
    /// Wrap an `<mj-group>` element, given the parent section's layout-child count.
    #[must_use]
    pub fn new(element: &'a Element<'a>, siblings: u32) -> Self {
        Self { element, siblings }
    }
}

impl<'a> Render<'a> for Group<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        // The group's width as a percentage of its section, and in pixels (its share of
        // the section content box). Integer math is exact for the divisors mjml emits.
        let gpct = 100 / self.siblings;
        let group_width = ctx.container_width / self.siblings;

        // The group registers its own responsive class first (before its columns), so
        // the head lists `mj-column-per-{gpct}` ahead of the column rules.
        let class = format!("mj-column-per-{gpct}");
        ctx.add_media_query(
            &class,
            format!("width: {gpct}% !important;\n        max-width: {gpct}%;"),
        );

        let direction = ctx
            .semantic
            .attribute(self.element, "direction", Some("ltr"))
            .unwrap_or("ltr");
        // vertical-align and background-color have no default: emitted only when set.
        let vertical_align = ctx.semantic.attribute(self.element, "vertical-align", None);
        let background_color = ctx
            .semantic
            .attribute(self.element, "background-color", None);

        // The group div is always inline-block at 100% (its outer ghost cell sizes it);
        // note `font-size:0`/`line-height:0` (not `0px`), unlike a column.
        let mut div_style = format!(
            "font-size:0;line-height:0;text-align:left;display:inline-block;width:100%;direction:{direction};"
        );
        if let Some(va) = vertical_align {
            div_style.push_str(&format!("vertical-align:{va};"));
        }
        if let Some(bg) = background_color {
            div_style.push_str(&format!("background-color:{bg};"));
        }
        // mjml's `render`: `${getColumnClass()} mj-outlook-group-fix` + `css-class`.
        let div_class = append_css_class(
            &format!("{class} mj-outlook-group-fix"),
            css_class(ctx, self.element).as_deref(),
        );
        buf.push_str(&format!(
            "<div class=\"{div_class}\" style=\"{div_style}\">"
        ));

        // The group's own Outlook ghost table holding one cell per column; it carries
        // the group background as a `bgcolor`.
        let mso_bgcolor = background_color
            .map(|bg| format!("bgcolor=\"{bg}\" "))
            .unwrap_or_default();
        buf.push_str(&format!(
            "<!--[if mso | IE]><table {mso_bgcolor}border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" ><tr><![endif]-->"
        ));

        let columns: Vec<&Element> = self
            .element
            .element_children()
            .filter(|el| el.tag == MjmlTag::MjColumn)
            .collect();
        let num = columns.len().max(1) as u32;

        // Columns size against the group width, not the section.
        let saved = ctx.container_width;
        ctx.container_width = group_width;
        render_children(self.element, ctx, buf, |col, ctx, buf| {
            // Only columns are laid out in a group; any other element child is skipped.
            if col.tag != MjmlTag::MjColumn {
                return Ok(());
            }
            // mjml's `getElementWidth(component.getWidthAsPixel())`: the column's own
            // width resolved against the *group* box.
            let raw = ctx.semantic.attribute(col, "width", None);
            let width = ColumnWidth::resolve(raw, num);
            let col_px = width.as_pixel(f64::from(group_width));
            let va = ctx
                .semantic
                .attribute(col, "vertical-align", Some("top"))
                .unwrap_or("top");
            buf.push_str(&format!(
                "<!--[if mso | IE]><td style=\"vertical-align:{va};width:{col_px}px;\" ><![endif]-->"
            ));
            crate::render::traced(col, ctx, buf, |ctx, buf| {
                Column::new_grouped(col, width, raw, num).render(ctx, buf)
            })?;
            buf.push_str("<!--[if mso | IE]></td><![endif]-->");
            Ok(())
        })?;
        ctx.container_width = saved;

        buf.push_str("<!--[if mso | IE]></tr></table><![endif]-->");
        buf.push_str("</div>");
        Ok(())
    }
}
