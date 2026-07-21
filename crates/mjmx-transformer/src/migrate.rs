//! The MJML **v3 → v4** migration pass.
//!
//! This is a faithful port of mjml's own `mjml-migrate` package (4.18.0,
//! `lib/migrate.js` + `lib/config.js`) — the thing behind `mjml --migrate` — so that
//! `mjmx migrate` and `mjml -m` agree on what a v3 document becomes. Like every pass
//! in this crate it is an AST → AST rebuild through [`AstBuilder`]; serializing the
//! result back to MJML source is `mjmx-formatter`'s job.
//!
//! # Rules
//!
//! | v3 | v4 |
//! | --- | --- |
//! | `<mj-body><mj-container a="1">…` | `<mj-body a="1">…` (container unwrapped, its attributes hoisted onto the body) |
//! | `<mj-navbar>` | `<mj-section full-width="full-width">` (the v3 navbar was the full-width wrapper) |
//! | `<mj-inline-links>` | `<mj-navbar>` |
//! | `<mj-link>` | `<mj-navbar-link>` |
//! | `<mj-hero><mj-hero-content b="2">…` | `<mj-hero b="2">…` (hero-content dissolved, its attributes merged in and winning ties) |
//! | `<mj-social display="facebook twitter" facebook-content="Share" twitter-icon-color="#0af">` | `<mj-social><mj-social-element name="facebook">Share</mj-social-element><mj-social-element name="twitter" icon-color="#0af"/></mj-social>` |
//! | `<mj-html>`, `<mj-invoice>`, `<mj-list>`, `<mj-location>` | dropped (no v4 equivalent) |
//! | unitless `padding="10"`, `font-size="13"`, … | `10px`, `13px` (for the attribute list mjml treats as lengths) |
//!
//! Every one of those produces a [`Diagnostic`] describing what happened; dropped
//! tags and a `<mj-hero>` that cannot be migrated are warnings, the rest are advice.
//!
//! # Faithfulness notes
//!
//! Two of mjml's behaviors are destructive and are reproduced anyway, because
//! matching `mjml --migrate` is the point:
//!
//! - unwrapping `<mj-container>` replaces the body's children *entirely*, so any
//!   sibling of the container is discarded;
//! - migrating `<mj-social>` replaces its children entirely and rebuilds them from
//!   the `display` attribute, so a document that already uses v4
//!   `<mj-social-element>` children loses them. Migration is meant to be run once,
//!   on a v3 document.
//!
//! The `<mj-navbar>` rule is reproduced even though its output is not valid v4:
//! mjml turns the v3 navbar wrapper into a bare `<mj-section>` whose child is an
//! `<mj-navbar>`, and mjml's own validator then rejects that (v4 wants the navbar
//! inside an `<mj-column>`). We emit the same tree — matching `mjml --migrate` is
//! the point — but raise a warning telling the author to add the column.
//!
//! One behavior is deliberately *not* reproduced: mjml throws on a `<mj-hero>` with
//! no `<mj-hero-content>` child. We leave the element alone and warn.

use mjmx_allocator::Vec as ArenaVec;
use mjmx_ast::{AstBuilder, Attribute, Document, Element, Node};
use mjmx_diagnostics::Diagnostic;
use mjmx_span::{Atom, Span};
use mjmx_syntax::MjmlTag;

/// Tags removed in v4 with no replacement (mjml-migrate's `unavailableTags`).
const UNAVAILABLE_TAGS: [&str; 4] = ["mj-html", "mj-invoice", "mj-list", "mj-location"];

/// Attributes whose bare-number v3 values gain a `px` unit in v4
/// (mjml-migrate's `attributesWithUnit`, de-duplicated).
const ATTRIBUTES_WITH_UNIT: [&str; 25] = [
    "background-size",
    "border-radius",
    "border-width",
    "cellpadding",
    "cellspacing",
    "font-size",
    "height",
    "icon-height",
    "ico-padding",
    "ico-padding-bottom",
    "ico-font-size",
    "ico-line-height",
    "ico-padding-left",
    "ico-padding-right",
    "ico-padding-top",
    "icon-size",
    "icon-width",
    "inner-padding",
    "letter-spacing",
    "padding",
    "padding-bottom",
    "padding-left",
    "padding-right",
    "tb-border-radius",
    "tb-width",
];

/// The networks a v3 `<mj-social>` renders when it has no `display` attribute.
const DEFAULT_SOCIAL_DISPLAY: &str = "facebook twitter google";

/// Migrate an MJML v3 `document` to v4, returning the rebuilt tree and a note for
/// every change made (plus warnings for anything that could not be migrated).
///
/// The pass is a pure AST rebuild: it does no I/O and never panics. Running it on a
/// document that is already v4 is *not* a no-op — see the module docs' faithfulness
/// notes — so migrate once, on v3 input.
#[must_use]
pub fn migrate<'a>(
    builder: &AstBuilder<'a>,
    document: &Document<'a>,
) -> (Document<'a>, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let children = migrate_nodes(builder, &document.children, &mut diagnostics);
    (builder.document(children, document.span), diagnostics)
}

/// An element after tag/attribute/child rewriting, before its children recurse.
struct Staged<'a> {
    tag: MjmlTag,
    raw_tag: Atom<'a>,
    attributes: Vec<Attribute<'a>>,
    children: Vec<Node<'a>>,
}

impl<'a> Staged<'a> {
    /// The element unchanged.
    fn identity(el: &Element<'a>) -> Self {
        Staged {
            tag: el.tag,
            raw_tag: el.raw_tag,
            attributes: el.attributes.iter().copied().collect(),
            children: el.children.iter().copied().collect(),
        }
    }
}

/// Migrate a child list, dropping tags that do not exist in v4.
fn migrate_nodes<'a>(
    builder: &AstBuilder<'a>,
    nodes: &[Node<'a>],
    diags: &mut Vec<Diagnostic>,
) -> ArenaVec<'a, Node<'a>> {
    let mut out = builder.vec();
    for node in nodes {
        match node {
            Node::Element(el) if UNAVAILABLE_TAGS.contains(&el.raw_tag.as_str()) => {
                diags.push(
                    Diagnostic::warning(format!(
                        "dropped <{}>: the tag was removed in MJML v4",
                        el.raw_tag.as_str()
                    ))
                    .with_label(el.span, "no v4 equivalent exists")
                    .with_help("rebuild this content with mj-text / mj-table / mj-raw"),
                );
            }
            Node::Element(el) => {
                out.push(builder.element_node(migrate_element(builder, el, diags)))
            }
            // Text and comment nodes are `Copy` leaves; carry them over unchanged.
            other => out.push(*other),
        }
    }
    out
}

/// Migrate one element: rewrite it, fix unitless lengths, then recurse.
fn migrate_element<'a>(
    builder: &AstBuilder<'a>,
    el: &Element<'a>,
    diags: &mut Vec<Diagnostic>,
) -> &'a Element<'a> {
    let mut staged = match el.raw_tag.as_str() {
        "mj-body" => unwrap_container(el, diags),
        "mj-social" => migrate_social(builder, el, diags),
        "mj-navbar" => {
            // mjml's own rule, warts included: it produces `<mj-section><mj-navbar>`,
            // which mjml's *validator* then rejects (mj-navbar belongs in a column).
            // We reproduce the rewrite and flag the hole rather than silently
            // diverging from `mjml --migrate`.
            diags.push(
                Diagnostic::warning(
                    "migrated: <mj-navbar> became <mj-section full-width>, which leaves \
                     the inner <mj-navbar> directly inside a section",
                )
                .with_label(el.span, "rewritten")
                .with_help(
                    "MJML v4 only allows mj-navbar inside mj-column / mj-hero: wrap the \
                     inner <mj-navbar> in an <mj-column> by hand (mjml --migrate has the \
                     same gap)",
                ),
            );
            rename(el, MjmlTag::MjSection, builder.atom("mj-section"))
                .tap(|s| s.attributes.push(flag(builder, "full-width", el.span)))
        }
        "mj-inline-links" => rename(el, MjmlTag::MjNavbar, builder.atom("mj-navbar")).note(
            diags,
            el.span,
            "<mj-inline-links> became <mj-navbar>",
        ),
        "mj-link" => rename(el, MjmlTag::MjNavbarLink, builder.atom("mj-navbar-link")).note(
            diags,
            el.span,
            "<mj-link> became <mj-navbar-link>",
        ),
        "mj-hero" => migrate_hero(el, diags),
        _ => Staged::identity(el),
    };

    add_missing_units(builder, &mut staged.attributes, diags, el.span);

    let children = migrate_nodes(builder, &staged.children, diags);
    builder.element(
        staged.tag,
        staged.raw_tag,
        builder.vec_from_iter(staged.attributes),
        children,
        el.span,
    )
}

impl<'a> Staged<'a> {
    /// Mutate in a builder chain.
    fn tap(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }

    /// Record an advice-level note about this rewrite.
    fn note(self, diags: &mut Vec<Diagnostic>, span: Span, message: &str) -> Self {
        diags
            .push(Diagnostic::advice(format!("migrated: {message}")).with_label(span, "rewritten"));
        self
    }
}

/// The same element under a different tag.
fn rename<'a>(el: &Element<'a>, tag: MjmlTag, raw_tag: Atom<'a>) -> Staged<'a> {
    Staged {
        tag,
        raw_tag,
        ..Staged::identity(el)
    }
}

/// A valueless-style flag attribute rendered as `name="name"`, matching mjml.
fn flag<'a>(builder: &AstBuilder<'a>, name: &str, span: Span) -> Attribute<'a> {
    builder.attribute(builder.atom(name), Some(builder.atom(name)), span)
}

/// `<mj-body><mj-container …>` → `<mj-body …>`: the container's attributes and
/// children replace the body's.
fn unwrap_container<'a>(el: &Element<'a>, diags: &mut Vec<Diagnostic>) -> Staged<'a> {
    let Some(container) = el
        .children
        .iter()
        .find_map(Node::as_element)
        .filter(|c| c.raw_tag.as_str() == "mj-container")
    else {
        return Staged::identity(el);
    };

    diags.push(
        Diagnostic::advice("migrated: <mj-container> unwrapped into <mj-body>").with_label(
            container.span,
            "attributes hoisted onto the parent <mj-body>",
        ),
    );
    Staged {
        tag: el.tag,
        raw_tag: el.raw_tag,
        attributes: container.attributes.iter().copied().collect(),
        children: container.children.iter().copied().collect(),
    }
}

/// `<mj-hero><mj-hero-content …>` → `<mj-hero …>`: the hero-content is dissolved and
/// its attributes merged into the hero's (child wins on conflicts).
fn migrate_hero<'a>(el: &Element<'a>, diags: &mut Vec<Diagnostic>) -> Staged<'a> {
    let Some(content) = el
        .children
        .iter()
        .filter_map(Node::as_element)
        .find(|c| c.raw_tag.as_str() == "mj-hero-content")
    else {
        diags.push(
            Diagnostic::warning("could not migrate <mj-hero>: no <mj-hero-content> child")
                .with_label(el.span, "left unchanged")
                .with_help("a v3 hero always wraps its content in <mj-hero-content>"),
        );
        return Staged::identity(el);
    };

    let mut attributes: Vec<Attribute<'a>> = el.attributes.iter().copied().collect();
    for attr in &content.attributes {
        match attributes
            .iter_mut()
            .find(|a| a.name.as_str() == attr.name.as_str())
        {
            Some(existing) => *existing = *attr,
            None => attributes.push(*attr),
        }
    }

    diags.push(
        Diagnostic::advice("migrated: <mj-hero-content> dissolved into <mj-hero>")
            .with_label(content.span, "attributes merged into the parent <mj-hero>"),
    );
    Staged {
        tag: el.tag,
        raw_tag: el.raw_tag,
        attributes,
        children: content.children.iter().copied().collect(),
    }
}

/// v3 `<mj-social display="a b" a-content="…" a-icon-color="…">` → v4
/// `<mj-social><mj-social-element name="a" icon-color="…">…</mj-social-element></mj-social>`.
fn migrate_social<'a>(
    builder: &AstBuilder<'a>,
    el: &Element<'a>,
    diags: &mut Vec<Diagnostic>,
) -> Staged<'a> {
    let mut root: Vec<Attribute<'a>> = el.attributes.iter().copied().collect();

    let display = root
        .iter()
        .find(|a| a.name.as_str() == "display")
        .and_then(Attribute::value_str)
        .unwrap_or(DEFAULT_SOCIAL_DISPLAY)
        .to_string();
    root.retain(|a| a.name.as_str() != "display");

    let mut children: Vec<Node<'a>> = Vec::new();
    for network in display.split(' ').filter(|s| !s.is_empty()) {
        // `facebook:url` → a non-sharing icon, `facebook:share` → the plain icon.
        let name = network
            .replacen(":url", "-noshare", 1)
            .replacen(":share", "", 1);
        let bare = name.replacen("-noshare", "", 1);

        let content = root
            .iter()
            .find(|a| a.name.as_str() == format!("{bare}-content"))
            .and_then(Attribute::value_str)
            .unwrap_or_default()
            .to_string();

        let mut attributes =
            vec![builder.attribute(builder.atom("name"), Some(builder.atom(&name)), el.span)];
        // Every `<network>-*` attribute (except the content one) moves to the child
        // with the network prefix stripped.
        let mut moved: Vec<&str> = Vec::new();
        for attr in &root {
            let attr_name = attr.name.as_str();
            if attr_name.contains(bare.as_str()) && !attr_name.contains("content") {
                let key = attr_name.replacen(&format!("{bare}-"), "", 1);
                attributes.push(builder.attribute(builder.atom(&key), attr.value, attr.span));
                moved.push(attr_name);
            }
        }
        root.retain(|a| !moved.contains(&a.name.as_str()));

        let child_children = if content.is_empty() {
            builder.vec()
        } else {
            builder
                .vec_from_iter([builder.text_node(builder.text(builder.atom(&content), el.span))])
        };
        children.push(builder.element_node(builder.element(
            MjmlTag::MjSocialElement,
            builder.atom("mj-social-element"),
            builder.vec_from_iter(attributes),
            child_children,
            el.span,
        )));
    }

    // Any leftover `*content*` attribute has now been folded into a child.
    root.retain(|a| !a.name.as_str().contains("content"));

    diags.push(
        Diagnostic::advice(format!(
            "migrated: <mj-social display=\"{display}\"> became {} <mj-social-element> child(ren)",
            children.len()
        ))
        .with_label(el.span, "per-network attributes moved onto the children"),
    );
    Staged {
        tag: el.tag,
        raw_tag: el.raw_tag,
        attributes: root,
        children,
    }
}

/// Append `px` to bare-number values of length-ish attributes (mjml's `fixUnits`).
fn add_missing_units<'a>(
    builder: &AstBuilder<'a>,
    attributes: &mut [Attribute<'a>],
    diags: &mut Vec<Diagnostic>,
    span: Span,
) {
    for attr in attributes.iter_mut() {
        if !ATTRIBUTES_WITH_UNIT.contains(&attr.name.as_str()) {
            continue;
        }
        let Some(value) = attr.value_str() else {
            continue;
        };
        if !is_bare_number(value) {
            continue;
        }
        let fixed = format!("{value}px");
        diags.push(
            Diagnostic::advice(format!(
                "migrated: {}=\"{value}\" became \"{fixed}\"",
                attr.name.as_str()
            ))
            .with_label(span, "v4 requires an explicit unit"),
        );
        attr.value = Some(builder.atom(&fixed));
    }
}

/// Whether `value` is what JavaScript's `!isNaN(value)` accepts — a bare number, or
/// (as in mjml) the empty string, which `Number("")` coerces to `0`.
fn is_bare_number(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.is_empty() || trimmed.parse::<f64>().is_ok_and(|n| !n.is_nan())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mjmx_allocator::Allocator;

    /// Migrate `source` and return the formatted v4 MJML plus the diagnostics.
    fn run(source: &str) -> (String, Vec<Diagnostic>) {
        let alloc = Allocator::new();
        let builder = AstBuilder::new(&alloc);
        let parsed = mjmx_parser::parse(source, &alloc);
        assert!(
            parsed.errors.is_empty(),
            "parse errors: {:?}",
            parsed.errors
        );
        let (document, diags) = migrate(&builder, &parsed.document);
        (render_source(&document), diags)
    }

    /// A minimal MJML serializer, so this crate's tests do not depend on the
    /// formatter (which depends on nothing here, but the umbrella wires them up).
    fn render_source(document: &Document<'_>) -> String {
        fn write_node(out: &mut String, current: &Node<'_>) {
            match current {
                Node::Element(el) => {
                    out.push('<');
                    out.push_str(el.raw_tag.as_str());
                    for attr in &el.attributes {
                        out.push(' ');
                        out.push_str(attr.name.as_str());
                        if let Some(value) = attr.value_str() {
                            out.push_str(&format!("=\"{value}\""));
                        }
                    }
                    out.push('>');
                    for child in &el.children {
                        write_node(out, child);
                    }
                    out.push_str(&format!("</{}>", el.raw_tag.as_str()));
                }
                Node::Text(text) => out.push_str(text.value.as_str()),
                Node::Comment(_) => {}
            }
        }
        let mut out = String::new();
        for child in &document.children {
            write_node(&mut out, child);
        }
        out
    }

    #[test]
    fn container_is_unwrapped_into_body() {
        let (out, diags) = run(
            "<mjml><mj-body><mj-container background-color=\"#eee\"><mj-section><mj-column><mj-text>hi</mj-text></mj-column></mj-section></mj-container></mj-body></mjml>",
        );
        assert!(!out.contains("mj-container"), "{out}");
        assert!(out.contains("<mj-body background-color=\"#eee\">"), "{out}");
        assert!(out.contains("<mj-text>hi</mj-text>"), "{out}");
        assert!(diags.iter().any(|d| d.message.contains("mj-container")));
    }

    #[test]
    fn navbar_and_links_are_rewritten() {
        let (out, diags) = run(
            "<mjml><mj-body><mj-container><mj-navbar><mj-inline-links><mj-link href=\"/a\">A</mj-link></mj-inline-links></mj-navbar></mj-container></mj-body></mjml>",
        );
        assert!(
            out.contains("<mj-section full-width=\"full-width\">"),
            "{out}"
        );
        assert!(out.contains("<mj-navbar>"), "{out}");
        assert!(
            out.contains("<mj-navbar-link href=\"/a\">A</mj-navbar-link>"),
            "{out}"
        );
        // mjml's rewrite is structurally incomplete; we must say so.
        assert!(
            diags
                .iter()
                .any(|d| d.severity == mjmx_diagnostics::Severity::Warning
                    && d.message.contains("mj-navbar")),
            "{diags:?}"
        );
    }

    #[test]
    fn hero_content_is_dissolved() {
        let (out, _) = run(
            "<mjml><mj-body><mj-hero mode=\"fixed-height\"><mj-hero-content background-color=\"#f00\"><mj-text>x</mj-text></mj-hero-content></mj-hero></mj-body></mjml>",
        );
        assert!(!out.contains("mj-hero-content"), "{out}");
        assert!(
            out.contains("<mj-hero mode=\"fixed-height\" background-color=\"#f00\">"),
            "{out}"
        );
    }

    #[test]
    fn hero_without_content_warns_instead_of_panicking() {
        let (out, diags) =
            run("<mjml><mj-body><mj-hero><mj-text>x</mj-text></mj-hero></mj-body></mjml>");
        assert!(out.contains("<mj-hero>"), "{out}");
        assert!(
            diags
                .iter()
                .any(|d| d.message.contains("could not migrate"))
        );
    }

    #[test]
    fn social_is_split_into_elements() {
        let (out, _) = run(
            "<mjml><mj-body><mj-social display=\"facebook twitter\" facebook-content=\"Share\" twitter-icon-color=\"#0af\" font-size=\"13\" /></mj-body></mjml>",
        );
        assert!(
            out.contains("<mj-social-element name=\"facebook\">Share</mj-social-element>"),
            "{out}"
        );
        assert!(
            out.contains(
                "<mj-social-element name=\"twitter\" icon-color=\"#0af\"></mj-social-element>"
            ),
            "{out}"
        );
        // Shared attributes stay on the parent, and gain their unit.
        assert!(out.contains("<mj-social font-size=\"13px\">"), "{out}");
    }

    #[test]
    fn social_default_display_and_noshare() {
        let (out, _) =
            run("<mjml><mj-body><mj-social display=\"facebook:url\" /></mj-body></mjml>");
        assert!(out.contains("name=\"facebook-noshare\""), "{out}");

        let (out, _) = run("<mjml><mj-body><mj-social /></mj-body></mjml>");
        for network in ["facebook", "twitter", "google"] {
            assert!(out.contains(&format!("name=\"{network}\"")), "{out}");
        }
    }

    #[test]
    fn unavailable_tags_are_dropped_with_a_warning() {
        let (out, diags) = run(
            "<mjml><mj-body><mj-container><mj-section><mj-column><mj-list>x</mj-list><mj-text>keep</mj-text></mj-column></mj-section></mj-container></mj-body></mjml>",
        );
        assert!(!out.contains("mj-list"), "{out}");
        assert!(out.contains("keep"), "{out}");
        assert!(diags.iter().any(|d| d.message.contains("mj-list")));
    }

    #[test]
    fn units_are_added_only_where_mjml_adds_them() {
        let (out, _) = run(
            "<mjml><mj-body><mj-section padding=\"10\"><mj-column><mj-text font-size=\"13\" color=\"12\" padding=\"10px 5px\">x</mj-text></mj-column></mj-section></mj-body></mjml>",
        );
        assert!(out.contains("<mj-section padding=\"10px\">"), "{out}");
        assert!(out.contains("font-size=\"13px\""), "{out}");
        // Not a length attribute: untouched. Already has units: untouched.
        assert!(out.contains("color=\"12\""), "{out}");
        assert!(out.contains("padding=\"10px 5px\""), "{out}");
    }

    #[test]
    fn bare_number_detection_matches_javascript() {
        assert!(is_bare_number("10"));
        assert!(is_bare_number(" 10 "));
        assert!(is_bare_number("0"));
        assert!(is_bare_number("-1.5"));
        assert!(is_bare_number("")); // Number("") === 0, so mjml appends px
        assert!(!is_bare_number("10px"));
        assert!(!is_bare_number("10 5"));
        assert!(!is_bare_number("auto"));
        assert!(!is_bare_number("NaN"));
    }

    #[test]
    fn a_v4_document_without_v3_constructs_is_unchanged() {
        let source = "<mjml><mj-body><mj-section><mj-column><mj-text>hi</mj-text></mj-column></mj-section></mj-body></mjml>";
        let (out, diags) = run(source);
        assert_eq!(out, source);
        assert!(diags.is_empty(), "{diags:?}");
    }
}
