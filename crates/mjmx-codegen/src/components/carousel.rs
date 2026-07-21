//! `<mj-carousel>` and `<mj-carousel-image>` — a CSS radio-button image carousel.
//!
//! mjml renders a hidden radio input per image plus a large `.mj-carousel-*` head
//! stylesheet whose `:checked` sibling selectors drive which slide/arrow/thumbnail is
//! shown. Every id-bearing class is keyed by a per-carousel token; mjml uses a random
//! 16-hex string there — we mint a deterministic one via
//! [`RenderContext::next_carousel_id`](crate::context::RenderContext::next_carousel_id)
//! and pin the golden's token to match. Non-Outlook clients see the interactive markup
//! (guarded by `<!--[if !mso]><!-->`); Outlook gets only the first image as a fallback.

use mjmx_ast::Element;
use mjmx_syntax::MjmlTag;

use std::borrow::Cow;

use crate::components::css_class::{class_attr, css_class, suffix_css_classes};
use crate::context::RenderContext;
use crate::render::{Render, RenderResult};

// mjml defaults (mj-carousel).
const ALIGN: &str = "center";
const BORDER_RADIUS: &str = "6px";
const ICON_WIDTH: &str = "44px";
const LEFT_ICON: &str = "https://i.imgur.com/xTh3hln.png";
const RIGHT_ICON: &str = "https://i.imgur.com/os7o9kz.png";
const THUMBNAILS: &str = "visible";
const TB_BORDER: &str = "2px solid transparent";
const TB_BORDER_RADIUS: &str = "6px";
const TB_HOVER_BORDER_COLOR: &str = "#fead0d";
// mjml default is #ccc; the head-CSS beautifier expands it to #cccccc.
const TB_SELECTED_BORDER_COLOR: &str = "#cccccc";
// mj-carousel-image default.
const IMG_TARGET: &str = "_blank";
const IMG_ALT: &str = "";

/// Renderer for `<mj-carousel>`.
pub struct Carousel<'a> {
    element: &'a Element<'a>,
}

impl<'a> Carousel<'a> {
    /// Wrap an `<mj-carousel>` element.
    #[must_use]
    pub fn new(element: &'a Element<'a>) -> Self {
        Self { element }
    }
}

impl<'a> Render<'a> for Carousel<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        let attr = |name: &str, default: &'a str| {
            ctx.semantic
                .attribute(self.element, name, Some(default))
                .unwrap_or(default)
        };

        let align = attr("align", ALIGN);
        let border_radius = attr("border-radius", BORDER_RADIUS);
        let icon_width = attr("icon-width", ICON_WIDTH);
        let left_icon = attr("left-icon", LEFT_ICON);
        let right_icon = attr("right-icon", RIGHT_ICON);
        let thumbnails = attr("thumbnails", THUMBNAILS);
        let tb_border = attr("tb-border", TB_BORDER);
        let tb_border_radius = attr("tb-border-radius", TB_BORDER_RADIUS);

        let images: Vec<&Element> = self
            .element
            .element_children()
            .filter(|el| el.tag == MjmlTag::MjCarouselImage)
            .collect();
        let n = images.len();
        let container_width = ctx.container_width;
        let icon_width_num = parse_px(icon_width);
        // Thumbnail width: min(containerWidth / count, 110).
        let tb_width = if n == 0 {
            0
        } else {
            (container_width / n as u32).min(110)
        };

        let id = ctx.next_carousel_id();
        if n > 0 {
            ctx.add_style(build_head_css(
                &id,
                n,
                icon_width,
                TB_SELECTED_BORDER_COLOR,
                TB_HOVER_BORDER_COLOR,
            ));
        }

        // Outer content cell (align, no padding).
        // mjml puts the child's `css-class` on the wrapping `<td>` (mj-column /
        // mj-hero `renderChildren`), between `align` and `style`.
        let class = class_attr(css_class(ctx, self.element).as_deref());
        buf.push_str(&format!(
            "<td align=\"{align}\" {class}style=\"font-size:0px;word-break:break-word;\">"
        ));
        buf.push_str("<!--[if !mso]><!--><div class=\"mj-carousel\">");

        // Radios.
        for i in 0..n {
            let checked = if i == 0 { " checked=\"checked\"" } else { "" };
            buf.push_str(&format!(
                "<input class=\"mj-carousel-radio mj-carousel-{id}-radio mj-carousel-{id}-radio-{idx}\"{checked} type=\"radio\" name=\"mj-carousel-radio-{id}\" id=\"mj-carousel-{id}-radio-{idx}\" style=\"display:none;mso-hide:all;\" />",
                idx = i + 1,
            ));
        }

        buf.push_str(&format!(
            "<div class=\"mj-carousel-content mj-carousel-{id}-content\" style=\"display:table;width:100%;table-layout:fixed;text-align:center;font-size:0px;\">"
        ));

        // Thumbnails (rendered for `visible`/`supported`).
        if matches!(thumbnails, "visible" | "supported") {
            let disp = if thumbnails == "supported" {
                "none"
            } else {
                "inline-block"
            };
            for (i, img) in images.iter().enumerate() {
                let idx = i + 1;
                let target = img
                    .attr("target")
                    .or_else(|| ctx.semantic.attribute(img, "target", Some(IMG_TARGET)))
                    .unwrap_or(IMG_TARGET);
                let alt = img.attr("alt").unwrap_or(IMG_ALT);
                let thumb_src = img
                    .attr("thumbnails-src")
                    .or_else(|| img.attr("src"))
                    .unwrap_or("");
                // mjml's `MjCarouselImage.renderThumbnail`: the image's `css-class`,
                // suffixed with `-thumbnail`, is appended to the generated classes
                // (the trailing space is mjml's, present even when unset).
                let thumb_class = suffix_css_classes(css_class(ctx, img).as_deref(), "thumbnail");
                buf.push_str(&format!(
                    "<a style=\"border:{tb_border};border-radius:{tb_border_radius};display:{disp};overflow:hidden;width:{tb_width}px;\" href=\"#{idx}\" target=\"{target}\" class=\"mj-carousel-thumbnail mj-carousel-{id}-thumbnail mj-carousel-{id}-thumbnail-{idx} {thumb_class}\"><label for=\"mj-carousel-{id}-radio-{idx}\"><img style=\"display:block;width:100%;height:auto;\" src=\"{thumb_src}\" alt=\"{alt}\" width=\"{tb_width}\" /></label></a>"
                ));
            }
        }

        // The main carousel table (previous controls, images, next controls).
        buf.push_str(
            "<table style=\"caption-side:top;display:table-caption;table-layout:fixed;width:100%;\" border=\"0\" cellpadding=\"0\" cellspacing=\"0\" width=\"100%\" role=\"presentation\" class=\"mj-carousel-main\"><tbody><tr>",
        );
        render_controls(
            buf,
            &id,
            n,
            "previous",
            left_icon,
            icon_width,
            icon_width_num,
        );
        // Images cell.
        buf.push_str("<td style=\"padding:0px;\"><div class=\"mj-carousel-images\">");
        for (i, img) in images.iter().enumerate() {
            let idx = i + 1;
            let src = img.attr("src").unwrap_or("");
            let alt = img.attr("alt").unwrap_or(IMG_ALT);
            let div_style = if i == 0 {
                ""
            } else {
                "display:none;mso-hide:all;"
            };
            // mjml's `MjCarouselImage.render`: `mj-carousel-image mj-carousel-image-N `
            // plus the raw `css-class` (empty string when unset).
            let image_class = css_class(ctx, img).unwrap_or(Cow::Borrowed(""));
            buf.push_str(&format!(
                "<div class=\"mj-carousel-image mj-carousel-image-{idx} {image_class}\" style=\"{div_style}\">{img_html}</div>",
                img_html = carousel_img_html(img, src, alt, border_radius, container_width),
            ));
        }
        buf.push_str("</div></td>");
        render_controls(buf, &id, n, "next", right_icon, icon_width, icon_width_num);
        buf.push_str("</tr></tbody></table>");

        buf.push_str("</div></div><!--<![endif]-->");

        // Outlook fallback: only the first image (not beautified — raw ` >` preserved).
        if let Some(first) = images.first() {
            let src = first.attr("src").unwrap_or("");
            let alt = first.attr("alt").unwrap_or(IMG_ALT);
            let first_class = css_class(ctx, first).unwrap_or(Cow::Borrowed(""));
            buf.push_str(&format!(
                "<!--[if mso]><div class=\"mj-carousel-image mj-carousel-image-1 {first_class}\" style=\"\" >{img_html}</div><![endif]-->",
                img_html = carousel_img_html(first, src, alt, border_radius, container_width),
            ));
        }

        buf.push_str("</td>");
        Ok(())
    }
}

/// The `<img>` (optionally wrapped in an `<a href>`) for one carousel slide.
fn carousel_img_html(
    img: &Element<'_>,
    src: &str,
    alt: &str,
    border_radius: &str,
    container_width: u32,
) -> String {
    let inner = format!(
        "<img src=\"{src}\" alt=\"{alt}\" style=\"border-radius:{border_radius};display:block;width:{container_width}px;max-width:100%;height:auto;\" width=\"{container_width}\" border=\"0\" />"
    );
    match img.attr("href") {
        Some(href) => {
            let mut a = format!("<a href=\"{href}\"");
            if let Some(rel) = img.attr("rel") {
                a.push_str(&format!(" rel=\"{rel}\""));
            }
            a.push_str(" target=\"_blank\">");
            format!("{a}{inner}</a>")
        }
        None => inner,
    }
}

/// Render a previous/next controls `<td>` (one label per image).
fn render_controls(
    buf: &mut String,
    id: &str,
    n: usize,
    direction: &str,
    icon: &str,
    icon_width: &str,
    icon_width_num: u32,
) {
    buf.push_str(&format!(
        "<td class=\"mj-carousel-{id}-icons-cell\" style=\"font-size:0px;display:none;mso-hide:all;padding:0px;\"><div class=\"mj-carousel-{direction}-icons\" style=\"display:none;mso-hide:all;\">"
    ));
    for i in 0..n {
        let idx = i + 1;
        buf.push_str(&format!(
            "<label for=\"mj-carousel-{id}-radio-{idx}\" class=\"mj-carousel-{direction} mj-carousel-{direction}-{idx}\"><img src=\"{icon}\" alt=\"{direction}\" style=\"display:block;width:{icon_width};height:auto;\" width=\"{icon_width_num}\" /></label>"
        ));
    }
    buf.push_str("</div></td>");
}

/// Parse a leading pixel count out of a CSS length like `"44px"`.
fn parse_px(value: &str) -> u32 {
    value
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

/// The sibling combinator for a radio/thumbnail `reps` positions away, minified as mjml's
/// head-CSS beautifier emits it: `+`, `+*+`, `+*+*+`, …
fn combinator(reps: usize) -> String {
    let mut s = String::from("+");
    for _ in 0..reps {
        s.push_str("*+");
    }
    s
}

/// Build the `.mj-carousel-*` head stylesheet for `n` images.
fn build_head_css(
    id: &str,
    n: usize,
    icon_width: &str,
    tb_selected: &str,
    tb_hover: &str,
) -> String {
    let mut c = String::new();
    c.push_str("\n    .mj-carousel {\n      -webkit-user-select: none;\n      -moz-user-select: none;\n      user-select: none;\n    }\n\n");
    c.push_str(&format!(
        "    .mj-carousel-{id}-icons-cell {{\n      display: table-cell !important;\n      width: {icon_width} !important;\n    }}\n\n"
    ));
    c.push_str("    .mj-carousel-radio,\n    .mj-carousel-next,\n    .mj-carousel-previous {\n      display: none !important;\n    }\n\n");
    c.push_str("    .mj-carousel-thumbnail,\n    .mj-carousel-next,\n    .mj-carousel-previous {\n      touch-action: manipulation;\n    }\n\n");

    // Hide every slide (radio :checked, forward combinators).
    let e: Vec<String> = (0..n)
        .map(|i| {
            format!(
                ".mj-carousel-{id}-radio:checked{comb}.mj-carousel-content .mj-carousel-image",
                comb = combinator(i)
            )
        })
        .collect();
    c.push_str(&format!(
        "    {} {{\n      display: none !important;\n    }}\n\n",
        e.join(",\n    ")
    ));

    // Show the checked slide (reverse combinators).
    let f: Vec<String> = (0..n)
        .map(|i| {
            format!(
                ".mj-carousel-{id}-radio-{idx}:checked{comb}.mj-carousel-content .mj-carousel-image-{idx}",
                idx = i + 1,
                comb = combinator(n - 1 - i)
            )
        })
        .collect();
    c.push_str(&format!(
        "    {} {{\n      display: block !important;\n    }}\n\n",
        f.join(",\n    ")
    ));

    // Arrows: statics + rotated next/previous targets.
    let mut g: Vec<String> = vec![
        ".mj-carousel-previous-icons".to_string(),
        ".mj-carousel-next-icons".to_string(),
    ];
    for i in 0..n {
        let next = (i + (1 % n) + n) % n + 1;
        g.push(format!(
            ".mj-carousel-{id}-radio-{idx}:checked{comb}.mj-carousel-content .mj-carousel-next-{next}",
            idx = i + 1,
            comb = combinator(n - 1 - i)
        ));
    }
    for i in 0..n {
        let prev = (i + n - (1 % n)) % n + 1;
        g.push(format!(
            ".mj-carousel-{id}-radio-{idx}:checked{comb}.mj-carousel-content .mj-carousel-previous-{prev}",
            idx = i + 1,
            comb = combinator(n - 1 - i)
        ));
    }
    c.push_str(&format!(
        "    {} {{\n      display: block !important;\n    }}\n\n",
        g.join(",\n    ")
    ));

    // Selected thumbnail border color.
    let h: Vec<String> = (0..n)
        .map(|i| {
            format!(
                ".mj-carousel-{id}-radio-{idx}:checked{comb}.mj-carousel-content .mj-carousel-{id}-thumbnail-{idx}",
                idx = i + 1,
                comb = combinator(n - 1 - i)
            )
        })
        .collect();
    c.push_str(&format!(
        "    {} {{\n      border-color: {tb_selected} !important;\n    }}\n\n",
        h.join(",\n    ")
    ));

    // Selected thumbnails become inline-block.
    let ii: Vec<String> = (0..n)
        .map(|i| {
            format!(
                ".mj-carousel-{id}-radio-{idx}:checked{comb}.mj-carousel-content .mj-carousel-{id}-thumbnail",
                idx = i + 1,
                comb = combinator(n - 1 - i)
            )
        })
        .collect();
    c.push_str(&format!(
        "    {} {{\n      display: inline-block !important;\n    }}\n\n",
        ii.join(",\n    ")
    ));

    c.push_str("    .mj-carousel-image img+div,\n    .mj-carousel-thumbnail img+div {\n      display: none !important;\n    }\n\n");

    // Thumbnail hover hides all slides.
    let k: Vec<String> = (0..n)
        .map(|i| {
            format!(
                ".mj-carousel-{id}-thumbnail:hover{comb}.mj-carousel-main .mj-carousel-image",
                comb = combinator(n - 1 - i)
            )
        })
        .collect();
    c.push_str(&format!(
        "    {} {{\n      display: none !important;\n    }}\n\n",
        k.join(",\n    ")
    ));

    c.push_str(&format!(
        "    .mj-carousel-thumbnail:hover {{\n      border-color: {tb_hover} !important;\n    }}\n\n"
    ));

    // Thumbnail-i hover shows slide i.
    let m: Vec<String> = (0..n)
        .map(|i| {
            format!(
                ".mj-carousel-{id}-thumbnail-{idx}:hover{comb}.mj-carousel-main .mj-carousel-image-{idx}",
                idx = i + 1,
                comb = combinator(n - 1 - i)
            )
        })
        .collect();
    c.push_str(&format!(
        "    {} {{\n      display: block !important;\n    }}\n",
        m.join(",\n    ")
    ));

    // Fallback block.
    c.push_str("\n    .mj-carousel noinput {\n      display: block !important;\n    }\n\n");
    c.push_str("    .mj-carousel noinput .mj-carousel-image-1 {\n      display: block !important;\n    }\n\n");
    c.push_str("    .mj-carousel noinput .mj-carousel-arrows,\n    .mj-carousel noinput .mj-carousel-thumbnails {\n      display: none !important;\n    }\n\n");
    c.push_str("    [owa] .mj-carousel-thumbnail {\n      display: none !important;\n    }\n\n");
    c.push_str("    @media screen yahoo {\n\n");
    c.push_str(&format!(
        "      .mj-carousel-{id}-icons-cell,\n      .mj-carousel-previous-icons,\n      .mj-carousel-next-icons {{\n        display: none !important;\n      }}\n\n"
    ));
    c.push_str(&format!(
        "      .mj-carousel-{id}-radio-1:checked{comb}.mj-carousel-content .mj-carousel-{id}-thumbnail-1 {{\n        border-color: transparent;\n      }}\n    }}\n\n  ",
        comb = combinator(n - 1)
    ));

    c
}
