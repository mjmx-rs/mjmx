//! Arena-allocated, spanned AST for `mrml`.
//!
//! MJML is uniform — every element is a tag plus attributes plus children — so the
//! node model is small: [`Element`], [`Text`], [`Comment`], wrapped by the [`Node`]
//! enum, with a [`Document`] root.
//!
//! # Arena model
//!
//! All nodes carry a lifetime `'a` and borrow from a single
//! [`Allocator`](mrml_allocator::Allocator) arena. Strings are [`Atom`]s (copied
//! into the arena) and child lists are arena [`Vec`](mrml_allocator::Vec)s — no node
//! owns heap memory of its own, and the whole tree is freed at once when the arena
//! drops. [`Node`] is `Copy`: an element is stored once in the arena and referenced
//! by a cheap shared `&'a Element<'a>`.
//!
//! Construct nodes through [`AstBuilder`], which threads one borrowed arena through
//! the whole parse.
//!
//! ```
//! use mrml_ast::{AstBuilder, MjmlTag, Span};
//! use mrml_allocator::Allocator;
//!
//! let alloc = Allocator::new();
//! let b = AstBuilder::new(&alloc);
//!
//! let text = b.text(b.atom("hi"), Span::new(0, 2));
//! let mj_text = b.element(
//!     MjmlTag::MjText,
//!     b.atom("mj-text"),
//!     b.vec(),
//!     b.vec_from_iter([b.text_node(text)]),
//!     Span::new(0, 2),
//! );
//! assert_eq!(mj_text.tag, MjmlTag::MjText);
//! ```

pub mod builder;
pub mod document;
pub mod node;

pub use builder::AstBuilder;
pub use document::Document;
pub use node::{Attribute, Comment, Element, Node, Text};

pub use mrml_span::{Atom, GetSpan, Span};
pub use mrml_syntax::MjmlTag;

#[cfg(test)]
mod tests {
    use super::*;
    use mrml_allocator::Allocator;

    /// Build `<mjml><mj-body><mj-section background-color="#fff"><mj-column>
    /// <mj-text>hi</mj-text></mj-column></mj-section></mj-body></mjml>`.
    #[test]
    fn builds_and_walks_a_nested_tree() {
        let alloc = Allocator::new();
        let b = AstBuilder::new(&alloc);

        let text = b.text(b.atom("hi"), Span::new(40, 42));
        let mj_text = b.element(
            MjmlTag::MjText,
            b.atom("mj-text"),
            b.vec(),
            b.vec_from_iter([b.text_node(text)]),
            Span::new(30, 55),
        );

        let mj_column = b.element(
            MjmlTag::MjColumn,
            b.atom("mj-column"),
            b.vec(),
            b.vec_from_iter([b.element_node(mj_text)]),
            Span::new(20, 65),
        );

        let section_attrs = b.vec_from_iter([b.attribute(
            b.atom("background-color"),
            Some(b.atom("#fff")),
            Span::new(12, 34),
        )]);
        let mj_section = b.element(
            MjmlTag::MjSection,
            b.atom("mj-section"),
            section_attrs,
            b.vec_from_iter([b.element_node(mj_column)]),
            Span::new(10, 75),
        );

        let mj_body = b.element(
            MjmlTag::MjBody,
            b.atom("mj-body"),
            b.vec(),
            b.vec_from_iter([b.element_node(mj_section)]),
            Span::new(6, 85),
        );

        let mjml = b.element(
            MjmlTag::Mjml,
            b.atom("mjml"),
            b.vec(),
            b.vec_from_iter([b.element_node(mj_body)]),
            Span::new(0, 92),
        );

        let doc = b.document(b.vec_from_iter([b.element_node(mjml)]), Span::new(0, 92));

        // root_element resolves to <mjml>.
        let root = doc.root_element().expect("root element");
        assert_eq!(root.tag, MjmlTag::Mjml);
        assert_eq!(root.raw_tag.as_str(), "mjml");

        // Walk mjml -> mj-body -> mj-section.
        let body = root.element_children().next().expect("mj-body");
        assert_eq!(body.tag, MjmlTag::MjBody);
        let section = body.element_children().next().expect("mj-section");
        assert_eq!(section.tag, MjmlTag::MjSection);

        // Attribute accessors on mj-section.
        assert_eq!(section.attr("background-color"), Some("#fff"));
        assert!(section.has_attr("background-color"));
        assert!(!section.has_attr("color"));
        assert_eq!(section.attr("color"), None);

        // Walk to mj-text and check its single text child.
        let column = section.element_children().next().expect("mj-column");
        let text_el = column.element_children().next().expect("mj-text");
        assert_eq!(text_el.tag, MjmlTag::MjText);
        assert_eq!(text_el.element_children().count(), 0);

        let child = text_el.children().next().expect("text child");
        assert!(child.is_text());
        match child {
            Node::Text(t) => assert_eq!(t.value.as_str(), "hi"),
            _ => panic!("expected text node"),
        }

        // GetSpan returns the spans we set.
        assert_eq!(text_el.span(), Span::new(30, 55));
        assert_eq!(doc.span(), Span::new(0, 92));
        assert_eq!(child.span(), Span::new(40, 42));
    }

    #[test]
    fn as_element_and_element_children_filter() {
        let alloc = Allocator::new();
        let b = AstBuilder::new(&alloc);

        let el = b.element(
            MjmlTag::MjText,
            b.atom("mj-text"),
            b.vec(),
            b.vec(),
            Span::new(0, 10),
        );

        let element_node = b.element_node(el);
        let text_node = b.text_node(b.text(b.atom("x"), Span::new(0, 1)));
        let comment_node = b.comment_node(b.comment(b.atom(" c "), Span::new(0, 5)));

        assert!(element_node.as_element().is_some());
        assert!(text_node.as_element().is_none());
        assert!(comment_node.is_comment());
        assert!(text_node.is_text());

        // A parent holding one element and two non-element children.
        let parent = b.element(
            MjmlTag::MjBody,
            b.atom("mj-body"),
            b.vec(),
            b.vec_from_iter([element_node, text_node, comment_node]),
            Span::new(0, 20),
        );
        assert_eq!(parent.children().count(), 3);
        assert_eq!(parent.element_children().count(), 1);
    }

    #[test]
    fn valueless_attribute() {
        let alloc = Allocator::new();
        let b = AstBuilder::new(&alloc);

        let attr = b.attribute(b.atom("disabled"), None, Span::new(0, 8));
        assert_eq!(attr.name.as_str(), "disabled");
        assert_eq!(attr.value, None);
        assert_eq!(attr.value_str(), None);
        assert_eq!(attr.span(), Span::new(0, 8));
    }
}
