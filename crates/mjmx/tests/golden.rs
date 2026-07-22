//! End-to-end golden tests: each `fixtures/<name>.mjml` must render to
//! `fixtures/<name>.html` (the vendored mjml 4.18.0 reference), compared
//! semantically via the [`mjmx_html_compare`] crate (structure/attribute/CSS
//! aware, tolerant of insignificant layout differences).

/// Absolute path to a fixture file under `fixtures/`.
macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/",
            $name
        ))
    };
}

/// Render `mjml` and assert it is semantically equivalent to `golden`, surfacing
/// the first divergence (with a tag-path breadcrumb) to make failures debuggable.
fn assert_golden(name: &str, mjml: &str, golden: &str) {
    let rendered =
        mjmx::render_default(mjml).unwrap_or_else(|e| panic!("{name}.mjml should render: {e:?}"));

    if let Err(mismatch) = mjmx_html_compare::compare(golden, &rendered) {
        panic!("{name}: rendered HTML does not match golden reference.\n{mismatch}");
    }
}

#[test]
fn slice_matches_mjml_reference() {
    assert_golden("slice", fixture!("slice.mjml"), fixture!("slice.html"));
}

#[test]
fn attrs_all_matches_mjml_reference() {
    assert_golden(
        "attrs_all",
        fixture!("attrs_all.mjml"),
        fixture!("attrs_all.html"),
    );
}

#[test]
fn attrs_class_matches_mjml_reference() {
    assert_golden(
        "attrs_class",
        fixture!("attrs_class.mjml"),
        fixture!("attrs_class.html"),
    );
}

#[test]
fn head_breakpoint_matches_mjml_reference() {
    assert_golden(
        "head_breakpoint",
        fixture!("head_breakpoint.mjml"),
        fixture!("head_breakpoint.html"),
    );
}

#[test]
fn head_title_preview_matches_mjml_reference() {
    assert_golden(
        "head_title_preview",
        fixture!("head_title_preview.mjml"),
        fixture!("head_title_preview.html"),
    );
}

#[test]
fn head_font_matches_mjml_reference() {
    assert_golden(
        "head_font",
        fixture!("head_font.mjml"),
        fixture!("head_font.html"),
    );
}

#[test]
fn text_attrs_matches_mjml_reference() {
    assert_golden(
        "text_attrs",
        fixture!("text_attrs.mjml"),
        fixture!("text_attrs.html"),
    );
}

#[test]
fn img_matches_mjml_reference() {
    assert_golden("img", fixture!("img.mjml"), fixture!("img.html"));
}

#[test]
fn image_attrs_matches_mjml_reference() {
    assert_golden(
        "image_attrs",
        fixture!("image_attrs.mjml"),
        fixture!("image_attrs.html"),
    );
}

#[test]
fn img_link_matches_mjml_reference() {
    assert_golden(
        "img_link",
        fixture!("img_link.mjml"),
        fixture!("img_link.html"),
    );
}

#[test]
fn divider_matches_mjml_reference() {
    assert_golden(
        "divider",
        fixture!("divider.mjml"),
        fixture!("divider.html"),
    );
}

#[test]
fn divider_attrs_matches_mjml_reference() {
    assert_golden(
        "divider_attrs",
        fixture!("divider_attrs.mjml"),
        fixture!("divider_attrs.html"),
    );
}

#[test]
fn spacer_matches_mjml_reference() {
    assert_golden("spacer", fixture!("spacer.mjml"), fixture!("spacer.html"));
}

#[test]
fn spacer_attrs_matches_mjml_reference() {
    assert_golden(
        "spacer_attrs",
        fixture!("spacer_attrs.mjml"),
        fixture!("spacer_attrs.html"),
    );
}

#[test]
fn button_matches_mjml_reference() {
    assert_golden("button", fixture!("button.mjml"), fixture!("button.html"));
}

#[test]
fn button_attrs_matches_mjml_reference() {
    assert_golden(
        "button_attrs",
        fixture!("button_attrs.mjml"),
        fixture!("button_attrs.html"),
    );
}

#[test]
fn section_attrs_matches_mjml_reference() {
    assert_golden(
        "section_attrs",
        fixture!("section_attrs.mjml"),
        fixture!("section_attrs.html"),
    );
}

#[test]
fn twocol_matches_mjml_reference() {
    assert_golden("twocol", fixture!("twocol.mjml"), fixture!("twocol.html"));
}

#[test]
fn column_attrs_matches_mjml_reference() {
    assert_golden(
        "column_attrs",
        fixture!("column_attrs.mjml"),
        fixture!("column_attrs.html"),
    );
}

#[test]
fn column_px_width_matches_mjml_reference() {
    assert_golden(
        "column_px_width",
        fixture!("column_px_width.mjml"),
        fixture!("column_px_width.html"),
    );
}

#[test]
fn column_inner_border_matches_mjml_reference() {
    assert_golden(
        "column_inner_border",
        fixture!("column_inner_border.mjml"),
        fixture!("column_inner_border.html"),
    );
}

#[test]
fn body_attrs_matches_mjml_reference() {
    assert_golden(
        "body_attrs",
        fixture!("body_attrs.mjml"),
        fixture!("body_attrs.html"),
    );
}

#[test]
fn body_attrs_head_matches_mjml_reference() {
    assert_golden(
        "body_attrs_head",
        fixture!("body_attrs_head.mjml"),
        fixture!("body_attrs_head.html"),
    );
}

#[test]
fn raw_matches_mjml_reference() {
    assert_golden("raw", fixture!("raw.mjml"), fixture!("raw.html"));
}

#[test]
fn head_raw_matches_mjml_reference() {
    assert_golden(
        "head_raw",
        fixture!("head_raw.mjml"),
        fixture!("head_raw.html"),
    );
}

#[test]
fn head_raw_file_start_matches_mjml_reference() {
    assert_golden(
        "head_raw_file_start",
        fixture!("head_raw_file_start.mjml"),
        fixture!("head_raw_file_start.html"),
    );
}

#[test]
fn group_matches_mjml_reference() {
    assert_golden("group", fixture!("group.mjml"), fixture!("group.html"));
}

#[test]
fn group_attrs_matches_mjml_reference() {
    assert_golden(
        "group_attrs",
        fixture!("group_attrs.mjml"),
        fixture!("group_attrs.html"),
    );
}

#[test]
fn wrapper_matches_mjml_reference() {
    assert_golden(
        "wrapper",
        fixture!("wrapper.mjml"),
        fixture!("wrapper.html"),
    );
}

#[test]
fn wrapper_full_width_matches_mjml_reference() {
    assert_golden(
        "wrapper_full_width",
        fixture!("wrapper_full_width.mjml"),
        fixture!("wrapper_full_width.html"),
    );
}

#[test]
fn wrapper_attrs_matches_mjml_reference() {
    assert_golden(
        "wrapper_attrs",
        fixture!("wrapper_attrs.mjml"),
        fixture!("wrapper_attrs.html"),
    );
}

#[test]
fn section_bg_url_matches_mjml_reference() {
    assert_golden(
        "section_bg_url",
        fixture!("section_bg_url.mjml"),
        fixture!("section_bg_url.html"),
    );
}

#[test]
fn section_bg_full_matches_mjml_reference() {
    assert_golden(
        "section_bg_full",
        fixture!("section_bg_full.mjml"),
        fixture!("section_bg_full.html"),
    );
}

#[test]
fn section_bg_attrs_matches_mjml_reference() {
    assert_golden(
        "section_bg_attrs",
        fixture!("section_bg_attrs.mjml"),
        fixture!("section_bg_attrs.html"),
    );
}

#[test]
fn section_bg_full_width_matches_mjml_reference() {
    assert_golden(
        "section_bg_full_width",
        fixture!("section_bg_full_width.mjml"),
        fixture!("section_bg_full_width.html"),
    );
}

#[test]
fn wrapper_bg_url_matches_mjml_reference() {
    assert_golden(
        "wrapper_bg_url",
        fixture!("wrapper_bg_url.mjml"),
        fixture!("wrapper_bg_url.html"),
    );
}

#[test]
fn wrapper_bg_full_width_matches_mjml_reference() {
    assert_golden(
        "wrapper_bg_full_width",
        fixture!("wrapper_bg_full_width.mjml"),
        fixture!("wrapper_bg_full_width.html"),
    );
}

#[test]
fn table_matches_mjml_reference() {
    assert_golden("table", fixture!("table.mjml"), fixture!("table.html"));
}

#[test]
fn table_attrs_matches_mjml_reference() {
    assert_golden(
        "table_attrs",
        fixture!("table_attrs.mjml"),
        fixture!("table_attrs.html"),
    );
}

#[test]
fn social_matches_mjml_reference() {
    assert_golden("social", fixture!("social.mjml"), fixture!("social.html"));
}

#[test]
fn social_attrs_matches_mjml_reference() {
    assert_golden(
        "social_attrs",
        fixture!("social_attrs.mjml"),
        fixture!("social_attrs.html"),
    );
}

#[test]
fn social_vertical_matches_mjml_reference() {
    assert_golden(
        "social_vertical",
        fixture!("social_vertical.mjml"),
        fixture!("social_vertical.html"),
    );
}

#[test]
fn navbar_matches_mjml_reference() {
    assert_golden("navbar", fixture!("navbar.mjml"), fixture!("navbar.html"));
}

#[test]
fn navbar_hamburger_matches_mjml_reference() {
    // The vendored golden's random 16-hex checkbox id has been pinned to the
    // deterministic value our renderer emits (see RenderContext::next_hamburger_id);
    // every other byte is mjml 4.18.0's own output.
    assert_golden(
        "navbar_hamburger",
        fixture!("navbar_hamburger.mjml"),
        fixture!("navbar_hamburger.html"),
    );
}

#[test]
fn navbar_attrs_matches_mjml_reference() {
    assert_golden(
        "navbar_attrs",
        fixture!("navbar_attrs.mjml"),
        fixture!("navbar_attrs.html"),
    );
}

#[test]
fn hero_matches_mjml_reference() {
    assert_golden("hero", fixture!("hero.mjml"), fixture!("hero.html"));
}

#[test]
fn hero_attrs_matches_mjml_reference() {
    assert_golden(
        "hero_attrs",
        fixture!("hero_attrs.mjml"),
        fixture!("hero_attrs.html"),
    );
}

#[test]
fn hero_fluid_matches_mjml_reference() {
    assert_golden(
        "hero_fluid",
        fixture!("hero_fluid.mjml"),
        fixture!("hero_fluid.html"),
    );
}

#[test]
fn carousel_matches_mjml_reference() {
    // mjml stamps the carousel radio group with a per-carousel random 16-hex token
    // (`genRandomHexString(16)`); the vendored golden's token is pinned to the
    // deterministic value our renderer emits (see RenderContext::next_carousel_id).
    assert_golden(
        "carousel",
        fixture!("carousel.mjml"),
        fixture!("carousel.html"),
    );
}

#[test]
fn accordion_matches_mjml_reference() {
    assert_golden(
        "accordion",
        fixture!("accordion.mjml"),
        fixture!("accordion.html"),
    );
}

#[test]
fn accordion_attrs_matches_mjml_reference() {
    assert_golden(
        "accordion_attrs",
        fixture!("accordion_attrs.mjml"),
        fixture!("accordion_attrs.html"),
    );
}

#[test]
fn css_class_layout_matches_mjml_reference() {
    assert_golden(
        "css_class_layout",
        fixture!("css_class_layout.mjml"),
        fixture!("css_class_layout.html"),
    );
}

#[test]
fn css_class_content_matches_mjml_reference() {
    assert_golden(
        "css_class_content",
        fixture!("css_class_content.mjml"),
        fixture!("css_class_content.html"),
    );
}

#[test]
fn css_class_composite_matches_mjml_reference() {
    assert_golden(
        "css_class_composite",
        fixture!("css_class_composite.mjml"),
        fixture!("css_class_composite.html"),
    );
}

#[test]
fn css_class_carousel_matches_mjml_reference() {
    assert_golden(
        "css_class_carousel",
        fixture!("css_class_carousel.mjml"),
        fixture!("css_class_carousel.html"),
    );
}

#[test]
fn css_class_multi_matches_mjml_reference() {
    assert_golden(
        "css_class_multi",
        fixture!("css_class_multi.mjml"),
        fixture!("css_class_multi.html"),
    );
}

// ---------------------------------------------------------------------------
// <mj-style>
//
// mjml keeps two disjoint buckets: `globalData.style` (plain `<mj-style>`), which
// becomes a head `<style>` tag, and `globalData.inlineStyle` (`<mj-style
// inline="inline">`), which is *never* emitted as a tag — it is handed to juice as
// `extraCss` and applied to the markup as `style=""` attributes.
//
// The plain-bucket fixtures pass today. The inline-bucket ones are the explicit TODO
// list for the css-inline milestone: they carry the vendored mjml 4.18.0 output that the
// inliner has to reproduce, and are `#[ignore]`d (not weakened, not deleted) until it
// exists. Run them with `cargo test -p mjmx --test golden -- --ignored`.
macro_rules! pending_inliner {
    () => {
        "pending the css-inline milestone: <mj-style inline=\"inline\"> is collected but \
         not yet applied to the markup"
    };
}

#[test]
fn mj_style_plain_matches_mjml_reference() {
    assert_golden(
        "mj_style_plain",
        fixture!("mj_style_plain.mjml"),
        fixture!("mj_style_plain.html"),
    );
}

#[test]
fn mj_style_inline_not_literal_matches_mjml_reference() {
    // Regression test for the `inline` attribute comparison: mjml checks
    // `getAttribute('inline') === 'inline'`, so `inline="true"` and a bare `inline` are
    // *plain* styles and must still reach the head `<style>` block.
    assert_golden(
        "mj_style_inline_not_literal",
        fixture!("mj_style_inline_not_literal.mjml"),
        fixture!("mj_style_inline_not_literal.html"),
    );
}

#[test]
#[ignore = pending_inliner!()]
fn mj_style_inline_basic_matches_mjml_reference() {
    assert_golden(
        "mj_style_inline_basic",
        fixture!("mj_style_inline_basic.mjml"),
        fixture!("mj_style_inline_basic.html"),
    );
}

#[test]
#[ignore = pending_inliner!()]
fn mj_style_both_buckets_matches_mjml_reference() {
    assert_golden(
        "mj_style_both_buckets",
        fixture!("mj_style_both_buckets.mjml"),
        fixture!("mj_style_both_buckets.html"),
    );
}

#[test]
#[ignore = pending_inliner!()]
fn mj_style_important_matches_mjml_reference() {
    assert_golden(
        "mj_style_important",
        fixture!("mj_style_important.mjml"),
        fixture!("mj_style_important.html"),
    );
}

#[test]
#[ignore = pending_inliner!()]
fn mj_style_specificity_conflict_matches_mjml_reference() {
    assert_golden(
        "mj_style_specificity_conflict",
        fixture!("mj_style_specificity_conflict.mjml"),
        fixture!("mj_style_specificity_conflict.html"),
    );
}

#[test]
#[ignore = pending_inliner!()]
fn mj_style_specificity_order_matches_mjml_reference() {
    assert_golden(
        "mj_style_specificity_order",
        fixture!("mj_style_specificity_order.mjml"),
        fixture!("mj_style_specificity_order.html"),
    );
}

#[test]
#[ignore = pending_inliner!()]
fn mj_style_non_inlinable_matches_mjml_reference() {
    assert_golden(
        "mj_style_non_inlinable",
        fixture!("mj_style_non_inlinable.mjml"),
        fixture!("mj_style_non_inlinable.html"),
    );
}

#[test]
#[ignore = pending_inliner!()]
fn mj_style_dimensions_matches_mjml_reference() {
    assert_golden(
        "mj_style_dimensions",
        fixture!("mj_style_dimensions.mjml"),
        fixture!("mj_style_dimensions.html"),
    );
}

#[test]
#[ignore = pending_inliner!()]
fn mj_style_table_attrs_matches_mjml_reference() {
    assert_golden(
        "mj_style_table_attrs",
        fixture!("mj_style_table_attrs.mjml"),
        fixture!("mj_style_table_attrs.html"),
    );
}

#[test]
#[ignore = pending_inliner!()]
fn mj_style_combinators_matches_mjml_reference() {
    assert_golden(
        "mj_style_combinators",
        fixture!("mj_style_combinators.mjml"),
        fixture!("mj_style_combinators.html"),
    );
}

#[test]
#[ignore = pending_inliner!()]
fn mj_style_conditional_order_matches_mjml_reference() {
    assert_golden(
        "mj_style_conditional_order",
        fixture!("mj_style_conditional_order.mjml"),
        fixture!("mj_style_conditional_order.html"),
    );
}

#[test]
fn include_matches_mjml_reference() {
    // `render_file` resolves the `<mj-include>` in main.mjml against the file's own
    // directory (via the LocalLoader), exactly as the mjml CLI did when generating the
    // vendored golden.
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/include/main.mjml"
    );
    let rendered = mjmx::render_file(std::path::Path::new(path), &mjmx::RenderOptions::default())
        .unwrap_or_else(|e| panic!("include/main.mjml should render: {e:?}"));

    let golden = fixture!("include/main.html");
    if let Err(mismatch) = mjmx_html_compare::compare(golden, &rendered) {
        panic!("include: rendered HTML does not match golden reference.\n{mismatch}");
    }
}

#[test]
fn comparator_rejects_tampered_golden() {
    // The real golden must compare equal to itself (sanity), but the comparator
    // must NOT be so tolerant that a tampered golden slips through: flipping a
    // single style declaration on a real reference must be detected.
    let golden = fixture!("slice.html");
    assert!(
        mjmx_html_compare::compare(golden, golden).is_ok(),
        "an untouched golden must compare equal to itself"
    );

    let tampered = golden.replace("padding:10px 25px;", "padding:20px 25px;");
    assert_ne!(golden, tampered, "tamper string must exist in the golden");
    assert!(
        mjmx_html_compare::compare(golden, &tampered).is_err(),
        "a wrong style declaration in the golden must be detected"
    );

    // ...yet the comparator stays tolerant of pure inter-tag whitespace.
    assert!(
        mjmx_html_compare::compare("<a>\n  <b>x</b>\n</a>", "<a><b>x</b></a>").is_ok(),
        "pure inter-tag whitespace must be ignored"
    );
}
