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
fn section_full_width_bare_matches_mjml_reference() {
    // A bare valueless attribute (`<mj-section full-width>`) is NOT the same as
    // `full-width="full-width"`: mjml's htmlparser2 gives the bare form the value `""`
    // (convertBooleans only maps `"true"`/`"false"`), and `isFullWidth()` checks
    // `=== 'full-width'`, so the section renders as a normal (non-full-width) section.
    // mjmx stores a valueless attribute as absent, which fails the same equality check —
    // so the two agree. This fixture locks that agreement in.
    assert_golden(
        "section_full_width_bare",
        fixture!("section_full_width_bare.mjml"),
        fixture!("section_full_width_bare.html"),
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

// ---------------------------------------------------------------------------
// <mj-raw> in a scaffolding parent.
//
// mjml's `isRawElement()` branch: a raw child renders its content directly at its
// document position, exempt from the parent's row/cell scaffolding — inside `<tbody>`
// (column, hero) or between the opened-and-closed Outlook conditionals (section, group),
// never wrapped in a `<tr>` or a ghost `<td>`.

#[test]
fn mj_raw_in_column_matches_mjml_reference() {
    assert_golden(
        "mj_raw_in_column",
        fixture!("mj_raw_in_column.mjml"),
        fixture!("mj_raw_in_column.html"),
    );
}

#[test]
fn mj_raw_in_group_matches_mjml_reference() {
    assert_golden(
        "mj_raw_in_group",
        fixture!("mj_raw_in_group.mjml"),
        fixture!("mj_raw_in_group.html"),
    );
}

#[test]
fn mj_raw_in_section_matches_mjml_reference() {
    assert_golden(
        "mj_raw_in_section",
        fixture!("mj_raw_in_section.mjml"),
        fixture!("mj_raw_in_section.html"),
    );
}

#[test]
fn mj_raw_in_hero_matches_mjml_reference() {
    assert_golden(
        "mj_raw_in_hero",
        fixture!("mj_raw_in_hero.mjml"),
        fixture!("mj_raw_in_hero.html"),
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
// Both buckets are covered here. The inline-bucket fixtures were written first and run
// `#[ignore]`d as the specification of the css-inline milestone; `mjmx_css_inline` now
// implements it, so they run by default like every other golden test.

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
fn mj_style_inline_basic_matches_mjml_reference() {
    assert_golden(
        "mj_style_inline_basic",
        fixture!("mj_style_inline_basic.mjml"),
        fixture!("mj_style_inline_basic.html"),
    );
}

#[test]
fn mj_style_both_buckets_matches_mjml_reference() {
    assert_golden(
        "mj_style_both_buckets",
        fixture!("mj_style_both_buckets.mjml"),
        fixture!("mj_style_both_buckets.html"),
    );
}

#[test]
fn mj_style_important_matches_mjml_reference() {
    assert_golden(
        "mj_style_important",
        fixture!("mj_style_important.mjml"),
        fixture!("mj_style_important.html"),
    );
}

#[test]
fn mj_style_specificity_conflict_matches_mjml_reference() {
    assert_golden(
        "mj_style_specificity_conflict",
        fixture!("mj_style_specificity_conflict.mjml"),
        fixture!("mj_style_specificity_conflict.html"),
    );
}

#[test]
fn mj_style_specificity_order_matches_mjml_reference() {
    assert_golden(
        "mj_style_specificity_order",
        fixture!("mj_style_specificity_order.mjml"),
        fixture!("mj_style_specificity_order.html"),
    );
}

#[test]
fn mj_style_non_inlinable_matches_mjml_reference() {
    assert_golden(
        "mj_style_non_inlinable",
        fixture!("mj_style_non_inlinable.mjml"),
        fixture!("mj_style_non_inlinable.html"),
    );
}

#[test]
fn mj_style_dimensions_matches_mjml_reference() {
    assert_golden(
        "mj_style_dimensions",
        fixture!("mj_style_dimensions.mjml"),
        fixture!("mj_style_dimensions.html"),
    );
}

#[test]
fn mj_style_table_attrs_matches_mjml_reference() {
    assert_golden(
        "mj_style_table_attrs",
        fixture!("mj_style_table_attrs.mjml"),
        fixture!("mj_style_table_attrs.html"),
    );
}

#[test]
fn mj_style_combinators_matches_mjml_reference() {
    assert_golden(
        "mj_style_combinators",
        fixture!("mj_style_combinators.mjml"),
        fixture!("mj_style_combinators.html"),
    );
}

#[test]
fn mj_style_conditional_order_matches_mjml_reference() {
    assert_golden(
        "mj_style_conditional_order",
        fixture!("mj_style_conditional_order.mjml"),
        fixture!("mj_style_conditional_order.html"),
    );
}

#[test]
fn mj_style_templating_matches_mjml_reference() {
    assert_golden(
        "mj_style_templating",
        fixture!("mj_style_templating.mjml"),
        fixture!("mj_style_templating.html"),
    );
}

// ---------------------------------------------------------------------------
// <mj-html-attributes>
//
// mjml collects `(selector, name, value)` triples from `<mj-html-attributes>` and, in a
// cheerio pass over the assembled body (before juice), adds `name="value"` to every
// element the CSS `path` matches — overwriting an attribute that already exists, a no-op
// on no match, and never descending into Outlook conditional comments.

#[test]
fn mj_html_attributes_matches_mjml_reference() {
    assert_golden(
        "mj_html_attributes",
        fixture!("mj_html_attributes.mjml"),
        fixture!("mj_html_attributes.html"),
    );
}

#[test]
fn mj_html_attributes_edge_matches_mjml_reference() {
    // Overwrites `border="0"` in place, appends `data-x`, and the `.nomatchclass`
    // selector adds nothing (no match).
    assert_golden(
        "mj_html_attributes_edge",
        fixture!("mj_html_attributes_edge.mjml"),
        fixture!("mj_html_attributes_edge.html"),
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
