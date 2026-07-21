//! An oxc-style [`AstBuilder`]: the single entry point for constructing AST nodes
//! in the arena.
//!
//! The builder is a `Copy` handle wrapping a borrowed [`Allocator`]. Every method
//! allocates in that arena, so the parser can thread one builder through the whole
//! parse and every node it produces shares the arena's lifetime.

use mrml_allocator::{Allocator, Vec};
use mrml_span::{Atom, Span};
use mrml_syntax::MjmlTag;

use crate::document::Document;
use crate::node::{Attribute, Comment, Element, Node, Text};

/// A lightweight, copyable handle for building AST nodes in an [`Allocator`] arena.
#[derive(Debug, Clone, Copy)]
pub struct AstBuilder<'a> {
    alloc: &'a Allocator,
}

impl<'a> AstBuilder<'a> {
    /// Create a builder over the given arena.
    #[must_use]
    pub fn new(alloc: &'a Allocator) -> Self {
        Self { alloc }
    }

    /// The arena this builder allocates into.
    #[must_use]
    pub fn allocator(&self) -> &'a Allocator {
        self.alloc
    }

    /// Copy `s` into the arena and wrap it in an [`Atom`].
    ///
    /// The parser passes transient `&str` slices borrowed from its input; copying
    /// into the arena lets the returned [`Atom`] outlive that borrow. A real string
    /// interner will replace the per-call copy later without changing this API.
    #[must_use]
    pub fn atom(&self, s: &str) -> Atom<'a> {
        Atom::new(self.alloc.alloc_str(s))
    }

    /// A new, empty arena-backed [`Vec`].
    #[must_use]
    pub fn vec<T>(&self) -> Vec<'a, T> {
        Vec::new_in(self.alloc)
    }

    /// An arena-backed [`Vec`] populated from `it`.
    pub fn vec_from_iter<T>(&self, it: impl IntoIterator<Item = T>) -> Vec<'a, T> {
        let mut vec = self.vec();
        vec.extend(it);
        vec
    }

    /// Allocate `value` in the arena and return a shared reference to it.
    #[must_use]
    pub fn alloc<T>(&self, value: T) -> &'a T {
        &*self.alloc.alloc(value)
    }

    /// Build an [`Element`] and return a shared arena reference to it.
    #[must_use]
    pub fn element(
        &self,
        tag: MjmlTag,
        raw_tag: Atom<'a>,
        attributes: Vec<'a, Attribute<'a>>,
        children: Vec<'a, Node<'a>>,
        span: Span,
    ) -> &'a Element<'a> {
        self.alloc(Element {
            tag,
            raw_tag,
            attributes,
            children,
            span,
        })
    }

    /// Build an [`Attribute`].
    #[must_use]
    pub fn attribute(&self, name: Atom<'a>, value: Option<Atom<'a>>, span: Span) -> Attribute<'a> {
        Attribute { name, value, span }
    }

    /// Build a [`Text`] node.
    #[must_use]
    pub fn text(&self, value: Atom<'a>, span: Span) -> Text<'a> {
        Text { value, span }
    }

    /// Build a [`Comment`] node.
    #[must_use]
    pub fn comment(&self, value: Atom<'a>, span: Span) -> Comment<'a> {
        Comment { value, span }
    }

    /// Wrap an element reference as a [`Node`].
    #[must_use]
    pub fn element_node(&self, el: &'a Element<'a>) -> Node<'a> {
        Node::Element(el)
    }

    /// Wrap a [`Text`] as a [`Node`].
    #[must_use]
    pub fn text_node(&self, text: Text<'a>) -> Node<'a> {
        Node::Text(text)
    }

    /// Wrap a [`Comment`] as a [`Node`].
    #[must_use]
    pub fn comment_node(&self, comment: Comment<'a>) -> Node<'a> {
        Node::Comment(comment)
    }

    /// Build a [`Document`] root.
    #[must_use]
    pub fn document(&self, children: Vec<'a, Node<'a>>, span: Span) -> Document<'a> {
        Document { children, span }
    }
}
