//! Self-consistency properties of the formatter, exercised over every
//! `fixtures/*.mjml`:
//!
//! 1. **Idempotence** — `format(parse(format(parse(src)))) == format(parse(src))`.
//! 2. **Reparse-equivalence** — parsing the formatted output yields a tree that is
//!    structurally equal (via [`docs_eq`]) to parsing the original.
//!
//! Render-equivalence (property 3) lives in the umbrella crate's tests, where the
//! renderer is available (`crates/mjmx/tests/format.rs`).

use std::fs;
use std::path::PathBuf;

use mjmx_allocator::Allocator;
use mjmx_ast::{Document, Element, Node};
use mjmx_formatter::{FormatOptions, format};
use mjmx_parser::parse;

/// The absolute path to the workspace `fixtures/` directory.
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// Every top-level `fixtures/*.mjml` file as `(name, source)` pairs.
fn mjml_fixtures() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for entry in fs::read_dir(fixtures_dir()).expect("read fixtures dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) == Some("mjml") {
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            let src = fs::read_to_string(&path).expect("read fixture");
            out.push((name, src));
        }
    }
    assert!(!out.is_empty(), "no .mjml fixtures found");
    out.sort();
    out
}

/// Parse `src` (asserting no parse errors) and format it with default options.
fn format_source(src: &str) -> String {
    let alloc = Allocator::new();
    let parsed = parse(src, &alloc);
    assert!(
        parsed.errors.is_empty(),
        "parse errors: {:?}",
        parsed.errors
    );
    format(&parsed.document, &FormatOptions::default())
}

#[test]
fn formatting_is_idempotent() {
    for (name, src) in mjml_fixtures() {
        let once = format_source(&src);
        let twice = format_source(&once);
        assert_eq!(twice, once, "{name}: formatting is not idempotent");
    }
}

#[test]
fn formatting_preserves_structure() {
    for (name, src) in mjml_fixtures() {
        let formatted = format_source(&src);

        let original_alloc = Allocator::new();
        let original = parse(&src, &original_alloc);

        let formatted_alloc = Allocator::new();
        let reparsed = parse(&formatted, &formatted_alloc);
        assert!(
            reparsed.errors.is_empty(),
            "{name}: formatted output failed to parse: {:?}",
            reparsed.errors
        );

        assert!(
            docs_eq(&original.document, &reparsed.document),
            "{name}: formatted output is not structurally equal to the original\n--- formatted ---\n{formatted}"
        );
    }
}

// --- Structural AST equality -------------------------------------------------
//
// Two documents are "structurally equal" when they carry the same content once
// insignificant layout is normalized: the same sequence of elements/comments, the
// same ordered attribute name→value pairs, byte-identical raw-content bodies, and
// equal comment/text values (text compared after trimming; whitespace-only text is
// insignificant layout and ignored).

/// Whether two documents are structurally equal.
fn docs_eq(a: &Document, b: &Document) -> bool {
    nodes_eq(&a.children, &b.children)
}

/// Compare two child lists, ignoring insignificant whitespace-only text nodes.
fn nodes_eq(a: &[Node], b: &[Node]) -> bool {
    let a: Vec<&Node> = a.iter().filter(|n| is_significant(n)).collect();
    let b: Vec<&Node> = b.iter().filter(|n| is_significant(n)).collect();
    a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| node_eq(x, y))
}

/// Whether a node carries significant content (drops whitespace-only text).
fn is_significant(node: &Node) -> bool {
    match node {
        Node::Element(_) | Node::Comment(_) => true,
        Node::Text(text) => !text.value.as_str().trim().is_empty(),
    }
}

/// Structural equality of two individual nodes.
fn node_eq(a: &Node, b: &Node) -> bool {
    match (a, b) {
        (Node::Element(x), Node::Element(y)) => element_eq(x, y),
        (Node::Comment(x), Node::Comment(y)) => x.value == y.value,
        (Node::Text(x), Node::Text(y)) => x.value.as_str().trim() == y.value.as_str().trim(),
        _ => false,
    }
}

/// Structural equality of two elements: same tag, same ordered attributes, and —
/// for raw-content elements — a byte-identical body; otherwise equal children.
fn element_eq(a: &Element, b: &Element) -> bool {
    if a.raw_tag != b.raw_tag {
        return false;
    }
    if a.attributes.len() != b.attributes.len() {
        return false;
    }
    for (x, y) in a.attributes.iter().zip(b.attributes.iter()) {
        if x.name != y.name || x.value != y.value {
            return false;
        }
    }
    if a.tag.is_ending_tag() {
        // Raw bodies must survive verbatim (byte-for-byte).
        raw_body(a) == raw_body(b)
    } else {
        nodes_eq(&a.children, &b.children)
    }
}

/// The concatenated verbatim text body of a raw-content element (empty when none).
fn raw_body(el: &Element) -> String {
    el.children
        .iter()
        .filter_map(|child| match child {
            Node::Text(text) => Some(text.value.as_str()),
            _ => None,
        })
        .collect()
}
