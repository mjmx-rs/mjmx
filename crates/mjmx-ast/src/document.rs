//! The [`Document`] root produced by parsing a single MJML file.

use mjmx_allocator::Vec;
use mjmx_span::{GetSpan, Span};
use mjmx_syntax::MjmlTag;

use crate::node::{Element, Node};

/// The root of a parsed MJML file.
///
/// A document is a list of top-level nodes (typically a single `<mjml>` element,
/// possibly preceded by comments or stray text) plus the span covering the file.
#[derive(Debug)]
pub struct Document<'a> {
    /// The top-level nodes, in source order.
    pub children: Vec<'a, Node<'a>>,
    /// Source span covering the whole document.
    pub span: Span,
}

impl<'a> Document<'a> {
    /// The root `<mjml>` element, or the first element child if none is tagged
    /// [`MjmlTag::Mjml`].
    #[must_use]
    pub fn root_element(&self) -> Option<&'a Element<'a>> {
        let mut first = None;
        for node in &self.children {
            if let Some(el) = node.as_element() {
                if el.tag == MjmlTag::Mjml {
                    return Some(el);
                }
                if first.is_none() {
                    first = Some(el);
                }
            }
        }
        first
    }
}

impl GetSpan for Document<'_> {
    fn span(&self) -> Span {
        self.span
    }
}
