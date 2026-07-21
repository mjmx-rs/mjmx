//! Output-HTML ↔ MJML-source position mapping for `mjmx`.
//!
//! # Why this is not a `.map` file
//!
//! The familiar JS/CSS source map (VLQ `mappings`, `.map` sidecar, `//# sourceMappingURL`)
//! exists because *browser devtools consume it*. Nothing consumes a source map for
//! **HTML** — devtools have no HTML source-map support, and the rendered artifact of an
//! MJML compile is an email, which is opened in a mail client, not a debugger. Emitting
//! VLQ would be ceremony with no reader.
//!
//! What is actually wanted from "MJML source maps" is one question:
//!
//! > *Which MJML source element produced this bit of HTML?*
//!
//! …which serves click-to-source in the dev server, go-to-output in the language server,
//! and "why did this render like that?" debugging. So this crate is a **queryable span
//! index**, not a serialization format: an ordered list of [`Mapping`]s from a byte range
//! in the produced HTML to the [`Span`] of the MJML element that wrote it, with
//! [`SourceMap::lookup`] (output → source, innermost element wins) and
//! [`SourceMap::mappings_for_source`] (source → output). [`SourceMap::to_json`] exists for
//! external tooling that wants the same data over a wire.
//!
//! # Nesting
//!
//! Mappings nest, because MJML nests: a byte inside an `<mj-text>` is also inside the
//! `<mj-column>`, `<mj-section>` and `<mj-body>` that contain it. Every mapping carries a
//! [`Mapping::depth`], and [`SourceMap::lookup`] returns the deepest mapping covering the
//! offset — the most specific answer, which is the one a "jump to source" action wants.
//!
//! ```
//! use mjmx_span::Span;
//! use mjmx_sourcemap::SourceMapBuilder;
//! use mjmx_syntax::MjmlTag;
//!
//! let mut b = SourceMapBuilder::default();
//! let outer = b.enter();
//! let inner = b.enter();
//! b.record(Span::new(30, 60), MjmlTag::MjText, 10..20, inner);
//! b.record(Span::new(10, 90), MjmlTag::MjSection, 0..40, outer);
//! let map = b.finish();
//!
//! // The innermost element covering output offset 12 is the mj-text.
//! assert_eq!(map.lookup(12).unwrap().tag, MjmlTag::MjText);
//! // Outside the text, the section still answers.
//! assert_eq!(map.lookup(30).unwrap().tag, MjmlTag::MjSection);
//! ```

use std::ops::Range;

pub use mjmx_span::Span;
pub use mjmx_syntax::MjmlTag;

/// One output-HTML range and the MJML element that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
    /// Byte range `[start, end)` in the **produced HTML**.
    pub output: Range<u32>,
    /// Byte range of the element in the **MJML source**.
    pub source: Span,
    /// The kind of MJML element that produced the output range.
    pub tag: MjmlTag,
    /// Nesting depth, `0` for the outermost recorded element. Mappings nest, so the
    /// deepest mapping covering an offset is the most specific one.
    pub depth: u32,
}

impl Mapping {
    /// Whether `offset` (a byte offset into the produced HTML) falls in this mapping.
    #[must_use]
    pub fn contains(&self, offset: u32) -> bool {
        self.output.start <= offset && offset < self.output.end
    }

    /// The length of the output range in bytes.
    #[must_use]
    pub fn output_len(&self) -> u32 {
        self.output.end - self.output.start
    }
}

/// A queryable mapping from produced-HTML byte offsets to MJML source spans.
///
/// Built by a [`SourceMapBuilder`] during codegen. Mappings are stored innermost-first
/// (they are recorded when an element *finishes* writing), which is exactly the order
/// [`lookup`](Self::lookup) wants to scan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceMap {
    mappings: Vec<Mapping>,
}

impl SourceMap {
    /// Every mapping, in the order they were recorded (innermost element first).
    #[must_use]
    pub fn mappings(&self) -> &[Mapping] {
        &self.mappings
    }

    /// The number of recorded mappings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.mappings.len()
    }

    /// Whether nothing was recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    /// The **innermost** MJML element whose output covers `output_offset`.
    ///
    /// Mappings nest; this returns the deepest one covering the offset, so an offset
    /// inside an `<mj-text>`'s markup resolves to the `mj-text` rather than to the
    /// enclosing `<mj-section>`. Returns `None` for offsets in output no element claims
    /// (the doctype, the `<head>`, the `<body>` wrapper).
    #[must_use]
    pub fn lookup(&self, output_offset: usize) -> Option<&Mapping> {
        let offset = u32::try_from(output_offset).ok()?;
        self.mappings
            .iter()
            .filter(|m| m.contains(offset))
            // Deepest wins; on a tie (an element whose output is fully claimed by one
            // child) prefer the shorter range, then the later record.
            .max_by_key(|m| (m.depth, u32::MAX - m.output_len()))
    }

    /// Every mapping whose source span overlaps `span` — the reverse direction.
    ///
    /// Used for "where in the output did this source element end up?": pass the span of
    /// an MJML element (or a cursor's span) and get the output ranges it produced,
    /// outermost-last (record order). An empty `span` matches nothing.
    pub fn mappings_for_source(&self, span: Span) -> impl Iterator<Item = &Mapping> {
        self.mappings
            .iter()
            .filter(move |m| m.source.start < span.end && span.start < m.source.end)
    }

    /// Serialize as JSON for external tooling.
    ///
    /// Shape: `{"version":1,"mappings":[{"output":[s,e],"source":[s,e],"tag":"mj-text",
    /// "depth":n},…]}`. Offsets are byte offsets, output into the rendered HTML and
    /// source into the MJML source text. Hand-rolled: the payload is trivial and this
    /// crate stays dependency-free.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut out = String::from("{\"version\":1,\"mappings\":[");
        for (i, m) in self.mappings.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{{\"output\":[{},{}],\"source\":[{},{}],\"tag\":\"{}\",\"depth\":{}}}",
                m.output.start,
                m.output.end,
                m.source.start,
                m.source.end,
                m.tag.as_str(),
                m.depth,
            ));
        }
        out.push_str("]}");
        out
    }
}

/// Accumulates [`Mapping`]s while codegen writes HTML.
///
/// Codegen calls [`enter`](Self::enter) on the way *into* an element (to claim a depth)
/// and [`record`](Self::record) on the way *out* (when the element's output range is
/// known). Because the produced HTML is assembled around the body buffer and then
/// post-processed, [`shift`](Self::shift) and [`remap_deletions`](Self::remap_deletions)
/// translate the recorded offsets into final-document coordinates before
/// [`finish`](Self::finish).
#[derive(Debug, Clone, Default)]
pub struct SourceMapBuilder {
    mappings: Vec<Mapping>,
    depth: u32,
}

impl SourceMapBuilder {
    /// Enter a nesting level, returning the depth to hand back to [`record`](Self::record).
    ///
    /// Depth is a stack counter: it is incremented here and decremented by `record`, so a
    /// balanced enter/record pair per element keeps depths consistent.
    pub fn enter(&mut self) -> u32 {
        let depth = self.depth;
        self.depth += 1;
        depth
    }

    /// Record that the element at `source` wrote `output`, leaving nesting level `depth`.
    ///
    /// Empty output ranges are dropped: an element that emitted nothing has nothing to
    /// look up, and a zero-length range can never contain an offset.
    pub fn record(&mut self, source: Span, tag: MjmlTag, output: Range<usize>, depth: u32) {
        self.depth = depth;
        if output.start >= output.end {
            return;
        }
        let (Ok(start), Ok(end)) = (u32::try_from(output.start), u32::try_from(output.end)) else {
            return;
        };
        self.mappings.push(Mapping {
            output: start..end,
            source,
            tag,
            depth,
        });
    }

    /// Shift every recorded output offset by `delta` bytes.
    ///
    /// Codegen renders the body into its own buffer, which is later spliced into the full
    /// document at a known offset; this rebases the body-relative offsets onto it.
    pub fn shift(&mut self, delta: usize) {
        let Ok(delta) = u32::try_from(delta) else {
            return;
        };
        for m in &mut self.mappings {
            m.output.start += delta;
            m.output.end += delta;
        }
    }

    /// Rewrite output offsets for a post-process that **deleted** the given byte ranges.
    ///
    /// `deleted` must be sorted and non-overlapping, in the coordinates the mappings
    /// currently use. Each offset moves back by the total deleted length before it, and
    /// offsets inside a deleted range collapse to its start — so a mapping whose output
    /// straddles a deletion stays contiguous and correct. Used for mjml's
    /// `mergeOutlookConditionnals` pass, which splices out fused conditional-comment
    /// seams after the body is rendered.
    pub fn remap_deletions(&mut self, deleted: &[Range<usize>]) {
        if deleted.is_empty() {
            return;
        }
        // Prefix sums: `cuts[i]` = (start of deletion i, end, bytes deleted before it).
        let mut cuts: Vec<(u32, u32, u32)> = Vec::with_capacity(deleted.len());
        let mut removed = 0u32;
        for range in deleted {
            let (Ok(start), Ok(end)) = (u32::try_from(range.start), u32::try_from(range.end))
            else {
                return;
            };
            cuts.push((start, end, removed));
            removed += end - start;
        }

        let translate = |offset: u32| -> u32 {
            // Last cut starting at or before `offset`.
            let idx = cuts.partition_point(|&(start, _, _)| start <= offset);
            if idx == 0 {
                return offset;
            }
            let (start, end, before) = cuts[idx - 1];
            if offset < end {
                // Inside a deleted region: collapse onto its (new) start.
                start - before
            } else {
                offset - before - (end - start)
            }
        };

        self.mappings.retain_mut(|m| {
            m.output.start = translate(m.output.start);
            m.output.end = translate(m.output.end);
            m.output.start < m.output.end
        });
    }

    /// Finish building, yielding the queryable [`SourceMap`].
    #[must_use]
    pub fn finish(self) -> SourceMap {
        SourceMap {
            mappings: self.mappings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(entries: &[(u32, u32, u32, u32, MjmlTag, u32)]) -> SourceMap {
        let mut b = SourceMapBuilder::default();
        for &(os, oe, ss, se, tag, depth) in entries {
            b.mappings.push(Mapping {
                output: os..oe,
                source: Span::new(ss, se),
                tag,
                depth,
            });
        }
        b.finish()
    }

    #[test]
    fn lookup_returns_innermost() {
        let m = map(&[
            (10, 20, 30, 60, MjmlTag::MjText, 2),
            (5, 40, 20, 80, MjmlTag::MjColumn, 1),
            (0, 50, 10, 90, MjmlTag::MjSection, 0),
        ]);
        assert_eq!(m.lookup(12).unwrap().tag, MjmlTag::MjText);
        assert_eq!(m.lookup(25).unwrap().tag, MjmlTag::MjColumn);
        assert_eq!(m.lookup(45).unwrap().tag, MjmlTag::MjSection);
        assert!(m.lookup(50).is_none());
    }

    #[test]
    fn reverse_lookup_finds_overlapping_sources() {
        let m = map(&[
            (10, 20, 30, 60, MjmlTag::MjText, 2),
            (0, 50, 10, 90, MjmlTag::MjSection, 0),
        ]);
        let hits: Vec<_> = m
            .mappings_for_source(Span::new(35, 40))
            .map(|x| x.tag)
            .collect();
        assert_eq!(hits, vec![MjmlTag::MjText, MjmlTag::MjSection]);
        assert_eq!(m.mappings_for_source(Span::new(95, 99)).count(), 0);
    }

    #[test]
    fn record_skips_empty_ranges_and_tracks_depth() {
        let mut b = SourceMapBuilder::default();
        let outer = b.enter();
        let inner = b.enter();
        b.record(Span::new(0, 1), MjmlTag::MjRaw, 5..5, inner);
        b.record(Span::new(0, 9), MjmlTag::MjSection, 0..10, outer);
        // Depth unwound back to the outer level after recording.
        assert_eq!(b.depth, 0);
        let m = b.finish();
        assert_eq!(m.len(), 1);
        assert_eq!(m.mappings()[0].depth, 0);
    }

    #[test]
    fn shift_rebases_offsets() {
        let mut b = SourceMapBuilder::default();
        let d = b.enter();
        b.record(Span::new(0, 5), MjmlTag::MjText, 10..20, d);
        b.shift(100);
        assert_eq!(b.finish().mappings()[0].output, 110..120);
    }

    #[test]
    fn remap_deletions_moves_offsets_back() {
        let mut b = SourceMapBuilder::default();
        let d = b.enter();
        b.record(Span::new(0, 5), MjmlTag::MjText, 30..40, d);
        b.record(Span::new(0, 5), MjmlTag::MjImage, 0..10, d);
        // Delete bytes 10..20 (before the text, after the image).
        let deleted = [Range { start: 10, end: 20 }];
        b.remap_deletions(&deleted);
        let m = b.finish();
        assert_eq!(m.mappings()[0].output, 20..30);
        assert_eq!(m.mappings()[1].output, 0..10);
    }

    #[test]
    fn remap_deletions_collapses_straddling_ranges() {
        let mut b = SourceMapBuilder::default();
        let d = b.enter();
        // Straddles the deletion 10..20 entirely inside it.
        b.record(Span::new(0, 5), MjmlTag::MjText, 12..18, d);
        b.record(Span::new(0, 5), MjmlTag::MjImage, 5..25, d);
        let deleted = [Range { start: 10, end: 20 }];
        b.remap_deletions(&deleted);
        let m = b.finish();
        // Fully-deleted output drops out; the straddling one shrinks by the deletion.
        assert_eq!(m.len(), 1);
        assert_eq!(m.mappings()[0].output, 5..15);
    }

    #[test]
    fn json_round_trips_shape() {
        let m = map(&[(0, 4, 1, 9, MjmlTag::MjText, 0)]);
        assert_eq!(
            m.to_json(),
            "{\"version\":1,\"mappings\":[{\"output\":[0,4],\"source\":[1,9],\"tag\":\"mj-text\",\"depth\":0}]}"
        );
    }
}
