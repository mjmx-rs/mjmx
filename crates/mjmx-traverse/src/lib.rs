//! A read-only AST visitor for `mjmx` — the shared, immutable traversal.
//!
//! This is the `oxc_ast_visit`/`syn::visit` analogue: one small, dependency-light
//! [`Visit`] trait with default methods that delegate to free `walk_*` functions.
//! Overriding a `visit_*` method lets a visitor **observe** a node and then choose to
//! recurse (by calling the matching walker) or stop. The default implementations
//! recurse, so a visitor that overrides nothing simply walks the whole tree.
//!
//! It is deliberately read-only: it borrows the arena tree by shared reference and
//! never mutates it. Tools that need to *rewrite* the tree (e.g. `mjmx-transformer`'s
//! `mj-include` resolution) rebuild via [`AstBuilder`](mjmx_ast::AstBuilder) instead.
//! The linter, formatter, and future LSP are expected to adopt this trait so tree
//! traversal lives in exactly one place.
//!
//! # Example
//!
//! ```
//! use mjmx_allocator::Allocator;
//! use mjmx_ast::Element;
//! use mjmx_traverse::{walk_element, Visit};
//!
//! /// Count how many elements carry each tag.
//! #[derive(Default)]
//! struct TagCounter {
//!     mj_text: usize,
//! }
//!
//! impl<'a> Visit<'a> for TagCounter {
//!     fn visit_element(&mut self, el: &Element<'a>) {
//!         if el.raw_tag.as_str() == "mj-text" {
//!             self.mj_text += 1;
//!         }
//!         walk_element(self, el); // keep descending
//!     }
//! }
//!
//! let alloc = Allocator::new();
//! let ret = mjmx_parser::parse(
//!     "<mjml><mj-body><mj-section><mj-column>\
//!      <mj-text>a</mj-text><mj-text>b</mj-text></mj-column></mj-section></mj-body></mjml>",
//!     &alloc,
//! );
//! let mut counter = TagCounter::default();
//! counter.visit_document(&ret.document);
//! assert_eq!(counter.mj_text, 2);
//! ```

use mjmx_ast::{Comment, Document, Element, Node, Text};

/// A read-only visitor over the `mjmx` AST.
///
/// Every method has a default body that recurses via the corresponding `walk_*`
/// function, so implementors only override the nodes they care about. To keep
/// descending after observing a node, call the matching walker (e.g.
/// [`walk_element`]); to prune the subtree, simply return without calling it.
///
/// The lifetime `'a` is the document arena: the visitor borrows nodes for the
/// duration of the traversal and cannot outlive the tree.
pub trait Visit<'a>: Sized {
    /// Visit the document root, then its top-level nodes.
    fn visit_document(&mut self, doc: &Document<'a>) {
        walk_document(self, doc);
    }

    /// Visit a single node (element, text, or comment).
    fn visit_node(&mut self, node: &Node<'a>) {
        walk_node(self, node);
    }

    /// Visit an element, then its attributes' children.
    fn visit_element(&mut self, element: &Element<'a>) {
        walk_element(self, element);
    }

    /// Visit a text node. Leaf: the default does nothing.
    fn visit_text(&mut self, text: &Text<'a>) {
        let _ = text;
    }

    /// Visit a comment node. Leaf: the default does nothing.
    fn visit_comment(&mut self, comment: &Comment<'a>) {
        let _ = comment;
    }
}

/// Walk every top-level node of `doc` in source order.
pub fn walk_document<'a, V: Visit<'a>>(visitor: &mut V, doc: &Document<'a>) {
    for node in &doc.children {
        visitor.visit_node(node);
    }
}

/// Dispatch a node to the matching `visit_*` method by kind.
pub fn walk_node<'a, V: Visit<'a>>(visitor: &mut V, node: &Node<'a>) {
    match node {
        Node::Element(el) => visitor.visit_element(el),
        Node::Text(text) => visitor.visit_text(text),
        Node::Comment(comment) => visitor.visit_comment(comment),
    }
}

/// Walk an element's children in source order.
///
/// Attributes are leaves with no sub-nodes, so only child nodes are descended into.
pub fn walk_element<'a, V: Visit<'a>>(visitor: &mut V, element: &Element<'a>) {
    for child in &element.children {
        visitor.visit_node(child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mjmx_allocator::Allocator;
    use std::collections::BTreeMap;

    /// A visitor that counts elements grouped by their raw tag string.
    #[derive(Default)]
    struct CountByTag {
        counts: BTreeMap<String, usize>,
    }

    impl<'a> Visit<'a> for CountByTag {
        fn visit_element(&mut self, element: &Element<'a>) {
            *self
                .counts
                .entry(element.raw_tag.as_str().to_string())
                .or_default() += 1;
            walk_element(self, element);
        }
    }

    #[test]
    fn counts_elements_by_tag() {
        let alloc = Allocator::new();
        let ret = mjmx_parser::parse(
            "<mjml><mj-body>\
               <mj-section><mj-column>\
                 <mj-text>a</mj-text><mj-text>b</mj-text>\
               </mj-column></mj-section>\
               <mj-section><mj-column><mj-image src=\"x\" /></mj-column></mj-section>\
             </mj-body></mjml>",
            &alloc,
        );

        let mut counter = CountByTag::default();
        counter.visit_document(&ret.document);

        assert_eq!(counter.counts["mjml"], 1);
        assert_eq!(counter.counts["mj-body"], 1);
        assert_eq!(counter.counts["mj-section"], 2);
        assert_eq!(counter.counts["mj-column"], 2);
        assert_eq!(counter.counts["mj-text"], 2);
        assert_eq!(counter.counts["mj-image"], 1);
    }

    /// A visitor can prune: overriding `visit_element` without calling the walker
    /// stops descent, so nested elements are not counted.
    #[test]
    fn pruning_visitor_does_not_descend() {
        let alloc = Allocator::new();
        let ret = mjmx_parser::parse(
            "<mjml><mj-body><mj-section><mj-column>x</mj-column></mj-section></mj-body></mjml>",
            &alloc,
        );

        struct TopOnly {
            seen: usize,
        }
        impl<'a> Visit<'a> for TopOnly {
            fn visit_element(&mut self, _element: &Element<'a>) {
                self.seen += 1;
                // Deliberately do not call walk_element: stop at the first element.
            }
        }

        let mut v = TopOnly { seen: 0 };
        v.visit_document(&ret.document);
        assert_eq!(v.seen, 1, "only the top-level <mjml> should be visited");
    }
}
