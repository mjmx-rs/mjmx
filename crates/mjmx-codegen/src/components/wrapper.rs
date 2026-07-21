//! `<mj-wrapper>` — a section-shaped container that wraps whole `<mj-section>`s so a
//! background can span the full row.
//!
//! Structurally it renders exactly like a section (the centered `max-width` container,
//! the `20px 0` padding cell, the presentation ghost table), but its ghost row wraps
//! each *section* child in `<tr><td width=…>` instead of laying out columns. A
//! `full-width="full-width"` wrapper adds an outer 100%-width table around that same
//! inner markup.
//!
//! The wrapper emits its Outlook conditional comments as independent blocks; the
//! document-level `mergeOutlookConditionnals` pass fuses the adjacent
//! `<![endif]--><!--[if mso | IE]>` seams (e.g. the wrapper cell flowing straight into
//! a nested section's ghost table), reproducing mjml byte-for-byte.

use mjmx_ast::Element;
use mjmx_syntax::MjmlTag;

use crate::components::Section;
use crate::components::background::Background;
use crate::components::css_class::{class_attr, css_class, suffix_css_classes};
use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

/// Renderer for `<mj-wrapper>`.
pub struct Wrapper<'a> {
    element: &'a Element<'a>,
}

impl<'a> Wrapper<'a> {
    /// Wrap an `<mj-wrapper>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }

    /// Whether this is a `full-width="full-width"` wrapper.
    fn is_full_width(&self) -> bool {
        self.element.attr("full-width") == Some("full-width")
    }

    /// The centered-container body shared by both variants (renderBefore + the section
    /// container + the wrapped section children + renderAfter).
    fn render_inner(
        &self,
        ctx: &mut RenderContext<'a>,
        buf: &mut String,
        bg: &Background<'a>,
        full_width: bool,
    ) -> RenderResult {
        let width = ctx.container_width;

        // Themeable attributes (a wrapper shares mj-section's styling).
        let attr = |name: &str, default: &'a str| {
            ctx.semantic
                .attribute(self.element, name, Some(default))
                .unwrap_or(default)
        };
        let opt = |name: &str| ctx.semantic.attribute(self.element, name, None);
        let padding = attr("padding", "20px 0");
        let direction = attr("direction", "ltr");
        let text_align = attr("text-align", "center");
        let border = opt("border");
        let border_radius = opt("border-radius");

        // Content box handed to nested sections: wrapper width minus horizontal padding
        // and borders.
        let box_width = width - horizontal_padding(padding) - border.map_or(0, horizontal_border);

        // In the full-width variant the background moved out to the outer 100% table.
        let bg_prefix = if full_width { String::new() } else { bg.css() };
        let bg_attr = match (full_width, bg.url()) {
            (false, Some(url)) => format!("background=\"{url}\" "),
            _ => String::new(),
        };

        // The centered container.
        let radius_div = border_radius
            .map(|br| format!("border-radius:{br};overflow:hidden;"))
            .unwrap_or_default();
        // A wrapper reuses mj-section's `renderSection`: `css-class` on the centered
        // div, unless full-width (where it moved to the outer 100% table).
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

        // Presentation ghost table wrapping each section child in its own <tr><td>.
        buf.push_str(
            "<!--[if mso | IE]><table role=\"presentation\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\"><![endif]-->",
        );

        let saved = ctx.container_width;
        ctx.container_width = box_width;
        for child in self
            .element
            .element_children()
            .filter(|el| el.tag == MjmlTag::MjSection)
        {
            // mjml's `renderWrappedChildren`: the ghost cell carries the *section*
            // child's `css-class`, suffixed with `-outlook`.
            let ghost_class = suffix_css_classes(css_class(ctx, child).as_deref(), "outlook");
            buf.push_str(&format!(
                "<!--[if mso | IE]><tr><td class=\"{ghost_class}\" width=\"{width}px\" ><![endif]-->"
            ));
            Section::new(child).render(ctx, buf)?;
            buf.push_str("<!--[if mso | IE]></td></tr><![endif]-->");
        }
        ctx.container_width = saved;

        buf.push_str("<!--[if mso | IE]></table><![endif]-->");
        buf.push_str("</td></tr></tbody></table>");
        if bg.has_url() {
            buf.push_str("</div>");
        }
        buf.push_str("</div>");
        Ok(())
    }

    /// mjml's `renderBefore`: the section-shaped mso ghost table.
    fn render_before(&self, ctx: &RenderContext<'a>, buf: &mut String, bg: &Background<'a>) {
        let width = ctx.container_width;
        let mso_bgcolor = bg
            .color()
            .map(|c| format!("bgcolor=\"{c}\" "))
            .unwrap_or_default();
        let ghost_class = suffix_css_classes(css_class(ctx, self.element).as_deref(), "outlook");
        buf.push_str(&format!(
            "<!--[if mso | IE]><table align=\"center\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" class=\"{ghost_class}\" role=\"presentation\" style=\"width:{width}px;\" width=\"{width}\" {mso_bgcolor}><tr><td style=\"line-height:0px;font-size:0px;mso-line-height-rule:exactly;\"><![endif]-->"
        ));
    }

    /// mjml's `renderAfter`.
    fn render_after(buf: &mut String) {
        buf.push_str("<!--[if mso | IE]></td></tr></table><![endif]-->");
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

impl<'a> Render<'a> for Wrapper<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        let bg = Background::resolve(self.element, ctx);
        let width = ctx.container_width;

        if self.is_full_width() {
            // Outer full-width table (which carries the background), then the identical
            // centered-container body wrapped in the VML rect.
            let bg_attr = bg
                .url()
                .map(|url| format!("background=\"{url}\" "))
                .unwrap_or_default();
            let class = class_attr(css_class(ctx, self.element).as_deref());
            buf.push_str(&format!(
                "<table align=\"center\" {class}{bg_attr}border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"{}width:100%;\"><tbody><tr><td>",
                bg.css()
            ));
            bg.vml_open(buf, true, width);
            self.render_before(ctx, buf, &bg);
            self.render_inner(ctx, buf, &bg, true)?;
            Self::render_after(buf);
            bg.vml_close(buf);
            buf.push_str("</td></tr></tbody></table>");
            Ok(())
        } else {
            self.render_before(ctx, buf, &bg);
            bg.vml_open(buf, false, width);
            self.render_inner(ctx, buf, &bg, false)?;
            bg.vml_close(buf);
            Self::render_after(buf);
            Ok(())
        }
    }
}
