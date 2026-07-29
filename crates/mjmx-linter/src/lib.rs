//! Structural validator/linter for `mjmx` — the `mjml-validator` / `oxc_linter`
//! analogue.
//!
//! [`lint`] walks a parsed [`Document`] and returns advisory
//! [`Diagnostic`]s for MJML structural rule violations. It is **purely advisory**:
//! it never panics and never blocks rendering — a caller renders regardless and uses
//! the diagnostics for feedback (a `mjmx lint` command, an editor, a `--validate`
//! flag).
//!
//! # Rules
//!
//! 1. **Allowed children** — every container permits only certain child tags
//!    ([`rules::allowed_children`]); a disallowed child is an error on the child.
//! 2. **Unknown components** — a tag the parser could not recognize
//!    ([`MjmlTag::Unknown`]) is an error; the renderer rejects it, as does mjml.
//! 3. **Misplaced elements** — a head-only element in the body subtree (or vice
//!    versa) is an error with a targeted message.
//! 4. **Dangling child-only elements** — `mj-social-element`, `mj-navbar-link`,
//!    `mj-carousel-image`, `mj-accordion-element`/`-title`/`-text` outside their
//!    required container ([`rules::required_parent`]) is an error.
//! 5. **Required attributes** — e.g. `mj-image` should carry `src`; `mj-social-element`
//!    a `name` or `src` ([`rules::required_attributes`]). These are warnings.
//!
//! Each element yields at most one *structural* diagnostic (rules 1/3/4 are mutually
//! exclusive per element), so there is no double-reporting; rule 5 is independent.
//!
//! ```
//! use mjmx_allocator::Allocator;
//!
//! let alloc = Allocator::new();
//! let ret = mjmx_parser::parse(
//!     "<mjml><mj-body><mj-column>x</mj-column></mj-body></mjml>",
//!     &alloc,
//! );
//! let diags = mjmx_linter::lint(&ret.document);
//! // <mj-column> is not valid directly inside <mj-body>.
//! assert_eq!(diags.len(), 1);
//! ```

pub mod rules;

use std::collections::BTreeSet;

use mjmx_ast::{Document, Element};
use mjmx_diagnostics::Diagnostic;
use mjmx_syntax::MjmlTag;

/// Which part of the document tree an element lives in.
///
/// Established by the enclosing `<mj-head>` / `<mj-body>` and propagated to
/// descendants so misplacement (rule 3) can be reported with a precise message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Region {
    /// Above `<mj-head>`/`<mj-body>` (i.e. directly under `<mjml>` or the document).
    Root,
    /// Inside the `<mj-head>` subtree.
    Head,
    /// Inside the `<mj-body>` subtree.
    Body,
}

/// Lint a parsed [`Document`], returning all structural diagnostics.
///
/// Never panics and never mutates the AST. An empty result means the document is
/// structurally clean by the encoded rules. Ordering follows a depth-first,
/// source-order walk.
#[must_use]
pub fn lint<'a>(document: &Document<'a>) -> Vec<Diagnostic> {
    lint_with_components(document, &BTreeSet::new())
}

/// [`lint`], treating each tag in `registered` as a known component.
///
/// Custom components (`mjmx::render_with_components`) register renderers for tags the
/// parser leaves as [`MjmlTag::Unknown`]. A registered tag must not be flagged as an
/// unknown component, or lint and render would disagree — so the render pipeline passes
/// the registered names here. With an empty set this is exactly [`lint`].
#[must_use]
pub fn lint_with_components<'a>(
    document: &Document<'a>,
    registered: &BTreeSet<&str>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for node in &document.children {
        if let Some(el) = node.as_element() {
            walk(el, None, Region::Root, registered, &mut out);
        }
    }
    out
}

/// Depth-first walk: check `el` against its `parent`/`region`, then recurse.
fn walk<'a>(
    el: &'a Element<'a>,
    parent: Option<MjmlTag>,
    region: Region,
    registered: &BTreeSet<&str>,
    out: &mut Vec<Diagnostic>,
) {
    check_element(el, parent, region, registered, out);

    // `<mj-attributes>` children are not elements in a position — they are attribute
    // *rows*, keyed by tag name. `<mj-all>` sets a default for every component,
    // `<mj-class name="x">` defines a named set, and any other name is the per-tag row
    // for that component (`AttributesRegistry::ingest` matches on the raw tag string and
    // accepts anything). So every structural rule is meaningless in here: a row for a
    // component the document never uses is perfectly valid, and `mj-all`/`mj-class` are
    // not components at all — they exist only in this context, which is why they are
    // absent from `MjmlTag` and would otherwise be reported as unknown components.
    //
    // Not descending matches mjml, which reads these rows as data and ignores a name it
    // does not recognize.
    if el.tag == MjmlTag::MjAttributes {
        return;
    }

    let child_region = match el.tag {
        MjmlTag::MjHead => Region::Head,
        MjmlTag::MjBody => Region::Body,
        _ => region,
    };
    for child in el.element_children() {
        walk(child, Some(el.tag), child_region, registered, out);
    }
}

/// Run every rule that applies to a single element.
fn check_element<'a>(
    el: &'a Element<'a>,
    parent: Option<MjmlTag>,
    region: Region,
    registered: &BTreeSet<&str>,
    out: &mut Vec<Diagnostic>,
) {
    // Structural checks are mutually exclusive: unknown tags are opaque, child-only
    // elements are validated from the child side, everything else from the parent's
    // allowed-children table.
    if el.tag == MjmlTag::Unknown {
        // A host-registered custom component is known; only an unregistered tag is the
        // usual unknown-component error.
        if registered.contains(el.raw_tag.as_str()) {
            check_custom_placement(el, region, out);
        } else {
            check_unknown(el, out);
        }
    } else if let Some(required) = rules::required_parent(el.tag) {
        check_dangling(el, parent, required, out);
    } else if let Some(parent_tag) = parent {
        check_allowed_child(el, parent_tag, region, out);
    }

    // Rule 5 is independent of the structural verdict.
    check_required_attributes(el, out);
}

/// A registered Tier 1 custom component is a body leaf element: codegen only dispatches
/// it while rendering the body, so a placement in `<mj-head>` (or above the body) would
/// silently render nothing. Flag it — the same way a misplaced built-in is flagged — so a
/// host registration mistake is a diagnostic rather than a quiet no-op. Lint stays at
/// least as strict as render (which drops it).
fn check_custom_placement<'a>(el: &'a Element<'a>, region: Region, out: &mut Vec<Diagnostic>) {
    if region != Region::Body {
        out.push(
            Diagnostic::error(format!(
                "custom component '<{}>' is only valid inside the body",
                el.raw_tag.as_str()
            ))
            .with_label(el.span, "custom component outside <mj-body>")
            .with_help("Tier 1 custom components render as body leaf elements"),
        );
    }
}

/// Rule 2: an unrecognized component is an error. The renderer rejects it — nothing is
/// passed through — and so does mjml ("Element X doesn't exist or is not registered").
/// Lint and render agree so both stages tell the same story.
fn check_unknown<'a>(el: &'a Element<'a>, out: &mut Vec<Diagnostic>) {
    out.push(
        Diagnostic::error(format!(
            "unknown mjml component '<{}>'",
            el.raw_tag.as_str()
        ))
        .with_label(el.span, "not a recognized MJML component")
        .with_help("check the spelling; MJML does not define this element"),
    );
}

/// Rule 4: a child-only element whose parent is not its required container.
fn check_dangling<'a>(
    el: &'a Element<'a>,
    parent: Option<MjmlTag>,
    required: MjmlTag,
    out: &mut Vec<Diagnostic>,
) {
    if parent == Some(required) {
        return;
    }
    let tag = el.tag.as_str();
    let req = required.as_str();
    out.push(
        Diagnostic::error(format!("<{tag}> must be a direct child of <{req}>"))
            .with_label(el.span, format!("<{tag}> is only valid inside <{req}>"))
            .with_help(format!("move this <{tag}> inside a <{req}>")),
    );
}

/// Rules 1 & 3: validate `el` against its parent's allowed-children set, preferring
/// the misplacement message when a head element strays into the body (or vice versa).
fn check_allowed_child<'a>(
    el: &'a Element<'a>,
    parent_tag: MjmlTag,
    region: Region,
    out: &mut Vec<Diagnostic>,
) {
    let Some(allowed) = rules::allowed_children(parent_tag) else {
        // Parent is a leaf/free-form container; nothing to enforce.
        return;
    };
    // mj-include expands to arbitrary content, so it is accepted anywhere.
    if el.tag == MjmlTag::MjInclude || allowed.contains(&el.tag) {
        return;
    }

    let tag = el.tag.as_str();
    let diagnostic = if region == Region::Body && el.tag.is_head_element() {
        // Rule 3: head-only element in the body subtree.
        Diagnostic::error(format!(
            "<{tag}> is a head-only component but appears in the body"
        ))
        .with_label(el.span, format!("<{tag}> belongs inside <mj-head>"))
        .with_help(format!("move <{tag}> into the <mj-head> section"))
    } else if region == Region::Head && el.tag.is_body_element() {
        // Rule 3: body-only element in the head subtree.
        Diagnostic::error(format!(
            "<{tag}> is a body component but appears in <mj-head>"
        ))
        .with_label(el.span, format!("<{tag}> belongs inside <mj-body>"))
        .with_help(format!("move <{tag}> into the <mj-body> section"))
    } else {
        // Rule 1: simply not permitted in this container.
        let parent = parent_tag.as_str();
        Diagnostic::error(format!("<{tag}> is not allowed inside <{parent}>"))
            .with_label(
                el.span,
                format!("<{tag}> cannot be a direct child of <{parent}>"),
            )
            .with_help(format!("expected one of: {}", expected_list(allowed)))
    };
    out.push(diagnostic);
}

/// Rule 5: warn when a component is missing a recommended attribute group.
fn check_required_attributes<'a>(el: &'a Element<'a>, out: &mut Vec<Diagnostic>) {
    for group in rules::required_attributes(el.tag) {
        if group.iter().any(|name| el.has_attr(name)) {
            continue;
        }
        let tag = el.tag.as_str();
        let names = group
            .iter()
            .map(|n| format!("`{n}`"))
            .collect::<Vec<_>>()
            .join(" or ");
        out.push(
            Diagnostic::warning(format!("<{tag}> is missing a {names} attribute"))
                .with_label(el.span, format!("add {names} to <{tag}>")),
        );
    }
}

/// Render an allowed-children set as a human-readable `mj-a/mj-b/...` list.
fn expected_list(allowed: &[MjmlTag]) -> String {
    allowed
        .iter()
        .map(|t| t.as_str())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mjmx_allocator::Allocator;
    use mjmx_diagnostics::Severity;

    /// Parse `src` and lint it, returning the diagnostics.
    ///
    /// `lint` returns owned diagnostics, so the arena borrow ends when this returns.
    fn lint_src(src: &str) -> Vec<Diagnostic> {
        let alloc = Allocator::new();
        let ret = mjmx_parser::parse(src, &alloc);
        lint(&ret.document)
    }

    fn errors(diags: &[Diagnostic]) -> usize {
        diags
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }

    #[test]
    fn valid_document_is_clean() {
        let diags = lint_src(
            "<mjml><mj-body><mj-section><mj-column>\
             <mj-text>Hello</mj-text></mj-column></mj-section></mj-body></mjml>",
        );
        assert!(diags.is_empty(), "expected clean, got: {diags:?}");
    }

    #[test]
    fn full_document_with_head_is_clean() {
        let diags = lint_src(
            "<mjml>\
               <mj-head><mj-title>T</mj-title><mj-preview>P</mj-preview></mj-head>\
               <mj-body>\
                 <mj-wrapper><mj-section><mj-column>\
                   <mj-image src=\"https://x/y.png\" />\
                   <mj-social><mj-social-element name=\"twitter\" /></mj-social>\
                   <mj-navbar><mj-navbar-link href=\"#\">Home</mj-navbar-link></mj-navbar>\
                   <mj-accordion><mj-accordion-element>\
                     <mj-accordion-title>T</mj-accordion-title>\
                     <mj-accordion-text>X</mj-accordion-text>\
                   </mj-accordion-element></mj-accordion>\
                   <mj-carousel><mj-carousel-image src=\"https://x/a.png\" /></mj-carousel>\
                 </mj-column></mj-section></mj-wrapper>\
                 <mj-hero background-url=\"u\"><mj-text>hi</mj-text></mj-hero>\
               </mj-body>\
             </mjml>",
        );
        assert!(diags.is_empty(), "expected clean, got: {diags:?}");
    }

    #[test]
    fn column_directly_in_body_is_error() {
        let diags = lint_src("<mjml><mj-body><mj-column>x</mj-column></mj-body></mjml>");
        assert_eq!(errors(&diags), 1);
        assert!(diags[0].message.contains("mj-column"));
        assert!(diags[0].message.contains("mj-body"));
    }

    #[test]
    fn unknown_tag_is_error() {
        let diags = lint_src(
            "<mjml><mj-body><mj-section><mj-column>\
             <mj-foo>x</mj-foo></mj-column></mj-section></mj-body></mjml>",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(diags[0].message.contains("mj-foo"));
        assert_eq!(errors(&diags), 1);
    }

    #[test]
    fn social_element_outside_social_is_error() {
        let diags = lint_src(
            "<mjml><mj-body><mj-section><mj-column>\
             <mj-social-element name=\"twitter\" /></mj-column></mj-section></mj-body></mjml>",
        );
        assert_eq!(errors(&diags), 1);
        assert!(diags[0].message.contains("mj-social-element"));
        assert!(diags[0].message.contains("mj-social"));
    }

    #[test]
    fn head_element_in_body_is_misplaced_error() {
        let diags = lint_src(
            "<mjml><mj-body><mj-section><mj-column>\
             <mj-title>oops</mj-title></mj-column></mj-section></mj-body></mjml>",
        );
        assert_eq!(errors(&diags), 1);
        assert!(diags[0].message.contains("head-only"));
    }

    #[test]
    fn body_element_in_head_is_misplaced_error() {
        let diags = lint_src("<mjml><mj-head><mj-section></mj-section></mj-head></mjml>");
        assert_eq!(errors(&diags), 1);
        assert!(diags[0].message.contains("body component"));
    }

    #[test]
    fn missing_image_src_is_warning() {
        let diags = lint_src(
            "<mjml><mj-body><mj-section><mj-column>\
             <mj-image /></mj-column></mj-section></mj-body></mjml>",
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Severity::Warning);
        assert!(diags[0].message.contains("src"));
    }

    #[test]
    fn dangling_element_reported_once() {
        // A misplaced child-only element must not be double-reported (rule 4 vs rule 1).
        let diags = lint_src("<mjml><mj-body><mj-navbar-link href=\"#\" /></mj-body></mjml>");
        assert_eq!(errors(&diags), 1);
    }
}
