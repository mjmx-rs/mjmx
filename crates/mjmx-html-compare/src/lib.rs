//! Semantic HTML comparator for mjmx's golden tests.
//!
//! Byte-exact comparison of rendered HTML against mjml's reference output is far
//! too brittle: indentation, attribute order, and CSS declaration order all vary
//! without changing meaning. This crate compares two documents *semantically* —
//! tolerant of insignificant layout differences, strict about anything that
//! changes what the email renders as.
//!
//! The pipeline is: [`mjmx_html_tokenizer::tokenize`] each input into a flat
//! event stream, normalize that stream (drop insignificant whitespace,
//! canonicalize attributes/`style`/`class`), then walk the two normalized
//! streams in lock-step. The first divergence becomes a readable [`Mismatch`].
//!
//! The tokenizer is shared with [`mjmx_css_inline`](../mjmx_css_inline/index.html)
//! so there is exactly one description of mjml's HTML shapes in the workspace.
//! Its byte spans are of no interest here and are simply ignored.
//!
//! ## Equivalence rules
//! - **Whitespace**: all-whitespace text nodes are ignored; other text collapses
//!   internal whitespace runs to a single space and is trimmed.
//! - **Attributes**: compared as an order-independent name→value map; a missing,
//!   extra, or differing attribute is a mismatch.
//! - **`style`**: compared as a set of `prop:value` declarations (order-independent).
//! - **`class`**: compared as a set of tokens (order-independent).
//! - **Tag names / doctype**: case-insensitive.
//! - **Comments** (incl. MSO conditionals): inner content compared after
//!   whitespace-collapse — they are structural and must match.

use std::collections::BTreeMap;
use std::fmt;

use mjmx_html_tokenizer::{Event, tokenize};

/// A single semantic difference between two documents, pointing at the first
/// divergence with a tag-path breadcrumb and expected-vs-actual snippets.
#[derive(Debug, Clone)]
pub struct Mismatch {
    index: usize,
    path: String,
    reason: String,
    expected: String,
    actual: String,
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "HTML mismatch at event #{} (path: {})",
            self.index,
            if self.path.is_empty() {
                "<root>"
            } else {
                &self.path
            }
        )?;
        writeln!(f, "  reason:   {}", self.reason)?;
        writeln!(f, "  expected: {}", self.expected)?;
        write!(f, "  actual:   {}", self.actual)
    }
}

impl std::error::Error for Mismatch {}

/// Compare two HTML documents semantically. Returns `Ok(())` when they are
/// equivalent, or the first [`Mismatch`] otherwise.
pub fn compare(expected: &str, actual: &str) -> Result<(), Mismatch> {
    let exp = normalize(&tokenize(expected));
    let act = normalize(&tokenize(actual));
    diff(&exp, &act)
}

/// Assert that two HTML documents are semantically equivalent, panicking with a
/// readable diff otherwise. For use in tests.
pub fn assert_html_eq(expected: &str, actual: &str) {
    if let Err(mismatch) = compare(expected, actual) {
        panic!("{mismatch}");
    }
}

// --- normalized model --------------------------------------------------------

/// An attribute value canonicalized for order-independent comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
enum AttrValue {
    /// A plain string value.
    Plain(String),
    /// A `style` attribute as a canonical, sorted declaration string.
    Style(String),
    /// A `class` attribute as a canonical, sorted token string.
    Class(String),
}

impl AttrValue {
    fn as_str(&self) -> &str {
        match self {
            AttrValue::Plain(s) | AttrValue::Style(s) | AttrValue::Class(s) => s,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NEvent {
    DocType(String),
    Open {
        tag: String,
        attrs: BTreeMap<String, AttrValue>,
    },
    Close {
        tag: String,
    },
    Text(String),
    Comment(String),
}

impl NEvent {
    /// A compact, human-readable rendering used in mismatch messages.
    fn render(&self) -> String {
        match self {
            NEvent::DocType(s) => format!("<!{s}>"),
            NEvent::Open { tag, attrs } => {
                let mut out = format!("<{tag}");
                for (k, v) in attrs {
                    out.push_str(&format!(" {}=\"{}\"", k, v.as_str()));
                }
                out.push('>');
                out
            }
            NEvent::Close { tag } => format!("</{tag}>"),
            NEvent::Text(s) => format!("{s:?}"),
            NEvent::Comment(s) => format!("<!--{s}-->"),
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            NEvent::DocType(_) => "doctype",
            NEvent::Open { .. } => "element",
            NEvent::Close { .. } => "close tag",
            NEvent::Text(_) => "text",
            NEvent::Comment(_) => "comment",
        }
    }
}

/// Collapse internal whitespace runs to single spaces and trim the ends.
fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Canonicalize a single `style` declaration: trim around the first colon and
/// collapse whitespace in the value, so `width: 100%` == `width:100%`.
fn canon_declaration(decl: &str) -> String {
    let decl = decl.trim();
    match decl.split_once(':') {
        Some((prop, value)) => format!("{}:{}", prop.trim(), collapse_ws(value)),
        None => collapse_ws(decl),
    }
}

fn canon_style(value: &str) -> String {
    let mut decls: Vec<String> = value
        .split(';')
        .map(canon_declaration)
        .filter(|d| !d.is_empty())
        .collect();
    decls.sort();
    decls.join(";")
}

fn canon_class(value: &str) -> String {
    let mut tokens: Vec<&str> = value.split_whitespace().collect();
    tokens.sort_unstable();
    tokens.join(" ")
}

fn canon_attr(name: &str, value: &str) -> AttrValue {
    match name {
        "style" => AttrValue::Style(canon_style(value)),
        "class" => AttrValue::Class(canon_class(value)),
        _ => AttrValue::Plain(value.to_string()),
    }
}

/// Normalize a raw event stream: drop insignificant whitespace text, collapse
/// remaining text/comments, lower-case tag names, and canonicalize attributes.
fn normalize(events: &[Event]) -> Vec<NEvent> {
    let mut out = Vec::with_capacity(events.len());
    for ev in events {
        match ev {
            Event::DocType { text, .. } => {
                out.push(NEvent::DocType(collapse_ws(text).to_ascii_lowercase()));
            }
            Event::Text { text, .. } => {
                let collapsed = collapse_ws(text);
                if !collapsed.is_empty() {
                    out.push(NEvent::Text(collapsed));
                }
            }
            Event::Comment { text, .. } => out.push(NEvent::Comment(collapse_ws(text))),
            Event::Close { tag, .. } => out.push(NEvent::Close {
                tag: tag.to_ascii_lowercase(),
            }),
            Event::Open { tag, attrs, .. } => {
                let mut map = BTreeMap::new();
                for attr in attrs {
                    let name = attr.name.to_ascii_lowercase();
                    let canon = canon_attr(&name, attr.value);
                    map.insert(name, canon);
                }
                out.push(NEvent::Open {
                    tag: tag.to_ascii_lowercase(),
                    attrs: map,
                });
            }
        }
    }
    out
}

// --- comparison --------------------------------------------------------------

/// Breadcrumb path of open tags up to (not including) `index` in `events`.
fn path_at(events: &[NEvent], index: usize) -> String {
    let mut stack: Vec<&str> = Vec::new();
    for ev in events.iter().take(index) {
        match ev {
            NEvent::Open { tag, .. } => stack.push(tag),
            NEvent::Close { .. } => {
                stack.pop();
            }
            _ => {}
        }
    }
    stack.join(" > ")
}

/// Describe how two normalized `Open` elements' attributes differ, if at all.
fn attr_reason(
    exp: &BTreeMap<String, AttrValue>,
    act: &BTreeMap<String, AttrValue>,
) -> Option<String> {
    let mut keys: Vec<&String> = exp.keys().chain(act.keys()).collect();
    keys.sort_unstable();
    keys.dedup();
    for key in keys {
        match (exp.get(key), act.get(key)) {
            (Some(_), None) => return Some(format!("missing attribute `{key}` in actual")),
            (None, Some(a)) => {
                return Some(format!(
                    "unexpected attribute `{key}=\"{}\"` in actual",
                    a.as_str()
                ));
            }
            (Some(e), Some(a)) if e != a => {
                let label = match e {
                    AttrValue::Style(_) => "style declaration set",
                    AttrValue::Class(_) => "class token set",
                    AttrValue::Plain(_) => "attribute",
                };
                return Some(format!(
                    "{label} `{key}` differs: expected `{}`, found `{}`",
                    e.as_str(),
                    a.as_str()
                ));
            }
            _ => {}
        }
    }
    None
}

/// Compare two aligned events; return a mismatch reason string if they differ.
fn event_reason(exp: &NEvent, act: &NEvent) -> Option<String> {
    match (exp, act) {
        (NEvent::DocType(e), NEvent::DocType(a)) => {
            (e != a).then(|| format!("doctype differs: expected `{e}`, found `{a}`"))
        }
        (NEvent::Text(e), NEvent::Text(a)) => {
            (e != a).then(|| format!("text differs: expected {e:?}, found {a:?}"))
        }
        (NEvent::Comment(e), NEvent::Comment(a)) => {
            (e != a).then(|| format!("comment differs: expected {e:?}, found {a:?}"))
        }
        (NEvent::Close { tag: e }, NEvent::Close { tag: a }) => {
            (e != a).then(|| format!("close tag differs: expected `</{e}>`, found `</{a}>`"))
        }
        (NEvent::Open { tag: et, attrs: ea }, NEvent::Open { tag: at, attrs: aa }) => {
            if et != at {
                Some(format!("tag differs: expected `<{et}>`, found `<{at}>`"))
            } else {
                attr_reason(ea, aa)
            }
        }
        (e, a) => Some(format!("expected {} but found {}", e.kind(), a.kind())),
    }
}

fn diff(exp: &[NEvent], act: &[NEvent]) -> Result<(), Mismatch> {
    let len = exp.len().max(act.len());
    for i in 0..len {
        match (exp.get(i), act.get(i)) {
            (Some(e), Some(a)) => {
                if let Some(reason) = event_reason(e, a) {
                    return Err(Mismatch {
                        index: i,
                        path: path_at(exp, i),
                        reason,
                        expected: e.render(),
                        actual: a.render(),
                    });
                }
            }
            (Some(e), None) => {
                return Err(Mismatch {
                    index: i,
                    path: path_at(exp, i),
                    reason: format!(
                        "actual document ended early; expected {} more",
                        exp.len() - i
                    ),
                    expected: e.render(),
                    actual: "<end of document>".to_string(),
                });
            }
            (None, Some(a)) => {
                return Err(Mismatch {
                    index: i,
                    path: path_at(act, i),
                    reason: format!(
                        "actual document has {} unexpected trailing event(s)",
                        act.len() - i
                    ),
                    expected: "<end of document>".to_string(),
                    actual: a.render(),
                });
            }
            (None, None) => break,
        }
    }
    Ok(())
}
