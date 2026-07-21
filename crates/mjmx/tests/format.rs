//! Render-equivalence of the formatter over every `fixtures/*.mjml`:
//! rendering the FORMATTED source must be [`mjmx_html_compare`]-equal to rendering
//! the original source. This closes the loop with the two source-level properties
//! (idempotence + reparse-equivalence) checked in the `mjmx-formatter` crate.
//!
//! The `include/` fixture is skipped here: it depends on `<mj-include>` resolution
//! against a base directory, which needs a file path rather than an in-memory
//! formatted string.

use std::fs;
use std::path::PathBuf;

use mjmx::{FormatOptions, RenderOptions};

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

#[test]
fn rendering_formatted_source_matches_rendering_the_original() {
    for (name, src) in mjml_fixtures() {
        let original = mjmx::render_default(&src)
            .unwrap_or_else(|e| panic!("{name}.mjml should render: {e:?}"));

        let formatted = mjmx::format(&src, &FormatOptions::default())
            .unwrap_or_else(|e| panic!("{name}.mjml should format: {e:?}"));
        let via_formatter = mjmx::render(&formatted, &RenderOptions::default())
            .unwrap_or_else(|e| panic!("{name}.mjml (formatted) should render: {e:?}"));

        if let Err(mismatch) = mjmx_html_compare::compare(&original, &via_formatter) {
            panic!("{name}: rendering formatted source diverged from the original.\n{mismatch}");
        }
    }
}
