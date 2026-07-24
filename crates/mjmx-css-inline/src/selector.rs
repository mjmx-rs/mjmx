//! Selector parsing, specificity, and matching against a [`Tree`].
//!
//! The supported grammar is the part of Selectors Level 3/4 that can be decided
//! from a static document: type, universal, `#id`, `.class` and attribute simple
//! selectors, joined by the four combinators (descendant, `>`, `+`, `~`), plus the
//! **structural and functional pseudo-classes** — `:first-child`, `:last-child`,
//! `:only-child`, `:nth-child`, `:nth-last-child`, the four `-of-type` variants,
//! `:empty`, `:root`, and `:not`/`:is`/`:where`. These match real juice: an inline
//! `<mj-style>` rule keyed on `li:nth-child(2)` or `td:not(.skip)` rewrites the same
//! elements juice would (verified against juice's own output).
//!
//! What still makes [`Selector::parse`] return `None` — and so **drops** the rule
//! (not inlined, not preserved, not re-emitted):
//!
//! - the **state-dependent** pseudos (`:hover`, `:active`, `:focus`, `:visited`,
//!   `:link`, …): their target box exists only in a browser state, so no static
//!   `style` attribute can carry the declaration — juice drops them too;
//! - any **unknown** pseudo, so an unrecognised construct is never mis-applied
//!   (the crate only ever rewrites an element a selector definitely matched);
//! - `::before`/`::after`, split off earlier by [`strip_pseudo_element`] and handled
//!   by the pseudo-element pass, not here.
//!
//! **Deliberate divergence from the vendored juice**, toward correctness: the
//! *list* form of `:not`/`:is`/`:where` (`:is(.a, .b)`) matches nothing in the old
//! css-select juice ships, a bug fixed in later versions; mjmx implements the
//! standard semantics (`:is`/`:where` match if any argument matches, `:not` if
//! none does). The single-argument form — the realistic case — matches juice
//! exactly. **Still unsupported:** CSS Nesting (nested qualified rules), dropped at
//! the stylesheet level; see [`crate::css`].

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

/// The `An+B` argument of an `:nth-*` pseudo-class.
///
/// `odd` is `2n+1`, `even` is `2n`, a bare integer `B` is `0n+B`. A 1-based position
/// `p` matches when there is a non-negative integer `k` with `p == a*k + b`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AnB {
    a: i32,
    b: i32,
}

impl AnB {
    fn matches(self, position: i32) -> bool {
        if self.a == 0 {
            return position == self.b;
        }
        let diff = position - self.b;
        diff % self.a == 0 && diff / self.a >= 0
    }
}

/// A structural or functional pseudo-class. Everything here is decidable from the
/// static tree; the state-dependent pseudos never reach this type (they drop the
/// rule in [`Selector::parse`]).
#[derive(Debug)]
enum PseudoClass {
    FirstChild,
    LastChild,
    OnlyChild,
    NthChild(AnB),
    NthLastChild(AnB),
    FirstOfType,
    LastOfType,
    OnlyOfType,
    NthOfType(AnB),
    NthLastOfType(AnB),
    Empty,
    Root,
    /// `:not(list)` — matches when *no* argument selector matches.
    Not(Vec<Selector>),
    /// `:is(list)` / `:where(list)` — matches when *any* argument selector matches.
    /// The two differ only in specificity, which is folded in at parse time.
    Is(Vec<Selector>),
}

/// A sequence of simple selectors with no combinator between them, e.g.
/// `td.probe[data-x="1"]:first-child`.
#[derive(Debug, Default)]
struct Compound {
    /// The lower-cased type selector; `None` for `*` or an implied universal.
    tag: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
    attrs: Vec<AttrTest>,
    pseudos: Vec<PseudoClass>,
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
                // A pseudo-class: structural/functional ones are parsed and matched,
                // state-dependent and unknown ones drop the whole selector (`None`).
                b':' => {
                    let (pseudo, spec) = parse_pseudo(src, &mut i)?;
                    compound.pseudos.push(pseudo);
                    specificity.0 += spec.0;
                    specificity.1 += spec.1;
                    specificity.2 += spec.2;
                }
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
            && self.pseudos.iter().all(|pseudo| pseudo.matches(tree, index))
    }
}

impl PseudoClass {
    /// Whether the element at `index` satisfies this pseudo-class.
    fn matches(&self, tree: &Tree<'_>, index: usize) -> bool {
        let element = &tree.elements[index];
        let preceding = || tree.preceding_siblings(index).count();
        let following = || tree.following_siblings(index).count();
        let same_tag = |&s: &usize| tree.elements[s].tag == element.tag;
        let preceding_of_type = || tree.preceding_siblings(index).filter(same_tag).count();
        let following_of_type = || tree.following_siblings(index).filter(same_tag).count();
        // A 1-based position from a 0-based count of siblings on one side.
        let position = |count: usize| count as i32 + 1;
        match self {
            PseudoClass::FirstChild => preceding() == 0,
            PseudoClass::LastChild => following() == 0,
            PseudoClass::OnlyChild => preceding() == 0 && following() == 0,
            PseudoClass::NthChild(anb) => anb.matches(position(preceding())),
            PseudoClass::NthLastChild(anb) => anb.matches(position(following())),
            PseudoClass::FirstOfType => preceding_of_type() == 0,
            PseudoClass::LastOfType => following_of_type() == 0,
            PseudoClass::OnlyOfType => preceding_of_type() == 0 && following_of_type() == 0,
            PseudoClass::NthOfType(anb) => anb.matches(position(preceding_of_type())),
            PseudoClass::NthLastOfType(anb) => anb.matches(position(following_of_type())),
            PseudoClass::Empty => !element.has_content,
            PseudoClass::Root => element.parent.is_none(),
            PseudoClass::Not(list) => !list.iter().any(|s| s.matches(tree, index)),
            PseudoClass::Is(list) => list.iter().any(|s| s.matches(tree, index)),
        }
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

/// Parse one pseudo-class at `*i` (the `:`), advancing past it. Returns the
/// pseudo-class and its specificity contribution, or `None` to drop the selector —
/// for a pseudo-element (`::…`), a state-dependent pseudo, an unknown name, or a
/// wrong-arity spelling (`:nth-child` with no argument, `:first-child(…)`).
fn parse_pseudo(src: &str, i: &mut usize) -> Option<(PseudoClass, Specificity)> {
    let bytes = src.as_bytes();
    debug_assert_eq!(bytes[*i], b':');
    *i += 1;
    // A second colon is a pseudo-element; `::before`/`::after` were split off
    // upstream, and nothing else here has a `style` attribute to receive.
    if *i < bytes.len() && bytes[*i] == b':' {
        return None;
    }
    let name = read_ident(src, i).to_ascii_lowercase();
    if name.is_empty() {
        return None;
    }
    let arg = if *i < bytes.len() && bytes[*i] == b'(' {
        Some(read_paren_arg(src, i)?)
    } else {
        None
    };
    // A structural pseudo counts as one class in specificity, like css-select.
    let class_spec = (0, 1, 0);
    let pseudo = match (name.as_str(), arg) {
        ("first-child", None) => PseudoClass::FirstChild,
        ("last-child", None) => PseudoClass::LastChild,
        ("only-child", None) => PseudoClass::OnlyChild,
        ("first-of-type", None) => PseudoClass::FirstOfType,
        ("last-of-type", None) => PseudoClass::LastOfType,
        ("only-of-type", None) => PseudoClass::OnlyOfType,
        ("empty", None) => PseudoClass::Empty,
        ("root", None) => PseudoClass::Root,
        ("nth-child", Some(a)) => PseudoClass::NthChild(parse_anb(a)?),
        ("nth-last-child", Some(a)) => PseudoClass::NthLastChild(parse_anb(a)?),
        ("nth-of-type", Some(a)) => PseudoClass::NthOfType(parse_anb(a)?),
        ("nth-last-of-type", Some(a)) => PseudoClass::NthLastOfType(parse_anb(a)?),
        // `:not`/`:is` take the argument list's specificity; `:where` takes none.
        ("not", Some(a)) => {
            let (list, spec) = parse_selector_list(a)?;
            return Some((PseudoClass::Not(list), spec));
        }
        ("is", Some(a)) => {
            let (list, spec) = parse_selector_list(a)?;
            return Some((PseudoClass::Is(list), spec));
        }
        ("where", Some(a)) => {
            let (list, _) = parse_selector_list(a)?;
            return Some((PseudoClass::Is(list), (0, 0, 0)));
        }
        // `:hover`/`:focus`/… (state-dependent), an unknown name, or wrong arity.
        _ => return None,
    };
    Some((pseudo, class_spec))
}

/// Read a balanced `(…)` argument at `*i` (the `(`), advancing past the `)`. Commas,
/// brackets and quotes inside are the argument's own; returns the trimmed inner text,
/// or `None` if the parentheses are unbalanced.
fn read_paren_arg<'a>(src: &'a str, i: &mut usize) -> Option<&'a str> {
    let bytes = src.as_bytes();
    debug_assert_eq!(bytes[*i], b'(');
    let start = *i + 1;
    let mut depth = 1usize;
    let mut quote: Option<u8> = None;
    let mut j = start;
    while j < bytes.len() {
        let b = bytes[j];
        match quote {
            Some(_) if b == b'\\' => j += 1,
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None => match b {
                b'"' | b'\'' => quote = Some(b),
                b'(' | b'[' => depth += 1,
                b')' | b']' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            },
        }
        j += 1;
    }
    if depth != 0 {
        return None;
    }
    let arg = src[start..j].trim();
    *i = j + 1; // past the closing ')'
    Some(arg)
}

/// Parse an `An+B` argument: `odd`, `even`, a bare integer `B` (`0n+B`), or the full
/// `An+B` / `An-B` / `n` / `-n+B` forms.
fn parse_anb(src: &str) -> Option<AnB> {
    let s = src.trim().to_ascii_lowercase();
    match s.as_str() {
        "odd" => return Some(AnB { a: 2, b: 1 }),
        "even" => return Some(AnB { a: 2, b: 0 }),
        _ => {}
    }
    // No `n`: a plain `B`.
    let Some((a_part, b_part)) = s.split_once('n') else {
        return s.parse::<i32>().ok().map(|b| AnB { a: 0, b });
    };
    let a = match a_part.trim() {
        "" | "+" => 1,
        "-" => -1,
        other => other.parse::<i32>().ok()?,
    };
    let b_part = b_part.trim();
    let b = if b_part.is_empty() {
        0
    } else {
        // The offset after `n` carries an explicit sign, e.g. `2n + 1` or `2n-1`.
        let (sign, digits) = b_part
            .strip_prefix('+')
            .map(|r| (1, r))
            .or_else(|| b_part.strip_prefix('-').map(|r| (-1, r)))?;
        sign * digits.trim().parse::<i32>().ok()?
    };
    Some(AnB { a, b })
}

/// Parse a comma-separated selector list (a `:not`/`:is`/`:where` argument) into its
/// selectors and their greatest specificity. `None` if any argument fails to parse,
/// so an unknown inner selector drops the whole rule rather than mis-matching.
fn parse_selector_list(arg: &str) -> Option<(Vec<Selector>, Specificity)> {
    let mut selectors = Vec::new();
    let mut max = (0, 0, 0);
    for part in split_list(arg) {
        let selector = Selector::parse(part)?;
        max = max.max(selector.specificity);
        selectors.push(selector);
    }
    (!selectors.is_empty()).then_some((selectors, max))
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
            // State-dependent pseudos: juice cannot inline these either.
            ".a:hover",
            "a:visited",
            "a:focus",
            "input:checked",
            // A pseudo-element, and an unknown pseudo.
            ".a::before",
            "p::first-line",
            ".a:fictional",
            // A wrong-arity structural pseudo.
            ":nth-child",
            ":first-child(2)",
            // An unparseable argument drops the rule rather than mis-matching.
            ":nth-child(banana)",
            ":not(:hover)",
            "> p",
            "",
        ] {
            assert!(Selector::parse(src).is_none(), "{src} should not parse");
        }
    }

    #[test]
    fn structural_and_functional_pseudos_parse() {
        for src in [
            "li:first-child",
            "li:last-child",
            "li:only-child",
            "li:nth-child(2n+1)",
            "li:nth-child(odd)",
            "li:nth-child(3)",
            "li:nth-last-child(1)",
            "p:first-of-type",
            "p:nth-of-type(2)",
            "td:empty",
            ":root",
            "li:not(.skip)",
            "li:is(.a)",
            "li:where(.a)",
        ] {
            assert!(Selector::parse(src).is_some(), "{src} should parse");
        }
    }

    #[test]
    fn anb_matches_positions() {
        let odd = parse_anb("odd").unwrap();
        assert!(odd.matches(1) && odd.matches(3) && !odd.matches(2));
        let even = parse_anb("even").unwrap();
        assert!(even.matches(2) && !even.matches(1));
        let two_n_plus_one = parse_anb("2n+1").unwrap();
        assert!(two_n_plus_one.matches(1) && two_n_plus_one.matches(3));
        let bare = parse_anb("3").unwrap();
        assert!(bare.matches(3) && !bare.matches(2));
        let every = parse_anb("n").unwrap();
        assert!(every.matches(1) && every.matches(9));
        let minus = parse_anb("-n+2").unwrap();
        assert!(minus.matches(1) && minus.matches(2) && !minus.matches(3));
        assert!(parse_anb("banana").is_none());
    }

    /// Every expectation here was cross-checked against real juice output.
    #[test]
    fn pseudo_classes_match_like_juice() {
        // <ul> with four <li>, the second carrying class="skip".
        let tree = crate::dom::build(
            "<ul><li>a</li><li class=\"skip\">b</li><li>c</li><li>d</li></ul>",
        );
        let li: Vec<usize> = tree
            .elements
            .iter()
            .enumerate()
            .filter(|(_, e)| e.tag == "li")
            .map(|(i, _)| i)
            .collect();
        let m = |src: &str, index: usize| Selector::parse(src).unwrap().matches(&tree, index);

        // :first-child / :last-child / :nth-child / :nth-last-child.
        assert!(m("li:first-child", li[0]) && !m("li:first-child", li[1]));
        assert!(m("li:last-child", li[3]) && !m("li:last-child", li[0]));
        assert!(m("li:nth-child(odd)", li[0]) && m("li:nth-child(odd)", li[2]));
        assert!(!m("li:nth-child(odd)", li[1]));
        assert!(m("li:nth-child(3)", li[2]) && !m("li:nth-child(3)", li[1]));
        assert!(m("li:nth-last-child(1)", li[3]) && !m("li:nth-last-child(1)", li[0]));

        // :not — matches every li without .skip (juice: a, c, d).
        assert!(m("li:not(.skip)", li[0]) && !m("li:not(.skip)", li[1]));
        assert!(m("li:not(.skip)", li[2]) && m("li:not(.skip)", li[3]));

        // :is / :where — single-argument form matches like juice.
        assert!(m("li:is(.skip)", li[1]) && !m("li:is(.skip)", li[0]));
        assert!(m("li:where(.skip)", li[1]));
    }

    #[test]
    fn only_child_and_of_type_and_empty_and_root() {
        let tree = crate::dom::build(
            "<div><span>s</span><p>a</p><p>b</p></div><ul><li>solo</li></ul>\
             <table><tr><td></td><td> </td><td>x</td></tr></table>",
        );
        let position = |tag: &str| tree.elements.iter().position(|e| e.tag == tag).unwrap();
        let m = |src: &str, index: usize| Selector::parse(src).unwrap().matches(&tree, index);

        // :only-child — the lone <li>.
        assert!(m("li:only-child", position("li")));

        // :first-of-type — the first <p>, with a <span> before it (juice-verified).
        let first_p = position("p");
        assert!(m("p:first-of-type", first_p));
        assert!(!m("span:first-of-type", first_p)); // wrong type

        // :nth-of-type(2) — the second <p>.
        let second_p = tree
            .elements
            .iter()
            .enumerate()
            .filter(|(_, e)| e.tag == "p")
            .nth(1)
            .unwrap()
            .0;
        assert!(m("p:nth-of-type(2)", second_p));

        // :empty — an empty <td>, but not one holding whitespace or a child.
        let tds: Vec<usize> = tree
            .elements
            .iter()
            .enumerate()
            .filter(|(_, e)| e.tag == "td")
            .map(|(i, _)| i)
            .collect();
        assert!(m("td:empty", tds[0]), "<td></td> is empty");
        assert!(!m("td:empty", tds[1]), "<td> </td> has whitespace text");
        assert!(!m("td:empty", tds[2]), "<td>x</td> has text");

        // :root — the top-level element (no parent).
        assert!(m(":root", position("div")));
        assert!(!m(":root", first_p));
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
