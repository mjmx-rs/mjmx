//! End-to-end golden tests for **mjml 5.x-only** behaviour: each `fixtures/<name>.mjml`
//! must render under [`MjmlCompat::V5`] to `fixtures/<name>.html` (the vendored
//! `mjml@5.4.0` reference), compared semantically via [`mjmx_html_compare`].
//!
//! These complement `tests/golden.rs` (the 4.18.0 gate): the constructs here — notably
//! the mjml 5 section `gutter` / gap / direction column-layout rework — do not exist in
//! 4.18.0, so they are pinned to V5 rather than the frozen V4 fixtures.

/// The options every V5 golden renders under: mjmx's defaults (which are V5), pinned
/// explicitly so the intent is clear and a future default flip cannot move these.
fn v5_options() -> mjmx::RenderOptions {
    mjmx::RenderOptions {
        compat: mjmx::MjmlCompat::V5,
        ..Default::default()
    }
}

/// Render `mjml` under V5 and assert it is semantically equivalent to `golden`.
fn assert_golden_v5(name: &str, mjml: &str, golden: &str) {
    let rendered = mjmx::render(mjml, &v5_options())
        .unwrap_or_else(|e| panic!("{name}.mjml should render: {e:?}"));

    if let Err(mismatch) = mjmx_html_compare::compare(golden, &rendered) {
        panic!("{name}: rendered HTML does not match mjml@5.4.0 golden.\n{mismatch}");
    }
}

macro_rules! golden_v5 {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            assert_golden_v5(
                $name,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../fixtures/",
                    $name,
                    ".mjml"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../fixtures/",
                    $name,
                    ".html"
                )),
            );
        }
    };
}

// mjml 5 section `gutter` — px, default equal-share %, explicit %, RTL, explicit-px
// widths (fractional-pixel distribution), and a gutter inside an mj-group.
golden_v5!(section_gutter_px, "section_gutter_px");
golden_v5!(section_gutter_3col, "section_gutter_3col");
golden_v5!(section_gutter_pct, "section_gutter_pct");
golden_v5!(section_gutter_rtl, "section_gutter_rtl");
golden_v5!(section_gutter_px_widths, "section_gutter_px_widths");
golden_v5!(group_gutter, "group_gutter");
