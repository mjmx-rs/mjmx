//! Source-map tests: the HTML must not move, and the mapping must be *right*.
//!
//! The correctness proof used throughout is a round trip: take a distinctive substring of
//! the rendered HTML, find its byte offset, `lookup()` it, then slice the returned span
//! out of the **original `.mjml` source** and assert it is the element that should have
//! produced it. Output offset → source span → correct source text, end to end.

use std::path::{Path, PathBuf};

use mjmx::{RenderOptions, SourceMap};

/// The repository `fixtures/` directory.
fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// Every `fixtures/*.mjml`, as `(name, source)`, sorted for determinism.
fn all_fixtures() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = std::fs::read_dir(fixtures_dir())
        .expect("fixtures/ should exist")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "mjml"))
        .map(|p| {
            let name = p.file_stem().unwrap().to_string_lossy().into_owned();
            (name, std::fs::read_to_string(&p).expect("fixture readable"))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(out.len() >= 50, "expected the full fixture corpus");
    out
}

/// [`all_fixtures`] minus the documents a source map is *refused* for.
///
/// Two post-codegen passes rewrite the HTML and so invalidate every recorded offset,
/// making `render_with_sourcemap` return a diagnostic instead of a map:
///
/// - a `<mj-style inline="inline">` runs the CSS inliner (see
///   [`sourcemap_is_rejected_for_an_inlined_document`]);
/// - an `<mj-html-attributes>` runs the html-attributes pass (see
///   [`sourcemap_is_rejected_for_an_html_attributes_document`]).
///
/// The sweeps below are about mappings being *right*, so they run over the documents
/// that have them.
fn mappable_fixtures() -> Vec<(String, String)> {
    let mut out = all_fixtures();
    out.retain(|(_, source)| {
        !source.contains("inline=\"inline\"") && !source.contains("<mj-html-attributes")
    });
    assert!(out.len() >= 50, "expected the full fixture corpus");
    out
}

/// Render a fixture by name with a source map.
fn render_fixture(name: &str) -> (String, String, SourceMap) {
    let source = std::fs::read_to_string(fixtures_dir().join(format!("{name}.mjml")))
        .unwrap_or_else(|e| panic!("{name}.mjml should be readable: {e}"));
    let (html, map) = mjmx::render_with_sourcemap(&source, &RenderOptions::default())
        .unwrap_or_else(|e| panic!("{name}.mjml should render: {e:?}"));
    (source, html, map)
}

/// Look up the innermost element covering the first occurrence of `needle` in `html`, and
/// return the slice of MJML source it maps back to.
fn source_at<'s>(source: &'s str, html: &str, map: &SourceMap, needle: &str) -> (&'s str, &'s str) {
    let offset = html
        .find(needle)
        .unwrap_or_else(|| panic!("output should contain {needle:?}"));
    let mapping = map
        .lookup(offset)
        .unwrap_or_else(|| panic!("offset {offset} ({needle:?}) should map to an element"));
    let slice = &source[mapping.source.start as usize..mapping.source.end as usize];
    (slice, mapping.tag.as_str())
}

// ---------------------------------------------------------------------------
// The HTML must not move.
// ---------------------------------------------------------------------------

#[test]
fn sourcemap_collection_does_not_change_the_html() {
    let options = RenderOptions::default();
    for (name, source) in mappable_fixtures() {
        let plain = mjmx::render(&source, &options)
            .unwrap_or_else(|e| panic!("{name}.mjml should render: {e:?}"));
        let (mapped, _) = mjmx::render_with_sourcemap(&source, &options)
            .unwrap_or_else(|e| panic!("{name}.mjml should render with a sourcemap: {e:?}"));
        assert_eq!(
            plain, mapped,
            "{name}: HTML differs between render() and render_with_sourcemap()"
        );
    }
}

#[test]
fn every_fixture_produces_mappings_inside_the_html() {
    for (name, source) in mappable_fixtures() {
        let (html, map) = mjmx::render_with_sourcemap(&source, &RenderOptions::default())
            .unwrap_or_else(|e| panic!("{name}.mjml should render: {e:?}"));
        assert!(!map.is_empty(), "{name}: no mappings recorded");
        for m in map.mappings() {
            // Output ranges stay inside the produced HTML and on char boundaries.
            assert!(
                m.output.end as usize <= html.len(),
                "{name}: mapping {m:?} runs past the HTML ({} bytes)",
                html.len()
            );
            assert!(
                html.is_char_boundary(m.output.start as usize)
                    && html.is_char_boundary(m.output.end as usize),
                "{name}: mapping {m:?} does not land on char boundaries"
            );
            // Source spans stay inside the MJML source and really are that element.
            let slice = &source[m.source.start as usize..m.source.end as usize];
            assert!(
                slice.starts_with(&format!("<{}", m.tag.as_str())),
                "{name}: span for {} slices to {:?}",
                m.tag.as_str(),
                &slice[..slice.len().min(40)]
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Round trip: output offset → source span → the right source element.
// ---------------------------------------------------------------------------

#[test]
fn round_trip_text_content_to_mj_text() {
    let (source, html, map) = render_fixture("slice");
    let (slice, tag) = source_at(&source, &html, &map, "Hello world");
    assert_eq!(tag, "mj-text");
    assert!(slice.starts_with("<mj-text"), "sliced {slice:?}");
    assert!(slice.ends_with("</mj-text>"), "sliced {slice:?}");
    assert!(slice.contains("Hello world"));
}

#[test]
fn round_trip_resolves_to_the_innermost_element_not_the_section() {
    let (source, html, map) = render_fixture("slice");
    let text_offset = html.find("Hello world").unwrap();

    // Innermost at the text: the mj-text.
    assert_eq!(map.lookup(text_offset).unwrap().tag.as_str(), "mj-text");

    // Everything enclosing it is still recorded, and all of them cover the offset.
    let enclosing: Vec<&str> = map
        .mappings()
        .iter()
        .filter(|m| m.contains(text_offset as u32))
        .map(|m| m.tag.as_str())
        .collect();
    for tag in ["mj-text", "mj-column", "mj-section", "mj-body"] {
        assert!(
            enclosing.contains(&tag),
            "{tag} should cover the text offset"
        );
    }

    // The first byte of the section's output belongs to the section, not the text.
    let section_span = map
        .mappings()
        .iter()
        .find(|m| m.tag.as_str() == "mj-section")
        .unwrap();
    let outer = map.lookup(section_span.output.start as usize).unwrap();
    assert_eq!(outer.tag.as_str(), "mj-section");
    assert!(
        source[outer.source.start as usize..outer.source.end as usize].starts_with("<mj-section"),
    );
}

#[test]
fn round_trip_image_src_to_mj_image() {
    let (source, html, map) = render_fixture("image_attrs");
    let src = source
        .split("src=\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .expect("image fixture should set a src");
    let (slice, tag) = source_at(&source, &html, &map, src);
    assert_eq!(tag, "mj-image");
    assert!(slice.starts_with("<mj-image"), "sliced {slice:?}");
}

#[test]
fn round_trip_button_markup_to_mj_button() {
    let (source, html, map) = render_fixture("button");
    // The button's own href only appears in the output because the mj-button put it there.
    let (slice, tag) = source_at(&source, &html, &map, "https://example.com");
    assert_eq!(tag, "mj-button");
    assert!(slice.starts_with("<mj-button"), "sliced {slice:?}");
    assert!(slice.contains("Click me"), "sliced {slice:?}");
}

#[test]
fn round_trip_section_background_color_to_mj_section() {
    // `body_attrs` and friends carry a background-color on a section; pick whichever
    // fixture declares one on an mj-section and confirm the styled markup maps back.
    let (source, html, map) = render_fixture("section_attrs");
    let offset = html
        .find("background-color:")
        .expect("output should carry a background-color declaration");
    let mapping = map.lookup(offset).expect("styled markup should map");
    let slice = &source[mapping.source.start as usize..mapping.source.end as usize];
    assert!(
        slice.starts_with(&format!("<{}", mapping.tag.as_str())),
        "sliced {slice:?}"
    );
}

// ---------------------------------------------------------------------------
// Reverse direction: source span → output ranges.
// ---------------------------------------------------------------------------

#[test]
fn reverse_lookup_finds_the_output_of_a_source_element() {
    let (source, html, map) = render_fixture("slice");

    // Span of the `<mj-text>` in the source, taken straight from the text itself.
    let start = source.find("<mj-text").unwrap();
    let end = source.find("</mj-text>").unwrap() + "</mj-text>".len();
    let span = mjmx::Span::new(start as u32, end as u32);

    let hits: Vec<_> = map.mappings_for_source(span).collect();
    // The text, plus every ancestor whose source span contains it.
    let tags: Vec<&str> = hits.iter().map(|m| m.tag.as_str()).collect();
    assert!(tags.contains(&"mj-text"));
    assert!(tags.contains(&"mj-column"));

    // The mj-text's own output range really does contain its rendered content.
    let text = hits.iter().find(|m| m.tag.as_str() == "mj-text").unwrap();
    let rendered = &html[text.output.start as usize..text.output.end as usize];
    assert!(rendered.contains("Hello world"), "rendered {rendered:?}");

    // A span pointing at nothing maps to nothing.
    let past_end = source.len() as u32;
    assert_eq!(
        map.mappings_for_source(mjmx::Span::new(past_end, past_end + 1))
            .count(),
        0
    );
}

#[test]
fn reverse_then_forward_is_consistent() {
    let (source, html, map) = render_fixture("slice");
    let start = source.find("<mj-text").unwrap() as u32;
    let text = map
        .mappings_for_source(mjmx::Span::new(start, start + 1))
        .find(|m| m.tag.as_str() == "mj-text")
        .expect("the mj-text should be reachable from its own span");
    // Forward from the middle of that output range lands back on the same mapping.
    let mid = (text.output.start + text.output.end) as usize / 2;
    let back = map.lookup(mid).unwrap();
    assert_eq!(back.source, text.source);
    assert!(html.is_char_boundary(mid));
}

// ---------------------------------------------------------------------------
// Contract details.
// ---------------------------------------------------------------------------

#[test]
fn sourcemap_is_rejected_with_whitespace_post_processing() {
    let source = std::fs::read_to_string(fixtures_dir().join("slice.mjml")).unwrap();
    for options in [
        RenderOptions {
            minify: true,
            ..Default::default()
        },
        RenderOptions {
            beautify: true,
            ..Default::default()
        },
    ] {
        let err = mjmx::render_with_sourcemap(&source, &options)
            .expect_err("minify/beautify should be refused, not silently mis-mapped");
        assert!(err[0].message.contains("sourcemap"), "{err:?}");
    }
}

/// Unlike minify/beautify, this refusal is a property of the *document*: the same
/// options render fine for a document with no inline style block, so the check can only
/// happen after the source has been parsed and its `<mj-style>` buckets partitioned.
#[test]
fn sourcemap_is_rejected_for_an_inlined_document() {
    let options = RenderOptions::default();
    let source =
        std::fs::read_to_string(fixtures_dir().join("mj_style_inline_basic.mjml")).unwrap();
    let err = mjmx::render_with_sourcemap(&source, &options)
        .expect_err("CSS inlining should be refused, not silently mis-mapped");
    assert!(err[0].message.contains("inline=\"inline\""), "{err:?}");

    // The plain bucket does not inline, so it still maps.
    let plain = std::fs::read_to_string(fixtures_dir().join("mj_style_plain.mjml")).unwrap();
    assert!(mjmx::render_with_sourcemap(&plain, &options).is_ok());
}

/// Same shape as the inline-style refusal: the html-attributes pass mutates the
/// assembled HTML after codegen, so a source map cannot be honest and is refused —
/// a property of the document, detectable only after semantic analysis.
#[test]
fn sourcemap_is_rejected_for_an_html_attributes_document() {
    let options = RenderOptions::default();
    let source = std::fs::read_to_string(fixtures_dir().join("mj_html_attributes.mjml")).unwrap();
    let err = mjmx::render_with_sourcemap(&source, &options)
        .expect_err("the html-attributes pass should be refused, not silently mis-mapped");
    assert!(err[0].message.contains("mj-html-attributes"), "{err:?}");
}

#[test]
fn json_export_covers_every_mapping() {
    let (_, _, map) = render_fixture("slice");
    let json = map.to_json();
    assert!(json.starts_with("{\"version\":1,\"mappings\":["));
    assert_eq!(json.matches("\"output\"").count(), map.len());
    assert!(json.contains("\"tag\":\"mj-text\""));
}
