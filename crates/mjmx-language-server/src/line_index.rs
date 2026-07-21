//! Byte offset ↔ LSP [`Position`] conversion.
//!
//! `mjmx` spans are **byte** offsets into UTF-8 source. The Language Server
//! Protocol, by default, addresses text as `(line, character)` where `character`
//! counts **UTF-16 code units** (`PositionEncodingKind::UTF16`). Those two agree
//! only for ASCII; a single `é` is 2 bytes but 1 UTF-16 unit, and `🎉` is 4 bytes
//! but 2 UTF-16 units. Converting naively silently misplaces every squiggle on any
//! line containing non-ASCII text, so the conversion is done here, once, and unit
//! tested against multi-byte input.
//!
//! Line breaks: `\n` ends a line, and a `\r` immediately preceding it is treated as
//! part of the terminator (so CRLF documents produce the same columns as LF ones).
//! A lone `\r` is *not* a line break (matching how the parser and every modern
//! editor treat it).

use mjmx_span::Span;
use tower_lsp::lsp_types::{Position, Range};

/// A precomputed map of line starts for one document revision.
#[derive(Debug, Clone)]
pub struct LineIndex {
    /// The document text this index describes.
    text: String,
    /// Byte offset of the first character of each line. Always starts with `0`.
    line_starts: Vec<usize>,
}

impl LineIndex {
    /// Build an index over `text`.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let mut line_starts = vec![0usize];
        line_starts.extend(
            text.bytes()
                .enumerate()
                .filter(|(_, b)| *b == b'\n')
                .map(|(i, _)| i + 1),
        );
        Self { text, line_starts }
    }

    /// The indexed source text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The number of lines (always at least 1).
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Convert a byte `offset` into an LSP [`Position`].
    ///
    /// Offsets past the end of the document clamp to the end; offsets landing in the
    /// middle of a UTF-8 sequence round *down* to the enclosing character boundary.
    #[must_use]
    pub fn position(&self, offset: usize) -> Position {
        let offset = self.floor_char_boundary(offset.min(self.text.len()));
        // The last line whose start is <= offset.
        let line = match self.line_starts.binary_search(&offset) {
            Ok(exact) => exact,
            Err(next) => next - 1,
        };
        let line_start = self.line_starts[line];
        let character = utf16_len(&self.text[line_start..offset]);
        Position {
            line: line as u32,
            character: character as u32,
        }
    }

    /// Convert an LSP [`Position`] back into a byte offset.
    ///
    /// Out-of-range lines clamp to the end of the document; a `character` beyond the
    /// end of its line clamps to the line's terminator.
    #[must_use]
    pub fn offset(&self, position: Position) -> usize {
        let line = position.line as usize;
        let Some(&line_start) = self.line_starts.get(line) else {
            return self.text.len();
        };
        let line_end = self
            .line_starts
            .get(line + 1)
            .map_or(self.text.len(), |&next| {
                // Exclude the terminator (`\n`, and the `\r` of a CRLF pair).
                let end = next - 1;
                if end > line_start && self.text.as_bytes()[end - 1] == b'\r' {
                    end - 1
                } else {
                    end
                }
            });

        let mut remaining = position.character as usize;
        for (rel, ch) in self.text[line_start..line_end].char_indices() {
            let units = ch.len_utf16();
            if remaining < units {
                return line_start + rel;
            }
            remaining -= units;
        }
        line_end
    }

    /// Convert a `mjmx` byte [`Span`] into an LSP [`Range`].
    #[must_use]
    pub fn range(&self, span: Span) -> Range {
        Range {
            start: self.position(span.start as usize),
            end: self.position(span.end as usize),
        }
    }

    /// A [`Range`] covering the whole document (for full-document edits).
    #[must_use]
    pub fn full_range(&self) -> Range {
        Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: self.position(self.text.len()),
        }
    }

    /// Round `offset` down to the nearest UTF-8 character boundary.
    fn floor_char_boundary(&self, mut offset: usize) -> usize {
        while offset > 0 && !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }
}

/// The length of `s` in UTF-16 code units.
fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    #[test]
    fn ascii_offsets_map_to_the_same_column() {
        let index = LineIndex::new("<mjml>\n  <mj-body>\n");
        assert_eq!(index.position(0), pos(0, 0));
        assert_eq!(index.position(5), pos(0, 5));
        assert_eq!(index.position(7), pos(1, 0));
        assert_eq!(index.position(9), pos(1, 2));
    }

    #[test]
    fn two_byte_characters_count_as_one_utf16_unit() {
        // "é" is 2 bytes in UTF-8, 1 UTF-16 code unit.
        let text = "<mj-text>café</mj-text>";
        let index = LineIndex::new(text);
        let byte = text.find("</mj-text>").unwrap();
        assert_eq!(byte, 14, "12 ascii bytes + 2 bytes for é");
        assert_eq!(index.position(byte), pos(0, 13));
    }

    #[test]
    fn astral_characters_count_as_two_utf16_units() {
        // "🎉" is 4 bytes in UTF-8, 2 UTF-16 code units (a surrogate pair).
        let text = "a🎉b";
        let index = LineIndex::new(text);
        assert_eq!(index.position(0), pos(0, 0));
        assert_eq!(index.position(1), pos(0, 1));
        assert_eq!(index.position(5), pos(0, 3));
        assert_eq!(index.position(6), pos(0, 4));
    }

    #[test]
    fn mixed_multibyte_lines_keep_columns_independent() {
        let text = "héllo\n🎉 wörld\nplain";
        let index = LineIndex::new(text);
        assert_eq!(index.line_count(), 3);
        // End of line 0: 5 characters, all single-unit.
        assert_eq!(index.position(text.find('\n').unwrap()), pos(0, 5));
        // Start of line 1.
        let line1 = text.find('🎉').unwrap();
        assert_eq!(index.position(line1), pos(1, 0));
        // After the emoji: 2 UTF-16 units.
        assert_eq!(index.position(line1 + 4), pos(1, 2));
        // "ö" is 2 bytes; the "d" of "wörld".
        let d = text.find('d').unwrap();
        assert_eq!(index.position(d), pos(1, 7));
        assert_eq!(index.position(text.find("plain").unwrap()), pos(2, 0));
    }

    #[test]
    fn crlf_terminators_do_not_shift_columns() {
        let text = "<mjml>\r\n  <mj-body>\r\n";
        let index = LineIndex::new(text);
        assert_eq!(index.line_count(), 3);
        assert_eq!(index.position(0), pos(0, 0));
        // The `\r` sits at column 6, the `\n` at 7; line 1 starts right after.
        assert_eq!(index.position(8), pos(1, 0));
        assert_eq!(index.position(10), pos(1, 2));
        // Round-tripping a position on a CRLF line clamps before the `\r`.
        assert_eq!(index.offset(pos(0, 99)), 6);
    }

    #[test]
    fn offset_round_trips_through_position() {
        let text = "héllo\r\n🎉 wörld\nplain 🎉 tail";
        let index = LineIndex::new(text);
        for (byte, ch) in text.char_indices() {
            // Line terminators have no `Position` of their own: a caret can sit
            // before or after them, never "inside" one.
            if ch == '\r' || ch == '\n' {
                continue;
            }
            assert_eq!(
                index.offset(index.position(byte)),
                byte,
                "round trip failed at byte {byte}"
            );
        }
    }

    #[test]
    fn out_of_range_inputs_clamp() {
        let text = "abc\ndef";
        let index = LineIndex::new(text);
        assert_eq!(index.position(1_000), pos(1, 3));
        assert_eq!(index.offset(pos(99, 0)), text.len());
        assert_eq!(index.offset(pos(0, 99)), 3);
    }

    #[test]
    fn offsets_inside_a_character_round_down() {
        let text = "🎉";
        let index = LineIndex::new(text);
        assert_eq!(index.position(2), pos(0, 0));
    }

    #[test]
    fn span_becomes_a_range() {
        let text = "<mjml>\n<mj-bogus />\n</mjml>";
        let index = LineIndex::new(text);
        let start = text.find("<mj-bogus").unwrap() as u32;
        let range = index.range(Span::new(start, start + 9));
        assert_eq!(range.start, pos(1, 0));
        assert_eq!(range.end, pos(1, 9));
    }

    #[test]
    fn full_range_covers_the_document() {
        let index = LineIndex::new("ab\ncdé");
        assert_eq!(index.full_range().start, pos(0, 0));
        assert_eq!(index.full_range().end, pos(1, 3));
    }
}
