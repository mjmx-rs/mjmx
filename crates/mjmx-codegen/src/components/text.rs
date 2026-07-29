//! `<mj-text>` — a text block; body captured verbatim, renders inside a `<td>`.

use mjmx_ast::Element;

use crate::components::css_class::{class_attr, css_class};
use crate::context::RenderContext;
use crate::render::{Render, RenderResult, STYLE_CAPACITY, inner_text, push_decl};

// mjml default: Ubuntu font stack; color #000000. Whether the Ubuntu face is actually
// imported in the head is decided later by scanning the rendered body for the family
// name (see head.rs), so a resolved non-Ubuntu family emits no import.
const FONT_FAMILY: &str = "Ubuntu, Helvetica, Arial, sans-serif";
const COLOR: &str = "#000000";
const FONT_SIZE: &str = "13px";
const LINE_HEIGHT: &str = "1";
const ALIGN: &str = "left";
const PADDING: &str = "10px 25px";

/// Renderer for `<mj-text>`.
pub struct Text<'a> {
    element: &'a Element<'a>,
}

impl<'a> Text<'a> {
    /// Wrap an `<mj-text>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }
}

impl<'a> Render<'a> for Text<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        // Every themeable attribute resolves through the semantic layer so that
        // <mj-attributes> / mj-class / inline overrides win over the component
        // default, in that precedence order. Attributes with no mjml default are
        // resolved with `None` and only emitted when a value is present.
        let attr = |name: &str, default: Option<&'a str>| {
            ctx.semantic.attribute(self.element, name, default)
        };

        let font_family = attr("font-family", Some(FONT_FAMILY)).unwrap_or(FONT_FAMILY);
        let font_size = attr("font-size", Some(FONT_SIZE)).unwrap_or(FONT_SIZE);
        let line_height = attr("line-height", Some(LINE_HEIGHT)).unwrap_or(LINE_HEIGHT);
        let align = attr("align", Some(ALIGN)).unwrap_or(ALIGN);
        let color = attr("color", Some(COLOR)).unwrap_or(COLOR);
        let padding = attr("padding", Some(PADDING)).unwrap_or(PADDING);
        // No mjml default — only emitted when set.
        let font_style = attr("font-style", None);
        let font_weight = attr("font-weight", None);
        let letter_spacing = attr("letter-spacing", None);
        let text_decoration = attr("text-decoration", None);
        let text_transform = attr("text-transform", None);
        let height = attr("height", None);
        let container_background_color = attr("container-background-color", None);

        // mj-text is an "ending" tag: its body is a single verbatim text run.
        let content = inner_text(self.element);

        // Inner <div> style, in mjml's declaration order. font-family/size, line-height,
        // text-align and color always appear (component defaults); the rest only when
        // resolved to a value.
        let mut div_style = format!("font-family:{font_family};font-size:{font_size};");
        if let Some(v) = font_style {
            push_decl(&mut div_style, "font-style", v);
        }
        if let Some(v) = font_weight {
            push_decl(&mut div_style, "font-weight", v);
        }
        if let Some(v) = letter_spacing {
            push_decl(&mut div_style, "letter-spacing", v);
        }
        push_decl(&mut div_style, "line-height", line_height);
        push_decl(&mut div_style, "text-align", align);
        if let Some(v) = text_decoration {
            push_decl(&mut div_style, "text-decoration", v);
        }
        if let Some(v) = text_transform {
            push_decl(&mut div_style, "text-transform", v);
        }
        push_decl(&mut div_style, "color", color);
        if let Some(v) = height {
            push_decl(&mut div_style, "height", v);
        }

        // Outer <td>: container-background-color prepends a `background:` declaration.
        let mut td_style = String::with_capacity(STYLE_CAPACITY);
        if let Some(cbg) = container_background_color {
            push_decl(&mut td_style, "background", cbg);
        }
        td_style.push_str("font-size:0px;padding:");
        td_style.push_str(padding);
        td_style.push_str(";word-break:break-word;");

        // mjml's `mj-column` / `mj-hero` wrap each child in `<td align class style>`,
        // taking the class from the *child's* `css-class`.
        let class = class_attr(css_class(ctx, self.element).as_deref());
        buf.push_str("<td align=\"");
        buf.push_str(align);
        buf.push_str("\" ");
        buf.push_str(&class);
        buf.push_str("style=\"");
        buf.push_str(&td_style);
        buf.push_str("\">");
        // A fixed height wraps the div in an Outlook ghost cell.
        if let Some(h) = height {
            buf.push_str("<!--[if mso | IE]><table role=\"presentation\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\"><tr><td height=\"");
            buf.push_str(h);
            buf.push_str("\" style=\"vertical-align:top;height:");
            buf.push_str(h);
            buf.push_str(";\"><![endif]-->");
        }
        buf.push_str("<div style=\"");
        buf.push_str(&div_style);
        buf.push_str("\">");
        buf.push_str(&content);
        buf.push_str("</div>");
        if height.is_some() {
            buf.push_str("<!--[if mso | IE]></td></tr></table><![endif]-->");
        }
        buf.push_str("</td>");
        Ok(())
    }
}
