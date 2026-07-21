//! The `mjmx` parser: MJML source text into an arena-allocated, spanned AST.
//!
//! A hand-rolled recursive-descent parser (no external dependencies) walks the
//! source as a byte cursor and constructs nodes through
//! [`AstBuilder`](mjmx_ast::AstBuilder). It is deliberately forgiving: it **never
//! bails on the first error**. Malformed input yields a
//! [`Diagnostic`](mjmx_diagnostics::Diagnostic) and parsing continues with a
//! best-effort recovery, so [`parse`] always returns a (possibly partial)
//! [`Document`](mjmx_ast::Document).
//!
//! # Behaviors
//!
//! - **Elements & nesting** — `<mj-section> ... </mj-section>`. Tag text maps through
//!   [`MjmlTag::from_str`](mjmx_syntax::MjmlTag::from_str); unrecognized tags become
//!   [`MjmlTag::Unknown`](mjmx_syntax::MjmlTag::Unknown) with `raw_tag` preserving the
//!   source spelling.
//! - **Attributes** — `name="v"`, `name='v'`, and valueless `disabled`; whitespace is
//!   tolerated around `=`. Values are stored raw and unescaped (entity decoding is a
//!   later semantic concern).
//! - **Self-closing** — `<mj-image ... />` yields an element with no children.
//! - **Comments** — `<!-- ... -->` become [`Comment`](mjmx_ast::Comment) nodes; the
//!   value is the inner text.
//! - **Raw-content elements** — for
//!   [`is_ending_tag`](mjmx_syntax::MjmlTag::is_ending_tag) elements (mj-text,
//!   mj-button, mj-divider, mj-spacer, mj-raw, mj-accordion-text, mj-accordion-title)
//!   the body is captured verbatim as a single [`Text`](mjmx_ast::Text) node — inner
//!   HTML is preserved, not parsed as MJML.
//! - **Text** — character data between elements is preserved verbatim (whitespace is
//!   **not** trimmed; that is left to a later semantic pass).
//! - **Prolog** — a leading `<?xml ... ?>` declaration and any `<!doctype ...>`
//!   (case-insensitive) are skipped without emitting a node.
//! - **Error recovery** — an unclosed tag at EOF or a mismatched closing tag pushes a
//!   diagnostic, implicitly closes the open element, and parsing continues.
//!   `panicked` is set only on an internal invariant violation (normally `false`).
//!
//! ```
//! use mjmx_allocator::Allocator;
//!
//! let alloc = Allocator::new();
//! let ret = mjmx_parser::parse("<mjml><mj-body></mj-body></mjml>", &alloc);
//! assert!(ret.errors.is_empty());
//! assert!(!ret.panicked);
//! let root = ret.document.root_element().unwrap();
//! assert_eq!(root.tag, mjmx_ast::MjmlTag::Mjml);
//! ```

mod parser;

use mjmx_allocator::Allocator;
use mjmx_ast::Document;
use mjmx_diagnostics::Diagnostic;

pub use parser::Parser;

/// The result of a [`parse`]: the AST plus any diagnostics.
#[derive(Debug)]
pub struct ParserReturn<'a> {
    /// The parsed document (partial if `errors` is non-empty).
    pub document: Document<'a>,
    /// Diagnostics gathered during parsing.
    pub errors: Vec<Diagnostic>,
    /// `true` only if an internal invariant forced a full stop (normally `false`).
    pub panicked: bool,
}

/// Parse `source` into a [`Document`] using `allocator` as the AST arena.
///
/// Always returns; on malformed input the returned document is a best-effort partial
/// tree and `errors` describes what went wrong. See the crate docs for the full list
/// of behaviors and recovery rules.
#[must_use]
pub fn parse<'a>(source: &'a str, allocator: &'a Allocator) -> ParserReturn<'a> {
    let builder = mjmx_ast::AstBuilder::new(allocator);
    let mut parser = Parser::new(source, builder);
    let document = parser.parse_document();
    ParserReturn {
        document,
        errors: parser.errors,
        panicked: parser.panicked,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mjmx_ast::{MjmlTag, Node};

    /// Parse `src`, asserting no diagnostics were produced, and return the document.
    fn parse_ok<'a>(src: &'a str, alloc: &'a Allocator) -> Document<'a> {
        let ret = parse(src, alloc);
        assert!(
            ret.errors.is_empty(),
            "expected no errors, got: {:?}",
            ret.errors
        );
        assert!(!ret.panicked);
        ret.document
    }

    fn only_element<'a>(node: &Node<'a>) -> &'a mjmx_ast::Element<'a> {
        node.as_element().expect("expected element node")
    }

    #[test]
    fn full_minimal_document() {
        let alloc = Allocator::new();
        let src = "<mjml><mj-body><mj-section background-color=\"#fff\"><mj-column>\
                   <mj-text>Hello <b>world</b></mj-text></mj-column></mj-section></mj-body></mjml>";
        let doc = parse_ok(src, &alloc);

        let mjml = doc.root_element().expect("mjml root");
        assert_eq!(mjml.tag, MjmlTag::Mjml);

        let body = mjml.element_children().next().expect("mj-body");
        assert_eq!(body.tag, MjmlTag::MjBody);

        let section = body.element_children().next().expect("mj-section");
        assert_eq!(section.tag, MjmlTag::MjSection);
        assert_eq!(section.attr("background-color"), Some("#fff"));

        let column = section.element_children().next().expect("mj-column");
        assert_eq!(column.tag, MjmlTag::MjColumn);

        let text = column.element_children().next().expect("mj-text");
        assert_eq!(text.tag, MjmlTag::MjText);

        // The mj-text body is a single verbatim Text node (inner HTML preserved).
        assert_eq!(text.children.len(), 1);
        match &text.children[0] {
            Node::Text(t) => assert_eq!(t.value.as_str(), "Hello <b>world</b>"),
            other => panic!("expected single text child, got {other:?}"),
        }
    }

    #[test]
    fn self_closing_element() {
        let alloc = Allocator::new();
        let doc = parse_ok("<mj-image src=\"a.png\" />", &alloc);
        let img = only_element(&doc.children[0]);
        assert_eq!(img.tag, MjmlTag::MjImage);
        assert_eq!(img.children.len(), 0);
        assert_eq!(img.attr("src"), Some("a.png"));
    }

    #[test]
    fn comment_preserved() {
        let alloc = Allocator::new();
        let doc = parse_ok("<!-- hi -->", &alloc);
        match &doc.children[0] {
            Node::Comment(c) => assert_eq!(c.value.as_str(), " hi "),
            other => panic!("expected comment, got {other:?}"),
        }
    }

    #[test]
    fn valueless_attribute() {
        let alloc = Allocator::new();
        let doc = parse_ok("<mj-button disabled>x</mj-button>", &alloc);
        let button = only_element(&doc.children[0]);
        let attr = button
            .attributes
            .iter()
            .find(|a| &*a.name == "disabled")
            .unwrap();
        assert_eq!(attr.value, None);
    }

    #[test]
    fn single_and_double_quotes() {
        let alloc = Allocator::new();
        let doc = parse_ok("<mj-section a='one' b=\"two\"></mj-section>", &alloc);
        let section = only_element(&doc.children[0]);
        assert_eq!(section.attr("a"), Some("one"));
        assert_eq!(section.attr("b"), Some("two"));
    }

    #[test]
    fn unknown_tag() {
        let alloc = Allocator::new();
        let doc = parse_ok("<custom-x>y</custom-x>", &alloc);
        let el = only_element(&doc.children[0]);
        assert_eq!(el.tag, MjmlTag::Unknown);
        assert_eq!(el.raw_tag.as_str(), "custom-x");
        // Its body is parsed as MJML children (a text run), not raw content.
        match &el.children[0] {
            Node::Text(t) => assert_eq!(t.value.as_str(), "y"),
            other => panic!("expected text child, got {other:?}"),
        }
    }

    #[test]
    fn whitespace_between_elements_preserved() {
        let alloc = Allocator::new();
        let doc = parse_ok("<mj-body>\n  <mj-section></mj-section>\n</mj-body>", &alloc);
        let body = only_element(&doc.children[0]);
        // Children: text("\n  "), <mj-section>, text("\n").
        assert_eq!(body.children.len(), 3);
        match &body.children[0] {
            Node::Text(t) => assert_eq!(t.value.as_str(), "\n  "),
            other => panic!("expected leading whitespace text, got {other:?}"),
        }
        assert!(body.children[1].is_element());
        match &body.children[2] {
            Node::Text(t) => assert_eq!(t.value.as_str(), "\n"),
            other => panic!("expected trailing whitespace text, got {other:?}"),
        }
    }

    #[test]
    fn error_recovery_unclosed_body() {
        let alloc = Allocator::new();
        let ret = parse("<mjml><mj-body></mjml>", &alloc);
        assert!(!ret.errors.is_empty(), "expected a diagnostic");
        assert!(!ret.panicked);

        // A partial tree is still returned: mjml > mj-body.
        let mjml = ret.document.root_element().expect("mjml root");
        assert_eq!(mjml.tag, MjmlTag::Mjml);
        let body = mjml.element_children().next().expect("mj-body");
        assert_eq!(body.tag, MjmlTag::MjBody);
        assert_eq!(body.children.len(), 0);
    }

    #[test]
    fn leading_xml_declaration_skipped() {
        let alloc = Allocator::new();
        let doc = parse_ok("<?xml version=\"1.0\"?><mjml></mjml>", &alloc);
        // The declaration emits no node; the first (and only) node is <mjml>.
        assert_eq!(doc.children.len(), 1);
        let mjml = only_element(&doc.children[0]);
        assert_eq!(mjml.tag, MjmlTag::Mjml);
    }

    #[test]
    fn doctype_skipped() {
        let alloc = Allocator::new();
        let doc = parse_ok("<!DOCTYPE html><mjml></mjml>", &alloc);
        assert_eq!(doc.children.len(), 1);
        assert_eq!(only_element(&doc.children[0]).tag, MjmlTag::Mjml);
    }
}
