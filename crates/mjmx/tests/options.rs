//! Render-option parity tests: the four mjml options that codegen and the umbrella
//! crate honor — `keepComments`, `validationLevel`, `filePath` and `fonts` — each
//! exercised at both of its settings.

use std::path::PathBuf;

use mjmx::{FontFace, RenderOptions, Severity, ValidationLevel};

/// The `fixtures/` directory of the workspace.
fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

// ---- keepComments -------------------------------------------------------

/// A comment at every body-side nesting level plus one inside an ending tag.
const COMMENTED: &str = r#"<mjml><mj-body>
    <!-- body level -->
    <mj-wrapper>
      <!-- wrapper level -->
      <mj-section>
        <!-- section level -->
        <mj-column>
          <!-- column level -->
          <mj-text>Hi <!-- inside mj-text --></mj-text>
        </mj-column>
      </mj-section>
    </mj-wrapper>
  </mj-body></mjml>"#;

#[test]
fn comments_are_kept_by_default() {
    // mjml's own default is `keepComments: true`, so a bare render keeps them.
    assert!(RenderOptions::default().keep_comments);
    let html = mjmx::render_default(COMMENTED).expect("renders");
    for comment in [
        "<!-- body level -->",
        "<!-- wrapper level -->",
        "<!-- section level -->",
        "<!-- column level -->",
    ] {
        assert!(html.contains(comment), "missing {comment}");
    }
}

#[test]
fn keep_comments_false_drops_them() {
    let options = RenderOptions {
        keep_comments: false,
        ..RenderOptions::default()
    };
    let html = mjmx::render(COMMENTED, &options).expect("renders");
    for comment in [
        "<!-- body level -->",
        "<!-- wrapper level -->",
        "<!-- section level -->",
        "<!-- column level -->",
    ] {
        assert!(!html.contains(comment), "still present: {comment}");
    }
    // A comment inside an ending tag is part of that tag's raw body — mjml captures
    // that body as a source substring, so it survives either setting.
    assert!(html.contains("<!-- inside mj-text -->"));
}

/// Assert no *downlevel-hidden* conditional (`<!--[if …]> … <![endif]-->`) contains a
/// stray `-->`.
///
/// HTML comments do not nest, so a `-->` inside an open conditional terminates it early
/// and leaks the Outlook-only markup that followed into every client. A kept comment
/// emitted inside a ghost row is exactly how that happens. The *downlevel-revealed*
/// form (`<!--[if !mso]><!-->` … `<!--<![endif]-->`) is deliberately closed early and is
/// skipped by this scan.
fn assert_conditionals_intact(name: &str, html: &str) {
    let mut rest = html;
    while let Some(at) = rest.find("<!--[if") {
        let block = &rest[at..];
        let body = &block[block.find("]>").expect("conditional is terminated") + 2..];
        if let Some(revealed) = body.strip_prefix("<!-->") {
            // Downlevel-revealed: already closed, its content is ordinary markup.
            rest = revealed;
            continue;
        }
        let end = body.find("<![endif]-->").unwrap_or_else(|| {
            panic!(
                "{name}: unclosed conditional: {}",
                &block[..80.min(block.len())]
            )
        });
        assert!(
            !body[..end].contains("-->"),
            "{name}: a `-->` inside an open conditional truncates it:\n{}",
            &block[..(end + 12).min(block.len())]
        );
        rest = &body[end + "<![endif]-->".len()..];
    }
}

/// A comment as a direct child of every container that renders comments.
const COMMENT_PER_CONTAINER: &[(&str, &str)] = &[
    (
        "body",
        "<mjml><mj-body><!-- c --><mj-section><mj-column><mj-text>x</mj-text></mj-column></mj-section></mj-body></mjml>",
    ),
    (
        "section",
        "<mjml><mj-body><mj-section><!-- c --><mj-column><mj-text>x</mj-text></mj-column></mj-section></mj-body></mjml>",
    ),
    (
        "section-between-columns",
        "<mjml><mj-body><mj-section><mj-column><mj-text>x</mj-text></mj-column><!-- c --><mj-column><mj-text>y</mj-text></mj-column></mj-section></mj-body></mjml>",
    ),
    (
        "column",
        "<mjml><mj-body><mj-section><mj-column><!-- c --><mj-text>x</mj-text></mj-column></mj-section></mj-body></mjml>",
    ),
    (
        "column-gutter",
        "<mjml><mj-body><mj-section><mj-column padding=\"10px\"><!-- c --><mj-text>x</mj-text></mj-column></mj-section></mj-body></mjml>",
    ),
    (
        "wrapper",
        "<mjml><mj-body><mj-wrapper><!-- c --><mj-section><mj-column><mj-text>x</mj-text></mj-column></mj-section></mj-wrapper></mj-body></mjml>",
    ),
    (
        "group",
        "<mjml><mj-body><mj-section><mj-group><!-- c --><mj-column><mj-text>x</mj-text></mj-column></mj-group></mj-section></mj-body></mjml>",
    ),
    (
        "hero",
        "<mjml><mj-body><mj-hero><!-- c --><mj-text>x</mj-text></mj-hero></mj-body></mjml>",
    ),
];

#[test]
fn a_comment_never_breaks_an_outlook_conditional() {
    for (name, source) in COMMENT_PER_CONTAINER {
        let html = mjmx::render_default(source).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert!(html.contains("<!-- c -->"), "{name}: comment was dropped");
        assert_conditionals_intact(name, &html);
    }
}

#[test]
fn a_kept_comment_is_not_wrapped_in_layout_markup() {
    // mjml turns a comment into an implicit <mj-raw>, and raw children skip the
    // row/cell scaffolding a column wraps its real children in.
    let html = mjmx::render_default(COMMENTED).expect("renders");
    assert!(!html.contains("<tr><!-- column level --></tr>"));
}

// ---- validationLevel ----------------------------------------------------

/// `<mj-text>` directly inside `<mj-section>` — a structural error for the linter,
/// which codegen nonetheless renders through (the section ignores non-layout children).
const INVALID: &str =
    "<mjml><mj-body><mj-section><mj-text>orphan</mj-text></mj-section></mj-body></mjml>";

#[test]
fn soft_validation_renders_and_surfaces_diagnostics() {
    assert_eq!(
        RenderOptions::default().validation_level,
        ValidationLevel::Soft
    );
    let (html, diagnostics) =
        mjmx::render_with_diagnostics(INVALID, &RenderOptions::default()).expect("renders anyway");
    assert!(html.starts_with("<!doctype html>"));
    assert!(
        diagnostics.iter().any(|d| d.severity == Severity::Error),
        "soft validation should still report the error"
    );
}

#[test]
fn strict_validation_fails_the_render() {
    let options = RenderOptions {
        validation_level: ValidationLevel::Strict,
        ..RenderOptions::default()
    };
    let diagnostics = mjmx::render(INVALID, &options).expect_err("strict must fail");
    assert!(diagnostics.iter().any(|d| d.severity == Severity::Error));

    // A valid document is unaffected by strict.
    let valid = "<mjml><mj-body><mj-section><mj-column><mj-text>ok</mj-text></mj-column></mj-section></mj-body></mjml>";
    assert!(mjmx::render(valid, &options).is_ok());
}

#[test]
fn skip_validation_reports_nothing() {
    let options = RenderOptions {
        validation_level: ValidationLevel::Skip,
        ..RenderOptions::default()
    };
    let (html, diagnostics) =
        mjmx::render_with_diagnostics(INVALID, &options).expect("renders anyway");
    assert!(html.starts_with("<!doctype html>"));
    assert!(diagnostics.is_empty(), "skip must not validate at all");
}

// ---- filePath -----------------------------------------------------------

const WITH_INCLUDE: &str =
    "<mjml><mj-body><mj-include path=\"./_section.mjml\" /></mj-body></mjml>";

#[test]
fn file_path_gives_a_string_render_an_include_base() {
    let options = RenderOptions {
        file_path: Some(fixtures().join("include")),
        ..RenderOptions::default()
    };
    let html = mjmx::render(WITH_INCLUDE, &options).expect("renders");
    assert!(html.contains("Included section content."));
}

#[test]
fn file_path_pointing_at_a_file_uses_its_parent_directory() {
    let options = RenderOptions {
        file_path: Some(fixtures().join("include/main.mjml")),
        ..RenderOptions::default()
    };
    let html = mjmx::render(WITH_INCLUDE, &options).expect("renders");
    assert!(html.contains("Included section content."));
}

#[test]
fn no_file_path_resolves_no_includes() {
    assert!(RenderOptions::default().file_path.is_none());
    // Unchanged pre-option behavior: with no base the `<mj-include>` survives into
    // codegen, which has no renderer for it.
    let err = mjmx::render_default(WITH_INCLUDE).expect_err("nothing resolves the include");
    assert!(err.iter().any(|d| d.message.contains("mj-include")));
}

#[test]
fn file_path_overrides_the_directory_derived_from_the_input_file() {
    let main = fixtures().join("include/main.mjml");
    // Pointing the base at `fixtures/` (where `_section.mjml` does not live) makes the
    // include unresolvable, proving the explicit base won over the file's own parent.
    let options = RenderOptions {
        file_path: Some(fixtures()),
        ..RenderOptions::default()
    };
    let err = mjmx::render_file(&main, &options).expect_err("include must not resolve");
    assert!(err.iter().any(|d| d.severity == Severity::Error));

    // With the default options the file's own directory is the base and it resolves.
    let html = mjmx::render_file(&main, &RenderOptions::default()).expect("renders");
    assert!(html.contains("Included section content."));
}

// ---- fonts --------------------------------------------------------------

const USES_FONTS: &str = "<mjml><mj-body><mj-section><mj-column>\
     <mj-text font-family=\"Ubuntu, Comic Sans\">hi</mj-text>\
     </mj-column></mj-section></mj-body></mjml>";

#[test]
fn default_fonts_import_the_built_in_table() {
    assert!(RenderOptions::default().fonts.is_none());
    let html = mjmx::render_default(USES_FONTS).expect("renders");
    assert!(html.contains("https://fonts.googleapis.com/css?family=Ubuntu:300,400,500,700"));
}

#[test]
fn a_supplied_font_table_replaces_the_built_ins() {
    let options = RenderOptions {
        fonts: Some(vec![FontFace::new(
            "Comic Sans",
            "https://fonts.test/comic-sans",
        )]),
        ..RenderOptions::default()
    };
    let html = mjmx::render(USES_FONTS, &options).expect("renders");
    // The supplied face is imported because the body references it …
    assert!(html.contains("https://fonts.test/comic-sans"));
    // … and the built-in Ubuntu is gone even though the body references *it* too:
    // mjml's `fonts` option replaces the default map rather than merging into it.
    assert!(!html.contains("fonts.googleapis.com"));
}

#[test]
fn a_supplied_font_is_still_only_imported_when_referenced() {
    let options = RenderOptions {
        fonts: Some(vec![FontFace::new("Unused", "https://fonts.test/unused")]),
        ..RenderOptions::default()
    };
    let html = mjmx::render(USES_FONTS, &options).expect("renders");
    assert!(!html.contains("https://fonts.test/unused"));
}

#[test]
fn mj_font_still_extends_a_supplied_table() {
    let source = "<mjml><mj-head><mj-font name=\"Comic Sans\" href=\"https://fonts.test/from-head\" /></mj-head>\
         <mj-body><mj-section><mj-column><mj-text font-family=\"Comic Sans\">hi</mj-text>\
         </mj-column></mj-section></mj-body></mjml>";
    let options = RenderOptions {
        fonts: Some(vec![FontFace::new(
            "Comic Sans",
            "https://fonts.test/from-options",
        )]),
        ..RenderOptions::default()
    };
    let html = mjmx::render(source, &options).expect("renders");
    // <mj-font> overrides by name, exactly as it overrides a built-in.
    assert!(html.contains("https://fonts.test/from-head"));
    assert!(!html.contains("https://fonts.test/from-options"));
}
