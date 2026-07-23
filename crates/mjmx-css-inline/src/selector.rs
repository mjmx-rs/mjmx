//! Selector parsing, specificity, and matching against a [`Tree`].
//!
//! The supported grammar is deliberately the part of Selectors Level 3 that can
//! be decided from a static document: type, universal, `#id`, `.class` and
//! attribute simple selectors, joined by the four combinators (descendant, `>`,
//! `+`, `~`).
//!
//! Everything else — every pseudo-class and pseudo-element, functional or not —
//! makes [`Selector::parse`] return `None`, and a selector that does not parse is
//! **dropped**: not inlined, not preserved, not re-emitted. That is not a
//! shortcut, it is the observed behaviour of mjml's pass for the state-dependent
//! pseudos (`:hover`, `:active`, `:focus`, `:visited`, `:link`) and for
//! `::before`/`::after`, whose declarations belong to a box that does not exist
//! in the document and so has no `style` attribute to receive them. The
//! remaining pseudos (`:nth-child`, `:not`, …) are decidable in principle and are
//! dropped only because they are not implemented; see the crate docs.

use crate::dom::Tree;

/// A selector's specificity, as the usual `(id, class, type)` triple compared
/// lexicographically.
pub(crate) type Specificity = (u32, u32, u32);

/// A generated pseudo-element, when `inlinePseudoElements` is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PseudoElement {
    /// `::before` — the box is prepended to the matched element's content.
    Before,
    /// `::after` — the box is appended to the matched element's content.
    After,
}

/// Split a trailing `::before` / `::after` (or their legacy single-colon spellings)
/// off a selector, returning the base selector and the pseudo-element kind.
///
/// The `::` forms are checked first so `a::before` strips to `a`, not `a:` — the
/// single-colon `:before` suffix would otherwise match the tail. Any *other* pseudo
/// left on the base still makes [`Selector::parse`] drop the rule (e.g.
/// `a:hover::before`), exactly as juice skips a rule carrying an ignored pseudo.
pub(crate) fn strip_pseudo_element(src: &str) -> (&str, Option<PseudoElement>) {
    let trimmed = src.trim_end();
    for (suffix, kind) in [
        ("::before", PseudoElement::Before),
        ("::after", PseudoElement::After),
        (":before", PseudoElement::Before),
        (":after", PseudoElement::After),
    ] {
        if trimmed.len() >= suffix.len() {
            let (head, tail) = trimmed.split_at(trimmed.len() - suffix.len());
            if tail.eq_ignore_ascii_case(suffix) {
                return (head.trim_end(), Some(kind));
            }
        }
    }
    (src, None)
}

/// How a compound selector relates to the one on its left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Combinator {
    /// `a b`
    Descendant,
    /// `a > b`
    Child,
    /// `a + b`
    NextSibling,
    /// `a ~ b`
    LaterSibling,
}

/// How an attribute selector compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttrOp {
    /// `[a]`
    Exists,
    /// `[a=v]`
    Equals,
    /// `[a~=v]` — one of a whitespace-separated list.
    Includes,
    /// `[a|=v]` — equal to `v`, or starting with `v-`.
    DashMatch,
    /// `[a^=v]`
    Prefix,
    /// `[a$=v]`
    Suffix,
    /// `[a*=v]`
    Substring,
}

#[derive(Debug)]
struct AttrTest {
    name: String,
    op: AttrOp,
    value: String,
}

/// A sequence of simple selectors with no combinator between them, e.g.
/// `td.probe[data-x="1"]`.
#[derive(Debug, Default)]
struct Compound {
    /// The lower-cased type selector; `None` for `*` or an implied universal.
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
    attrs: Vec<AttrTest>,
}

/// A complete complex selector: `compounds[0] combinators[0] compounds[1] …`.
#[derive(Debug)]
pub(crate) struct Selector {
    compounds: Vec<Compound>,
    combinators: Vec<Combinator>,
    pub(crate) specificity: Specificity,
}

/// Whether `b` may appear in an identifier (a deliberately loose test: escapes
/// are not decoded, and every non-ASCII byte is accepted as a name character).
fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b >= 0x80
}

/// Read an identifier at `*i`, advancing past it. Empty means "no identifier".
fn read_ident(src: &str, i: &mut usize) -> String {
    let bytes = src.as_bytes();
    let start = *i;
    while *i < bytes.len() && is_ident_byte(bytes[*i]) {
        *i += 1;
    }
    src[start..*i].to_owned()
}

fn skip_ws(src: &str, i: &mut usize) -> bool {
    let bytes = src.as_bytes();
    let start = *i;
    while *i < bytes.len() && bytes[*i].is_ascii_whitespace() {
        *i += 1;
    }
    *i > start
}

/// Split a selector list on its top-level commas.
///
/// Commas inside `(…)`, `[…]` or a quoted string belong to a functional pseudo,
/// an attribute value or a string and are not separators.
pub(crate) fn split_list(src: &str) -> Vec<&str> {
    let bytes = src.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut quote: Option<u8> = None;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(_) if b == b'\\' => i += 1,
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None => match b {
                b'"' | b'\'' => quote = Some(b),
                b'(' | b'[' => depth += 1,
                b')' | b']' => depth = depth.saturating_sub(1),
                b',' if depth == 0 => {
                    parts.push(src[start..i].trim());
                    start = i + 1;
                }
                _ => {}
            },
        }
        i += 1;
    }
    parts.push(src[start..].trim());
    parts.retain(|p| !p.is_empty());
    parts
}

impl Selector {
    /// Parse one complex selector, or `None` if it uses anything unsupported.
    pub(crate) fn parse(src: &str) -> Option<Self> {
        let bytes = src.as_bytes();
        let mut compounds: Vec<Compound> = Vec::new();
        let mut combinators: Vec<Combinator> = Vec::new();
        let mut specificity: Specificity = (0, 0, 0);
        let mut i = 0;
        // A combinator waiting for its right-hand compound. Whitespace sets the
        // descendant combinator, which an explicit `>`/`+`/`~` then overrides.
        let mut pending: Option<Combinator> = None;
        // Whether the compound under construction has any simple selector yet,
        // which is what decides whether an identifier is a type selector.
        let mut open = false;

        loop {
            if skip_ws(src, &mut i) && open {
                pending = Some(Combinator::Descendant);
                open = false;
            }
            if i >= bytes.len() {
                break;
            }
            match bytes[i] {
                b'>' | b'+' | b'~' => {
                    if compounds.is_empty() {
                        return None; // a leading combinator is not a selector
                    }
                    pending = Some(match bytes[i] {
                        b'>' => Combinator::Child,
                        b'+' => Combinator::NextSibling,
                        _ => Combinator::LaterSibling,
                    });
                    open = false;
                    i += 1;
                    continue;
                }
                _ => {}
            }

            if !open {
                if let Some(combinator) = pending.take() {
                    combinators.push(combinator);
                }
                compounds.push(Compound::default());
                open = true;
            }
            let compound = compounds.last_mut().expect("just pushed");

            match bytes[i] {
                b'*' => i += 1,
                b'.' => {
                    i += 1;
                    let name = read_ident(src, &mut i);
                    if name.is_empty() {
                        return None;
                    }
                    compound.classes.push(name);
                    specificity.1 += 1;
                }
                b'#' => {
                    i += 1;
                    let name = read_ident(src, &mut i);
                    if name.is_empty() {
                        return None;
                    }
                    compound.id = Some(name);
                    specificity.0 += 1;
                }
                b'[' => {
                    i += 1;
                    let test = parse_attr_test(src, &mut i)?;
                    compound.attrs.push(test);
                    specificity.1 += 1;
                }
                // Every pseudo-class and pseudo-element is unsupported; see the
                // module docs for why dropping is the right answer here.
                b':' => return None,
                b if is_ident_byte(b) => {
                    if compound.tag.is_some() || !compound.classes.is_empty() {
                        return None; // a type selector may only lead a compound
                    }
                    compound.tag = Some(read_ident(src, &mut i).to_ascii_lowercase());
                    specificity.2 += 1;
                }
                _ => return None,
            }
        }

        if compounds.is_empty() {
            return None;
        }
        Some(Self {
            compounds,
            combinators,
            specificity,
        })
    }

    /// Whether `index` in `tree` matches this selector.
    pub(crate) fn matches(&self, tree: &Tree<'_>, index: usize) -> bool {
        self.matches_from(tree, self.compounds.len() - 1, index)
    }

    /// Match `compounds[at]` against `index`, then satisfy everything to its left.
    ///
    /// Right-to-left, as every selector engine does: the rightmost compound is
    /// the one already known to be a candidate, and each combinator narrows the
    /// search to a bounded set of relatives.
    fn matches_from(&self, tree: &Tree<'_>, at: usize, index: usize) -> bool {
        if !self.compounds[at].matches(tree, index) {
            return false;
        }
        let Some(at) = at.checked_sub(1) else {
            return true;
        };
        match self.combinators[at] {
            Combinator::Descendant => tree
                .ancestors(index)
                .any(|ancestor| self.matches_from(tree, at, ancestor)),
            Combinator::Child => tree.elements[index]
                .parent
                .is_some_and(|parent| self.matches_from(tree, at, parent)),
            Combinator::NextSibling => tree.elements[index]
                .prev_sibling
                .is_some_and(|prev| self.matches_from(tree, at, prev)),
            Combinator::LaterSibling => tree
                .preceding_siblings(index)
                .any(|prev| self.matches_from(tree, at, prev)),
        }
    }
}

impl Compound {
    fn matches(&self, tree: &Tree<'_>, index: usize) -> bool {
        let element = &tree.elements[index];
        if let Some(tag) = &self.tag
            && *tag != element.tag
        {
            return false;
        }
        if let Some(id) = &self.id
            && element.id != Some(id.as_str())
        {
            return false;
        }
        if !self
            .classes
            .iter()
            .all(|class| element.classes.iter().any(|c| c == class))
        {
            return false;
        }
        self.attrs.iter().all(|test| test.matches(element))
    }
}

impl AttrTest {
    fn matches(&self, element: &crate::dom::Element<'_>) -> bool {
        let Some(attr) = element.attr(&self.name) else {
            return false;
        };
        let actual = attr.value;
        let expected = self.value.as_str();
        match self.op {
            AttrOp::Exists => true,
            AttrOp::Equals => actual == expected,
            AttrOp::Includes => {
                !expected.is_empty() && actual.split_whitespace().any(|v| v == expected)
            }
            AttrOp::DashMatch => {
                actual == expected
                    || (actual.len() > expected.len()
                        && actual.starts_with(expected)
                        && actual.as_bytes()[expected.len()] == b'-')
            }
            AttrOp::Prefix => !expected.is_empty() && actual.starts_with(expected),
            AttrOp::Suffix => !expected.is_empty() && actual.ends_with(expected),
            AttrOp::Substring => !expected.is_empty() && actual.contains(expected),
        }
    }
}

/// Parse the inside of an attribute selector, with `*i` just past the `[`.
fn parse_attr_test(src: &str, i: &mut usize) -> Option<AttrTest> {
    let bytes = src.as_bytes();
    skip_ws(src, i);
    let name = read_ident(src, i);
    if name.is_empty() {
        return None;
    }
    skip_ws(src, i);
    if *i >= bytes.len() {
        return None;
    }
    if bytes[*i] == b']' {
        *i += 1;
        return Some(AttrTest {
            name,
            op: AttrOp::Exists,
            value: String::new(),
        });
    }
    let op = match bytes[*i] {
        b'=' => AttrOp::Equals,
        b'~' => AttrOp::Includes,
        b'|' => AttrOp::DashMatch,
        b'^' => AttrOp::Prefix,
        b'$' => AttrOp::Suffix,
        b'*' => AttrOp::Substring,
        _ => return None,
    };
    *i += 1;
    if op != AttrOp::Equals {
        // The two-character operators still need their `=`.
        if *i >= bytes.len() || bytes[*i] != b'=' {
            return None;
        }
        *i += 1;
    }
    skip_ws(src, i);
    let value = if *i < bytes.len() && (bytes[*i] == b'"' || bytes[*i] == b'\'') {
        let quote = bytes[*i];
        *i += 1;
        let start = *i;
        while *i < bytes.len() && bytes[*i] != quote {
            *i += 1;
        }
        let value = src[start..*i].to_owned();
        *i += 1; // closing quote
        value
    } else {
        read_ident(src, i)
    };
    skip_ws(src, i);
    // Case-insensitivity flags (`i`/`s`) are not supported; anything but the
    // closing bracket drops the selector.
    if *i >= bytes.len() || bytes[*i] != b']' {
        return None;
    }
    *i += 1;
    Some(AttrTest { name, op, value })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(src: &str) -> Specificity {
        Selector::parse(src).expect("should parse").specificity
    }

    #[test]
    fn specificity_counts_each_kind() {
        assert_eq!(spec("*"), (0, 0, 0));
        assert_eq!(spec("p"), (0, 0, 1));
        assert_eq!(spec("div p"), (0, 0, 2));
        assert_eq!(spec(".a p"), (0, 1, 1));
        assert_eq!(spec("#a .b[c] d"), (1, 2, 1));
    }

    #[test]
    fn unsupported_selectors_are_dropped() {
        for src in [
            ".a:hover",
            "a:visited",
            ".a::before",
            ".a:before",
            "li:nth-child(2)",
            ":not(.a)",
            "> p",
            "",
        ] {
            assert!(Selector::parse(src).is_none(), "{src} should not parse");
        }
    }

    #[test]
    fn selector_lists_split_on_top_level_commas_only() {
        assert_eq!(split_list("a, b"), ["a", "b"]);
        assert_eq!(split_list(":is(a, b), c"), [":is(a, b)", "c"]);
        assert_eq!(split_list("[x=\"a,b\"], c"), ["[x=\"a,b\"]", "c"]);
    }

    #[test]
    fn combinators_match_the_right_relatives() {
        // Mirrors what mj-text renders: the styled span sits two divs deep, so
        // `.comb > div > span` must *not* reach it.
        let tree = crate::dom::build(
            "<div class=\"comb\"><div><div><span id=\"deep\">x</span></div></div>\
             <b>b</b><i>i</i></div>",
        );
        let index = |tag: &str| tree.elements.iter().position(|e| e.tag == tag).unwrap();
        let span = index("span");
        let i = index("i");

        assert!(Selector::parse(".comb span").unwrap().matches(&tree, span));
        assert!(
            Selector::parse(".comb div > span")
                .unwrap()
                .matches(&tree, span)
        );
        // The span is a grandchild of `.comb`, so a chain of `>` must not match.
        assert!(
            !Selector::parse(".comb > div > span")
                .unwrap()
                .matches(&tree, span)
        );
        assert!(Selector::parse(".comb b + i").unwrap().matches(&tree, i));
        assert!(!Selector::parse(".comb i + b").unwrap().matches(&tree, i));
        assert!(
            Selector::parse("div ~ b")
                .unwrap()
                .matches(&tree, index("b"))
        );
        assert!(Selector::parse("#deep").unwrap().matches(&tree, span));
    }
}
