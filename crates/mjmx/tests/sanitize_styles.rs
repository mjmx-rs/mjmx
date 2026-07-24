//! `sanitize_styles` / `allow_mixed_syntax` — mjml 5.4.0's template-variable
//! protection for the CSS-minify pass.
//!
//! Every expected behaviour here was checked against real `mjml@5.4.0`: with
//! `sanitizeStyles: true` mjml masks the variables across minification and restores
//! them; with it off (mjml's default) `cssnano` throws on the unparseable CSS. mjmx
//! turns that throw into an actionable error and, like mjml, leaves the default
//! (no-template-variable) render byte-identical.

use mjmx::{MjmlCompat, RenderOptions, render};

/// A document with a template variable in an `<mj-style>` value.
fn doc_with_value_var() -> &'static str {
    "<mjml><mj-head><mj-style>.x { color: {{brandColor}}; padding: {{pad}}px }</mj-style>\
     </mj-head><mj-body><mj-section><mj-column><mj-text>hi</mj-text></mj-column>\
     </mj-section></mj-body></mjml>"
}

fn minify_opts(compat: MjmlCompat) -> RenderOptions {
    RenderOptions {
        compat,
        minify: true,
        minify_options: if compat.is_v5() {
            mjmx::MinifyOptions::for_mjml5()
        } else {
            // V4 minifies CSS too here, so the corruption path is exercised in both.
            mjmx::MinifyOptions {
                minify_css: true,
                ..mjmx::MinifyOptions::default()
            }
        },
        ..RenderOptions::default()
    }
}

#[test]
fn minify_with_template_var_and_no_sanitize_is_an_error() {
    let err = render(doc_with_value_var(), &minify_opts(MjmlCompat::V5)).unwrap_err();
    let msg = err[0].message.clone();
    assert!(msg.contains("template variables in CSS"), "{msg}");
}

#[test]
fn sanitize_styles_preserves_the_variable_through_minify() {
    let options = RenderOptions {
        sanitize_styles: true,
        ..minify_opts(MjmlCompat::V5)
    };
    let html = render(doc_with_value_var(), &options).unwrap();
    // The variable survives, byte-for-byte as mjml@5.4.0 emits it (note `{{pad}} px`).
    assert!(html.contains("color:{{brandColor}}"), "{html}");
    assert!(html.contains("{{pad}} px"), "{html}");
}

#[test]
fn sanitize_styles_works_under_v4_too() {
    // The corruption exists in both compat modes, so the fix is available in both.
    let options = RenderOptions {
        sanitize_styles: true,
        ..minify_opts(MjmlCompat::V4)
    };
    let html = render(doc_with_value_var(), &options).unwrap();
    assert!(html.contains("{{brandColor}}"), "{html}");
}

#[test]
fn beautify_never_corrupts_so_it_needs_no_flag() {
    // mjmx's beautify emits `<style>` bodies verbatim, so template syntax is safe
    // without sanitize_styles — no error, and the variable is intact.
    let options = RenderOptions {
        beautify: true,
        ..RenderOptions::default()
    };
    let html = render(doc_with_value_var(), &options).unwrap();
    assert!(html.contains("{{brandColor}}"), "{html}");
}

#[test]
fn unbalanced_delimiters_are_a_clear_error() {
    let source = "<mjml><mj-head><mj-style>.x { color: {{oops }</mj-style></mj-head>\
                  <mj-body><mj-section><mj-column><mj-text>hi</mj-text></mj-column>\
                  </mj-section></mj-body></mjml>";
    let options = RenderOptions {
        sanitize_styles: true,
        ..minify_opts(MjmlCompat::V5)
    };
    let err = render(source, &options).unwrap_err();
    let msg = err[0].message.clone();
    assert!(msg.contains("unbalanced"), "{msg}");
}

#[test]
fn mixed_syntax_needs_allow_mixed() {
    let source = "<mjml><mj-head><mj-style>{{#if d}}.x{color:{{c}}}{{/if}}</mj-style>\
                  </mj-head><mj-body><mj-section><mj-column><mj-text>hi</mj-text>\
                  </mj-column></mj-section></mj-body></mjml>";
    let base = RenderOptions {
        sanitize_styles: true,
        ..minify_opts(MjmlCompat::V5)
    };
    let err = render(source, &base).unwrap_err();
    assert!(err[0].message.contains("mixed"), "{err:?}");

    let allowed = RenderOptions {
        allow_mixed_syntax: true,
        ..base
    };
    assert!(render(source, &allowed).is_ok());
}

#[test]
fn a_document_without_template_syntax_is_unaffected() {
    // The sanitize path must not touch an ordinary document: with the flag on but no
    // template variables present, output equals a plain minify.
    let source = "<mjml><mj-head><mj-style>.a { color: red }</mj-style></mj-head>\
                  <mj-body><mj-section><mj-column><mj-text>hi</mj-text></mj-column>\
                  </mj-section></mj-body></mjml>";
    let plain = render(source, &minify_opts(MjmlCompat::V5)).unwrap();
    let with_flag = render(
        source,
        &RenderOptions {
            sanitize_styles: true,
            ..minify_opts(MjmlCompat::V5)
        },
    )
    .unwrap();
    assert_eq!(plain, with_flag);
}

#[test]
fn custom_template_syntax_is_honoured() {
    let source = "<mjml><mj-head><mj-style>.x { color: <# brand #> }</mj-style></mj-head>\
                  <mj-body><mj-section><mj-column><mj-text>hi</mj-text></mj-column>\
                  </mj-section></mj-body></mjml>";
    let options = RenderOptions {
        sanitize_styles: true,
        template_syntax: Some(vec![("<#".into(), "#>".into())]),
        ..minify_opts(MjmlCompat::V5)
    };
    let html = render(source, &options).unwrap();
    assert!(html.contains("<# brand #>"), "{html}");
}
