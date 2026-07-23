//! Tier 1 native custom components: a host registers a renderer for an otherwise-unknown
//! tag, and it participates in the normal render (attribute cascade, head contributions)
//! under the "registered → renders, else the clean unknown-component error" rule.

use mjmx::{
    ComponentCx, ComponentRegistry, CustomComponent, Diagnostic, RenderOptions, ValidationLevel,
    render, render_with_components,
};

/// A leaf component that echoes its resolved `label` attribute into a `<span>`.
struct Badge;

impl CustomComponent for Badge {
    fn tag(&self) -> &str {
        "mj-badge"
    }

    fn render(&self, cx: &mut ComponentCx<'_, '_>) -> Result<(), Diagnostic> {
        let label = cx.attr_or("label", "");
        cx.write(&format!("<span class=\"badge\">{label}</span>"));
        Ok(())
    }
}

/// A component that exercises the `<head>` contribution channels and `raw_inner`.
struct Fancy;

impl CustomComponent for Fancy {
    fn tag(&self) -> &str {
        "mj-fancy"
    }

    fn render(&self, cx: &mut ComponentCx<'_, '_>) -> Result<(), Diagnostic> {
        cx.add_style(".fancy{color:red}");
        cx.add_font("Comic", "https://fonts.example/comic.css");
        let inner = cx.raw_inner();
        // `font-family:Comic` makes the family referenced, so the font is emitted.
        cx.write(&format!(
            "<div class=\"fancy\" style=\"font-family:Comic\">{inner}</div>"
        ));
        Ok(())
    }
}

/// Wrap body markup in the minimal document scaffolding.
fn doc(inner: &str) -> String {
    format!(
        "<mjml><mj-body><mj-section><mj-column>{inner}</mj-column></mj-section></mj-body></mjml>"
    )
}

#[test]
fn registered_component_renders() {
    let registry = ComponentRegistry::new().with(Badge);
    let html = render_with_components(
        &doc("<mj-badge label=\"Hi\" />"),
        &RenderOptions::default(),
        &registry,
    )
    .expect("registered custom component should render");
    // mjmx wraps the returned fragment in the standard leaf <td> cell (the author returns
    // content, not a cell), so the table markup stays valid. The badge sets no cell
    // attributes, so the cell is the bare mjml default.
    assert!(
        html.contains(
            "<td style=\"font-size:0px;word-break:break-word;\"><span class=\"badge\">Hi</span></td>"
        ),
        "expected the badge wrapped in the standard cell in:\n{html}"
    );
}

#[test]
fn unregistered_component_is_a_clean_error() {
    // No registry: the tag stays unknown and rendering fails with the usual message.
    let err = render(&doc("<mj-badge label=\"Hi\" />"), &RenderOptions::default())
        .expect_err("an unregistered custom tag must be rejected");
    assert!(
        err.iter()
            .any(|d| d.message.contains("not a recognized MJML component")),
        "expected the unknown-component error, got: {err:?}"
    );
}

#[test]
fn registered_component_survives_strict_validation() {
    // Strict validation flags unknown tags fatally — but a registered one is known.
    let registry = ComponentRegistry::new().with(Badge);
    let options = RenderOptions {
        validation_level: ValidationLevel::Strict,
        ..RenderOptions::default()
    };
    let html = render_with_components(&doc("<mj-badge label=\"Ok\" />"), &options, &registry)
        .expect("a registered tag must not be a fatal unknown under strict validation");
    assert!(html.contains("<span class=\"badge\">Ok</span>"));
}

#[test]
fn custom_cell_carries_resolved_attributes() {
    // The standard leaf cell reads align / css-class / container-background-color / padding
    // from the component's own (cascade-resolved) attributes, in mjml's key order:
    // align and class as attributes, then a style block of background, font-size:0px,
    // padding, word-break.
    let registry = ComponentRegistry::new().with(Badge);
    let html = render_with_components(
        &doc("<mj-badge label=\"Hi\" align=\"center\" padding=\"5px\" \
             container-background-color=\"#eee\" css-class=\"promo\" />"),
        &RenderOptions::default(),
        &registry,
    )
    .expect("render");
    assert!(
        html.contains(
            "<td align=\"center\" class=\"promo\" \
             style=\"background:#eee;font-size:0px;padding:5px;word-break:break-word;\">\
             <span class=\"badge\">Hi</span></td>"
        ),
        "expected the cell to carry the resolved attributes in mjml order in:\n{html}"
    );
}

#[test]
fn misplaced_custom_component_is_flagged() {
    // A Tier 1 component is body-only: codegen dispatches it only while rendering the
    // body, so a placement in <mj-head> would silently render nothing. The linter flags
    // it, so under strict validation the host mistake is a fatal diagnostic (not a quiet
    // no-op) — keeping lint at least as strict as render.
    let registry = ComponentRegistry::new().with(Badge);
    let options = RenderOptions {
        validation_level: ValidationLevel::Strict,
        ..RenderOptions::default()
    };
    let src = "<mjml><mj-head><mj-badge label=\"nope\" /></mj-head><mj-body></mj-body></mjml>";
    let err = render_with_components(src, &options, &registry)
        .expect_err("a custom component in <mj-head> must be flagged");
    assert!(
        err.iter()
            .any(|d| d.message.contains("only valid inside the body")),
        "expected the misplacement error, got: {err:?}"
    );
}

#[test]
fn head_contributions_and_raw_inner() {
    let registry = ComponentRegistry::new().with(Fancy);
    let html = render_with_components(
        &doc("<mj-fancy>inner text</mj-fancy>"),
        &RenderOptions::default(),
        &registry,
    )
    .expect("render");

    // add_style → a <style> block in the head carries the rule.
    assert!(
        html.contains(".fancy{color:red}"),
        "expected the contributed style in:\n{html}"
    );
    // raw_inner → the verbatim inner text is echoed.
    assert!(html.contains("<div class=\"fancy\" style=\"font-family:Comic\">inner text</div>"));
    // add_font → the referenced family emits a <link> and @import in the head.
    assert!(
        html.contains("<link href=\"https://fonts.example/comic.css\" rel=\"stylesheet\""),
        "expected the contributed font <link> in:\n{html}"
    );
    assert!(html.contains("@import url(https://fonts.example/comic.css);"));
}
