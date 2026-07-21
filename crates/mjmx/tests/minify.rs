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

    let default = mjmx::render_default(mjml).expect("slice renders");

    let minified = mjmx::render(
        mjml,
        &mjmx::RenderOptions {
            minify: true,
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
        },
    )
    .expect("slice renders (both)");
    assert_eq!(both, minified, "minify must take precedence over beautify");
}
