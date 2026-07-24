//! A read-only element tree recovered from the tokenizer's flat event stream.
//!
//! Only what selector matching and tag rewriting need is kept: the tag name, the
//! `id`/`class` shorthands, the raw attribute list, and enough links (parent,
//! previous element sibling) to walk every combinator. Text is not represented
//! at all — no supported selector can observe it.
//!
//! ## Comments are not markup
//!
//! [`mjmx_html_tokenizer`] yields a whole conditional comment
//! (`<!--[if mso | IE]><table><tr><td><![endif]-->`) as one opaque
//! [`Event::Comment`], and this builder ignores comments entirely. That is
//! load-bearing, not incidental: mjmx merges adjacent Outlook conditionals
//! *before* the inliner runs, where mjml runs juice first. The two orders agree
//! only because juice — a real HTML parser, fed a document where the ghost
//! tables live inside comments — never sees those elements either. An inliner
//! that descended into comments would diverge the moment a merge moved one.

use mjmx_html_tokenizer::{Attr, Event, is_void};

/// One element of the document.
pub(crate) struct Element<'a> {
    /// The lower-cased tag name.
    pub(crate) tag: String,
    /// The `id` attribute, if any.
    pub(crate) id: Option<&'a str>,
    /// The whitespace-separated `class` tokens.
    pub(crate) classes: Vec<&'a str>,
    /// Every attribute, in source order, with its span in the document.
    pub(crate) attrs: Vec<Attr<'a>>,
    /// Byte offset just past the last attribute of the start tag: where a new
    /// attribute is spliced in.
    pub(crate) attrs_end: usize,
    /// Index of the parent element.
    pub(crate) parent: Option<usize>,
    /// Index of the immediately preceding *element* sibling (text and comments
    /// are skipped, as they are for CSS's `+` and `~`).
    pub(crate) prev_sibling: Option<usize>,
    /// Whether a stylesheet may touch this element. False for the elements CSS
    /// inlining always skips, and for everything inside one.
    pub(crate) inlinable: bool,
    /// Byte offset just past this element's start tag `>` — where a `::before`
    /// pseudo-element is spliced (the start of the element's inner content).
    pub(crate) content_start: usize,
    /// Byte offset of this element's `</tag>` — where a `::after` pseudo-element is
    /// spliced. `None` for a void/self-closing element or one never closed (no inner
    /// content to append to).
    pub(crate) content_end: Option<usize>,
    /// Whether the element has any child content — a child element or any text,
    /// including a run of whitespace. This is exactly `:empty`'s negation: `<td></td>`
    /// is empty, but `<td> </td>` and `<td><span></span></td>` are not, matching CSS
    /// Selectors Level 3 (verified against juice).
    pub(crate) has_content: bool,
}

impl<'a> Element<'a> {
    /// The value of `name` (matched case-insensitively) with its span.
    pub(crate) fn attr(&self, name: &str) -> Option<&Attr<'a>> {
        self.attrs
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
    }
}

/// The element tree: a flat arena, since matching only ever walks upwards or
/// backwards and never needs a child list.
pub(crate) struct Tree<'a> {
    pub(crate) elements: Vec<Element<'a>>,
}

/// Elements a stylesheet is never applied to, along with their subtrees. These
/// are the non-rendered parts of the document (metadata and script/style bodies);
/// giving `<style>` a `style` attribute would be meaningless at best.
fn is_skipped(tag: &str) -> bool {
    matches!(
        tag,
        "head" | "title" | "base" | "link" | "style" | "meta" | "script" | "noscript"
    )
}

/// Build the element tree for `html`.
///
/// Nesting is recovered forgivingly, as a browser would: a void or self-closing
/// tag never opens a scope, and an end tag closes the nearest open element of
/// that name (with everything still open inside it), so stray or missing end
/// tags cannot corrupt the rest of the tree.
pub(crate) fn build(html: &str) -> Tree<'_> {
    let mut elements: Vec<Element<'_>> = Vec::new();
    // One frame per open element, plus a root frame for the top level. Each
    // carries the last element seen in it, which is the next child's `+` target.
    let mut stack: Vec<(Option<usize>, Option<usize>)> = vec![(None, None)];

    for event in mjmx_html_tokenizer::tokenize(html) {
        match event {
            Event::Open {
                tag,
                attrs,
                self_closing,
                attrs_end,
                span,
                ..
            } => {
                let frame = stack.last_mut().expect("the root frame is never popped");
                let parent = frame.0;
                let prev_sibling = frame.1;
                let index = elements.len();
                frame.1 = Some(index);
                // A child element is content for its parent's `:empty` test.
                if let Some(p) = parent {
                    elements[p].has_content = true;
                }

                let tag_lower = tag.to_ascii_lowercase();
                let inlinable =
                    !is_skipped(&tag_lower) && parent.is_none_or(|p: usize| elements[p].inlinable);
                let id = attrs
                    .iter()
                    .find(|a| a.name.eq_ignore_ascii_case("id"))
                    .map(|a| a.value);
                let classes = attrs
                    .iter()
                    .find(|a| a.name.eq_ignore_ascii_case("class"))
                    .map_or_else(Vec::new, |a| a.value.split_whitespace().collect());

                let opens_scope = !self_closing && !is_void(tag);
                elements.push(Element {
                    tag: tag_lower,
                    id,
                    classes,
                    attrs,
                    attrs_end,
                    parent,
                    prev_sibling,
                    inlinable,
                    // Inner content starts just past the `>` of the start tag.
                    content_start: span.end,
                    content_end: None,
                    has_content: false,
                });
                if opens_scope {
                    stack.push((Some(index), None));
                }
            }
            Event::Close { tag, span } => {
                let tag = tag.to_ascii_lowercase();
                let matching = stack
                    .iter()
                    .rposition(|(index, _)| index.is_some_and(|i| elements[i].tag == tag));
                if let Some(frame) = matching {
                    // The element whose scope this close ends: record where its inner
                    // content ends (the `<` of the `</tag>`) for `::after` splicing.
                    if let Some(closed) = stack[frame].0 {
                        elements[closed].content_end = Some(span.start);
                    }
                    stack.truncate(frame);
                }
            }
            // Text is not an element, but any non-empty run counts as content for the
            // enclosing element's `:empty` test (whitespace included, per Level 3).
            Event::Text { text, .. } => {
                if !text.is_empty()
                    && let Some(open) = stack.last().and_then(|frame| frame.0)
                {
                    elements[open].has_content = true;
                }
            }
            // Comments and the doctype contribute no elements and no `:empty` content.
            Event::Comment { .. } | Event::DocType { .. } => {}
        }
    }

    Tree { elements }
}

impl<'a> Tree<'a> {
    /// Iterate an element's ancestors, nearest first.
    pub(crate) fn ancestors(&self, index: usize) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(self.elements[index].parent, |&i| self.elements[i].parent)
    }

    /// Iterate an element's preceding element siblings, nearest first.
    pub(crate) fn preceding_siblings(&self, index: usize) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(self.elements[index].prev_sibling, |&i| {
            self.elements[i].prev_sibling
        })
    }

    /// The immediately following element sibling: the element whose `prev_sibling`
    /// points back here. Only `prev_sibling` is stored, so this is recovered by a
    /// scan — there is exactly one such element (or none, at the end of the list).
    fn next_sibling(&self, index: usize) -> Option<usize> {
        self.elements
            .iter()
            .position(|e| e.prev_sibling == Some(index))
    }

    /// Iterate an element's following element siblings, nearest first. Needed for the
    /// `:last-child` / `:nth-last-child` family, which count from the end.
    pub(crate) fn following_siblings(&self, index: usize) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(self.next_sibling(index), move |&i| self.next_sibling(i))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditional_comments_contribute_no_elements() {
        let tree = build(
            "<div><!--[if mso | IE]><table><tr><td><![endif]--><p>x</p>\
             <!--[if mso | IE]></td></tr></table><![endif]--></div>",
        );
        let tags: Vec<&str> = tree.elements.iter().map(|e| e.tag.as_str()).collect();
        assert_eq!(tags, ["div", "p"]);
        assert_eq!(tree.elements[1].parent, Some(0));
    }

    #[test]
    fn void_elements_do_not_open_a_scope() {
        let tree = build("<div><img src=\"a\"><br><span>x</span></div>");
        let span = tree.elements.iter().position(|e| e.tag == "span").unwrap();
        let br = tree.elements.iter().position(|e| e.tag == "br").unwrap();
        assert_eq!(
            tree.elements[span].parent,
            Some(0),
            "span is a child of div"
        );
        assert_eq!(
            tree.elements[span].prev_sibling,
            Some(br),
            "the <br> is a sibling, not an ancestor"
        );
    }

    #[test]
    fn head_content_is_never_inlinable() {
        let tree = build("<html><head><title>t</title></head><body><p>x</p></body></html>");
        for element in &tree.elements {
            let expected = matches!(element.tag.as_str(), "html" | "body" | "p");
            assert_eq!(element.inlinable, expected, "{}", element.tag);
        }
    }

    #[test]
    fn a_stray_end_tag_does_not_unwind_the_stack() {
        let tree = build("<div><p></span><b>x</b></p></div>");
        let b = tree.elements.iter().position(|e| e.tag == "b").unwrap();
        let p = tree.elements.iter().position(|e| e.tag == "p").unwrap();
        assert_eq!(tree.elements[b].parent, Some(p));
    }
}
