//! `<mj-column>` — an inline-block column that registers its responsive rule.

use mjmx_ast::Element;

use crate::components::css_class::{append_css_class, css_class};
use crate::context::RenderContext;
use crate::render::{Render, RenderResult, is_raw_element, render_children, render_element};

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

/// mjml's `normalizeUnitValue`: `Number(parseFloat(v).toFixed(6))` — round to 6 decimals
/// and drop trailing zeros. Keeps percentage gutter tokens/paddings stable.
fn normalize_unit(v: f64) -> f64 {
    (v * 1_000_000.0).round() / 1_000_000.0
}

/// mjml's `normalizePxValue`: `Math.round(parseFloat(v))`.
fn normalize_px(v: f64) -> f64 {
    v.round()
}

/// mjml's `formatPadding`: four sides joined with the unit, each normalized (px rounds to
/// a whole pixel, other units round to 6 decimals).
fn format_padding(top: f64, right: f64, bottom: f64, left: f64, unit: &str) -> String {
    if unit == "px" {
        format!(
            "{}px {}px {}px {}px",
            num(normalize_px(top)),
            num(normalize_px(right)),
            num(normalize_px(bottom)),
            num(normalize_px(left)),
        )
    } else {
        format!(
            "{}{unit} {}{unit} {}{unit} {}{unit}",
            num(normalize_unit(top)),
            num(normalize_unit(right)),
            num(normalize_unit(bottom)),
            num(normalize_unit(left)),
        )
    }
}

/// The mjml 5 section-`gutter` context handed to a column: the raw gutter value (an
/// empty/`None` gutter is inert), the direction that governs the gutter's padding sides
/// and class token, and the column's position among its layout siblings.
///
/// **v5 only.** Section renders under [`MjmlCompat::V4`](crate::MjmlCompat) construct a
/// column with `gutter: None`, which makes every gutter branch a no-op — so V4 output is
/// byte-identical to before this feature.
#[derive(Clone, Copy, Debug)]
pub struct Gutter<'a> {
    /// The section's `gutter` attribute (mjml's `context.gutter`); `None` or empty means
    /// "no column gutter", i.e. mjml's `hasColumnGutter()` is false.
    pub value: Option<&'a str>,
    /// The direction that decides which side each half-gutter lands on and the `-rtl`
    /// class-token suffix (mjml's `context.direction` — the *section's* for a plain
    /// column, the *group's* for a grouped one).
    pub direction: &'a str,
    /// This column's 0-based index among its parent's children (mjml's `props.index`).
    pub index: u32,
    /// The parent's total child count (mjml's `children.length`), so `first`/`last` are
    /// `index == 0` / `index == total - 1`.
    pub total: u32,
}

impl Gutter<'_> {
    /// A no-gutter context (V4 sections, or a v5 section with no `gutter`).
    #[must_use]
    pub fn none() -> Self {
        Self {
            value: None,
            direction: "ltr",
            index: 0,
            total: 1,
        }
    }

    /// mjml's `hasColumnGutter`: a non-empty section gutter is in effect.
    fn active(&self) -> bool {
        self.value.is_some_and(|g| !g.is_empty())
    }

    fn first(&self) -> bool {
        self.index == 0
    }

    fn last(&self) -> bool {
        self.index + 1 >= self.total
    }
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
    /// Non-raw sibling count in the parent section/group (mjml's `nonRawSiblings`, and
    /// its `props.sibling`).
    siblings: u32,
    /// Whether the column sits inside an `<mj-group>` (no mobile stacking). A group
    /// column keeps its share as the inline outlook-div width; a section column uses
    /// `100%` inline and lets the media query apply the share on desktop.
    in_group: bool,
    /// The mjml 5 section-gutter context (inert under V4 / no gutter).
    gutter: Gutter<'a>,
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
        gutter: Gutter<'a>,
    ) -> Self {
        Self {
            element,
            width,
            raw_width: raw,
            siblings: sibs,
            in_group: false,
            gutter,
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
        gutter: Gutter<'a>,
    ) -> Self {
        Self {
            element,
            width,
            raw_width: raw,
            siblings: sibs,
            in_group: true,
            gutter,
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
        // mjml: a grouped column with a section gutter takes its *reduced* desktop width
        // as the inline mobile width (group columns never stack).
        if self.gutter.active() {
            return match self.desktop_width(container) {
                ColumnWidth::Pct(v) => format!("{}%", num(v)),
                ColumnWidth::Px(v) => {
                    format!("{}%", num(normalize_unit(v / container.trunc() * 100.0)))
                }
            };
        }
        match (self.raw_width, self.width) {
            (None, _) => format!("{}%", 100 / self.siblings.max(1)),
            (Some(raw), ColumnWidth::Pct(_)) => raw.to_string(),
            (Some(_), ColumnWidth::Px(v)) => format!("{}%", num(v / container.trunc() * 100.0)),
        }
    }

    // ---- mjml 5 section-gutter geometry (all inert when `self.gutter` is inactive) ----

    /// mjml's `getNormalizedGutterValue`: the section gutter converted to `target_unit`,
    /// resolving `%`↔`px` against the column's parent `container` width. `0` when there
    /// is no gutter.
    fn normalized_gutter(&self, target_unit: &str, container: f64) -> f64 {
        let Some(raw) = self.gutter.value.filter(|g| !g.is_empty()) else {
            return 0.0;
        };
        let Some(g) = parse_width(raw) else {
            return 0.0;
        };
        let (unit, value) = (g.unit(), g.value());
        if unit == target_unit {
            return value;
        }
        match (target_unit, unit) {
            ("%", "px") => value / container * 100.0,
            ("px", "%") => container * value / 100.0,
            _ => value,
        }
    }

    /// mjml's `getDesktopWidth`: the column width reduced by its share of the gutter.
    /// Returns the parsed width unchanged when no gutter is active.
    fn desktop_width(&self, container: f64) -> ColumnWidth {
        let unit = self.width.unit();
        let parsed = self.width.value();
        if !self.gutter.active() {
            return match self.width {
                ColumnWidth::Px(v) => ColumnWidth::Px(normalize_px(v)),
                pct => pct,
            };
        }
        let sibling = f64::from(self.siblings.max(1));
        let gutter = self.normalized_gutter(unit, container);
        let reduction = gutter * (sibling - 1.0) / sibling;
        let reduced = normalize_unit((parsed - reduction).max(0.0));
        if unit == "px" {
            // Distribute the fractional remainder one pixel at a time, lowest index first.
            let floor = reduced.floor();
            let fractional = reduced - floor;
            let extra = (sibling * fractional).round().clamp(0.0, sibling);
            #[allow(clippy::cast_precision_loss)]
            let bump = if f64::from(self.gutter.index) < extra {
                1.0
            } else {
                0.0
            };
            ColumnWidth::Px(floor + bump)
        } else {
            ColumnWidth::Pct(reduced)
        }
    }

    /// The width whose `mj-column-per-*` / `mj-column-px-*` class and media query this
    /// column registers — the gutter-reduced width when a gutter is active, else the
    /// plain parsed width.
    fn class_width(&self, container: f64) -> ColumnWidth {
        if self.gutter.active() {
            self.desktop_width(container)
        } else {
            self.width
        }
    }

    /// mjml's `getDesktopGutterClassName`: the per-position gutter class, e.g.
    /// `mj-column-gutter-3-1-per-3-333333` (or `…-rtl` under RTL). Only meaningful when a
    /// gutter is active.
    fn gutter_class(&self, container: f64) -> String {
        let unit = self.width.unit();
        let gutter = self.normalized_gutter(unit, container);
        let unit_token = if unit == "%" { "per" } else { unit };
        let dir_token = if self.gutter.direction == "rtl" {
            "-rtl"
        } else {
            ""
        };
        let normalized = if unit == "px" {
            normalize_px(gutter)
        } else {
            gutter
        };
        let token = num(normalize_unit(normalized)).replace('.', "-");
        format!(
            "mj-column-gutter-{}-{}-{unit_token}-{token}{dir_token}",
            self.siblings,
            self.gutter.index + 1,
        )
    }

    /// mjml's `getDesktopPaddingValues` → `formatPadding`: the desktop gutter padding in
    /// `unit`, honouring direction (RTL swaps the leading/trailing sides).
    fn desktop_padding(&self, unit: &str, container: f64) -> String {
        let gutter = self.normalized_gutter(unit, container);
        let normalized = if unit == "px" {
            normalize_px(gutter)
        } else {
            gutter
        };
        let (half_leading, half_trailing) = if unit == "px" {
            ((normalized / 2.0).ceil(), (normalized / 2.0).floor())
        } else {
            (normalized / 2.0, normalized / 2.0)
        };
        if self.siblings == 1 {
            return format_padding(0.0, 0.0, 0.0, 0.0, unit);
        }
        let (first, last) = (self.gutter.first(), self.gutter.last());
        let (right, left) = if self.gutter.direction == "rtl" {
            (
                if first { 0.0 } else { half_trailing },
                if last { 0.0 } else { half_leading },
            )
        } else {
            (
                if last { 0.0 } else { half_leading },
                if first { 0.0 } else { half_trailing },
            )
        };
        format_padding(0.0, right, 0.0, left, unit)
    }

    /// mjml's `getMobilePaddingValues` → `formatPadding`: on mobile the gutter becomes
    /// vertical spacing between stacked columns (percentage, no outer top/bottom edge).
    fn mobile_padding(&self, container: f64) -> String {
        let half = self.normalized_gutter("%", container) / 2.0;
        let (first, last) = (self.gutter.first(), self.gutter.last());
        format_padding(
            if first { 0.0 } else { half },
            0.0,
            if last { 0.0 } else { half },
            0.0,
            "%",
        )
    }

    /// mjml's `getMobileGutterStyles`: the `padding` a gutter contributes to the column
    /// `<div>` — the desktop padding for a grouped column (never stacks), the mobile
    /// padding otherwise. `None` when no gutter is active.
    fn div_gutter_padding(&self, container: f64) -> Option<String> {
        if !self.gutter.active() {
            return None;
        }
        Some(if self.in_group {
            self.desktop_padding(self.width.unit(), container)
        } else {
            self.mobile_padding(container)
        })
    }

    /// mjml's `getOutlookGutterStyles`: the `padding` (in px) a gutter adds to this
    /// column's Outlook ghost `<td>` in its parent section. `None` when inactive or when
    /// the column sits in a group (group ghost cells carry no gutter padding). Called by
    /// the section that builds the ghost cell.
    #[must_use]
    pub fn outlook_gutter_padding(&self, container: f64) -> Option<String> {
        if !self.gutter.active() || self.in_group {
            return None;
        }
        Some(self.desktop_padding("px", container))
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
        let container = f64::from(ctx.container_width);

        // mjml's `getColumnClass`: a section gutter reduces the column's own width, so
        // the `mj-column-per-*` / `-px-*` class (and its media query) key off the
        // *reduced* width; without a gutter this is just the parsed width.
        let column_class = self.class_width(container).class();

        // Register the responsive rule so the head emits both the @media block and
        // the .moz-text-html <style media> block for this class.
        ctx.add_media_query(
            &column_class,
            self.class_width(container).media_query_body(),
        );

        // mjml's `render`: `${getColumnClass()}` then the gutter class (when active) then
        // `mj-outlook-group-fix`, then `css-class` appended when set.
        let mut base_class = column_class.clone();
        if self.gutter.active() {
            let gutter_class = self.gutter_class(container);
            // Only a section column registers the padding rule; a group column carries
            // the same padding inline on its `<div>` instead (mjml avoids the duplicate).
            if !self.in_group {
                ctx.add_media_query(
                    &gutter_class,
                    format!(
                        "padding: {} !important;",
                        self.desktop_padding(self.width.unit(), container)
                    ),
                );
            }
            base_class.push(' ');
            base_class.push_str(&gutter_class);
        }
        let class = append_css_class(
            &format!("{base_class} mj-outlook-group-fix"),
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

        let mobile_width = self.mobile_width(container);
        // mjml's `getMobileGutterStyles`: a gutter appends a `padding` after `width` in
        // the div style (vertical mobile spacing for a section column, the horizontal
        // desktop padding for a group column).
        let div_padding = self
            .div_gutter_padding(container)
            .map(|p| format!("padding:{p};"))
            .unwrap_or_default();
        buf.push_str(&format!(
            "<div class=\"{class}\" style=\"font-size:0px;text-align:left;direction:{direction};display:inline-block;vertical-align:{vertical_align};width:{mobile_width};{div_padding}\">"
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
            render_children(self.element, ctx, buf, |child, ctx, buf| {
                // mjml's `renderChildren`: a raw element (mj-raw) renders directly into
                // the tbody; every other child is wrapped in its own row.
                if is_raw_element(child) {
                    return render_element(child, ctx, buf);
                }
                buf.push_str("<tr>");
                render_element(child, ctx, buf)?;
                buf.push_str("</tr>");
                Ok(())
            })?;
            buf.push_str("</tbody></table></td></tr></tbody></table></div>");
        } else {
            // No gutter: the box styles sit directly on the single content table.
            buf.push_str(&format!(
                "<table border=\"0\" cellpadding=\"0\" cellspacing=\"0\" role=\"presentation\" style=\"{box_style}\" width=\"100%\">"
            ));
            buf.push_str("<tbody>");
            render_children(self.element, ctx, buf, |child, ctx, buf| {
                // mjml's `renderChildren`: a raw element (mj-raw) renders directly into
                // the tbody; every other child is wrapped in its own row.
                if is_raw_element(child) {
                    return render_element(child, ctx, buf);
                }
                buf.push_str("<tr>");
                render_element(child, ctx, buf)?;
                buf.push_str("</tr>");
                Ok(())
            })?;
            buf.push_str("</tbody></table></div>");
        }
        ctx.container_width = saved;
        Ok(())
    }
}
