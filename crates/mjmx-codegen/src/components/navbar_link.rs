//! `<mj-navbar-link>` — one link in an `<mj-navbar>` bar.
//!
//! `mj-navbar-link` is an "ending" tag: its label is captured verbatim. `<mj-navbar>`
//! owns the outer ghost table and interleaves the mso conditional `<td>`s (which carry
//! each link's padding), so it drives rendering by asking each link for its `<td>` style
//! and outlook class and then for its `<a>`. The dispatch entry point renders a
//! self-contained link (mso `<td>` + anchor) for a stray standalone element.

use mjmx_ast::{Element, Node};

use crate::components::css_class::{css_class, suffix_css_classes};
use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

// mjml defaults.
const COLOR: &str = "#000000";
const FONT_FAMILY: &str = "Ubuntu, Helvetica, Arial, sans-serif";
const FONT_SIZE: &str = "13px";
const FONT_WEIGHT: &str = "normal";
const LINE_HEIGHT: &str = "22px";
const PADDING: &str = "15px 10px";
const TARGET: &str = "_blank";
const TEXT_DECORATION: &str = "none";
const TEXT_TRANSFORM: &str = "uppercase";

/// Renderer for `<mj-navbar-link>`.
pub struct NavbarLink<'a> {
    element: &'a Element<'a>,
    /// The `<mj-navbar base-url>`, prepended to each `href` when set.
    base_url: Option<&'a str>,
}

impl<'a> NavbarLink<'a> {
    /// Wrap a link, threading the parent navbar's `base-url`.
    #[must_use]
    pub fn new(element: &'a Element<'a>, base_url: Option<&'a str>) -> Self {
        Self { element, base_url }
    }

    fn attr(&self, ctx: &RenderContext<'a>, name: &str, default: &'a str) -> &'a str {
        ctx.semantic
            .attribute(self.element, name, Some(default))
            .unwrap_or(default)
    }

    /// The mso ghost `<td>` inline style (the link's padding).
    pub fn td_style(&self, ctx: &RenderContext<'a>) -> String {
        let mut s = format!("padding:{};", self.attr(ctx, "padding", PADDING));
        for (name, prop) in [
            ("padding-top", "padding-top"),
            ("padding-right", "padding-right"),
            ("padding-bottom", "padding-bottom"),
            ("padding-left", "padding-left"),
        ] {
            if let Some(v) = self.element.attr(name) {
                s.push_str(&format!("{prop}:{v};"));
            }
        }
        s
    }

    /// The outlook-suffixed class list for the ghost `<td>` (empty without `css-class`),
    /// mjml's `suffixCssClasses(this.getAttribute('css-class'), 'outlook')`.
    pub fn outlook_class(&self, ctx: &RenderContext<'a>) -> String {
        suffix_css_classes(css_class(ctx, self.element).as_deref(), "outlook")
    }

    /// Render the `<a class="mj-link" …>` anchor.
    pub fn render_anchor(&self, ctx: &RenderContext<'a>, buf: &mut String) {
        let color = self.attr(ctx, "color", COLOR);
        let font_family = self.attr(ctx, "font-family", FONT_FAMILY);
        let font_size = self.attr(ctx, "font-size", FONT_SIZE);
        let font_style = self.element.attr("font-style");
        let font_weight = self.attr(ctx, "font-weight", FONT_WEIGHT);
        let letter_spacing = self.element.attr("letter-spacing");
        let line_height = self.attr(ctx, "line-height", LINE_HEIGHT);
        let text_decoration = self.attr(ctx, "text-decoration", TEXT_DECORATION);
        let text_transform = self.attr(ctx, "text-transform", TEXT_TRANSFORM);
        let padding = self.attr(ctx, "padding", PADDING);
        let target = self.attr(ctx, "target", TARGET);
        let rel = self.element.attr("rel");
        let name = self.element.attr("name");

        let href = self.element.attr("href").unwrap_or("");
        let link = match self.base_url {
            Some(base) => format!("{base}{href}"),
            None => href.to_string(),
        };
        // mjml's `renderContent`: `class="mj-link${css-class ? ` ${css-class}` : ''}"`.
        let a_class = match css_class(ctx, self.element) {
            Some(c) => format!("mj-link {c}"),
            None => "mj-link".to_string(),
        };

        // Anchor style: display, color, font-family, font-size, [font-style],
        // font-weight, [letter-spacing], line-height, text-decoration, text-transform,
        // padding — matching mjml's key order.
        let mut style = format!(
            "display:inline-block;color:{color};font-family:{font_family};font-size:{font_size};"
        );
        if let Some(fs) = font_style {
            style.push_str(&format!("font-style:{fs};"));
        }
        style.push_str(&format!("font-weight:{font_weight};"));
        if let Some(ls) = letter_spacing {
            style.push_str(&format!("letter-spacing:{ls};"));
        }
        style.push_str(&format!(
            "line-height:{line_height};text-decoration:{text_decoration};text-transform:{text_transform};padding:{padding};"
        ));

        buf.push_str(&format!("<a class=\"{a_class}\" href=\"{link}\""));
        if let Some(rel) = rel {
            buf.push_str(&format!(" rel=\"{rel}\""));
        }
        buf.push_str(&format!(" target=\"{target}\""));
        if let Some(name) = name {
            buf.push_str(&format!(" name=\"{name}\""));
        }
        buf.push_str(&format!(
            " style=\"{style}\"> {content} </a>",
            content = self.content()
        ));
    }

    /// The link's trimmed label text.
    fn content(&self) -> String {
        let mut s = String::new();
        for child in self.element.children() {
            if let Node::Text(t) = child {
                s.push_str(t.value.as_str());
            }
        }
        s.trim().to_string()
    }
}

impl<'a> Render<'a> for NavbarLink<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        // Standalone: emit mjml's self-contained mso `<td>` + anchor.
        buf.push_str(&format!(
            "<!--[if mso | IE]><td style=\"{}\" class=\"{}\" ><![endif]-->",
            self.td_style(ctx),
            self.outlook_class(ctx)
        ));
        self.render_anchor(ctx, buf);
        buf.push_str("<!--[if mso | IE]></td><![endif]-->");
        Ok(())
    }
}
