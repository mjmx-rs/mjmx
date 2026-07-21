//! Shared MJML syntax vocabulary for `mjmx`.
//!
//! This crate holds the small, dependency-free vocabulary types that the parser,
//! semantic pass, and codegen all agree on: the [`MjmlTag`] set of known tag kinds,
//! CSS-ish value types ([`Length`], [`Color`], [`Unit`]), and the alignment /
//! direction enums ([`Align`], [`VerticalAlign`], [`Direction`]).
//!
//! It is `std`-only and carries no external dependencies so that every downstream
//! crate can depend on it cheaply.

pub mod enums;
pub mod tag;
pub mod unit;

pub use enums::{Align, Direction, VerticalAlign};
pub use tag::MjmlTag;
pub use unit::{Color, Length, Unit};
