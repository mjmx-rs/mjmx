//! `<mj-section>` — a full-width row with an mso/IE ghost-table layout.

use mjmx_ast::Element;
use mjmx_syntax::MjmlTag;

use crate::components::background::Background;
use crate::components::css_class::{class_attr, css_class, suffix_css_classes};
use crate::components::{Column, ColumnWidth, Group, Gutter};
use crate::context::RenderContext;
use crate::render::{Render, RenderResult, render_children, render_element};

/// Renderer for `<mj-section>`.
pub struct Section<'a> {
    element: &'a Element<'a>,
}

impl<'a> Section<'a> {
    /// Wrap an `<mj-section>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }

    /// Whether this is a `full-width="full-width"` section.
    fn is_full_width(&self) -> bool {
        self.element.attr("full-width") == Some("full-width")
    }

    /// The centered container: everything between the mso ghost table's cell and its
    /// close (mjml's `renderSection`).
    fn render_section(
        &self,
        ctx: &mut RenderContext<'a>,
        buf: &mut String,
        bg: &Background<'a>,
        full_width: bool,
    ) -> RenderResult {
        // The section's own width is the container width handed down to it (600px at
        // the body root, or whatever a wrapper threads in).
        let width = ctx.container_width;

        // Themeable attributes.
        let attr = |name: &str, default: &'a str| {
            ctx.semantic
                .attribute(self.element, name, Some(default))
                .unwrap_or(default)
        };
        let opt = |name: &str| ctx.semantic.attribute(self.element, name, None);
        // mjml default: section padding 20px 0, direction ltr, text-align center.
        let padding = attr("padding", "20px 0");
        let direction = attr("direction", "ltr");
        let text_align = attr("text-align", "center");
        let border = opt("border");
        let border_radius = opt("border-radius");

        // Content box handed to children: section width minus horizontal padding and
        // borders (mjml's getBoxWidths).
        let box_width = width - horizontal_padding(padding) - border.map_or(0, horizontal_border);

        // Non-raw layout children (columns and groups) share the row equally; a single
        // child spans 100%. Integer math is exact for the divisors mjml emits.
        let children: Vec<&Element> = self
            .element
            .element_children()
            .filter(|el| matches!(el.tag, MjmlTag::MjColumn | MjmlTag::MjGroup))
            .collect();
        let count = children.len().max(1) as u32;

        // The background declarations ride on the div and the inner table — unless the
        // section is full-width, in which case they moved out to the outer 100% table.
        let bg_prefix = if full_width { String::new() } else { bg.css() };
        // A background image is painted through an extra zero-line-height div (mjml's
        // `innerDiv`), and the table repeats the url as an HTML attribute for clients
        // that ignore CSS backgrounds.
        let bg_attr = match (full_width, bg.url()) {
            (false, Some(url)) => format!("background=\"{url}\" "),
            _ => String::new(),
        };

        // Real centered container. Border-radius adds a rounded-clip on the div and
        // forces `border-collapse:separate` on the inner table.
        let radius_div = border_radius
            .map(|br| format!("border-radius:{br};overflow:hidden;"))
            .unwrap_or_default();
        // mjml's `renderSection`: the centered div carries `css-class` — but only when
        // the section is not full-width, where the class moved out to the 100% table.
        let div_class = if full_width {
            String::new()
        } else {
            class_attr(css_class(ctx, self.element).as_deref())
        };
        buf.push_str(&format!(
            "<div {div_class}style=\"{bg_prefix}margin:0px auto;max-width:{width}px;{radius_div}\">"
        ));
        if bg.has_url() {
            buf.push_str("<div style=\"line-height:0;font-size:0;\">");
        }
        let table_suffix = if border_radius.is_some() {
            "border-collapse:separate;"
        } else {
            ""
        };
        buf.push_str(&format!(
            "<table align=\"center\" {bg_attr}border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"{bg_prefix}width:100%;{table_suffix}\">"
        ));
        buf.push_str("<tbody><tr>");
        // Section content <td>: border and border-radius only when set; direction,
        // padding and text-align always (component defaults).
        let mut td_style = String::new();
        if let Some(b) = border {
            td_style.push_str(&format!("border:{b};"));
        }
        if let Some(br) = border_radius {
            td_style.push_str(&format!("border-radius:{br};"));
        }
        td_style.push_str(&format!(
            "direction:{direction};font-size:0px;padding:{padding};text-align:{text_align};"
        ));
        buf.push_str(&format!("<td style=\"{td_style}\">"));

        // mjml 5's section `gutter` (v5 only): threaded to each column, which reduces
        // its width and grows padding by its share. `None` under V4 leaves every gutter
        // branch inert, so V4 output is unchanged. `index`/`total` track each child's
        // position (mjml counts raw children too) for `first`/`last`.
        let section_gutter = if ctx.options.compat.is_v5() {
            opt("gutter")
        } else {
            None
        };
        let total_children = self.element.element_children().count() as u32;
        let mut child_index = 0u32;

        // Children get the content box as their container width.
        let saved = ctx.container_width;
        ctx.container_width = box_width;

        // Column ghost row: <table><tr>, then a ghost <td> per child interleaved with
        // the real child markup. Each ghost <td>'s style comes from the child: a column
        // carries `vertical-align:top`, a group carries only its pixel width.
        //
        // Every conditional block is opened *and closed* here, exactly as mjml's
        // `renderWrappedChildren` emits them; `merge_outlook_conditionals` fuses the
        // adjacent ones back into one block afterwards, so the usual output is
        // byte-identical. Keeping them closed is what lets a raw/comment child land
        // between two blocks instead of inside an open comment (comments do not nest —
        // an inner `-->` would end the conditional early and leak Outlook markup).
        buf.push_str(
            "<!--[if mso | IE]><table role=\"presentation\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\"><tr><![endif]-->",
        );
        render_children(self.element, ctx, buf, |child, ctx, buf| {
            // Every child consumes an index (mjml counts raw children too), so `first`/
            // `last` for the gutter geometry line up with mjml's `props.index`.
            let index = child_index;
            child_index += 1;

            // Only columns and groups take part in the ghost row. Any other child is a
            // raw element (mjml's `isRawElement`, i.e. mj-raw), which mjml's
            // `renderWrappedChildren` emits directly — between the opened-and-closed row
            // conditionals, never inside a ghost `<td>`.
            if !matches!(child.tag, MjmlTag::MjColumn | MjmlTag::MjGroup) {
                return render_element(child, ctx, buf);
            }
            // mjml's `renderWrappedChildren`: the ghost cell's class is the *child's*
            // `css-class`, suffixed with `-outlook` (empty string when unset, which is
            // why the bare `class=""` is always emitted).
            let ghost_class = suffix_css_classes(css_class(ctx, child).as_deref(), "outlook");
            match child.tag {
                MjmlTag::MjGroup => {
                    let raw = ctx.semantic.attribute(child, "width", None);
                    let child_px = ColumnWidth::resolve(raw, count).as_pixel(f64::from(box_width));
                    // The ghost cell carries the group's vertical-align when it is set.
                    let mut style = String::new();
                    if let Some(va) = ctx.semantic.attribute(child, "vertical-align", None) {
                        style.push_str(&format!("vertical-align:{va};"));
                    }
                    buf.push_str(&format!(
                        "<!--[if mso | IE]><td class=\"{ghost_class}\" style=\"{style}width:{child_px}px;\" ><![endif]-->"
                    ));
                    crate::render::traced(child, ctx, buf, |ctx, buf| {
                        Group::new(child, count, section_gutter).render(ctx, buf)
                    })?;
                }
                _ => {
                    // The ghost cell mirrors the column's own vertical-align and carries
                    // mjml's `getWidthAsPixel` — the column's own width (explicit `width`
                    // attribute, or its equal share) resolved against the content box.
                    let raw = ctx.semantic.attribute(child, "width", None);
                    let width = ColumnWidth::resolve(raw, count);
                    let child_px = width.as_pixel(f64::from(box_width));
                    let va = ctx
                        .semantic
                        .attribute(child, "vertical-align", Some("top"))
                        .unwrap_or("top");
                    // The column's section gutter: reduces the ghost cell nothing (width
                    // stays full) but adds `getOutlookGutterStyles` padding after it.
                    let column = Column::new(
                        child,
                        width,
                        raw,
                        count,
                        Gutter {
                            value: section_gutter,
                            direction,
                            index,
                            total: total_children,
                        },
                    );
                    let outlook_padding = column
                        .outlook_gutter_padding(f64::from(box_width))
                        .map(|p| format!("padding:{p};"))
                        .unwrap_or_default();
                    buf.push_str(&format!(
                        "<!--[if mso | IE]><td class=\"{ghost_class}\" style=\"vertical-align:{va};width:{child_px}px;{outlook_padding}\" ><![endif]-->"
                    ));
                    crate::render::traced(child, ctx, buf, |ctx, buf| {
                        column.render(ctx, buf)
                    })?;
                }
            }
            buf.push_str("<!--[if mso | IE]></td><![endif]-->");
            Ok(())
        })?;
        buf.push_str("<!--[if mso | IE]></tr></table><![endif]-->");

        ctx.container_width = saved;

        buf.push_str("</td></tr></tbody></table>");
        if bg.has_url() {
            buf.push_str("</div>");
        }
        buf.push_str("</div>");
        Ok(())
    }

    /// mjml's `renderBefore`: the mso/IE ghost table fixed to the section width, which
    /// Outlook honors in place of the centered div, carrying the background color.
    fn render_before(&self, ctx: &RenderContext<'a>, buf: &mut String, bg: &Background<'a>) {
        let width = ctx.container_width;
        let mso_bgcolor = bg
            .color()
            .map(|c| format!("bgcolor=\"{c}\" "))
            .unwrap_or_default();
        // mjml's `renderBefore`: `suffixCssClasses(css-class, 'outlook')`.
        let ghost_class = suffix_css_classes(css_class(ctx, self.element).as_deref(), "outlook");
        buf.push_str(&format!(
            "<!--[if mso | IE]><table align=\"center\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" class=\"{ghost_class}\" role=\"presentation\" style=\"width:{width}px;\" width=\"{width}\" {mso_bgcolor}><tr><td style=\"line-height:0px;font-size:0px;mso-line-height-rule:exactly;\"><![endif]-->"
        ));
    }

    /// mjml's `renderAfter`: closes the ghost table opened by [`Self::render_before`].
    fn render_after(buf: &mut String) {
        buf.push_str("<!--[if mso | IE]></td></tr></table><![endif]-->");
    }
}

impl<'a> Render<'a> for Section<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        let bg = Background::resolve(self.element, ctx);
        let full_width = self.is_full_width();
        let width = ctx.container_width;

        if full_width {
            // mjml's `renderFullWidth`: a 100%-wide table carries the background, and
            // the VML rect (stretched with `mso-width-percent`) wraps the whole ghost
            // table + centered container.
            let bg_attr = bg
                .url()
                .map(|url| format!("background=\"{url}\" "))
                .unwrap_or_default();
            // mjml's `renderFullWidth` key order: align, class, background, border, …
            let class = class_attr(css_class(ctx, self.element).as_deref());
            buf.push_str(&format!(
                "<table align=\"center\" {class}{bg_attr}border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"{}width:100%;\"><tbody><tr><td>",
                bg.css()
            ));
            bg.vml_open(buf, true, width);
            self.render_before(ctx, buf, &bg);
            self.render_section(ctx, buf, &bg, true)?;
            Self::render_after(buf);
            bg.vml_close(buf);
            buf.push_str("</td></tr></tbody></table>");
        } else {
            // mjml's `renderSimple`: the ghost table brackets the VML rect, which in
            // turn brackets the centered container.
            self.render_before(ctx, buf, &bg);
            bg.vml_open(buf, false, width);
            self.render_section(ctx, buf, &bg, false)?;
            bg.vml_close(buf);
            Self::render_after(buf);
        }
        Ok(())
    }
}

/// Total left + right padding, in pixels, from a CSS padding shorthand.
fn horizontal_padding(padding: &str) -> u32 {
    let parts: Vec<u32> = padding
        .split_whitespace()
        .map(|v| {
            v.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect();
    match parts.as_slice() {
        [all] => all * 2,
        [_tb, lr] => lr * 2,
        [_t, lr, _b] => lr * 2,
        [_t, r, _b, l] => r + l,
        _ => 0,
    }
}

/// Combined left + right border width from a `border` shorthand (`"2px solid #000"`).
/// mjml reads the same `border` value for both sides.
fn horizontal_border(border: &str) -> u32 {
    let one = border
        .split_whitespace()
        .find_map(|v| {
            v.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse::<u32>()
                .ok()
        })
        .unwrap_or(0);
    one * 2
}
