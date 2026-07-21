//! Source spans and interned strings for `mrml`.
//!
//! Every AST node carries a [`Span`] (a byte range into the source) so that
//! diagnostics, the formatter, and future LSP/sourcemap surfaces can map back to
//! precise source locations. [`Atom`] is a lightweight interned-string newtype used
//! for identifiers and other repeated strings.

use std::fmt;
use std::ops::Deref;

/// A byte range `[start, end)` into a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    /// Start byte offset (inclusive).
    pub start: u32,
    /// End byte offset (exclusive).
    pub end: u32,
}

impl Span {
    /// Create a span from a start (inclusive) and end (exclusive) offset.
    #[must_use]
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// The length of the span in bytes.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.end - self.start
    }

    /// Whether the span is empty (`start == end`).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Whether `offset` falls within the span (`start..end`).
    #[must_use]
    pub fn contains(&self, offset: u32) -> bool {
        (self.start..self.end).contains(&offset)
    }
}

/// A type that has a source [`Span`].
pub trait GetSpan {
    /// The source span of this value.
    fn span(&self) -> Span;
}

/// An interned string.
///
/// Currently a thin newtype over a borrowed `&str`; a real string interner will
/// replace the backing storage later without changing this public API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Atom<'a>(&'a str);

impl<'a> Atom<'a> {
    /// Create an atom from a string slice.
    #[must_use]
    pub const fn new(s: &'a str) -> Self {
        Self(s)
    }

    /// The underlying string slice.
    #[must_use]
    pub const fn as_str(&self) -> &'a str {
        self.0
    }
}

impl Deref for Atom<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a> From<&'a str> for Atom<'a> {
    fn from(s: &'a str) -> Self {
        Self::new(s)
    }
}

impl fmt::Display for Atom<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_len_and_emptiness() {
        let span = Span::new(3, 8);
        assert_eq!(span.len(), 5);
        assert!(!span.is_empty());
        assert!(Span::new(4, 4).is_empty());
        assert_eq!(Span::default(), Span::new(0, 0));
    }

    #[test]
    fn span_contains() {
        let span = Span::new(3, 8);
        assert!(span.contains(3));
        assert!(span.contains(7));
        assert!(!span.contains(8));
        assert!(!span.contains(2));
    }

    #[test]
    fn atom_deref_equality_and_display() {
        let atom = Atom::new("mj-section");
        assert_eq!(atom.as_str(), "mj-section");
        assert_eq!(&*atom, "mj-section");
        assert_eq!(atom.len(), 10);
        assert_eq!(atom, Atom::from("mj-section"));
        assert_eq!(atom.to_string(), "mj-section");
    }
}
