//! mjml 5.x behaviour, as selected by [`MjmlCompat::V5`].
//!
//! Every expected string here was read off `mjml@5.4.0`'s own output rather than
//! derived from its source, and the V4 counterpart is asserted alongside it so the
//! *difference* between the two releases is what the test pins down.
//!
//! The 4.18.0 golden fixtures in `tests/golden.rs` cover V4 in full; this file covers
//! only what 5.x changed.

use mjmx::{MjmlCompat, RenderOptions};

/// Render `source` under one mjml release.
fn render(source: &str, compat: MjmlCompat) -> String {
    let options = RenderOptions {
        compat,
        ..RenderOptions::default()
    };
    mjmx::render(source, &options).expect("fixture should render")
}

/// The `<body …>` open tag.
fn body_tag(html: &str) -> &str {
    let start = html
        .find("<body")
        .expect("a rendered document has a <body>");
    let end = html[start..].find('>').expect("the tag is closed") + start + 1;
    &html[start..end]
}

/// The wrapper `<div …>` open tag that carries the ARIA email role.
fn wrapper_div(html: &str) -> &str {
    let anchor = html
        .find("aria-roledescription")
        .expect("a rendered document has the ARIA wrapper");
    let start = html[..anchor].rfind("<div").expect("the wrapper is a div");
    let end = html[start..].find('>').expect("the tag is closed") + start + 1;
    &html[start..end]
}

const PLAIN: &str = r"<mjml><mj-body><mj-section><mj-column><mj-text>x</mj-text></mj-column></mj-section></mj-body></mjml>";

#[test]
fn v5_moves_the_body_style_onto_the_wrapper_div() {
    // mjml 4 left the div's `style` empty unless a background-color was set, and put it
    // before `role`. mjml 5 moved it to the end of the tag and fills it with the *whole*
    // body style, so the empty attribute no longer exists.
    assert_eq!(
        wrapper_div(&render(PLAIN, MjmlCompat::V4)),
        r#"<div aria-roledescription="email" style="" role="article" lang="und" dir="auto">"#
    );
    assert_eq!(
        wrapper_div(&render(PLAIN, MjmlCompat::V5)),
        r#"<div aria-roledescription="email" role="article" lang="und" dir="auto" style="word-spacing:normal;">"#
    );
}

#[test]
fn v5_keeps_the_two_spaces_after_the_body_tag_name() {
    // Upstream's template interpolates an `id` slot that is empty when unset, leaving
    // `<body  style=…>`. Reproduced rather than tidied, so output stays diffable
    // against mjml's own.
    assert_eq!(
        body_tag(&render(PLAIN, MjmlCompat::V5)),
        r#"<body  style="word-spacing:normal;">"#
    );
    assert_eq!(
        body_tag(&render(PLAIN, MjmlCompat::V4)),
        r#"<body style="word-spacing:normal;">"#
    );
}

#[test]
fn v5_moves_css_class_from_the_wrapper_div_to_the_body() {
    const WITH_CLASS: &str = r#"<mjml><mj-body css-class="cc"><mj-section><mj-column><mj-text>x</mj-text></mj-column></mj-section></mj-body></mjml>"#;

    let v4 = render(WITH_CLASS, MjmlCompat::V4);
    assert!(!body_tag(&v4).contains("class"), "{}", body_tag(&v4));
    assert!(wrapper_div(&v4).contains(r#"class="cc""#));

    let v5 = render(WITH_CLASS, MjmlCompat::V5);
    assert_eq!(
        body_tag(&v5),
        r#"<body  class="cc" style="word-spacing:normal;">"#
    );
    assert!(!wrapper_div(&v5).contains("class="), "{}", wrapper_div(&v5));
}

#[test]
fn v5_emits_the_mj_body_id_that_v4_has_no_attribute_for() {
    const WITH_ID: &str = r#"<mjml><mj-body id="hero"><mj-section><mj-column><mj-text>x</mj-text></mj-column></mj-section></mj-body></mjml>"#;

    assert_eq!(
        body_tag(&render(WITH_ID, MjmlCompat::V5)),
        r#"<body  id="hero" style="word-spacing:normal;">"#
    );
    // `id` is new on mj-body in 5.x; 4.18.0 does not declare it and drops it silently.
    assert_eq!(
        body_tag(&render(WITH_ID, MjmlCompat::V4)),
        r#"<body style="word-spacing:normal;">"#
    );
}

#[test]
fn v5_repeats_the_background_color_on_both_body_and_wrapper() {
    const WITH_BG: &str = r##"<mjml><mj-body background-color="#eeeeee"><mj-section><mj-column><mj-text>x</mj-text></mj-column></mj-section></mj-body></mjml>"##;

    let v5 = render(WITH_BG, MjmlCompat::V5);
    assert_eq!(
        body_tag(&v5),
        r#"<body  style="word-spacing:normal;background-color:#eeeeee;">"#
    );
    // The div repeats the body style in full, rather than carrying the colour alone.
    assert_eq!(
        wrapper_div(&v5),
        r#"<div aria-roledescription="email" role="article" lang="und" dir="auto" style="word-spacing:normal;background-color:#eeeeee;">"#
    );

    let v4 = render(WITH_BG, MjmlCompat::V4);
    assert_eq!(
        wrapper_div(&v4),
        r#"<div aria-roledescription="email" style="background-color:#eeeeee;" role="article" lang="und" dir="auto">"#
    );
}

#[test]
fn v5_keeps_the_aria_label_a_title_contributes() {
    const WITH_TITLE: &str = r#"<mjml><mj-head><mj-title>T</mj-title></mj-head><mj-body><mj-section><mj-column><mj-text>x</mj-text></mj-column></mj-section></mj-body></mjml>"#;

    assert_eq!(
        wrapper_div(&render(WITH_TITLE, MjmlCompat::V5)),
        r#"<div aria-label="T" aria-roledescription="email" role="article" lang="und" dir="auto" style="word-spacing:normal;">"#
    );
}
