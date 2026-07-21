//! `<mj-column>` — an inline-block column that registers its responsive rule.

use mjmx_ast::Element;

use crate::components::css_class::{append_css_class, css_class};
use crate::context::RenderContext;
use crate::render::{Render, RenderResult, render_element};

/// A column's resolved width, mirroring mjml's `widthParser` output (`{parsedWidth,
/// unit}`) as produced by `MjColumn.getParsedWidth`.
///
/// mjml parses a `%` width with `parseFloat` and a `px` (or unit-less) width with
/// `parseInt` — so a pixel width is always truncated to a whole number, while a
/// percentage keeps its fractional part (`100 / 3` stays `33.333333333333336`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColumnWidth {
    /// A percentage of the parent container.
    Pct(f64),
    /// An absolute pixel width.
    Px(f64),
}

impl ColumnWidth {
    /// mjml's `getParsedWidth`: the explicit `width` attribute, or an equal share
    /// (`100 / nonRawSiblings` percent) when it is absent.
    #[must_use]
    pub fn resolve(raw: Option<&str>, siblings: u32) -> Self {
        match raw.and_then(parse_width) {
            Some(w) => w,
            None => Self::Pct(100.0 / f64::from(siblings.max(1))),
        }
    }

    /// The numeric part, as mjml's `parsedWidth`.
    #[must_use]
    pub fn value(self) -> f64 {
        match self {
            Self::Pct(v) | Self::Px(v) => v,
        }
    }

    /// The CSS unit, as mjml's `unit`.
    #[must_use]
    pub fn unit(self) -> &'static str {
        match self {
            Self::Pct(_) => "%",
            Self::Px(_) => "px",
        }
    }

    /// mjml's `getColumnClass`: `mj-column-per-{n}` or `mj-column-px-{n}`, with the
    /// decimal point rewritten as a hyphen.
    #[must_use]
    pub fn class(self) -> String {
        let n = num(self.value()).replace('.', "-");
        match self {
            Self::Pct(_) => format!("mj-column-per-{n}"),
            Self::Px(_) => format!("mj-column-px-{n}"),
        }
    }

    /// The `@media` declaration body registered for [`Self::class`], matching mjml's
    /// `addMediaQuery` (`{ width:Nunit !important; max-width: Nunit; }`).
    #[must_use]
    pub fn media_query_body(self) -> String {
        let v = num(self.value());
        let u = self.unit();
        format!("width: {v}{u} !important;\n        max-width: {v}{u};")
    }

    /// mjml's `getWidthAsPixel`: the column's width resolved against `container`, in
    /// pixels. Drives the Outlook ghost cell (`tdOutlook`) and the child context width.
    #[must_use]
    pub fn as_pixel(self, container: f64) -> f64 {
        match self {
            Self::Pct(v) => container * v / 100.0,
            Self::Px(v) => v,
        }
    }
}

/// mjml's `widthParser` with `parseFloatToInt: false`.
fn parse_width(raw: &str) -> Option<ColumnWidth> {
    let raw = raw.trim();
    let digits: String = raw
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if digits.is_empty() {
        return None;
    }
    let unit = raw[digits.len()..].trim();
    if unit == "%" {
        // `parseFloat`: the fractional part survives.
        digits.parse::<f64>().ok().map(ColumnWidth::Pct)
    } else {
        // `parseInt`: `px` and the unit-less default truncate.
        let int: String = digits.chars().take_while(char::is_ascii_digit).collect();
        int.parse::<f64>().ok().map(ColumnWidth::Px)
    }
}

/// Format a number the way JS `${n}` does (no trailing `.0`, shortest round-trip).
fn num(v: f64) -> String {
    format!("{v}")
}

/// Renderer for `<mj-column>`.
pub struct Column<'a> {
    element: &'a Element<'a>,
    /// This column's resolved width — keys the `mj-column-per-*` / `mj-column-px-*`
    /// class and its media query.
    width: ColumnWidth,
    /// The raw `width` attribute, when set. mjml echoes it verbatim as the mobile
    /// width of a percentage-sized group column.
    raw_width: Option<&'a str>,
    /// Non-raw sibling count in the parent section/group (mjml's `nonRawSiblings`).
    siblings: u32,
    /// Whether the column sits inside an `<mj-group>` (no mobile stacking). A group
    /// column keeps its share as the inline outlook-div width; a section column uses
    /// `100%` inline and lets the media query apply the share on desktop.
    in_group: bool,
}

impl<'a> Column<'a> {
    /// Wrap a section (or top-level) `<mj-column>` — mobile-stacking, so the inline
    /// outlook-div width is `100%` and the share lives only in the media query.
    #[must_use]
    pub fn new(
        element: &'a Element<'a>,
        width: ColumnWidth,
        raw: Option<&'a str>,
        sibs: u32,
    ) -> Self {
        Self {
            element,
            width,
            raw_width: raw,
            siblings: sibs,
            in_group: false,
        }
    }

    /// Wrap an `<mj-group>` child `<mj-column>` — no mobile stacking, so the share is
    /// also the inline outlook-div width.
    #[must_use]
    pub fn new_grouped(
        element: &'a Element<'a>,
        width: ColumnWidth,
        raw: Option<&'a str>,
        sibs: u32,
    ) -> Self {
        Self {
            element,
            width,
            raw_width: raw,
            siblings: sibs,
            in_group: true,
        }
    }

    /// mjml's `getMobileWidth`: the inline `width` on the outlook-fix `<div>`.
    ///
    /// A stacking (section) column is always `100%`. A group column keeps its share:
    /// an absent width becomes `parseInt(100 / siblings)%`, an explicit percentage is
    /// echoed verbatim, and an explicit pixel width is expressed as a percentage of
    /// the container.
    fn mobile_width(&self, container: f64) -> String {
        if !self.in_group {
            return "100%".to_string();
        }
        match (self.raw_width, self.width) {
            (None, _) => format!("{}%", 100 / self.siblings.max(1)),
            (Some(raw), ColumnWidth::Pct(_)) => raw.to_string(),
            (Some(_), ColumnWidth::Px(v)) => format!("{}%", num(v / container.trunc() * 100.0)),
        }
    }
}

/// The `border`-shorthand width contributed by one side, in pixels.
fn border_width(value: Option<&str>) -> f64 {
    value
        .and_then(|v| {
            v.split_whitespace().find_map(|part| {
                let digits: String = part
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                if digits.is_empty() {
                    None
                } else {
                    digits.parse::<f64>().ok()
                }
            })
        })
        .unwrap_or(0.0)
}

/// A `px` length attribute's numeric value.
fn px(value: Option<&str>) -> f64 {
    value
        .map(|v| {
            let digits: String = v
                .trim()
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            digits.parse::<f64>().unwrap_or(0.0)
        })
        .unwrap_or(0.0)
}

/// Left + right padding, in pixels, from a CSS shorthand plus its longhand overrides
/// (mjml's `getShorthandAttrValue` over `padding` / `padding-left` / `padding-right`).
fn horizontal_padding(shorthand: Option<&str>, left: Option<&str>, right: Option<&str>) -> f64 {
    let side = |longhand: Option<&str>, idx_lr: usize| -> f64 {
        if let Some(v) = longhand {
            return px(Some(v));
        }
        let Some(sh) = shorthand else { return 0.0 };
        let parts: Vec<&str> = sh.split_whitespace().collect();
        let value = match parts.len() {
            1 => parts.first().copied(),
            2 | 3 => parts.get(1).copied(),
            4 => parts.get(idx_lr).copied(),
            _ => None,
        };
        px(value)
    };
    // In a 4-value shorthand, index 3 is left and index 1 is right.
    side(left, 3) + side(right, 1)
}

impl<'a> Render<'a> for Column<'a> {
    fn render(&self, ctx: &mut RenderContext<'a>, buf: &mut String) -> RenderResult {
        let column_class = self.width.class();

        // Register the responsive rule so the head emits both the @media block and
        // the .moz-text-html <style media> block for this class.
        ctx.add_media_query(&column_class, self.width.media_query_body());

        // mjml's `render`: `${getColumnClass()} mj-outlook-group-fix`, then `css-class`
        // appended when set.
        let class = append_css_class(
            &format!("{column_class} mj-outlook-group-fix"),
            css_class(ctx, self.element).as_deref(),
        );

        // Themeable attributes. `vertical-align` / `direction` have mjml defaults; the
        // others have none and only appear when set.
        let opt = |name: &str| ctx.semantic.attribute(self.element, name, None);
        let vertical_align = ctx
            .semantic
            .attribute(self.element, "vertical-align", Some("top"))
            .unwrap_or("top");
        let direction = ctx
            .semantic
            .attribute(self.element, "direction", Some("ltr"))
            .unwrap_or("ltr");

        let container = f64::from(ctx.container_width);
        let mobile_width = self.mobile_width(container);
        buf.push_str(&format!(
            "<div class=\"{class}\" style=\"font-size:0px;text-align:left;direction:{direction};display:inline-block;vertical-align:{vertical_align};width:{mobile_width};\">"
        ));

        // mjml's `tableStyle`: the box declarations shared by the no-gutter table and
        // the gutter cell, in mjml's key order.
        let border_radius = opt("border-radius");
        let mut box_style = String::new();
        for (prop, value) in [
            ("background-color", opt("background-color")),
            ("border", opt("border")),
            ("border-bottom", opt("border-bottom")),
            ("border-left", opt("border-left")),
            ("border-radius", border_radius),
            ("border-right", opt("border-right")),
            ("border-top", opt("border-top")),
        ] {
            if let Some(v) = value {
                box_style.push_str(&format!("{prop}:{v};"));
            }
        }
        box_style.push_str(&format!("vertical-align:{vertical_align};"));
        // `hasBorderRadius` forces separated borders so the radius is honoured.
        if border_radius.is_some() {
            box_style.push_str("border-collapse:separate;");
        }

        // mjml's `hasGutter`: any padding attribute turns the column into a "gutter"
        // column, wrapping the content in an extra cell that carries the box styles.
        let padding = opt("padding");
        let padding_top = opt("padding-top");
        let padding_right = opt("padding-right");
        let padding_bottom = opt("padding-bottom");
        let padding_left = opt("padding-left");
        let has_gutter = padding.is_some()
            || padding_top.is_some()
            || padding_right.is_some()
            || padding_bottom.is_some()
            || padding_left.is_some();

        // mjml's inner styles: only meaningful on a gutter column, where they dress the
        // inner content table instead of `tableStyle`.
        let inner_border_radius = opt("inner-border-radius");
        let mut inner_style = String::new();
        for (prop, value) in [
            ("background-color", opt("inner-background-color")),
            ("border", opt("inner-border")),
            ("border-bottom", opt("inner-border-bottom")),
            ("border-left", opt("inner-border-left")),
            ("border-radius", inner_border_radius),
            ("border-right", opt("inner-border-right")),
            ("border-top", opt("inner-border-top")),
        ] {
            if let Some(v) = value {
                inner_style.push_str(&format!("{prop}:{v};"));
            }
        }
        if inner_border_radius.is_some() {
            inner_style.push_str("border-collapse:separate;");
        }

        // mjml's `getChildContext`: the column's own pixel width minus its paddings,
        // borders and inner borders is what the children see as container width.
        let paddings = horizontal_padding(padding, padding_left, padding_right);
        let borders = border_width(opt("border-left").or_else(|| opt("border")))
            + border_width(opt("border-right").or_else(|| opt("border")));
        let inner_borders = border_width(opt("inner-border-left").or_else(|| opt("inner-border")))
            + border_width(opt("inner-border-right").or_else(|| opt("inner-border")));
        let child_width = self.width.as_pixel(container) - paddings - borders - inner_borders;
        let saved = ctx.container_width;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            ctx.container_width = child_width.max(0.0) as u32;
        }

        if has_gutter {
            // Gutter column: outer table wraps a cell that holds the box styles.
            let outer_style = if border_radius.is_some() {
                " style=\"border-collapse:separate;\""
            } else {
                ""
            };
            buf.push_str(&format!(
                "<table border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" width=\"100%\"{outer_style}>"
            ));
            buf.push_str("<tbody><tr>");
            let mut gutter_style = box_style.clone();
            for (prop, value) in [
                ("padding", padding),
                ("padding-top", padding_top),
                ("padding-right", padding_right),
                ("padding-bottom", padding_bottom),
                ("padding-left", padding_left),
            ] {
                if let Some(v) = value {
                    gutter_style.push_str(&format!("{prop}:{v};"));
                }
            }
            buf.push_str(&format!("<td style=\"{gutter_style}\">"));
            buf.push_str(&format!(
                "<table border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"{inner_style}\" width=\"100%\">"
            ));
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
        ctx.container_width = saved;
        Ok(())
    }
}
