//! `<mj-column>` — an inline-block column that registers its responsive rule.

use mjmx_ast::Element;

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

        // Themeable attributes. `vertical-align` defaults to `top`; the others have no
        // default and only appear when set.
        let opt = |name: &str| ctx.semantic.attribute(self.element, name, None);
        let vertical_align = ctx
            .semantic
            .attribute(self.element, "vertical-align", Some("top"))
            .unwrap_or("top");
        let background_color = opt("background-color");
        let border = opt("border");
        let border_radius = opt("border-radius");
        // Any padding turns the column into a "gutter" column: the content is wrapped
        // in an extra cell that carries background/border/padding.
        let padding = opt("padding");

        // Inline outlook-div width: a section column stacks on mobile (100%, share via
        // the media query); a group column holds its share inline (no stacking).
        let div_width = if self.in_group { pct } else { 100 };
        buf.push_str(&format!(
            "<div class=\"{class} mj-outlook-group-fix\" style=\"font-size:0px;text-align:left;direction:ltr;display:inline-block;vertical-align:{vertical_align};width:{div_width}%;\">"
        ));

        // The box declarations shared by the no-gutter table and the gutter cell, in
        // mjml's order: background-color, border, border-radius, vertical-align.
        let mut box_style = String::new();
        if let Some(bg) = background_color {
            box_style.push_str(&format!("background-color:{bg};"));
        }
        if let Some(b) = border {
            box_style.push_str(&format!("border:{b};"));
        }
        if let Some(br) = border_radius {
            box_style.push_str(&format!("border-radius:{br};"));
        }
        box_style.push_str(&format!("vertical-align:{vertical_align};"));

        if let Some(padding) = padding {
            // Gutter column: outer table wraps a cell that holds the box styles.
            buf.push_str(
                "<table border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" width=\"100%\" style=\"border-collapse:separate;\">",
            );
            buf.push_str("<tbody><tr>");
            buf.push_str(&format!(
                "<td style=\"{box_style}border-collapse:separate;padding:{padding};\">"
            ));
            buf.push_str(
                "<table border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"\" width=\"100%\">",
            );
            buf.push_str("<tbody>");
            for child in self.element.element_children() {
                buf.push_str("<tr>");
                render_element(child, ctx, buf)?;
                buf.push_str("</tr>");
            }
            buf.push_str("</tbody></table></td></tr></tbody></table></div>");
        } else {
            // No gutter: the box styles sit directly on the single content table.
            buf.push_str(&format!(
                "<table border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"{box_style}\" width=\"100%\">"
            ));
            buf.push_str("<tbody>");
            for child in self.element.element_children() {
                buf.push_str("<tr>");
                render_element(child, ctx, buf)?;
                buf.push_str("</tr>");
            }
            buf.push_str("</tbody></table></div>");
        }
        Ok(())
    }
}
