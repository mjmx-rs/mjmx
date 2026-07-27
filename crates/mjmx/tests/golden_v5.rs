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
        golden_v5!($test, $name, $name);
    };
    // Separate source and reference names, for a fixture whose `.mjml` is shared
    // with the V4 gate: one source, two references (`x.mjml` → `x.html` under V4,
    // `x_v5.html` here), so the two modes can never drift on a copied source.
    ($test:ident, $mjml:literal, $html:literal) => {
        #[test]
        fn $test() {
            assert_golden_v5(
                $mjml,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../fixtures/",
                    $mjml,
                    ".mjml"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../fixtures/",
                    $html,
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

// The MJML subset emitted by design-to-email generators: explicit padding on every
// block (a generator means "no padding", not "use mjml's default"), %-width columns
// from fill sizing, and `mj-raw` tables for rows mj-column cannot nest. Sources are
// shared with the V4 gate in `golden.rs`; only the reference differs.
golden_v5!(
    generated_subset_doc,
    "generated_subset_doc",
    "generated_subset_doc_v5"
);
golden_v5!(
    generated_subset_blocks,
    "generated_subset_blocks",
    "generated_subset_blocks_v5"
);
golden_v5!(
    generated_subset_raw_iconrow,
    "generated_subset_raw_iconrow",
    "generated_subset_raw_iconrow_v5"
);
