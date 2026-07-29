//! Every fixture must lint clean.
//!
//! `fixtures/*.mjml` are all valid MJML — each one is golden-tested against real mjml
//! output — so a lint diagnostic on any of them is a false positive by construction.
//! That makes the fixture corpus a standing oracle for the linter, which is how the
//! `<mj-attributes>` bug this file was added for went unnoticed: the golden tests render
//! under *soft* validation, where a diagnostic is reported and then ignored, so nothing
//! ever asserted that the corpus was diagnostic-free.

use std::path::{Path, PathBuf};

use mjmx_allocator::Allocator;

/// Absolute path to the repository's `fixtures/` directory.
fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// Every `fixtures/*.mjml`, sorted, as `(name, source)`.
fn fixtures() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(fixtures_dir()).expect("fixtures/ is readable") {
        let path = entry.expect("readable dir entry").path();
        if path.extension().and_then(|e| e.to_str()) == Some("mjml") {
            let name = path
                .file_stem()
                .expect("fixture has a stem")
                .to_string_lossy()
                .into_owned();
            out.push((
                name,
                std::fs::read_to_string(&path).expect("fixture is utf-8"),
            ));
        }
    }
    out.sort();
    assert!(!out.is_empty(), "no fixtures found");
    out
}

/// No fixture may produce a lint diagnostic.
#[test]
fn every_fixture_lints_clean() {
    let mut offenders = Vec::new();
    for (name, source) in fixtures() {
        let allocator = Allocator::new();
        let parsed = mjmx_parser::parse(&source, &allocator);
        assert!(
            parsed.errors.is_empty(),
            "{name}: fixture does not parse: {:?}",
            parsed.errors
        );
        for diagnostic in mjmx_linter::lint(&parsed.document) {
            offenders.push(format!(
                "{name}: [{:?}] {}",
                diagnostic.severity, diagnostic.message
            ));
        }
    }
    assert!(
        offenders.is_empty(),
        "fixtures are valid MJML, so every diagnostic here is a false positive:\n  {}",
        offenders.join("\n  ")
    );
}

/// The same corpus under `strict`, which is the failure the bug actually caused: a
/// document using `<mj-attributes>` could not be rendered at all.
#[test]
fn every_fixture_renders_under_strict_validation() {
    let options = mjmx::RenderOptions {
        validation_level: mjmx::ValidationLevel::Strict,
        ..Default::default()
    };
    for (name, source) in fixtures() {
        assert!(
            mjmx::render(&source, &options).is_ok(),
            "{name}: strict validation refused a valid document",
        );
    }
}

/// `<mj-attributes>` rows are data, not element positions: `mj-all` and `mj-class` are
/// not components, and a per-tag row may name *any* component — including one the
/// document never uses, and one that does not exist.
#[test]
fn mj_attributes_rows_are_not_validated_as_elements() {
    let source = r##"<mjml>
  <mj-head>
    <mj-attributes>
      <mj-all font-family="Helvetica" />
      <mj-class name="big" font-size="40px" />
      <mj-text color="#f00" />
      <mj-carousel-image border="1px solid" />
    </mj-attributes>
  </mj-head>
  <mj-body>
    <mj-section><mj-column><mj-text mj-class="big">hi</mj-text></mj-column></mj-section>
  </mj-body>
</mjml>"##;

    let allocator = Allocator::new();
    let parsed = mjmx_parser::parse(source, &allocator);
    let diagnostics = mjmx_linter::lint(&parsed.document);
    assert!(
        diagnostics.is_empty(),
        "no row inside <mj-attributes> is an element position: {diagnostics:?}"
    );
}

/// The exemption is scoped to `<mj-attributes>`: an attribute row is only meaningful
/// there, so the same tags are still rejected anywhere else and the fix cannot decay
/// into a blanket amnesty for `mj-all`.
///
/// This is **deliberately stricter than mjml**, which treats `mj-all`/`mj-class` as
/// "component-less" tags that pass validation in any position and then render to
/// nothing (it logs `No matching component for tag : mj-all` and drops it). mjmx reports
/// the misplacement instead, the same way it already does for a misplaced
/// `<mj-selector>`.
#[test]
fn mj_all_outside_mj_attributes_is_still_rejected() {
    let source = "<mjml><mj-body><mj-section><mj-column>\
                  <mj-all font-family=\"Helvetica\" />\
                  </mj-column></mj-section></mj-body></mjml>";

    let allocator = Allocator::new();
    let parsed = mjmx_parser::parse(source, &allocator);
    let diagnostics = mjmx_linter::lint(&parsed.document);
    assert!(
        diagnostics.iter().any(|d| d.message.contains("mj-all")),
        "a stray <mj-all> in the body is still a diagnostic: {diagnostics:?}"
    );
}

/// An unknown tag inside `<mj-attributes>` is still an unknown component.
///
/// The rows are unconstrained *structurally* — any component name is a valid row — but
/// the name must still exist. mjml agrees: its `validTag` rule runs on every element
/// regardless of position, so `<mj-foo>` is rejected there exactly as it is in the body
/// (verified against mjml 4.18.0, which throws `Element mj-foo doesn't exist or is not
/// registered`).
#[test]
fn unknown_row_inside_mj_attributes_is_rejected() {
    let source = "<mjml><mj-head><mj-attributes>\
                  <mj-foo color=\"#f00\" />\
                  </mj-attributes></mj-head><mj-body><mj-section><mj-column>\
                  <mj-text>x</mj-text></mj-column></mj-section></mj-body></mjml>";

    let allocator = Allocator::new();
    let parsed = mjmx_parser::parse(source, &allocator);
    let diagnostics = mjmx_linter::lint(&parsed.document);
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("unknown mjml component") && d.message.contains("mj-foo")),
        "an unknown row name is still an unknown component: {diagnostics:?}"
    );
}
