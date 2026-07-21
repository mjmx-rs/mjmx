//! `<mj-accordion>` and its children — a CSS-checkbox collapsible list.
//!
//! `mj-accordion` renders a `<table class="mj-accordion">` and registers the static
//! `.mj-accordion*` head stylesheet (the `:checked` show/hide hack). Each
//! `<mj-accordion-element>` is a `<label>` wrapping a hidden checkbox plus a title/content
//! pair. `<mj-accordion-title>` and `<mj-accordion-text>` are "ending" tags carrying raw
//! content. The border, font-family and icon-* attributes are resolved on the accordion
//! and threaded down to the children (mjml passes them as child attributes), so they are
//! carried here in [`AccordionInherited`].

use mrml_ast::{Element, Node};
use mrml_syntax::MjmlTag;

use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

// mjml defaults (mj-accordion).
const BORDER: &str = "2px solid black";
const FONT_FAMILY: &str = "Ubuntu, Helvetica, Arial, sans-serif";
const ICON_ALIGN: &str = "middle";
const ICON_WIDTH: &str = "32px";
const ICON_HEIGHT: &str = "32px";
const ICON_POSITION: &str = "right";
const ICON_WRAPPED_URL: &str = "https://i.imgur.com/bIXv1bk.png";
const ICON_WRAPPED_ALT: &str = "+";
const ICON_UNWRAPPED_URL: &str = "https://i.imgur.com/w4uTygT.png";
const ICON_UNWRAPPED_ALT: &str = "-";
const PADDING: &str = "10px 25px";
// mj-accordion-title / -text defaults.
const TITLE_FONT_SIZE: &str = "13px";
const TITLE_PADDING: &str = "16px";
const TEXT_FONT_SIZE: &str = "13px";
const TEXT_LINE_HEIGHT: &str = "1";
const TEXT_PADDING: &str = "16px";

/// The `.mj-accordion*` head stylesheet mjml always registers (static, no dynamic
/// values). Beautified to mjml 4.18.0's exact token stream.
const HEAD_CSS: &str = "\n    noinput.mj-accordion-checkbox {\n      display: block !important;\n    }\n\n    @media yahoo,\n    only screen and (min-width:0) {\n      .mj-accordion-element {\n        display: block;\n      }\n\n      input.mj-accordion-checkbox,\n      .mj-accordion-less {\n        display: none !important;\n      }\n\n      input.mj-accordion-checkbox+* .mj-accordion-title {\n        cursor: pointer;\n        touch-action: manipulation;\n        -webkit-user-select: none;\n        -moz-user-select: none;\n        user-select: none;\n      }\n\n      input.mj-accordion-checkbox+* .mj-accordion-content {\n        overflow: hidden;\n        display: none;\n      }\n\n      input.mj-accordion-checkbox+* .mj-accordion-more {\n        display: block !important;\n      }\n\n      input.mj-accordion-checkbox:checked+* .mj-accordion-content {\n        display: block;\n      }\n\n      input.mj-accordion-checkbox:checked+* .mj-accordion-more {\n        display: none !important;\n      }\n\n      input.mj-accordion-checkbox:checked+* .mj-accordion-less {\n        display: block !important;\n      }\n    }\n\n    .moz-text-html input.mj-accordion-checkbox+* .mj-accordion-title {\n      cursor: auto;\n      touch-action: auto;\n      -webkit-user-select: auto;\n      -moz-user-select: auto;\n      user-select: auto;\n    }\n\n    .moz-text-html input.mj-accordion-checkbox+* .mj-accordion-content {\n      overflow: hidden;\n      display: block;\n    }\n\n    .moz-text-html input.mj-accordion-checkbox+* .mj-accordion-ico {\n      display: none;\n    }\n\n    @goodbye {\n      @gmail\n    }\n\n  ";

/// Accordion-level attributes threaded down to the title/text children.
#[derive(Clone, Copy)]
struct AccordionInherited<'a> {
    border: &'a str,
    font_family: &'a str,
    icon_align: &'a str,
    icon_width: &'a str,
    icon_height: &'a str,
    icon_position: &'a str,
    icon_wrapped_url: &'a str,
    icon_wrapped_alt: &'a str,
    icon_unwrapped_url: &'a str,
    icon_unwrapped_alt: &'a str,
}

/// Renderer for `<mj-accordion>`.
pub struct Accordion<'a> {
    element: &'a Element<'a>,
}

impl<'a> Accordion<'a> {
    /// Wrap an `<mj-accordion>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }
}

impl<'a> Render<'a> for Accordion<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        let attr = |name: &str, default: &'a str| {
            ctx.semantic
                .attribute(self.element, name, Some(default))
                .unwrap_or(default)
        };

        let border = attr("border", BORDER);
        let font_family = attr("font-family", FONT_FAMILY);
        let padding = attr("padding", PADDING);
        let inherited = AccordionInherited {
            border,
            font_family,
            icon_align: attr("icon-align", ICON_ALIGN),
            icon_width: attr("icon-width", ICON_WIDTH),
            icon_height: attr("icon-height", ICON_HEIGHT),
            icon_position: attr("icon-position", ICON_POSITION),
            icon_wrapped_url: attr("icon-wrapped-url", ICON_WRAPPED_URL),
            icon_wrapped_alt: attr("icon-wrapped-alt", ICON_WRAPPED_ALT),
            icon_unwrapped_url: attr("icon-unwrapped-url", ICON_UNWRAPPED_URL),
            icon_unwrapped_alt: attr("icon-unwrapped-alt", ICON_UNWRAPPED_ALT),
        };

        // mjml always registers the accordion stylesheet when the component is used.
        ctx.add_style(HEAD_CSS);

        // Outer content cell (the column's generic wrapper, baked in as for other
        // components): accordion has no align, padding default 10px 25px.
        buf.push_str(&format!(
            "<td style=\"font-size:0px;padding:{padding};word-break:break-word;\">"
        ));
        buf.push_str(&format!(
            "<table cellspacing=\"0\" cellpadding=\"0\" class=\"mj-accordion\" style=\"width:100%;border-collapse:collapse;border:{border};border-bottom:none;font-family:{font_family};\">"
        ));
        buf.push_str("<tbody>");
        for el in self
            .element
            .element_children()
            .filter(|el| el.tag == MjmlTag::MjAccordionElement)
        {
            render_element_row(el, inherited, buf);
        }
        buf.push_str("</tbody></table></td>");
        Ok(())
    }
}

/// Render one `<mj-accordion-element>` row (the `<tr><td><label>…` block).
fn render_element_row(element: &Element<'_>, inh: AccordionInherited<'_>, buf: &mut String) {
    buf.push_str("<tr><td style=\"padding:0px;\">");
    buf.push_str("<label class=\"mj-accordion-element\" style=\"font-size:13px;\">");
    buf.push_str(
        "<!--[if !mso | IE]><!--><input class=\"mj-accordion-checkbox\" type=\"checkbox\" style=\"display:none;\" /><!--<![endif]-->",
    );
    buf.push_str("<div>");
    for child in element.element_children() {
        match child.tag {
            MjmlTag::MjAccordionTitle => render_title(child, inh, buf),
            MjmlTag::MjAccordionText => render_text(child, inh, buf),
            _ => {}
        }
    }
    buf.push_str("</div></label></td></tr>");
}

/// Render a `<mj-accordion-title>`.
fn render_title(element: &Element<'_>, inh: AccordionInherited<'_>, buf: &mut String) {
    let content = ending_content(element);
    buf.push_str("<div class=\"mj-accordion-title\">");
    buf.push_str(&format!(
        "<table cellspacing=\"0\" cellpadding=\"0\" style=\"width:100%;border-bottom:{};\">",
        inh.border
    ));
    buf.push_str("<tbody><tr>");

    let title_td = format!(
        "<td style=\"width:100%;font-size:{TITLE_FONT_SIZE};font-family:{};padding:{TITLE_PADDING};\"> {content} </td>",
        inh.font_family
    );
    let icon_td = format!(
        "<!--[if !mso | IE]><!--><td class=\"mj-accordion-ico\" style=\"padding:16px;vertical-align:{};\"><img src=\"{}\" alt=\"{}\" class=\"mj-accordion-more\" style=\"display:none;width:{};height:{};\" /><img src=\"{}\" alt=\"{}\" class=\"mj-accordion-less\" style=\"display:none;width:{};height:{};\" /></td><!--<![endif]-->",
        inh.icon_align,
        inh.icon_wrapped_url,
        inh.icon_wrapped_alt,
        inh.icon_width,
        inh.icon_height,
        inh.icon_unwrapped_url,
        inh.icon_unwrapped_alt,
        inh.icon_width,
        inh.icon_height,
    );

    if inh.icon_position == "left" {
        buf.push_str(&icon_td);
        buf.push_str(&title_td);
    } else {
        buf.push_str(&title_td);
        buf.push_str(&icon_td);
    }

    buf.push_str("</tr></tbody></table></div>");
}

/// Render a `<mj-accordion-text>`.
fn render_text(element: &Element<'_>, inh: AccordionInherited<'_>, buf: &mut String) {
    let content = ending_content(element);
    buf.push_str("<div class=\"mj-accordion-content\">");
    buf.push_str(&format!(
        "<table cellspacing=\"0\" cellpadding=\"0\" style=\"width:100%;border-bottom:{};\">",
        inh.border
    ));
    buf.push_str("<tbody><tr>");
    buf.push_str(&format!(
        "<td style=\"font-size:{TEXT_FONT_SIZE};font-family:{};line-height:{TEXT_LINE_HEIGHT};padding:{TEXT_PADDING};\"> {content} </td>",
        inh.font_family
    ));
    buf.push_str("</tr></tbody></table></div>");
}

/// The trimmed verbatim content of an ending tag (its text children concatenated).
fn ending_content(element: &Element<'_>) -> String {
    let mut s = String::new();
    for child in element.children() {
        if let Node::Text(t) = child {
            s.push_str(t.value.as_str());
        }
    }
    s.trim().to_string()
}
