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

// ---- component attributes (5.4) -----------------------------------------

const SOCIAL: &str = r#"<mjml><mj-body><mj-section><mj-column><mj-social><mj-social-element name="facebook">FB</mj-social-element><mj-social-element name="twitter" border="2px solid red">TW</mj-social-element></mj-social></mj-column></mj-section></mj-body></mjml>"#;

#[test]
fn v5_social_element_icon_carries_a_border() {
    // mjml 5.4 added `border` (default `0`) to the social icon `<img>`; 4.18 had none.
    let v5 = render(SOCIAL, MjmlCompat::V5);
    // Default: `border:0` prepended, byte-identical to mjml@5.4.0.
    assert!(
        v5.contains(r#"style="border:0;border-radius:3px;display:block;""#),
        "{v5}"
    );
    // A set `border` is applied verbatim.
    assert!(
        v5.contains(r#"style="border:2px solid red;border-radius:3px;display:block;""#),
        "{v5}"
    );

    // V4: the icon `<img>` has no `border`, and a set one is ignored.
    let v4 = render(SOCIAL, MjmlCompat::V4);
    assert!(
        v4.contains(r#"style="border-radius:3px;display:block;""#),
        "{v4}"
    );
    assert!(!v4.contains("border:2px solid red"), "{v4}");
}

const ACCORDION: &str = r#"<mjml><mj-body><mj-accordion><mj-accordion-element><mj-accordion-title>T</mj-accordion-title><mj-accordion-text>X</mj-accordion-text></mj-accordion-element></mj-accordion></mj-body></mjml>"#;

#[test]
fn v5_accordion_retargets_toggle_selectors_for_new_outlook() {
    // mjml 5.2.1+ swapped the `input.mj-accordion-checkbox` type selector for the
    // attribute selector `.mj-accordion-checkbox[type="checkbox"]`, and dropped the
    // `input` qualifier from the `:checked` rules. The `.moz-text-html` rules are unchanged.
    let v5 = render(ACCORDION, MjmlCompat::V5);
    assert!(
        v5.contains(r#".mj-accordion-checkbox[type="checkbox"]"#),
        "{v5}"
    );
    assert!(
        v5.contains(".mj-accordion-checkbox:checked+* .mj-accordion-content"),
        "{v5}"
    );
    // The type-selector rules replace the old `input.` ones inside the media query.
    assert!(!v5.contains("input.mj-accordion-checkbox:checked"), "{v5}");
    // The moz-text-html fallbacks keep `input.`.
    assert!(
        v5.contains(".moz-text-html input.mj-accordion-checkbox"),
        "{v5}"
    );

    // V4 keeps the original `input.`-qualified selectors.
    let v4 = render(ACCORDION, MjmlCompat::V4);
    assert!(v4.contains("input.mj-accordion-checkbox:checked"), "{v4}");
    assert!(
        !v4.contains(r#".mj-accordion-checkbox[type="checkbox"]"#),
        "{v4}"
    );
}

/// A two-column section with `gutter="20px"` (mjml 5's column-layout rework). The 480px
/// container is unaffected; the section content box is 600px, so a 20px gutter is
/// `20/600*100 = 3.333333%`, each column's width drops from `50%` to `48.333333%`, and a
/// per-position `mj-column-gutter-…` padding rule appears. mjml 4.18 has no `gutter`
/// attribute at all, so V4 renders a plain two-column section.
const GUTTER: &str = r#"<mjml><mj-body><mj-section gutter="20px"><mj-column><mj-text>a</mj-text></mj-column><mj-column><mj-text>b</mj-text></mj-column></mj-section></mj-body></mjml>"#;

#[test]
fn v5_section_gutter_reduces_column_width_and_adds_padding_rules() {
    let v5 = render(GUTTER, MjmlCompat::V5);
    // The width class keys off the gutter-reduced width, not the raw 50%.
    assert!(v5.contains("mj-column-per-48-333333"), "{v5}");
    assert!(!v5.contains("mj-column-per-50"), "{v5}");
    // Each column registers a per-position gutter padding rule and wears its class.
    assert!(v5.contains("mj-column-gutter-2-1-per-3-333333"), "{v5}");
    assert!(v5.contains("mj-column-gutter-2-2-per-3-333333"), "{v5}");
    // First column: padding only on the trailing (right) edge; last: only leading (left).
    assert!(
        v5.contains("padding: 0% 1.666667% 0% 0% !important"),
        "{v5}"
    );
    assert!(
        v5.contains("padding: 0% 0% 0% 1.666667% !important"),
        "{v5}"
    );
    // The Outlook ghost cell keeps the full 300px width but gains px gutter padding.
    assert!(v5.contains("padding:0px 10px 0px 0px;"), "{v5}");
}

#[test]
fn v4_ignores_the_section_gutter_attribute() {
    // The whole feature is v5-gated: V4 renders exactly as if `gutter` were absent.
    let v4 = render(GUTTER, MjmlCompat::V4);
    assert!(v4.contains("mj-column-per-50"), "{v4}");
    assert!(!v4.contains("mj-column-gutter"), "{v4}");
    assert!(!v4.contains("48-333333"), "{v4}");

    let plain = r#"<mjml><mj-body><mj-section><mj-column><mj-text>a</mj-text></mj-column><mj-column><mj-text>b</mj-text></mj-column></mj-section></mj-body></mjml>"#;
    assert_eq!(
        render(GUTTER, MjmlCompat::V4),
        render(plain, MjmlCompat::V4)
    );
}

#[test]
fn v5_group_gutter_lands_inline_without_a_media_query() {
    // A grouped column carries the gutter as inline `<div>` padding (group columns never
    // stack), and — unlike a section column — registers no per-position padding rule.
    let grouped = r#"<mjml><mj-body><mj-section gutter="20px"><mj-group><mj-column><mj-text>a</mj-text></mj-column><mj-column><mj-text>b</mj-text></mj-column></mj-group></mj-section></mj-body></mjml>"#;
    let v5 = render(grouped, MjmlCompat::V5);
    // The gutter class is on the div…
    assert!(v5.contains("mj-column-gutter-2-1-per-3-333333"), "{v5}");
    // …but there is no `mj-column-gutter-…{ padding }` media-query rule for it.
    assert!(
        !v5.contains("mj-column-gutter-2-1-per-3-333333 {"),
        "group gutter must not register a media query: {v5}"
    );
}

#[test]
fn css_variables_resolve_end_to_end_in_both_compat_modes() {
    // `resolveCSSVariables` is plain-juice behaviour, present in both mjml releases, so
    // it applies under V4 and V5 alike (not gated on compat). Threaded from the default
    // `InlineOptions` through `RenderOptions`.
    const DOC: &str = r#"<mjml><mj-head><mj-style inline="inline">.v{--c:#ff0000;color:var(--c)}</mj-style></mj-head><mj-body><mj-section><mj-column><mj-text css-class="v">x</mj-text></mj-column></mj-section></mj-body></mjml>"#;
    for compat in [MjmlCompat::V4, MjmlCompat::V5] {
        let out = render(DOC, compat);
        assert!(out.contains("color: #ff0000"), "{compat:?}: {out}");
        assert!(
            !out.contains("--c"),
            "{compat:?}: custom prop must drop: {out}"
        );
    }
}

#[test]
fn inline_pseudo_elements_thread_through_render_options() {
    // Opt-in (juice defaults it off, so does mjmx): a `::before` in an inline `<mj-style>`
    // synthesizes a span only when `inline_pseudo_elements` is set. Compat-independent.
    use mjmx::InlineOptions;
    const DOC: &str = r#"<mjml><mj-head><mj-style inline="inline">.v::before{content:"* "}</mj-style></mj-head><mj-body><mj-section><mj-column><mj-text css-class="v">hi</mj-text></mj-column></mj-section></mj-body></mjml>"#;

    let off = mjmx::render(DOC, &RenderOptions::default()).expect("renders");
    assert!(
        !off.contains("<span>* </span>"),
        "default drops the pseudo: {off}"
    );

    let on = mjmx::render(
        DOC,
        &RenderOptions {
            inline_options: InlineOptions {
                inline_pseudo_elements: true,
                ..InlineOptions::default()
            },
            ..RenderOptions::default()
        },
    )
    .expect("renders");
    assert!(
        on.contains("<span>* </span>"),
        "opt-in synthesizes the pseudo: {on}"
    );
}

/// A fractional column width, so the ghost cell's pixel value is not a whole number:
/// 58.33% and 41.67% of a 552px content box are 321.9816px and 230.0184px.
const FRACTIONAL_WIDTHS: &str = r#"<mjml><mj-body><mj-section padding="24px"><mj-column width="58.33%"><mj-text>a</mj-text></mj-column><mj-column width="41.67%"><mj-text>b</mj-text></mj-column></mj-section></mj-body></mjml>"#;

#[test]
fn v5_rounds_the_outlook_ghost_width_to_a_whole_pixel() {
    // mjml 5.4.0 wraps `getWidthAsPixel` in `normalizePxValue` (`Math.round`); 4.18.0
    // emits the raw quotient. Read off both releases' output.
    let v5 = render(FRACTIONAL_WIDTHS, MjmlCompat::V5);
    assert!(v5.contains("width:322px;"), "{v5}");
    assert!(v5.contains("width:230px;"), "{v5}");
    assert!(
        !v5.contains("321.9816"),
        "no unrounded width survives: {v5}"
    );
    assert!(
        !v5.contains("230.0184"),
        "no unrounded width survives: {v5}"
    );

    let v4 = render(FRACTIONAL_WIDTHS, MjmlCompat::V4);
    assert!(v4.contains("width:321.9816px;"), "{v4}");
    assert!(v4.contains("width:230.0184px;"), "{v4}");
}

#[test]
fn v5_rounds_only_the_emitted_width_not_the_child_container() {
    // mjml 5.4.0's `getChildContext` recomputes from `parsedWidth` *without*
    // `normalizePxValue`, so children are laid out against the unrounded width in both
    // releases — the rounding must not leak past the ghost cell.
    //
    // An `mj-image` with no width of its own reports the container width it inherited:
    // 321.9816 - 50 (its own 10px/25px padding) floors to 271, and 230.0184 - 50 to 180.
    // Both `mjml@4.18.0` and `mjml@5.4.0` emit exactly those two, despite disagreeing on
    // the ghost width above.
    let images = r#"<mjml><mj-body><mj-section padding="24px"><mj-column width="58.33%"><mj-image src="a.png" /></mj-column><mj-column width="41.67%"><mj-image src="b.png" /></mj-column></mj-section></mj-body></mjml>"#;
    let v5 = render(images, MjmlCompat::V5);
    let v4 = render(images, MjmlCompat::V4);

    for html in [&v4, &v5] {
        assert!(
            html.contains(r#"width="271""#),
            "child width unrounded: {html}"
        );
        assert!(
            html.contains(r#"width="180""#),
            "child width unrounded: {html}"
        );
    }

    // ...while the ghost cells still differ, which is the whole point.
    assert!(v5.contains("width:322px;"), "{v5}");
    assert!(v4.contains("width:321.9816px;"), "{v4}");

    // The rounding must not have reached the column div's own percentage class either.
    assert!(v5.contains("mj-column-per-58-33"), "{v5}");
    assert!(v4.contains("mj-column-per-58-33"), "{v4}");
}
