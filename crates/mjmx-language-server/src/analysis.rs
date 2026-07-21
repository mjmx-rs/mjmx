//! The pure, synchronous language features.
//!
//! Everything here is a plain function of `(source, position)` returning owned LSP
//! types — no tokio, no client, no state. The [`crate::server`] layer is a thin
//! adapter that stores document text and calls into this module, which keeps the
//! interesting logic directly unit-testable.
//!
//! All analysis parses the source afresh into a scratch [`Allocator`] and drops the
//! arena before returning; nothing borrows from it.

use mjmx_allocator::Allocator;
use mjmx_ast::{Document, Element, Node};
use mjmx_diagnostics::Severity;
use mjmx_formatter::FormatOptions;
use mjmx_linter::rules;
use mjmx_syntax::MjmlTag;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionTextEdit, Diagnostic,
    DiagnosticRelatedInformation, DiagnosticSeverity, DocumentSymbol, Hover, HoverContents,
    Location, MarkupContent, MarkupKind, Position, SymbolKind, TextEdit, Url,
};

use crate::context::{CursorContext, cursor_context};
use crate::docs;
use crate::line_index::LineIndex;

/// The `source` field stamped on every diagnostic we publish.
pub const DIAGNOSTIC_SOURCE: &str = "mjmx";

/// Every MJML tag that can be written in a document, in a sensible offering order.
const ALL_TAGS: &[MjmlTag] = &[
    MjmlTag::Mjml,
    MjmlTag::MjHead,
    MjmlTag::MjBody,
    MjmlTag::MjAttributes,
    MjmlTag::MjBreakpoint,
    MjmlTag::MjFont,
    MjmlTag::MjPreview,
    MjmlTag::MjStyle,
    MjmlTag::MjTitle,
    MjmlTag::MjHtmlAttributes,
    MjmlTag::MjSection,
    MjmlTag::MjColumn,
    MjmlTag::MjGroup,
    MjmlTag::MjWrapper,
    MjmlTag::MjText,
    MjmlTag::MjButton,
    MjmlTag::MjImage,
    MjmlTag::MjDivider,
    MjmlTag::MjSpacer,
    MjmlTag::MjRaw,
    MjmlTag::MjTable,
    MjmlTag::MjSocial,
    MjmlTag::MjSocialElement,
    MjmlTag::MjNavbar,
    MjmlTag::MjNavbarLink,
    MjmlTag::MjHero,
    MjmlTag::MjCarousel,
    MjmlTag::MjCarouselImage,
    MjmlTag::MjAccordion,
    MjmlTag::MjAccordionElement,
    MjmlTag::MjAccordionTitle,
    MjmlTag::MjAccordionText,
    MjmlTag::MjInclude,
];

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

/// Parse and lint `source`, returning LSP diagnostics.
///
/// Parser errors come first (they describe the syntax that made the tree partial),
/// then the linter's structural findings over whatever tree was recovered.
#[must_use]
pub fn diagnostics(source: &str, index: &LineIndex, uri: &Url) -> Vec<Diagnostic> {
    let allocator = Allocator::default();
    let parsed = mjmx_parser::parse(source, &allocator);
    let mut out: Vec<Diagnostic> = parsed
        .errors
        .iter()
        .map(|diagnostic| convert_diagnostic(diagnostic, index, uri))
        .collect();
    for diagnostic in mjmx_linter::lint(&parsed.document) {
        out.push(convert_diagnostic(&diagnostic, index, uri));
    }
    out
}

/// Convert one `mjmx` [`mjmx_diagnostics::Diagnostic`] into its LSP form.
///
/// The primary range is the first label's span (byte offsets → UTF-16 positions via
/// [`LineIndex`]); a diagnostic with no labels is anchored at the start of the
/// document. Any further labels are attached as related information, and `help` is
/// appended to the message so editors that show only the message still surface it.
#[must_use]
pub fn convert_diagnostic(
    diagnostic: &mjmx_diagnostics::Diagnostic,
    index: &LineIndex,
    uri: &Url,
) -> Diagnostic {
    let mut labels = diagnostic.labels.iter();
    let range = labels.next().map_or_else(
        || {
            let start = Position {
                line: 0,
                character: 0,
            };
            tower_lsp::lsp_types::Range { start, end: start }
        },
        |label| index.range(label.span),
    );

    let related: Vec<DiagnosticRelatedInformation> = labels
        .map(|label| DiagnosticRelatedInformation {
            location: Location {
                uri: uri.clone(),
                range: index.range(label.span),
            },
            message: label.message.clone(),
        })
        .collect();

    let mut message = diagnostic.message.clone();
    if let Some(help) = &diagnostic.help {
        message.push_str("\n\nhelp: ");
        message.push_str(help);
    }

    Diagnostic {
        range,
        severity: Some(match diagnostic.severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
            Severity::Advice => DiagnosticSeverity::HINT,
        }),
        source: Some(DIAGNOSTIC_SOURCE.to_string()),
        message,
        related_information: if related.is_empty() {
            None
        } else {
            Some(related)
        },
        ..Diagnostic::default()
    }
}

// ---------------------------------------------------------------------------
// Completion
// ---------------------------------------------------------------------------

/// Completion items for the caret at `position`.
#[must_use]
pub fn completion(index: &LineIndex, position: Position) -> Vec<CompletionItem> {
    let source = index.text();
    let offset = index.offset(position);
    let replace = |start: usize| tower_lsp::lsp_types::Range {
        start: index.position(start),
        end: position,
    };

    match cursor_context(source, offset) {
        CursorContext::TagName {
            start,
            parent,
            closing,
            ..
        } => {
            let range = replace(start);
            if closing {
                return parent
                    .map(|open| vec![tag_item_named(open.raw, open.tag, range, 0)])
                    .unwrap_or_default();
            }
            tag_candidates(parent.map(|open| open.tag))
                .into_iter()
                .enumerate()
                .map(|(rank, tag)| tag_item_named(tag.as_str(), tag, range, rank))
                .collect()
        }
        CursorContext::AttributeName { tag, start, .. } => {
            let range = replace(start);
            attribute_candidates(tag.tag)
                .into_iter()
                .enumerate()
                .map(|(rank, (name, doc))| CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::PROPERTY),
                    detail: Some(format!("{} attribute", tag.raw)),
                    documentation: doc.map(markup),
                    sort_text: Some(format!("{rank:03}")),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                        range,
                        new_text: format!("{name}=\"\""),
                    })),
                    ..CompletionItem::default()
                })
                .collect()
        }
        CursorContext::AttributeValue { .. }
        | CursorContext::Text { .. }
        | CursorContext::Ignored => Vec::new(),
    }
}

/// The tags offerable as a child of `parent` (`None` = top of the document).
///
/// Driven by [`rules::allowed_children`], the same table the linter validates
/// against, so completion can never suggest something the linter then flags.
#[must_use]
pub fn tag_candidates(parent: Option<MjmlTag>) -> Vec<MjmlTag> {
    let Some(parent) = parent else {
        return vec![MjmlTag::Mjml];
    };
    match rules::allowed_children(parent) {
        Some(allowed) => {
            let mut tags = allowed.to_vec();
            // `mj-include` expands to arbitrary content and is legal anywhere the
            // linter accepts element children, so the table omits it by design.
            tags.push(MjmlTag::MjInclude);
            tags
        }
        // Raw-content components hold HTML, not MJML: offer nothing.
        None if parent.is_ending_tag() => Vec::new(),
        // Unconstrained containers (`mj-attributes`) and unrecognised markup: offer
        // everything rather than nothing.
        None => ALL_TAGS.to_vec(),
    }
}

/// The attribute names offerable on `tag`, most relevant first.
///
/// Required attributes (from [`rules::required_attributes`]) are hoisted to the
/// front so the thing the linter will complain about is the first suggestion.
#[must_use]
pub fn attribute_candidates(tag: MjmlTag) -> Vec<(&'static str, Option<&'static str>)> {
    let mut names: Vec<&'static str> = Vec::new();
    for group in rules::required_attributes(tag) {
        for name in *group {
            if !names.contains(name) {
                names.push(name);
            }
        }
    }
    for (name, _) in docs::tag_attributes(tag) {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    // Usable on any component that `mj-attributes` can target.
    if !names.contains(&"mj-class") && tag != MjmlTag::Unknown {
        names.push("mj-class");
    }
    names
        .into_iter()
        .map(|name| (name, docs::attribute_doc(tag, name)))
        .collect()
}

/// Build a tag completion item.
fn tag_item_named(
    label: &str,
    tag: MjmlTag,
    range: tower_lsp::lsp_types::Range,
    rank: usize,
) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::CLASS),
        detail: docs::tag_summary(tag).map(ToString::to_string),
        documentation: Some(markup(&docs::tag_markdown(tag))),
        sort_text: Some(format!("{rank:03}")),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
            range,
            new_text: label.to_string(),
        })),
        ..CompletionItem::default()
    }
}

fn markup(value: &str) -> tower_lsp::lsp_types::Documentation {
    tower_lsp::lsp_types::Documentation::MarkupContent(MarkupContent {
        kind: MarkupKind::Markdown,
        value: value.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Hover
// ---------------------------------------------------------------------------

/// Hover documentation for the token under `position`.
#[must_use]
pub fn hover(index: &LineIndex, position: Position) -> Option<Hover> {
    let source = index.text();
    let offset = index.offset(position);
    // The cursor context is computed from the text *before* the caret, so extend to
    // the end of the identifier the caret is sitting in to see the whole word.
    let end = word_end(source, offset);

    let (value, range) = match cursor_context(source, end) {
        CursorContext::TagName { prefix, start, .. } => {
            let tag = MjmlTag::from_str(prefix)?;
            (docs::tag_markdown(tag), Some((start, end)))
        }
        CursorContext::AttributeName { tag, prefix, start } if !prefix.is_empty() => (
            docs::attribute_markdown(tag.tag, prefix),
            Some((start, end)),
        ),
        CursorContext::AttributeValue { tag, attribute } => {
            let attribute = attribute?;
            (docs::attribute_markdown(tag.tag, attribute), None)
        }
        _ => return None,
    };

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        range: range.map(|(start, end)| tower_lsp::lsp_types::Range {
            start: index.position(start),
            end: index.position(end),
        }),
    })
}

/// Extend `offset` forward past the rest of the name the caret is inside.
fn word_end(source: &str, offset: usize) -> usize {
    let mut end = offset.min(source.len());
    while end < source.len() && !source.is_char_boundary(end) {
        end += 1;
    }
    for (rel, ch) in source[end..].char_indices() {
        if !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == ':') {
            return end + rel;
        }
    }
    source.len()
}

// ---------------------------------------------------------------------------
// Document symbols
// ---------------------------------------------------------------------------

/// The nested element outline of `source`.
#[must_use]
pub fn document_symbols(index: &LineIndex) -> Vec<DocumentSymbol> {
    let allocator = Allocator::default();
    let parsed = mjmx_parser::parse(index.text(), &allocator);
    symbols_of(&parsed.document, index)
}

fn symbols_of(document: &Document<'_>, index: &LineIndex) -> Vec<DocumentSymbol> {
    document
        .children
        .iter()
        .filter_map(Node::as_element)
        .map(|element| symbol_of(element, index))
        .collect()
}

fn symbol_of(element: &Element<'_>, index: &LineIndex) -> DocumentSymbol {
    let children: Vec<DocumentSymbol> = element
        .children
        .iter()
        .filter_map(Node::as_element)
        .map(|child| symbol_of(child, index))
        .collect();

    let range = index.range(element.span);
    // The name occupies `<` + the tag text; that is what the editor highlights when
    // the symbol is selected.
    let name_start = element.span.start as usize + 1;
    let selection_range = tower_lsp::lsp_types::Range {
        start: index.position(name_start),
        end: index.position(name_start + element.raw_tag.as_str().len()),
    };

    #[allow(deprecated)] // `DocumentSymbol::deprecated` is required by the struct.
    DocumentSymbol {
        name: element.raw_tag.as_str().to_string(),
        detail: symbol_detail(element),
        kind: if children.is_empty() {
            SymbolKind::FIELD
        } else {
            SymbolKind::OBJECT
        },
        tags: None,
        deprecated: None,
        range,
        selection_range,
        children: if children.is_empty() {
            None
        } else {
            Some(children)
        },
    }
}

/// A short `detail` string for the outline: the most identifying attribute.
fn symbol_detail(element: &Element<'_>) -> Option<String> {
    for name in ["name", "src", "href", "mj-class", "css-class", "width"] {
        if let Some(value) = element.attr(name) {
            let value = if value.len() > 40 {
                let mut cut = 40;
                while cut > 0 && !value.is_char_boundary(cut) {
                    cut -= 1;
                }
                format!("{}…", &value[..cut])
            } else {
                value.to_string()
            };
            return Some(format!("{name}=\"{value}\""));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/// A single whole-document [`TextEdit`] reformatting `source`, if it changes.
///
/// Returns `None` when the document already matches canonical style, and also when
/// parsing produced hard errors — reformatting a document the parser only partially
/// recovered could drop text the user is still typing.
#[must_use]
pub fn formatting(index: &LineIndex, options: &FormatOptions) -> Option<Vec<TextEdit>> {
    let source = index.text();
    let allocator = Allocator::default();
    let parsed = mjmx_parser::parse(source, &allocator);
    if parsed.panicked
        || parsed
            .errors
            .iter()
            .any(|error| error.severity == Severity::Error)
    {
        return None;
    }
    let formatted = mjmx_formatter::format(&parsed.document, options);
    if formatted == source {
        return None;
    }
    Some(vec![TextEdit {
        range: index.full_range(),
        new_text: formatted,
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uri() -> Url {
        Url::parse("file:///test.mjml").unwrap()
    }

    fn at(source: &str, needle: &str, extra: usize) -> (LineIndex, Position) {
        let index = LineIndex::new(source);
        let offset = source.find(needle).unwrap() + extra;
        let position = index.position(offset);
        (index, position)
    }

    // -- diagnostics --------------------------------------------------------

    #[test]
    fn a_disallowed_child_is_reported_as_an_error_on_the_child() {
        let source = "<mjml>\n  <mj-body>\n    <mj-text>hi</mj-text>\n  </mj-body>\n</mjml>\n";
        let index = LineIndex::new(source);
        let diagnostics = diagnostics(source, &index, &uri());
        assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diagnostic.source.as_deref(), Some("mjmx"));
        assert!(diagnostic.message.contains("mj-text"));
        // Anchored on line 2 (0-based), at the `<mj-text>` element.
        assert_eq!(diagnostic.range.start.line, 2);
        assert_eq!(diagnostic.range.start.character, 4);
    }

    #[test]
    fn a_clean_document_produces_no_diagnostics() {
        let source = "<mjml><mj-body><mj-section><mj-column><mj-text>hi</mj-text></mj-column></mj-section></mj-body></mjml>";
        let index = LineIndex::new(source);
        assert!(diagnostics(source, &index, &uri()).is_empty());
    }

    #[test]
    fn diagnostic_ranges_are_utf16_correct_on_multibyte_lines() {
        let source = "<mjml>\n  <mj-body>café<mj-text>hi</mj-text></mj-body>\n</mjml>\n";
        let index = LineIndex::new(source);
        let diagnostics = diagnostics(source, &index, &uri());
        assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
        // `  <mj-body>café` is 15 UTF-16 units even though it is 16 bytes.
        assert_eq!(diagnostics[0].range.start.line, 1);
        assert_eq!(diagnostics[0].range.start.character, 15);
    }

    #[test]
    fn parser_errors_and_lint_findings_are_both_published() {
        let source = "<mjml><mj-body><mj-section></mjml>";
        let index = LineIndex::new(source);
        let diagnostics = diagnostics(source, &index, &uri());
        assert!(
            diagnostics
                .iter()
                .any(|d| d.severity == Some(DiagnosticSeverity::ERROR)),
            "{diagnostics:#?}"
        );
    }

    #[test]
    fn help_text_is_appended_to_the_message() {
        let diagnostic = mjmx_diagnostics::Diagnostic::warning("something")
            .with_label(mjmx_span::Span::new(0, 4), "here")
            .with_help("try this");
        let index = LineIndex::new("abcdefg");
        let converted = convert_diagnostic(&diagnostic, &index, &uri());
        assert_eq!(converted.severity, Some(DiagnosticSeverity::WARNING));
        assert!(converted.message.contains("help: try this"));
        assert_eq!(converted.range.end.character, 4);
    }

    #[test]
    fn extra_labels_become_related_information() {
        let diagnostic = mjmx_diagnostics::Diagnostic::error("two labels")
            .with_label(mjmx_span::Span::new(0, 1), "first")
            .with_label(mjmx_span::Span::new(2, 3), "second");
        let index = LineIndex::new("abcdefg");
        let converted = convert_diagnostic(&diagnostic, &index, &uri());
        let related = converted.related_information.unwrap();
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].message, "second");
    }

    #[test]
    fn an_unlabelled_diagnostic_anchors_at_the_document_start() {
        let diagnostic = mjmx_diagnostics::Diagnostic::advice("no labels");
        let index = LineIndex::new("abc");
        let converted = convert_diagnostic(&diagnostic, &index, &uri());
        assert_eq!(converted.severity, Some(DiagnosticSeverity::HINT));
        assert_eq!(converted.range.start, converted.range.end);
        assert_eq!(converted.range.start.line, 0);
    }

    // -- completion ---------------------------------------------------------

    fn labels(items: &[CompletionItem]) -> Vec<&str> {
        items.iter().map(|item| item.label.as_str()).collect()
    }

    #[test]
    fn inside_a_section_only_section_children_are_offered() {
        let source = "<mjml><mj-body><mj-section><";
        let index = LineIndex::new(source);
        let items = completion(&index, index.position(source.len()));
        let labels = labels(&items);
        assert!(labels.contains(&"mj-column"), "{labels:?}");
        assert!(labels.contains(&"mj-group"), "{labels:?}");
        assert!(labels.contains(&"mj-raw"), "{labels:?}");
        assert!(!labels.contains(&"mj-text"), "{labels:?}");
        assert!(!labels.contains(&"mj-section"), "{labels:?}");
    }

    #[test]
    fn inside_a_column_content_components_are_offered() {
        let source = "<mjml><mj-body><mj-section><mj-column><mj-";
        let index = LineIndex::new(source);
        let items = completion(&index, index.position(source.len()));
        let labels = labels(&items);
        assert!(labels.contains(&"mj-text"));
        assert!(labels.contains(&"mj-image"));
        assert!(!labels.contains(&"mj-column"));
        // The edit replaces the partial name the user already typed.
        let edit = match items[0].text_edit.as_ref().unwrap() {
            CompletionTextEdit::Edit(edit) => edit,
            other => panic!("expected a plain edit, got {other:?}"),
        };
        assert_eq!(edit.range.start.character, (source.len() - 3) as u32);
    }

    #[test]
    fn completion_carries_documentation() {
        let source = "<mjml><mj-body><mj-section><mj-column><";
        let index = LineIndex::new(source);
        let items = completion(&index, index.position(source.len()));
        let text = items
            .iter()
            .find(|item| item.label == "mj-image")
            .expect("mj-image offered");
        assert!(text.detail.is_some());
        assert!(text.documentation.is_some());
    }

    #[test]
    fn at_the_top_of_the_document_only_mjml_is_offered() {
        let source = "<";
        let index = LineIndex::new(source);
        assert_eq!(labels(&completion(&index, index.position(1))), ["mjml"]);
    }

    #[test]
    fn a_closing_tag_completes_the_enclosing_element() {
        let source = "<mjml><mj-body><mj-section></";
        let index = LineIndex::new(source);
        assert_eq!(
            labels(&completion(&index, index.position(source.len()))),
            ["mj-section"]
        );
    }

    #[test]
    fn raw_content_components_offer_no_mjml_tags() {
        let source = "<mjml><mj-body><mj-section><mj-column><mj-text><";
        let index = LineIndex::new(source);
        assert!(completion(&index, index.position(source.len())).is_empty());
    }

    #[test]
    fn attribute_completion_hoists_required_attributes() {
        let source = "<mjml><mj-body><mj-section><mj-column><mj-image ";
        let index = LineIndex::new(source);
        let items = completion(&index, index.position(source.len()));
        assert_eq!(items[0].label, "src");
        assert_eq!(items[0].kind, Some(CompletionItemKind::PROPERTY));
        let labels = labels(&items);
        assert!(labels.contains(&"alt"));
        assert!(labels.contains(&"padding"));
        assert!(labels.contains(&"mj-class"));
        match items[0].text_edit.as_ref().unwrap() {
            CompletionTextEdit::Edit(edit) => assert_eq!(edit.new_text, "src=\"\""),
            other => panic!("expected a plain edit, got {other:?}"),
        }
    }

    #[test]
    fn no_completion_inside_an_attribute_value_or_plain_text() {
        for source in [
            "<mjml><mj-body><mj-section><mj-column><mj-image src=\"htt",
            "<mjml><mj-body><mj-section><mj-column><mj-text>hello ",
        ] {
            let index = LineIndex::new(source);
            assert!(
                completion(&index, index.position(source.len())).is_empty(),
                "expected no completions for {source:?}"
            );
        }
    }

    #[test]
    fn tag_candidates_match_the_linter_tables() {
        assert_eq!(
            tag_candidates(Some(MjmlTag::MjBody)),
            vec![
                MjmlTag::MjSection,
                MjmlTag::MjWrapper,
                MjmlTag::MjHero,
                MjmlTag::MjRaw,
                MjmlTag::MjInclude,
            ]
        );
        assert!(tag_candidates(Some(MjmlTag::MjText)).is_empty());
        assert!(tag_candidates(Some(MjmlTag::Unknown)).len() > 10);
    }

    // -- hover --------------------------------------------------------------

    #[test]
    fn hovering_a_tag_name_documents_the_component() {
        let source = "<mjml><mj-body><mj-section><mj-column><mj-image src=\"a.png\" /></mj-column></mj-section></mj-body></mjml>";
        let (index, position) = at(source, "<mj-image", 4);
        let hover = hover(&index, position).expect("hover on mj-image");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup");
        };
        assert!(markup.value.contains("An image"));
        assert!(markup.value.contains("`src`"));
        let range = hover.range.unwrap();
        assert_eq!(
            index.offset(range.start),
            source.find("mj-image").unwrap(),
            "hover range covers the whole tag name"
        );
    }

    #[test]
    fn hovering_an_attribute_name_documents_the_attribute() {
        let source = "<mjml><mj-body><mj-section><mj-column><mj-image src=\"a.png\" /></mj-column></mj-section></mj-body></mjml>";
        let (index, position) = at(source, "src=", 1);
        let hover = hover(&index, position).expect("hover on src");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup");
        };
        assert!(markup.value.contains("`src`"));
        assert!(markup.value.contains("Image URL"));
    }

    #[test]
    fn hovering_an_undocumented_attribute_says_so() {
        let source = "<mjml><mj-body><mj-section wibble=\"1\"></mj-section></mj-body></mjml>";
        let (index, position) = at(source, "wibble", 2);
        let hover = hover(&index, position).expect("hover on wibble");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup");
        };
        assert!(markup.value.contains("No built-in documentation"));
    }

    #[test]
    fn hovering_plain_text_returns_nothing() {
        let source = "<mjml><mj-body><mj-section><mj-column><mj-text>hello</mj-text></mj-column></mj-section></mj-body></mjml>";
        let (index, position) = at(source, "hello", 2);
        assert!(hover(&index, position).is_none());
    }

    #[test]
    fn hover_works_on_a_multibyte_line() {
        let source = "<mjml>\n<!-- café 🎉 -->\n<mj-body></mj-body>\n</mjml>";
        let (index, position) = at(source, "<mj-body", 3);
        let hover = hover(&index, position).expect("hover on mj-body");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markup");
        };
        assert!(markup.value.contains("Document body"));
    }

    // -- document symbols ---------------------------------------------------

    #[test]
    fn document_symbols_mirror_the_element_tree() {
        let source = "<mjml>\n  <mj-head>\n    <mj-title>Hi</mj-title>\n  </mj-head>\n  <mj-body>\n    <mj-section>\n      <mj-column>\n        <mj-image src=\"a.png\" />\n      </mj-column>\n    </mj-section>\n  </mj-body>\n</mjml>\n";
        let index = LineIndex::new(source);
        let symbols = document_symbols(&index);

        assert_eq!(symbols.len(), 1);
        let root = &symbols[0];
        assert_eq!(root.name, "mjml");
        assert_eq!(root.kind, SymbolKind::OBJECT);
        assert_eq!(root.range.start.line, 0);
        assert_eq!(root.selection_range.start.character, 1);
        assert_eq!(root.selection_range.end.character, 5);

        let top: Vec<&str> = root
            .children
            .as_ref()
            .unwrap()
            .iter()
            .map(|child| child.name.as_str())
            .collect();
        assert_eq!(top, ["mj-head", "mj-body"]);

        let body = &root.children.as_ref().unwrap()[1];
        let section = &body.children.as_ref().unwrap()[0];
        let column = &section.children.as_ref().unwrap()[0];
        let image = &column.children.as_ref().unwrap()[0];
        assert_eq!(image.name, "mj-image");
        assert_eq!(image.kind, SymbolKind::FIELD);
        assert_eq!(image.detail.as_deref(), Some("src=\"a.png\""));
        assert!(image.children.is_none());
    }

    #[test]
    fn document_symbols_of_an_empty_document_are_empty() {
        let index = LineIndex::new("");
        assert!(document_symbols(&index).is_empty());
    }

    // -- formatting ---------------------------------------------------------

    #[test]
    fn formatting_returns_a_full_document_edit() {
        let source = "<mjml><mj-body><mj-section><mj-column><mj-text>hi</mj-text></mj-column></mj-section></mj-body></mjml>";
        let index = LineIndex::new(source);
        let edits = formatting(&index, &FormatOptions::default()).expect("an edit");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].range, index.full_range());
        assert_eq!(
            edits[0].new_text,
            "<mjml>\n  <mj-body>\n    <mj-section>\n      <mj-column>\n        <mj-text>hi</mj-text>\n      </mj-column>\n    </mj-section>\n  </mj-body>\n</mjml>\n"
        );
    }

    #[test]
    fn formatting_an_already_canonical_document_is_a_no_op() {
        // The formatter self-closes empty elements, so this *is* its own output.
        let source = "<mjml>\n  <mj-body />\n</mjml>\n";
        let index = LineIndex::new(source);
        assert!(formatting(&index, &FormatOptions::default()).is_none());
    }

    #[test]
    fn formatting_honours_the_indent_width() {
        let source = "<mjml><mj-body></mj-body></mjml>";
        let index = LineIndex::new(source);
        let edits = formatting(&index, &FormatOptions { indent_width: 4 }).expect("an edit");
        assert!(edits[0].new_text.contains("\n    <mj-body"));
    }

    #[test]
    fn a_document_with_syntax_errors_is_not_reformatted() {
        let source = "<mjml><mj-body></mjml>";
        let index = LineIndex::new(source);
        assert!(formatting(&index, &FormatOptions::default()).is_none());
    }
}
