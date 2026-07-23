//! `<mj-social-element>` — one social network icon (+ optional label) in a row.
//!
//! Each element renders a `<tr>` with an icon cell and, when it has text content, a
//! label cell. `<mj-social>` owns the surrounding markup and the mode (horizontal wraps
//! each element in its own inline table; vertical stacks the rows in one table), so the
//! parent calls [`SocialElement::render_row`]. The dispatch entry point renders a bare
//! row for a stray standalone element.
//!
//! `name` selects a built-in network carrying a baked icon `src`, `background-color`
//! and a share-`href` template; any of those is overridable with an explicit attribute.

use mjmx_ast::{Element, Node};

use crate::components::css_class::{css_class, leading_class_attr};
use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

const IMG_BASE_URL: &str = "https://www.mailjet.com/images/theme/v1/icons/ico-social/";

// mj-social-element's own defaults (mjml). Attributes the parent `<mj-social>` forwards
// override these via [`SocialInherited`].
const PADDING: &str = "4px";
const TEXT_PADDING: &str = "4px 4px 4px 0";
const VERTICAL_ALIGN: &str = "middle";
const ALIGN: &str = "left";
const COLOR: &str = "#000";
const BORDER_RADIUS: &str = "3px";
const FONT_FAMILY: &str = "Ubuntu, Helvetica, Arial, sans-serif";
const FONT_SIZE: &str = "13px";
const LINE_HEIGHT: &str = "1";
const ICON_SIZE: &str = "20px";
const TEXT_DECORATION: &str = "none";
const TARGET: &str = "_blank";
const ALT: &str = "";

/// Attribute values `<mj-social>` forwards to its elements, standing in as the element
/// defaults (an explicit attribute on the element still wins). A `None` means the parent
/// forwarded nothing, so the element falls back to its own default.
#[derive(Debug, Clone, Copy, Default)]
pub struct SocialInherited<'a> {
    /// `<mj-social border>`, forwarded as the element's `border` default (mjml 5.4+).
    pub border: Option<&'a str>,
    pub border_radius: Option<&'a str>,
    pub color: Option<&'a str>,
    pub font_family: Option<&'a str>,
    pub font_size: Option<&'a str>,
    pub font_weight: Option<&'a str>,
    pub font_style: Option<&'a str>,
    pub icon_size: Option<&'a str>,
    pub icon_height: Option<&'a str>,
    pub icon_padding: Option<&'a str>,
    pub text_padding: Option<&'a str>,
    pub line_height: Option<&'a str>,
    pub text_decoration: Option<&'a str>,
    /// `<mj-social inner-padding>`, forwarded as the element's `padding`.
    pub padding: Option<&'a str>,
}

/// A built-in social network's baked data.
struct Network {
    /// Share-URL template with a `[[URL]]` placeholder, if the network is shareable.
    share_url: Option<&'static str>,
    background_color: &'static str,
    /// Icon filename under [`IMG_BASE_URL`].
    file: &'static str,
}

/// Look up a built-in network by `name` (supporting the `-noshare` variants, which
/// keep the icon but drop the share wrapper so the raw `href` is used verbatim).
fn network(name: &str) -> Option<Network> {
    let (base, noshare) = match name.strip_suffix("-noshare") {
        Some(b) => (b, true),
        None => (name, false),
    };
    let (share, bg, file): (Option<&'static str>, &'static str, &'static str) = match base {
        "facebook" => (
            Some("https://www.facebook.com/sharer/sharer.php?u=[[URL]]"),
            "#3b5998",
            "facebook.png",
        ),
        "twitter" => (
            Some("https://twitter.com/intent/tweet?url=[[URL]]"),
            "#55acee",
            "twitter.png",
        ),
        "x" => (
            Some("https://twitter.com/intent/tweet?url=[[URL]]"),
            "#000000",
            "twitter-x.png",
        ),
        "google" => (
            Some("https://plus.google.com/share?url=[[URL]]"),
            "#dc4e41",
            "google-plus.png",
        ),
        "pinterest" => (
            Some("https://pinterest.com/pin/create/button/?url=[[URL]]&media=&description="),
            "#bd081c",
            "pinterest.png",
        ),
        "linkedin" => (
            Some(
                "https://www.linkedin.com/shareArticle?mini=true&url=[[URL]]&title=&summary=&source=",
            ),
            "#0077b5",
            "linkedin.png",
        ),
        "instagram" => (None, "#3f729b", "instagram.png"),
        "web" => (None, "#4BADE9", "web.png"),
        "snapchat" => (None, "#FFFA54", "snapchat.png"),
        "youtube" => (None, "#EB3323", "youtube.png"),
        "tumblr" => (
            Some("https://www.tumblr.com/widgets/share/tool?canonicalUrl=[[URL]]"),
            "#344356",
            "tumblr.png",
        ),
        "github" => (None, "#000000", "github.png"),
        "xing" => (
            Some("https://www.xing.com/app/user?op=share&url=[[URL]]"),
            "#296366",
            "xing.png",
        ),
        "vimeo" => (None, "#53B4E7", "vimeo.png"),
        "medium" => (None, "#000000", "medium.png"),
        "soundcloud" => (None, "#EF7F31", "soundcloud.png"),
        "dribbble" => (None, "#D95988", "dribbble.png"),
        _ => return None,
    };
    Some(Network {
        // The `-noshare` variants replace the share template with the bare placeholder.
        share_url: if noshare { Some("[[URL]]") } else { share },
        background_color: bg,
        file,
    })
}

/// Renderer for `<mj-social-element>`.
pub struct SocialElement<'a> {
    element: &'a Element<'a>,
    inherited: SocialInherited<'a>,
}

impl<'a> SocialElement<'a> {
    /// Wrap an element, forwarding the parent `<mj-social>`'s inherited attributes.
    #[must_use]
    pub fn new(element: &'a Element<'a>, inherited: SocialInherited<'a>) -> Self {
        Self { element, inherited }
    }

    fn attr(&self, ctx: &RenderContext<'a>, name: &str, default: &'a str) -> &'a str {
        ctx.semantic
            .attribute(self.element, name, Some(default))
            .unwrap_or(default)
    }

    /// Render this element's `<tr>` (icon cell + optional label cell). Shared by both
    /// social modes.
    pub fn render_row(&self, ctx: &mut RenderContext<'a>, buf: &mut String) {
        let inh = self.inherited;
        // mjml 5.4 added a `border` on the icon `<img>` (default `0`), forwardable from
        // `<mj-social>`. v4 had no such style, so this is gated on compat and prepended
        // only under v5 — v4 output is unchanged.
        let border = ctx
            .options
            .compat
            .is_v5()
            .then(|| self.attr(ctx, "border", inh.border.unwrap_or("0")));
        let border_radius = self.attr(
            ctx,
            "border-radius",
            inh.border_radius.unwrap_or(BORDER_RADIUS),
        );
        let color = self.attr(ctx, "color", inh.color.unwrap_or(COLOR));
        let font_family = self.attr(ctx, "font-family", inh.font_family.unwrap_or(FONT_FAMILY));
        let font_size = self.attr(ctx, "font-size", inh.font_size.unwrap_or(FONT_SIZE));
        let font_weight = self.element.attr("font-weight").or(inh.font_weight);
        let font_style = self.element.attr("font-style").or(inh.font_style);
        let line_height = self.attr(ctx, "line-height", inh.line_height.unwrap_or(LINE_HEIGHT));
        let text_decoration = self.attr(
            ctx,
            "text-decoration",
            inh.text_decoration.unwrap_or(TEXT_DECORATION),
        );
        let icon_size = self.attr(ctx, "icon-size", inh.icon_size.unwrap_or(ICON_SIZE));
        let icon_height = self.element.attr("icon-height").or(inh.icon_height);
        let icon_padding = self.element.attr("icon-padding").or(inh.icon_padding);
        let padding = self.attr(ctx, "padding", inh.padding.unwrap_or(PADDING));
        let text_padding = self.attr(
            ctx,
            "text-padding",
            inh.text_padding.unwrap_or(TEXT_PADDING),
        );
        let vertical_align = self.attr(ctx, "vertical-align", VERTICAL_ALIGN);
        let align = self.attr(ctx, "align", ALIGN);
        let alt = self.attr(ctx, "alt", ALT);
        let target = self.attr(ctx, "target", TARGET);

        let name = self.element.attr("name").unwrap_or("");
        let net = network(name);

        // Resolve icon src, background and the (templated) share href.
        let default_src = net.as_ref().map(|n| format!("{IMG_BASE_URL}{}", n.file));
        let src = self
            .element
            .attr("src")
            .map(str::to_string)
            .or(default_src)
            .unwrap_or_default();
        let background = self
            .element
            .attr("background-color")
            .or_else(|| net.as_ref().map(|n| n.background_color))
            .unwrap_or("");

        let raw_href = self.element.attr("href");
        let has_link = raw_href.is_some();
        let href = match (raw_href, net.as_ref().and_then(|n| n.share_url)) {
            (Some(h), Some(tmpl)) => tmpl.replace("[[URL]]", h),
            (Some(h), None) => h.to_string(),
            (None, _) => String::new(),
        };

        let icon_px = leading_digits(icon_size);
        let icon_height = icon_height.unwrap_or(icon_size);

        // Icon cell style: padding, [icon-padding as padding], font-size, height,
        // vertical-align, width — matching mjml's key order.
        let mut icon_style = String::new();
        if let Some(ip) = icon_padding {
            icon_style.push_str(&format!("padding:{ip};"));
        }
        icon_style.push_str(&format!(
            "font-size:0;height:{icon_height};vertical-align:middle;width:{icon_size};"
        ));

        // The label's <a>/<span> style: color, font-size, [font-weight], [font-style],
        // font-family, line-height, text-decoration.
        let mut text_style = format!("color:{color};font-size:{font_size};");
        if let Some(w) = font_weight {
            text_style.push_str(&format!("font-weight:{w};"));
        }
        if let Some(s) = font_style {
            text_style.push_str(&format!("font-style:{s};"));
        }
        text_style.push_str(&format!(
            "font-family:{font_family};line-height:{line_height};text-decoration:{text_decoration};"
        ));

        let content = self.content();

        // mjml puts `css-class` on the element's own `<tr>`.
        let class = leading_class_attr(css_class(ctx, self.element).as_deref());
        buf.push_str(&format!("<tr{class}>"));
        // Icon cell.
        buf.push_str(&format!(
            "<td style=\"padding:{padding};vertical-align:{vertical_align};\">"
        ));
        buf.push_str(&format!(
            "<table border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"background:{background};border-radius:{border_radius};width:{icon_size};\">"
        ));
        buf.push_str("<tbody><tr>");
        buf.push_str(&format!("<td style=\"{icon_style}\">"));
        if has_link {
            buf.push_str(&format!("<a href=\"{href}\" target=\"{target}\">"));
        }
        let border_css = border.map_or(String::new(), |b| format!("border:{b};"));
        buf.push_str(&format!(
            "<img alt=\"{alt}\" src=\"{src}\" style=\"{border_css}border-radius:{border_radius};display:block;\" width=\"{icon_px}\" />"
        ));
        if has_link {
            buf.push_str("</a>");
        }
        buf.push_str("</td></tr></tbody></table></td>");

        // Label cell, only when the element carries content.
        if !content.is_empty() {
            buf.push_str(&format!(
                "<td style=\"vertical-align:{vertical_align};padding:{text_padding};text-align:{align};\">"
            ));
            if has_link {
                buf.push_str(&format!(
                    "<a href=\"{href}\" style=\"{text_style}\" target=\"{target}\"> {content} </a>"
                ));
            } else {
                buf.push_str(&format!("<span style=\"{text_style}\"> {content} </span>"));
            }
            buf.push_str("</td>");
        }
        buf.push_str("</tr>");
    }

    /// The element's trimmed text content (its label).
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

impl<'a> Render<'a> for SocialElement<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        // A stray standalone element renders its bare row.
        self.render_row(ctx, buf);
        Ok(())
    }
}

/// Parse a leading integer out of a CSS length like `"20px"`.
fn leading_digits(value: &str) -> u32 {
    value
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}
