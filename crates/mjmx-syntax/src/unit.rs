//! CSS-ish value types shared across the pipeline: [`Unit`], [`Length`], and [`Color`].
//!
//! These are deliberately small and lossless-enough for MJML attribute values. They
//! are not a full CSS value model; parsing is best-effort and tolerant of surrounding
//! whitespace.

use std::fmt;

/// A length unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Unit {
    /// Pixels (`px`).
    Px,
    /// Percentage (`%`).
    Percent,
    /// Font-relative `em`.
    Em,
    /// Root-font-relative `rem`.
    Rem,
    /// The `auto` keyword.
    Auto,
    /// A bare number with no unit suffix.
    None,
}

impl Unit {
    /// The canonical suffix string for this unit (empty for [`Unit::None`]).
    pub const fn as_str(&self) -> &'static str {
        match self {
            Unit::Px => "px",
            Unit::Percent => "%",
            Unit::Em => "em",
            Unit::Rem => "rem",
            Unit::Auto => "auto",
            Unit::None => "",
        }
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A numeric length paired with its [`Unit`].
///
/// For [`Unit::Auto`] the `value` is `0.0` and carries no meaning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Length {
    /// The numeric magnitude.
    pub value: f32,
    /// The associated unit.
    pub unit: Unit,
}

impl Length {
    /// Construct a pixel length.
    pub const fn pixel(value: f32) -> Length {
        Length {
            value,
            unit: Unit::Px,
        }
    }

    /// Construct a percentage length.
    pub const fn percent(value: f32) -> Length {
        Length {
            value,
            unit: Unit::Percent,
        }
    }

    /// Parse an MJML length value.
    ///
    /// Accepts `"10px"`, `"1.5em"`, `"2rem"`, `"50%"`, `"auto"` (→ `{0.0, Auto}`),
    /// and a bare number like `"12"` (→ `{12.0, None}`). Surrounding whitespace is
    /// tolerated. Returns `None` for anything else.
    pub fn parse(s: &str) -> Option<Length> {
        let s = s.trim();
        if s.eq_ignore_ascii_case("auto") {
            return Some(Length {
                value: 0.0,
                unit: Unit::Auto,
            });
        }

        let (num, unit) = if let Some(rest) = s.strip_suffix('%') {
            (rest, Unit::Percent)
        } else if let Some(rest) = s.strip_suffix("px") {
            (rest, Unit::Px)
        } else if let Some(rest) = s.strip_suffix("rem") {
            (rest, Unit::Rem)
        } else if let Some(rest) = s.strip_suffix("em") {
            (rest, Unit::Em)
        } else {
            (s, Unit::None)
        };

        let value: f32 = num.trim().parse().ok()?;
        if !value.is_finite() {
            return None;
        }
        Some(Length { value, unit })
    }
}

impl fmt::Display for Length {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.unit {
            Unit::Auto => f.write_str("auto"),
            Unit::None => write!(f, "{}", self.value),
            unit => write!(f, "{}{}", self.value, unit.as_str()),
        }
    }
}

/// An sRGB color with an 8-bit alpha channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel (`255` = fully opaque).
    pub a: u8,
}

impl Color {
    /// Construct an opaque color from RGB channels.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b, a: 255 }
    }

    /// Parse a CSS-ish color value.
    ///
    /// Handles `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb(r, g, b)`, `rgba(r, g, b, a)`
    /// (with `a` as a `0.0`–`1.0` float), and a small set of named colors
    /// (`black`, `white`, `red`, `green`, `blue`, `transparent`). Whitespace-tolerant
    /// and case-insensitive. Returns `None` for anything else.
    // TODO: back this with the full CSS named-color table.
    pub fn parse(s: &str) -> Option<Color> {
        let s = s.trim();

        match s.to_ascii_lowercase().as_str() {
            "black" => return Some(Color::rgb(0, 0, 0)),
            "white" => return Some(Color::rgb(255, 255, 255)),
            "red" => return Some(Color::rgb(255, 0, 0)),
            "green" => return Some(Color::rgb(0, 128, 0)),
            "blue" => return Some(Color::rgb(0, 0, 255)),
            "transparent" => {
                return Some(Color {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                });
            }
            _ => {}
        }

        if let Some(hex) = s.strip_prefix('#') {
            return Color::parse_hex(hex);
        }

        if let Some(inner) = s
            .strip_prefix("rgba")
            .and_then(|r| r.trim_start().strip_prefix('('))
            .and_then(|r| r.strip_suffix(')'))
        {
            let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
            if parts.len() != 4 {
                return None;
            }
            let r = parts[0].parse().ok()?;
            let g = parts[1].parse().ok()?;
            let b = parts[2].parse().ok()?;
            let a_f: f32 = parts[3].parse().ok()?;
            if !(0.0..=1.0).contains(&a_f) {
                return None;
            }
            let a = (a_f * 255.0).round() as u8;
            return Some(Color { r, g, b, a });
        }

        if let Some(inner) = s
            .strip_prefix("rgb")
            .and_then(|r| r.trim_start().strip_prefix('('))
            .and_then(|r| r.strip_suffix(')'))
        {
            let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
            if parts.len() != 3 {
                return None;
            }
            let r = parts[0].parse().ok()?;
            let g = parts[1].parse().ok()?;
            let b = parts[2].parse().ok()?;
            return Some(Color::rgb(r, g, b));
        }

        None
    }

    fn parse_hex(hex: &str) -> Option<Color> {
        let hex = hex.trim();
        match hex.len() {
            3 => {
                let r = dup_nibble(hex.get(0..1)?)?;
                let g = dup_nibble(hex.get(1..2)?)?;
                let b = dup_nibble(hex.get(2..3)?)?;
                Some(Color::rgb(r, g, b))
            }
            6 => {
                let r = u8::from_str_radix(hex.get(0..2)?, 16).ok()?;
                let g = u8::from_str_radix(hex.get(2..4)?, 16).ok()?;
                let b = u8::from_str_radix(hex.get(4..6)?, 16).ok()?;
                Some(Color::rgb(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(hex.get(0..2)?, 16).ok()?;
                let g = u8::from_str_radix(hex.get(2..4)?, 16).ok()?;
                let b = u8::from_str_radix(hex.get(4..6)?, 16).ok()?;
                let a = u8::from_str_radix(hex.get(6..8)?, 16).ok()?;
                Some(Color { r, g, b, a })
            }
            _ => None,
        }
    }
}

/// Expand a single hex nibble (`"f"`) into a full byte (`0xff`).
fn dup_nibble(s: &str) -> Option<u8> {
    let n = u8::from_str_radix(s, 16).ok()?;
    Some(n << 4 | n)
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.a == 255 {
            write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            let a = self.a as f32 / 255.0;
            write!(f, "rgba({}, {}, {}, {})", self.r, self.g, self.b, a)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_parse_variants() {
        assert_eq!(Length::parse("10px"), Some(Length::pixel(10.0)));
        assert_eq!(Length::parse("50%"), Some(Length::percent(50.0)));
        assert_eq!(
            Length::parse("1.5em"),
            Some(Length {
                value: 1.5,
                unit: Unit::Em
            })
        );
        assert_eq!(
            Length::parse("2rem"),
            Some(Length {
                value: 2.0,
                unit: Unit::Rem
            })
        );
        assert_eq!(
            Length::parse("auto"),
            Some(Length {
                value: 0.0,
                unit: Unit::Auto
            })
        );
        assert_eq!(
            Length::parse("12"),
            Some(Length {
                value: 12.0,
                unit: Unit::None
            })
        );
        assert_eq!(Length::parse("  8px "), Some(Length::pixel(8.0)));
    }

    #[test]
    fn length_parse_invalid() {
        assert_eq!(Length::parse("nope"), None);
        assert_eq!(Length::parse("px"), None);
        assert_eq!(Length::parse(""), None);
    }

    #[test]
    fn length_display_round_trip() {
        assert_eq!(Length::pixel(10.0).to_string(), "10px");
        assert_eq!(Length::percent(50.0).to_string(), "50%");
        assert_eq!(
            Length {
                value: 1.5,
                unit: Unit::Em
            }
            .to_string(),
            "1.5em"
        );
        assert_eq!(
            Length {
                value: 0.0,
                unit: Unit::Auto
            }
            .to_string(),
            "auto"
        );
        assert_eq!(
            Length {
                value: 12.0,
                unit: Unit::None
            }
            .to_string(),
            "12"
        );
    }

    #[test]
    fn color_parse_variants() {
        assert_eq!(Color::parse("#fff"), Some(Color::rgb(255, 255, 255)));
        assert_eq!(Color::parse("#ffffff"), Some(Color::rgb(255, 255, 255)));
        assert_eq!(Color::parse("#ff000080").unwrap().a, 128);
        assert_eq!(Color::parse("rgb(255,0,0)"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(Color::parse("rgb(255, 0, 0)"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(
            Color::parse("rgba(0,0,0,0)"),
            Some(Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0
            })
        );
        assert_eq!(Color::parse("red"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(Color::parse("  BLUE "), Some(Color::rgb(0, 0, 255)));
    }

    #[test]
    fn color_parse_invalid() {
        assert_eq!(Color::parse("#ff"), None);
        assert_eq!(Color::parse("rgb(1,2)"), None);
        assert_eq!(Color::parse("rgba(0,0,0,2)"), None);
        assert_eq!(Color::parse("notacolor"), None);
    }

    #[test]
    fn color_display() {
        assert_eq!(Color::rgb(255, 255, 255).to_string(), "#ffffff");
        assert_eq!(Color::rgb(255, 0, 0).to_string(), "#ff0000");
        assert_eq!(
            Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0
            }
            .to_string(),
            "rgba(0, 0, 0, 0)"
        );
    }
}
