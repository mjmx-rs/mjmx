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

/// A Tier 2 **container**: emits its own wrapper `<div>`, narrows the width its columns
/// size against, and renders its `mj-column` children through mjmx dispatch. No `<td>`
/// cell is wrapped around it (unlike a Tier 1 leaf).
struct Grid;

impl CustomComponent for Grid {
    fn tag(&self) -> &str {
        "mj-grid"
    }

    fn is_container(&self) -> bool {
        true
    }

    fn render(&self, cx: &mut ComponentCx<'_, '_>) -> Result<(), Diagnostic> {
        cx.write(&format!("<div class=\"grid\" data-w=\"{}\">", cx.container_width()));
        cx.set_container_width(300);
        cx.render_children()?;
        cx.write("</div>");
        Ok(())
    }
}

#[test]
fn tier2_container_renders_its_children_without_a_cell() {
    let registry = ComponentRegistry::new().with(Grid);
    let source = "<mjml><mj-body><mj-grid>\
        <mj-column><mj-text>a</mj-text></mj-column>\
        <mj-column><mj-text>b</mj-text></mj-column>\
        </mj-grid></mj-body></mjml>";
    let html = render_with_components(source, &RenderOptions::default(), &registry)
        .expect("registered container should render");

    // The container's own wrapper is emitted verbatim — no `<td>` cell around it.
    assert!(html.contains(r#"<div class="grid" data-w="600">"#), "{html}");
    // The Tier 1 leaf-cell signature must NOT appear around the container.
    assert!(
        !html.contains(r#"word-break:break-word;"><div class="grid""#),
        "container must not be wrapped in the leaf cell: {html}"
    );
    // Both children rendered through normal dispatch (their text is present).
    assert!(html.contains(">a</") && html.contains(">b</"), "{html}");
    // Two column divs rendered inside the grid.
    assert!(html.matches("mj-outlook-group-fix").count() >= 2, "{html}");
}

#[test]
fn tier2_container_restores_width_for_a_sibling() {
    let registry = ComponentRegistry::new().with(Grid);
    // The grid narrows to 300 for its own children; a following section must still see the
    // body width (600), proving mjmx restores it after the container returns.
    let source = "<mjml><mj-body>\
        <mj-grid><mj-column><mj-text>a</mj-text></mj-column></mj-grid>\
        <mj-section><mj-column><mj-text>b</mj-text></mj-column></mj-section>\
        </mj-body></mjml>";
    let html = render_with_components(source, &RenderOptions::default(), &registry)
        .expect("renders");
    // The section's ghost table pins the restored 600px width.
    assert!(html.contains("width:600px;"), "sibling section must see 600px: {html}");
}

#[test]
fn an_unregistered_container_style_tag_is_still_the_clean_unknown_error() {
    // Nothing registered for `mj-grid`: the usual fatal unknown-component error, not a
    // container render.
    let err = render(
        "<mjml><mj-body><mj-grid></mj-grid></mj-body></mjml>",
        &RenderOptions::default(),
    )
    .unwrap_err();
    assert!(format!("{err:?}").contains("not a recognized MJML component"), "{err:?}");
}

#[test]
fn tier2_container_passes_strict_validation() {
    let registry = ComponentRegistry::new().with(Grid);
    let source = "<mjml><mj-body><mj-grid><mj-column><mj-text>a</mj-text></mj-column></mj-grid></mj-body></mjml>";
    let options = RenderOptions { validation_level: ValidationLevel::Strict, ..RenderOptions::default() };
    let result = render_with_components(source, &options, &registry);
    assert!(result.is_ok(), "registered container must pass strict validation: {result:?}");
}
