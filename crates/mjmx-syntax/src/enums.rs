//! Small closed enums for alignment and text direction attribute values.

use std::fmt;

/// Horizontal alignment (`align` / `text-align`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// `left`.
    Left,
    /// `center`.
    Center,
    /// `right`.
    Right,
}

impl Align {
    /// The canonical attribute string.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Align::Left => "left",
            Align::Center => "center",
            Align::Right => "right",
        }
    }

    /// Parse an alignment keyword, case-insensitively.
    pub fn parse(s: &str) -> Option<Align> {
        match s.trim().to_ascii_lowercase().as_str() {
            "left" => Some(Align::Left),
            "center" => Some(Align::Center),
            "right" => Some(Align::Right),
            _ => None,
        }
    }
}

impl fmt::Display for Align {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Vertical alignment (`vertical-align`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlign {
    /// `top`.
    Top,
    /// `middle`.
    Middle,
    /// `bottom`.
    Bottom,
}

impl VerticalAlign {
    /// The canonical attribute string.
    pub const fn as_str(&self) -> &'static str {
        match self {
            VerticalAlign::Top => "top",
            VerticalAlign::Middle => "middle",
            VerticalAlign::Bottom => "bottom",
        }
    }

    /// Parse a vertical-alignment keyword, case-insensitively.
    pub fn parse(s: &str) -> Option<VerticalAlign> {
        match s.trim().to_ascii_lowercase().as_str() {
            "top" => Some(VerticalAlign::Top),
            "middle" => Some(VerticalAlign::Middle),
            "bottom" => Some(VerticalAlign::Bottom),
            _ => None,
        }
    }
}

impl fmt::Display for VerticalAlign {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Text direction (`dir`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Left-to-right (`ltr`).
    Ltr,
    /// Right-to-left (`rtl`).
    Rtl,
}

impl Direction {
    /// The canonical attribute string.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Direction::Ltr => "ltr",
            Direction::Rtl => "rtl",
        }
    }

    /// Parse a direction keyword, case-insensitively.
    pub fn parse(s: &str) -> Option<Direction> {
        match s.trim().to_ascii_lowercase().as_str() {
            "ltr" => Some(Direction::Ltr),
            "rtl" => Some(Direction::Rtl),
            _ => None,
        }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_round_trip() {
        for v in [Align::Left, Align::Center, Align::Right] {
            assert_eq!(Align::parse(v.as_str()), Some(v));
            assert_eq!(v.to_string(), v.as_str());
        }
        assert_eq!(Align::parse("CENTER"), Some(Align::Center));
        assert_eq!(Align::parse("nope"), None);
    }

    #[test]
    fn vertical_align_round_trip() {
        for v in [
            VerticalAlign::Top,
            VerticalAlign::Middle,
            VerticalAlign::Bottom,
        ] {
            assert_eq!(VerticalAlign::parse(v.as_str()), Some(v));
            assert_eq!(v.to_string(), v.as_str());
        }
        assert_eq!(VerticalAlign::parse("bad"), None);
    }

    #[test]
    fn direction_round_trip() {
        for v in [Direction::Ltr, Direction::Rtl] {
            assert_eq!(Direction::parse(v.as_str()), Some(v));
            assert_eq!(v.to_string(), v.as_str());
        }
        assert_eq!(Direction::parse("bad"), None);
    }
}
