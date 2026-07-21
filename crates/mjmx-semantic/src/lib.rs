//! Semantic analysis for `mjmx`: `<mj-head>` aggregation and attribute resolution.
//!
//! This is the pipeline stage between parsing and codegen. It answers two questions
//! for the renderer, without rewriting the tree:
//!
//! 1. **What did the head declare?** [`Semantic::head`] exposes the aggregated
//!    [`HeadData`] — breakpoint, title, preview, fonts, and raw styles.
//! 2. **What is the effective value of an attribute?** [`Semantic::attribute`]
//!    resolves an attribute on a body element against `<mj-attributes>` defaults.
//!
//! # Resolution precedence
//!
//! [`Semantic::attribute`] walks these sources, highest priority first, and returns
//! the first hit:
//!
//! 1. an inline attribute on the element itself;
//! 2. an `mj-class="a b"` map (later classes win over earlier);
//! 3. a per-tag `<mj-text ...>` default;
//! 4. the global `<mj-all ...>` default;
//! 5. the component default the caller passes in (`component_default`).
//!
//! # On-the-fly, allocation-free
//!
//! No `NodeId` side-tables and no tree rewrite: everything is borrowed from the arena
//! for the document lifetime `'a`, and resolution is a handful of map lookups per
//! query. [`Semantic::build`] never panics — malformed or unknown head input becomes a
//! warning [`Diagnostic`], never a hard failure.

mod attributes;
mod head;

pub use attributes::AttributesRegistry;
pub use head::{DEFAULT_BREAKPOINT, FontFace, HeadData};

use mjmx_ast::{Document, Element, Node};
use mjmx_diagnostics::Diagnostic;
use mjmx_span::GetSpan;
use mjmx_syntax::{Length, MjmlTag};

/// The result of analysing a [`Document`]: aggregated head data plus the attribute
/// resolver, all borrowing the document's arena for lifetime `'a`.
#[derive(Debug)]
pub struct Semantic<'a> {
    head: HeadData<'a>,
    attributes: AttributesRegistry<'a>,
}

impl<'a> Semantic<'a> {
    /// Analyse a parsed [`Document`], returning the [`Semantic`] view and any
    /// non-fatal diagnostics (e.g. an unrecognized `<mj-head>` child).
    ///
    /// Never panics and never fails: a document with no head, or no root at all,
    /// yields a [`Semantic`] carrying only defaults.
    #[must_use]
    pub fn build(document: &Document<'a>) -> (Self, Vec<Diagnostic>) {
        let mut diagnostics = Vec::new();
        let mut head = HeadData::with_defaults();
        let mut attributes = AttributesRegistry::default();

        if let Some(root) = document.root_element() {
            if let Some(head_el) = root.element_children().find(|el| el.tag == MjmlTag::MjHead) {
                for child in head_el.element_children() {
                    Self::ingest_head_child(child, &mut head, &mut attributes, &mut diagnostics);
                }
            }
            // mjml's `mjOutsideRaws`: `<mj-raw>` siblings of `<mj-head>`/`<mj-body>`.
            // Only those carrying `position="file-start"` are emitted — before the
            // doctype; the rest are silently dropped.
            for child in root.element_children() {
                if child.tag == MjmlTag::MjRaw && child.attr("position") == Some("file-start") {
                    head.file_start_raws.push(inner_text(child));
                }
            }
        }

        (Self { head, attributes }, diagnostics)
    }

    /// Fold one `<mj-head>` child into the accumulated head/attributes state.
    fn ingest_head_child(
        child: &Element<'a>,
        head: &mut HeadData<'a>,
        attributes: &mut AttributesRegistry<'a>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        match child.tag {
            MjmlTag::MjBreakpoint => {
                if let Some(px) = child.attr("width").and_then(parse_px) {
                    head.breakpoint = Some(px);
                }
            }
            MjmlTag::MjFont => {
                if let (Some(name), Some(href)) = (child.attr("name"), child.attr("href")) {
                    head.add_font(name, href);
                }
            }
            MjmlTag::MjTitle => head.title = Some(inner_text(child)),
            MjmlTag::MjPreview => head.preview = Some(inner_text(child)),
            MjmlTag::MjStyle => {
                if child.has_attr("inline") {
                    head.has_inline_styles = true;
                }
                head.styles.push(inner_text(child));
            }
            // mjml renders a head `<mj-raw>` like any body component (it is a
            // `BodyComponent`), so its content lands in `globalData.headRaw` and is
            // injected verbatim at the end of the document `<head>`.
            MjmlTag::MjRaw => head.raws.push(inner_text(child)),
            MjmlTag::MjAttributes => attributes.ingest(child),
            _ => diagnostics.push(
                Diagnostic::warning(format!(
                    "unknown <mj-head> child `<{}>` ignored",
                    child.raw_tag.as_str()
                ))
                .with_label(child.span(), "not a recognized head element"),
            ),
        }
    }

    /// The aggregated `<mj-head>` metadata.
    #[must_use]
    pub fn head(&self) -> &HeadData<'a> {
        &self.head
    }

    /// The `<mj-attributes>` registry backing [`Semantic::attribute`].
    #[must_use]
    pub fn attributes(&self) -> &AttributesRegistry<'a> {
        &self.attributes
    }

    /// Resolve attribute `name` on body element `el`.
    ///
    /// Applies the documented precedence — inline > `mj-class` > per-tag > `mj-all` >
    /// `component_default` — and returns the first source that provides a value. The
    /// returned string borrows the arena (lifetime `'a`), so it composes directly into
    /// the rendered output. Pass the component's own mjml default as `component_default`
    /// so a document with no `<mj-attributes>` behaves exactly as before.
    #[must_use]
    pub fn attribute(
        &self,
        el: &Element<'a>,
        name: &str,
        component_default: Option<&'a str>,
    ) -> Option<&'a str> {
        // 1. inline attribute on the element itself.
        if let Some(value) = el.attr(name) {
            return Some(value);
        }
        // 2. mj-class maps (later class wins).
        if let Some(classes) = el.attr("mj-class") {
            if let Some(value) = self.attributes.class(classes, name) {
                return Some(value);
            }
        }
        // 3. per-tag default.
        if let Some(value) = self.attributes.per_tag(el.raw_tag.as_str(), name) {
            return Some(value);
        }
        // 4. mj-all default.
        if let Some(value) = self.attributes.all(name) {
            return Some(value);
        }
        // 5. the component's own default.
        component_default
    }
}

/// Parse an `<mj-breakpoint width>` value (`"320px"`, `"320"`) into whole pixels.
fn parse_px(value: &str) -> Option<u32> {
    Length::parse(value).map(|l| l.value as u32)
}

/// Concatenate the direct text children of `el` (used for `<mj-title>` / `<mj-preview>`
/// / `<mj-style>` bodies, which the parser captures as text nodes).
fn inner_text(el: &Element<'_>) -> String {
    let mut text = String::new();
    for child in el.children() {
        if let Node::Text(t) = child {
            text.push_str(t.value.as_str());
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use mjmx_allocator::Allocator;

    fn analyse<'a>(src: &'a str, alloc: &'a Allocator) -> Semantic<'a> {
        let parsed = mjmx_parser::parse(src, alloc);
        Semantic::build(&parsed.document).0
    }

    fn find<'a>(el: &'a Element<'a>, tag: MjmlTag) -> &'a Element<'a> {
        fn walk<'a>(el: &'a Element<'a>, tag: MjmlTag) -> Option<&'a Element<'a>> {
            if el.tag == tag {
                return Some(el);
            }
            el.element_children().find_map(|c| walk(c, tag))
        }
        walk(el, tag).expect("element present")
    }

    #[test]
    fn no_head_yields_defaults() {
        let alloc = Allocator::new();
        let sem = analyse(
            "<mjml><mj-body><mj-text>hi</mj-text></mj-body></mjml>",
            &alloc,
        );
        assert_eq!(sem.head().breakpoint(), DEFAULT_BREAKPOINT);
        assert!(sem.head().title.is_none());
        assert!(sem.head().preview.is_none());
        // Built-in fonts are always seeded.
        assert!(sem.head().fonts.iter().any(|f| f.name == "Ubuntu"));
    }

    #[test]
    fn head_metadata_is_aggregated() {
        let alloc = Allocator::new();
        let sem = analyse(
            r#"<mjml><mj-head>
                 <mj-breakpoint width="320px" />
                 <mj-title>T</mj-title>
                 <mj-preview>P</mj-preview>
                 <mj-font name="Raleway" href="https://example.test/raleway" />
               </mj-head><mj-body></mj-body></mjml>"#,
            &alloc,
        );
        assert_eq!(sem.head().breakpoint(), 320);
        assert_eq!(sem.head().title.as_deref(), Some("T"));
        assert_eq!(sem.head().preview.as_deref(), Some("P"));
        let raleway = sem
            .head()
            .fonts
            .iter()
            .find(|f| f.name == "Raleway")
            .expect("mj-font added");
        assert_eq!(raleway.href, "https://example.test/raleway");
    }

    #[test]
    fn precedence_inline_over_class_over_tag_over_all() {
        let alloc = Allocator::new();
        let parsed = mjmx_parser::parse(
            r#"<mjml><mj-head><mj-attributes>
                 <mj-all color="all" font-family="allfam" />
                 <mj-text color="tag" />
                 <mj-class name="c" color="cls" />
               </mj-attributes></mj-head>
               <mj-body><mj-section><mj-column>
                 <mj-text mj-class="c" color="inline">x</mj-text>
                 <mj-text mj-class="c">y</mj-text>
                 <mj-text>z</mj-text>
               </mj-column></mj-section></mj-body></mjml>"#,
            &alloc,
        );
        let sem = Semantic::build(&parsed.document).0;
        let root = parsed.document.root_element().unwrap();
        let column = find(root, MjmlTag::MjColumn);
        let texts: Vec<_> = column.element_children().collect();

        // inline wins.
        assert_eq!(
            sem.attribute(texts[0], "color", Some("def")),
            Some("inline")
        );
        // class beats per-tag and mj-all.
        assert_eq!(sem.attribute(texts[1], "color", Some("def")), Some("cls"));
        // per-tag beats mj-all.
        assert_eq!(sem.attribute(texts[2], "color", Some("def")), Some("tag"));
        // mj-all supplies what nothing else does.
        assert_eq!(
            sem.attribute(texts[2], "font-family", Some("def")),
            Some("allfam")
        );
        // component default is the floor.
        assert_eq!(
            sem.attribute(texts[2], "padding", Some("10px 25px")),
            Some("10px 25px")
        );
    }

    #[test]
    fn later_class_wins() {
        let alloc = Allocator::new();
        let parsed = mjmx_parser::parse(
            r#"<mjml><mj-head><mj-attributes>
                 <mj-class name="a" color="AA" />
                 <mj-class name="b" color="BB" />
               </mj-attributes></mj-head>
               <mj-body><mj-column>
                 <mj-text mj-class="a b">x</mj-text>
               </mj-column></mj-body></mjml>"#,
            &alloc,
        );
        let sem = Semantic::build(&parsed.document).0;
        let root = parsed.document.root_element().unwrap();
        let text = find(root, MjmlTag::MjText);
        assert_eq!(sem.attribute(text, "color", None), Some("BB"));
    }

    #[test]
    fn unknown_head_child_warns_but_does_not_fail() {
        let alloc = Allocator::new();
        let parsed = mjmx_parser::parse(
            "<mjml><mj-head><mj-bogus/></mj-head><mj-body></mj-body></mjml>",
            &alloc,
        );
        let (_sem, diags) = Semantic::build(&parsed.document);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("mj-bogus"));
    }

    #[test]
    fn mj_style_inline_flag_recorded() {
        let alloc = Allocator::new();
        let sem = analyse(
            "<mjml><mj-head><mj-style inline=\"inline\">.a{color:red}</mj-style></mj-head><mj-body></mj-body></mjml>",
            &alloc,
        );
        assert!(sem.head().has_inline_styles());
        assert_eq!(sem.head().styles, vec![".a{color:red}".to_string()]);
    }
}
