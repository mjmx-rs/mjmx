//! End-to-end checks for `mjmx::migrate`: v3 source in, v4 source out, and the v4
//! source must be something this renderer actually accepts.
//!
//! The migrated text in `EXPECTED` was cross-checked against `npx mjml@4.18.0 -m`
//! (mjml's own `--migrate`): mjml's output, run through `mjmx fmt` to normalize
//! formatting — which is ours to choose — is byte-identical to this. No test here
//! shells out, so the suite stays offline and fast.

/// A v3 document exercising every migration rule that has a v4 equivalent.
const V3: &str = r##"<mjml>
  <mj-body>
    <mj-container background-color="#f5f5f5">
      <mj-section padding="10">
        <mj-column>
          <mj-text font-size="13" padding="20">Hello v3</mj-text>
          <mj-social display="facebook twitter" facebook-content="Share" twitter-icon-color="#00acee" icon-size="30" />
        </mj-column>
      </mj-section>
      <mj-hero mode="fixed-height" height="200">
        <mj-hero-content background-color="#ffffff" padding="15">
          <mj-text>Hero body</mj-text>
        </mj-hero-content>
      </mj-hero>
    </mj-container>
  </mj-body>
</mjml>
"##;

/// What `mjml --migrate` produces for [`V3`], in this project's canonical format.
const EXPECTED: &str = r##"<mjml>
  <mj-body background-color="#f5f5f5">
    <mj-section padding="10px">
      <mj-column>
        <mj-text font-size="13px" padding="20px">Hello v3</mj-text>
        <mj-social icon-size="30px">
          <mj-social-element name="facebook">Share</mj-social-element>
          <mj-social-element name="twitter" icon-color="#00acee" />
        </mj-social>
      </mj-column>
    </mj-section>
    <mj-hero mode="fixed-height" height="200px" background-color="#ffffff" padding="15px">
      <mj-text>Hero body</mj-text>
    </mj-hero>
  </mj-body>
</mjml>
"##;

#[test]
fn migrate_matches_the_mjml_migrate_oracle() {
    let migrated = mjmx::migrate(V3).expect("v3 sample should migrate");
    assert_eq!(migrated, EXPECTED);
}

#[test]
fn migrated_source_renders() {
    let migrated = mjmx::migrate(V3).unwrap();
    let html = mjmx::render_default(&migrated).expect("migrated MJML must be renderable v4");
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("Hello v3"));
    assert!(html.contains("Hero body"));
    // The social element survived the split and rendered as a real icon.
    assert!(html.contains("facebook.png"));
}

#[test]
fn migration_is_reported() {
    let (_migrated, notes) = mjmx::migrate_with_notes(V3, &mjmx::FormatOptions::default()).unwrap();
    for expected in ["mj-container", "mj-hero-content", "mj-social", "10px"] {
        assert!(
            notes.iter().any(|n| n.message.contains(expected)),
            "no note mentioned {expected}: {notes:?}"
        );
    }
}

#[test]
fn migrating_a_v4_document_leaves_it_alone() {
    let v4 = "<mjml>\n  <mj-body>\n    <mj-section>\n      <mj-column>\n        <mj-text>hi</mj-text>\n      </mj-column>\n    </mj-section>\n  </mj-body>\n</mjml>\n";
    let (migrated, notes) = mjmx::migrate_with_notes(v4, &mjmx::FormatOptions::default()).unwrap();
    assert_eq!(migrated, v4);
    assert!(notes.is_empty(), "{notes:?}");
}

#[test]
fn malformed_source_is_an_error_not_a_panic() {
    assert!(mjmx::migrate("<mjml><mj-body>").is_err());
}
