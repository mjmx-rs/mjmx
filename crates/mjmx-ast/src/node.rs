//! The AST node model.
//!
//! MJML is uniform: every element is a tag plus attributes plus children, so the
//! node set is deliberately small. All nodes carry a [`Span`] and borrow from the
//! arena for a shared lifetime `'a`; strings are [`Atom`]s and child lists are
//! arena [`Vec`]s. Nothing here owns heap memory of its own.

use mjmx_allocator::Vec;
use mjmx_span::{Atom, GetSpan, Span};
use mjmx_syntax::MjmlTag;

/// A run of text content (character data between tags).
#[derive(Debug, Clone, Copy)]
pub struct Text<'a> {
    /// The raw text value.
    pub value: Atom<'a>,
    /// Source span covering the text.
    pub span: Span,
}

/// An HTML/MJML comment (`<!-- ... -->`).
#[derive(Debug, Clone, Copy)]
pub struct Comment<'a> {
    /// The comment body (without the delimiters).
    pub value: Atom<'a>,
    /// Source span covering the comment, delimiters included.
    pub span: Span,
}

/// A single element attribute, e.g. `background-color="#fff"`.
///
/// The value is optional so that valueless (boolean) attributes are representable.
#[derive(Debug, Clone, Copy)]
pub struct Attribute<'a> {
    /// The attribute name.
    pub name: Atom<'a>,
    /// The attribute value, or `None` for a valueless attribute.
    pub value: Option<Atom<'a>>,
    /// Source span covering the whole `name="value"` pair.
    pub span: Span,
}

impl<'a> Attribute<'a> {
    /// The attribute value as a plain `&str`, or `None` if valueless.
    #[must_use]
    pub fn value_str(&self) -> Option<&'a str> {
        self.value.map(|v| v.as_str())
    }
}

/// An MJML element: a tag with attributes and children.
#[derive(Debug)]
pub struct Element<'a> {
    /// The recognized tag kind (`MjmlTag::Unknown` for unrecognized tags).
    pub tag: MjmlTag,
    /// The raw tag string exactly as it appeared in the source.
    pub raw_tag: Atom<'a>,
    /// The element's attributes, in source order.
    pub attributes: Vec<'a, Attribute<'a>>,
    /// The element's children, in source order.
    pub children: Vec<'a, Node<'a>>,
    /// Source span covering the element, from open tag to close tag.
    pub span: Span,
}

impl<'a> Element<'a> {
    /// The value of the first attribute named `name`, or `None`.
    #[must_use]
    pub fn attr(&self, name: &str) -> Option<&'a str> {
        self.attributes
            .iter()
            .find(|a| a.name.as_str() == name)
            .and_then(Attribute::value_str)
    }

    /// Whether an attribute named `name` is present (regardless of value).
    #[must_use]
    pub fn has_attr(&self, name: &str) -> bool {
        self.attributes.iter().any(|a| a.name.as_str() == name)
    }

    /// Iterate over all child nodes.
    pub fn children(&self) -> impl Iterator<Item = &Node<'a>> {
        self.children.iter()
    }

    /// Iterate over only the children that are elements.
    pub fn element_children(&self) -> impl Iterator<Item = &'a Element<'a>> + '_ {
        self.children.iter().filter_map(Node::as_element)
    }
}

/// A node in the tree: an element reference, text, or a comment.
#[derive(Debug, Clone, Copy)]
pub enum Node<'a> {
    /// An element, allocated in the arena and referenced by shared borrow.
    Element(&'a Element<'a>),
    /// A text node.
    Text(Text<'a>),
    /// A comment node.
    Comment(Comment<'a>),
}

impl<'a> Node<'a> {
    /// The inner element reference if this node is an [`Node::Element`].
    #[must_use]
    pub fn as_element(&self) -> Option<&'a Element<'a>> {
        match self {
            Node::Element(el) => Some(el),
            _ => None,
        }
    }

    /// Whether this node is an element.
    #[must_use]
    pub fn is_element(&self) -> bool {
        matches!(self, Node::Element(_))
    }

    /// Whether this node is a text node.
    #[must_use]
    pub fn is_text(&self) -> bool {
        matches!(self, Node::Text(_))
    }

    /// Whether this node is a comment node.
    #[must_use]
    pub fn is_comment(&self) -> bool {
        matches!(self, Node::Comment(_))
    }
}

impl GetSpan for Text<'_> {
    fn span(&self) -> Span {
        self.span
    }
}

impl GetSpan for Comment<'_> {
    fn span(&self) -> Span {
        self.span
    }
}

impl GetSpan for Attribute<'_> {
    fn span(&self) -> Span {
        self.span
    }
}

impl GetSpan for Element<'_> {
    fn span(&self) -> Span {
        self.span
    }
}

impl GetSpan for Node<'_> {
    fn span(&self) -> Span {
        match self {
            Node::Element(el) => el.span,
            Node::Text(t) => t.span,
            Node::Comment(c) => c.span,
        }
    }
}
