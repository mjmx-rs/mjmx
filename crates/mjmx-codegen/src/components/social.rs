//! `<mj-social>` — a row (or stack) of social network icons.
//!
//! `mj-social` owns the outer `<td>` and the layout: **horizontal** mode wraps each
//! `<mj-social-element>` in its own `display:inline-table` table interleaved with mso
//! conditional cells; **vertical** mode stacks every element's row in a single table.
//! Themeable attributes (colors, sizes, fonts) are forwarded to the children as their
//! defaults via [`SocialInherited`].

use mjmx_ast::Element;
use mjmx_syntax::MjmlTag;

use crate::components::css_class::{class_attr, css_class};
use crate::components::social_element::{SocialElement, SocialInherited};
use crate::context::RenderContext;
use crate::render::{Render, RenderResult, STYLE_CAPACITY, push_decl};

// mjml defaults.
const ALIGN: &str = "center";
const PADDING: &str = "10px 25px";
const MODE: &str = "horizontal";
const BORDER_RADIUS: &str = "3px";
const COLOR: &str = "#333333";
const FONT_FAMILY: &str = "Ubuntu, Helvetica, Arial, sans-serif";
const FONT_SIZE: &str = "13px";
const ICON_SIZE: &str = "20px";
const LINE_HEIGHT: &str = "22px";
const TEXT_DECORATION: &str = "none";

/// Renderer for `<mj-social>`.
pub struct Social<'a> {
    element: &'a Element<'a>,
}

impl<'a> Social<'a> {
    /// Wrap an `<mj-social>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }
}

impl<'a> Render<'a> for Social<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        let some = |name: &str, default: &'a str| {
            ctx.semantic
                .attribute(self.element, name, Some(default))
                .unwrap_or(default)
        };
        let opt = |name: &str| ctx.semantic.attribute(self.element, name, None);

        let align = some("align", ALIGN);
        let padding = some("padding", PADDING);
        let mode = some("mode", MODE);
        let container_background_color = opt("container-background-color");

        // Attributes forwarded to the elements as their defaults (mjml's
        // `getSocialElementAttributes`): the defaulted ones plus any explicitly-set
        // pass-throughs. `inner-padding` maps onto the element `padding`.
        let inherited = SocialInherited {
            // mjml 5.4+ forwards `border` to the elements, but only when set (it has no
            // default on `<mj-social>`); the element's own `border` default is `0`. Under
            // v4 the element renderer ignores this, so forwarding it is harmless there.
            border: opt("border"),
            border_radius: Some(some("border-radius", BORDER_RADIUS)),
            color: Some(some("color", COLOR)),
            font_family: Some(some("font-family", FONT_FAMILY)),
            font_size: Some(some("font-size", FONT_SIZE)),
            font_weight: opt("font-weight"),
            font_style: opt("font-style"),
            icon_size: Some(some("icon-size", ICON_SIZE)),
            icon_height: opt("icon-height"),
            icon_padding: opt("icon-padding"),
            text_padding: opt("text-padding"),
            line_height: Some(some("line-height", LINE_HEIGHT)),
            text_decoration: Some(some("text-decoration", TEXT_DECORATION)),
            padding: opt("inner-padding"),
        };

        let elements: Vec<&Element> = self
            .element
            .element_children()
            .filter(|el| el.tag == MjmlTag::MjSocialElement)
            .collect();

        let mut td_style = String::with_capacity(STYLE_CAPACITY);
        if let Some(cbg) = container_background_color {
            push_decl(&mut td_style, "background", cbg);
        }
        td_style.push_str("font-size:0px;padding:");
        td_style.push_str(padding);
        td_style.push_str(";word-break:break-word;");
        // mjml puts the child's `css-class` on the wrapping `<td>` (mj-column /
        // mj-hero `renderChildren`), between `align` and `style`.
        let class = class_attr(css_class(ctx, self.element).as_deref());
        buf.push_str("<td align=\"");
        buf.push_str(align);
        buf.push_str("\" ");
        buf.push_str(&class);
        buf.push_str("style=\"");
        buf.push_str(&td_style);
        buf.push_str("\">");

        if mode == "vertical" {
            buf.push_str(
                "<table border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"margin:0px;\">",
            );
            buf.push_str("<tbody>");
            for el in &elements {
                crate::render::traced(el, ctx, buf, |ctx, buf| {
                    SocialElement::new(el, inherited).render_row(ctx, buf);
                });
            }
            buf.push_str("</tbody></table>");
        } else {
            // Horizontal: mjml merges the adjacent mso conditional comments, so the
            // ghost `<table><tr><td>` opens once, `</td><td>` separates elements, and
            // `</td></tr></table>` closes.
            buf.push_str("<!--[if mso | IE]><table align=\"");
            buf.push_str(align);
            buf.push_str("\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" ><tr><td><![endif]-->");
            for (i, el) in elements.iter().enumerate() {
                if i > 0 {
                    buf.push_str("<!--[if mso | IE]></td><td><![endif]-->");
                }
                buf.push_str("<table align=\"");
                buf.push_str(align);
                buf.push_str("\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"float:none;display:inline-table;\">");
                buf.push_str("<tbody>");
                crate::render::traced(el, ctx, buf, |ctx, buf| {
                    SocialElement::new(el, inherited).render_row(ctx, buf);
                });
                buf.push_str("</tbody></table>");
            }
            buf.push_str("<!--[if mso | IE]></td></tr></table><![endif]-->");
        }

        buf.push_str("</td>");
        Ok(())
    }
}
