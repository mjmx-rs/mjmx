//! Background handling shared by `<mj-section>` and `<mj-wrapper>`.
//!
//! `mj-wrapper` is `mj-section` with a different child layout, so both resolve and
//! paint their background through this module — a faithful port of mjml 4.18.0's
//! `mjml-section/lib/index.js` (`getBackground`, `getBackgroundString`,
//! `parseBackgroundPosition`, `renderWithBackground`).
//!
//! Two things are emitted:
//!
//! * the CSS declarations ([`Background::css`]) that go on the centered `div` and the
//!   inner table (or, for a `full-width` section/wrapper, on the outer 100% table);
//! * a VML `<v:rect>`/`<v:fill>` pair ([`Background::vml_open`] / [`vml_close`]) that
//!   paints the image behind the whole block in Outlook, since Outlook ignores CSS
//!   background images.

use mjmx_ast::Element;
use std::fmt::Write;

use crate::context::RenderContext;
use crate::render::STYLE_CAPACITY;

// mjml defaults for mj-section / mj-wrapper.
const BACKGROUND_REPEAT: &str = "repeat";
const BACKGROUND_SIZE: &str = "auto";
const BACKGROUND_POSITION: &str = "top center";

/// The resolved background attributes of a section-shaped component.
pub(crate) struct Background<'a> {
    /// `background-url`, when set — its presence switches on the whole image path.
    url: Option<&'a str>,
    /// `background-color`, when set.
    color: Option<&'a str>,
    /// `background-repeat` (`repeat` | `no-repeat`).
    repeat: &'a str,
    /// `background-size` (`auto`, `cover`, `contain`, one length, or two).
    size: &'a str,
    /// Horizontal position, after splitting `background-position` and applying an
    /// explicit `background-position-x` override.
    pos_x: &'a str,
    /// Vertical position (same, for `background-position-y`).
    pos_y: &'a str,
}

impl<'a> Background<'a> {
    /// Resolve every background attribute of `element` through the semantic layer, so
    /// `mj-attributes` / `mj-class` overrides apply.
    pub(crate) fn resolve(element: &'a Element<'a>, ctx: &RenderContext<'a>) -> Self {
        let attr = |name: &str, default: &'a str| {
            ctx.semantic
                .attribute(element, name, Some(default))
                .unwrap_or(default)
        };
        let opt = |name: &str| {
            ctx.semantic
                .attribute(element, name, None)
                .filter(|v| !v.is_empty())
        };

        let position = attr("background-position", BACKGROUND_POSITION);
        let (x, y) = parse_position(position);
        Self {
            url: opt("background-url"),
            color: ctx.semantic.attribute(element, "background-color", None),
            repeat: attr("background-repeat", BACKGROUND_REPEAT),
            size: attr("background-size", BACKGROUND_SIZE),
            pos_x: opt("background-position-x").unwrap_or(x),
            pos_y: opt("background-position-y").unwrap_or(y),
        }
    }

    /// Whether a background image is set (mjml's `hasBackground`).
    pub(crate) fn has_url(&self) -> bool {
        self.url.is_some()
    }

    /// `background-url`, for the `background="…"` HTML attribute mjml puts on the table.
    pub(crate) fn url(&self) -> Option<&'a str> {
        self.url
    }

    /// `background-color`, for the ghost table's `bgcolor="…"`.
    pub(crate) fn color(&self) -> Option<&'a str> {
        self.color
    }

    /// The CSS declarations for this background.
    ///
    /// Without an image this is the plain `background`/`background-color` pair; with one
    /// it is the `background` shorthand plus the separate position/repeat/size
    /// longhands (mjml keeps them separate because Yahoo ignores the shorthand).
    pub(crate) fn css(&self) -> String {
        let Some(url) = self.url else {
            return self
                .color
                .map(|c| format!("background:{c};background-color:{c};"))
                .unwrap_or_default();
        };
        // makeBackgroundString: drop the empty parts, join with a space.
        let mut shorthand = String::with_capacity(STYLE_CAPACITY);
        if let Some(color) = self.color {
            shorthand.push_str(color);
            shorthand.push(' ');
        }
        let _ = write!(
            shorthand,
            "url('{url}') {} {} / {} {}",
            self.pos_x, self.pos_y, self.size, self.repeat
        );
        format!(
            "background:{shorthand};background-position:{} {};background-repeat:{};background-size:{};",
            self.pos_x, self.pos_y, self.repeat, self.size
        )
    }

    /// The opening half of the Outlook-only VML background (`renderWithBackground`).
    ///
    /// `full_width` switches the rect from a pixel width to `mso-width-percent:1000`.
    pub(crate) fn vml_open(&self, buf: &mut String, full_width: bool, container_width: u32) {
        let Some(url) = self.url else { return };

        let ((origin_x, pos_x), (origin_y, pos_y)) = self.vml_position();

        // cover/contain keep the aspect ratio and fill the shape; an explicit size is
        // passed through (a single length gets `atmost`, reproducing `height: auto`).
        let mut size_attrs = String::with_capacity(STYLE_CAPACITY);
        match self.size {
            "cover" => size_attrs.push_str(" size=\"1,1\" aspect=\"atleast\""),
            "contain" => size_attrs.push_str(" size=\"1,1\" aspect=\"atmost\""),
            "auto" => {}
            size => {
                let parts: Vec<&str> = size.split(' ').collect();
                if parts.len() == 1 {
                    size_attrs.push_str(" size=\"");
                    size_attrs.push_str(size);
                    size_attrs.push_str("\" aspect=\"atmost\"");
                } else {
                    let _ = write!(size_attrs, " size=\"{}\"", parts.join(","));
                }
            }
        }

        // `frame` honors the size; `tile` repeats. With `background-size:auto` Outlook
        // cannot use the image's own size with `frame`, so mjml keeps tiling.
        let vml_type = if self.size == "auto" {
            "tile"
        } else if self.repeat == "no-repeat" {
            "frame"
        } else {
            "tile"
        };

        let rect_style = if full_width {
            "mso-width-percent:1000;".to_string()
        } else {
            format!("width:{container_width}px;")
        };
        let color_attr = self
            .color
            .map(|c| format!(" color=\"{c}\""))
            .unwrap_or_default();

        let _ = write!(
            buf,
            "<!--[if mso | IE]><v:rect style=\"{rect_style}\" xmlns:v=\"urn:schemas-microsoft-com:vml\" fill=\"true\" stroke=\"false\"><v:fill origin=\"{origin_x}, {origin_y}\" position=\"{pos_x}, {pos_y}\" src=\"{url}\"{color_attr} type=\"{vml_type}\"{size_attrs} /><v:textbox style=\"mso-fit-shape-to-text:true\" inset=\"0,0,0,0\"><![endif]-->"
        );
    }

    /// The closing half of the VML background.
    pub(crate) fn vml_close(&self, buf: &mut String) {
        if self.url.is_some() {
            buf.push_str("<!--[if mso | IE]></v:textbox></v:rect><![endif]-->");
        }
    }

    /// VML `origin`/`position` for both axes, as `((originX, posX), (originY, posY))`.
    ///
    /// mjml first normalizes each keyword to a percentage (`left`→`0%`, `center`→`50%`,
    /// `right`→`100%`; `top`→`0%`, `center`→`50%`, `bottom`→`100%`, anything that is not
    /// already a percentage falling back to `50%` on x and `0%` on y). VML's origin is
    /// image-relative, so a tiled background maps the percentage straight to a fraction,
    /// while a non-repeating one is shifted by half the shape (`(-50 + pct) / 100`).
    /// With `background-size:auto` the whole thing collapses back to the legacy
    /// `(0.5, 0)` crop.
    fn vml_position(&self) -> ((f64, f64), (f64, f64)) {
        if self.size == "auto" {
            return ((0.5, 0.5), (0.0, 0.0));
        }
        let x = to_percent(self.pos_x, true);
        let y = to_percent(self.pos_y, false);
        let map = |percent: f64| {
            let decimal = percent / 100.0;
            if self.repeat == "repeat" {
                decimal
            } else {
                (-50.0 + decimal * 100.0) / 100.0
            }
        };
        let (vx, vy) = (map(x), map(y));
        ((vx, vx), (vy, vy))
    }
}

/// Normalize one axis of `background-position` to its percentage value.
///
/// Keywords map to 0/50/100; an explicit `N%` (integer-truncated, like mjml's
/// `parseInt`) passes through; anything else falls back to the axis default.
fn to_percent(value: &str, is_x: bool) -> f64 {
    match value {
        "left" | "top" => 0.0,
        "center" => 50.0,
        "right" | "bottom" => 100.0,
        other => match percentage(other) {
            Some(p) => p,
            None if is_x => 50.0,
            None => 0.0,
        },
    }
}

/// Parse `"25%"` / `"25.5%"` into its integer percentage (mjml applies `parseInt`).
fn percentage(value: &str) -> Option<f64> {
    let digits = value.strip_suffix('%')?;
    let is_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let int = match digits.split_once('.') {
        Some((int, frac)) if is_digits(frac) => int,
        Some(_) => return None,
        None => digits,
    };
    if !is_digits(int) {
        return None;
    }
    int.parse::<f64>().ok()
}

/// Split `background-position` into its `(x, y)` halves (mjml's
/// `parseBackgroundPosition`).
///
/// A single value fills the axis it names and centers the other; two values may appear
/// in either order and are disambiguated by which keywords they use; anything longer is
/// discarded in favour of the default.
fn parse_position(position: &str) -> (&str, &str) {
    let parts: Vec<&str> = position.split(' ').collect();
    match parts.as_slice() {
        [one] => {
            if matches!(*one, "top" | "bottom") {
                ("center", one)
            } else {
                (one, "center")
            }
        }
        [first, second] => {
            if matches!(*first, "top" | "bottom")
                || (*first == "center" && matches!(*second, "left" | "right"))
            {
                (second, first)
            } else {
                (first, second)
            }
        }
        _ => ("center", "top"),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_position, percentage};

    #[test]
    fn splits_positions_like_mjml() {
        assert_eq!(parse_position("top center"), ("center", "top"));
        assert_eq!(parse_position("bottom right"), ("right", "bottom"));
        assert_eq!(parse_position("left"), ("left", "center"));
        assert_eq!(parse_position("bottom"), ("center", "bottom"));
        assert_eq!(parse_position("center left"), ("left", "center"));
        assert_eq!(parse_position("25% 75%"), ("25%", "75%"));
        assert_eq!(parse_position("a b c"), ("center", "top"));
    }

    #[test]
    fn parses_percentages_like_parse_int() {
        assert_eq!(percentage("25%"), Some(25.0));
        assert_eq!(percentage("25.5%"), Some(25.0));
        assert_eq!(percentage("25px"), None);
        assert_eq!(percentage("-25%"), None);
        assert_eq!(percentage("%"), None);
    }
}
