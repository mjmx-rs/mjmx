//! End-to-end minify/beautify tests over the vendored mjml goldens.
//!
//! These prove the two whitespace post-processors are *semantically invariant*
//! (via [`mjmx_html_compare`]) and that `minify` actually shrinks a real
//! document, without pinning any byte-exact output.

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

/// Every multi-line golden used here — a representative spread of components.
const FIXTURES: &[(&str, &str)] = &[
    ("slice", fixture!("slice.html")),
    ("twocol", fixture!("twocol.html")),
    ("button", fixture!("button.html")),
    ("social", fixture!("social.html")),
    ("navbar_hamburger", fixture!("navbar_hamburger.html")),
    ("carousel", fixture!("carousel.html")),
    ("accordion", fixture!("accordion.html")),
    ("hero", fixture!("hero.html")),
    ("group", fixture!("group.html")),
    ("raw", fixture!("raw.html")),
    ("head_font", fixture!("head_font.html")),
];

#[test]
fn minify_is_semantically_invariant() {
    for (name, html) in FIXTURES {
        if let Err(m) = mjmx_html_compare::compare(html, &mjmx_minifier::minify(html)) {
            panic!("{name}: minify changed semantics.\n{m}");
        }
    }
}

#[test]
fn beautify_is_semantically_invariant() {
    for (name, html) in FIXTURES {
        if let Err(m) = mjmx_html_compare::compare(html, &mjmx_minifier::beautify(html)) {
            panic!("{name}: beautify changed semantics.\n{m}");
        }
    }
}

#[test]
fn minify_shrinks_the_document() {
    for (name, html) in FIXTURES {
        let minified = mjmx_minifier::minify(html);
        assert!(
            minified.len() < html.len(),
            "{name}: minify did not shrink ({} -> {} bytes)",
            html.len(),
            minified.len()
        );
    }
}

#[test]
fn beautify_of_minify_round_trips() {
    // minify then beautify must still be semantically equal to the original.
    for (name, html) in FIXTURES {
        let round = mjmx_minifier::beautify(&mjmx_minifier::minify(html));
        if let Err(m) = mjmx_html_compare::compare(html, &round) {
            panic!("{name}: beautify(minify(x)) changed semantics.\n{m}");
        }
    }
}

#[test]
fn minify_core_is_idempotent() {
    // minify(beautify(x)) must be compare-equal to minify(x): beautify only moves
    // insignificant whitespace, which minify then strips back out.
    for (name, html) in FIXTURES {
        let via_beautify = mjmx_minifier::minify(&mjmx_minifier::beautify(html));
        let direct = mjmx_minifier::minify(html);
        if let Err(m) = mjmx_html_compare::compare(&direct, &via_beautify) {
            panic!("{name}: minify(beautify(x)) != minify(x).\n{m}");
        }
    }
}

#[test]
fn umbrella_render_honors_minify_and_beautify() {
    let mjml = fixture!("slice.mjml");

    // Pinned to V4. mjml 5 minifies CSS by default, and rewriting a `<style>` body
    // necessarily changes the text `mjmx_html_compare` sees there — so the semantic
    // equality asserted below is a V4 property, not a V5 one.
    // `v5_minifies_css_by_default` covers the other side.
    let base = || mjmx::RenderOptions {
        compat: mjmx::MjmlCompat::V4,
        minify_options: mjmx::MinifyOptions::default(),
        ..Default::default()
    };

    let default = mjmx::render(mjml, &base()).expect("slice renders");

    let minified = mjmx::render(
        mjml,
        &mjmx::RenderOptions {
            minify: true,
            ..base()
        },
    )
    .expect("slice renders (minify)");
    assert!(
        minified.len() < default.len(),
        "minify option should shrink output ({} -> {} bytes)",
        default.len(),
        minified.len()
    );
    mjmx_html_compare::assert_html_eq(&default, &minified);

    let beautified = mjmx::render(
        mjml,
        &mjmx::RenderOptions {
            beautify: true,
            ..base()
        },
    )
    .expect("slice renders (beautify)");
    mjmx_html_compare::assert_html_eq(&default, &beautified);

    // When both flags are set, minify wins.
    let both = mjmx::render(
        mjml,
        &mjmx::RenderOptions {
            minify: true,
            beautify: true,
            ..base()
        },
    )
    .expect("slice renders (both)");
    assert_eq!(both, minified, "minify must take precedence over beautify");
}

#[test]
fn v5_minifies_css_by_default_and_v4_does_not() {
    let mjml = fixture!("slice.mjml");
    let render = |compat| {
        let options = mjmx::RenderOptions {
            minify: true,
            minify_options: if compat == mjmx::MjmlCompat::V5 {
                mjmx::MinifyOptions::for_mjml5()
            } else {
                mjmx::MinifyOptions::default()
            },
            compat,
            ..Default::default()
        };
        mjmx::render(mjml, &options).expect("slice renders")
    };

    // mjml 5.4.0 emits `#outlook a{padding:0}`.
    let v5 = render(mjmx::MjmlCompat::V5);
    assert!(v5.contains("#outlook a{padding:0}"), "{v5}");

    // 4.18.0 leaves the `<style>` body exactly as rendered, newlines included.
    let v4 = render(mjmx::MjmlCompat::V4);
    assert!(v4.contains("#outlook a {\n"), "{v4}");
    assert!(v4.contains("padding: 0;\n"), "{v4}");
}
