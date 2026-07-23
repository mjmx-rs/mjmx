//! juice's `inlinePseudoElements`: synthesize `::before` / `::after` boxes from their
//! `content` and splice them into the document. Off by default (juice's default, which
//! mjml never overrides), so a normal render drops pseudo-element rules exactly as before.
//!
//! Modelled on juice 10.0.1's `inlinePseudoElements` / `parseContent`:
//!
//! - a `::before` box is **prepended** to the matched element's inner content, a
//!   `::after` box **appended**; both are `<span>`s carrying the rule's other
//!   declarations, unless `content` is a lone `url(...)`, in which case the box is an
//!   `<img>`;
//! - `content` is parsed into text: quoted strings verbatim, `attr(name)` → the parent's
//!   attribute, `var(--x[, fb])` → the custom-property cascade, `counter(name[, style])`
//!   → the running CSS counter (`counter-reset` / `counter-increment`, roman/alpha
//!   styles);
//! - the box's style is the pseudo rule's declarations minus `content` (and minus `--*`
//!   when `resolveCSSVariables` is on), `var()`-resolved.
//!
//! Counters are tracked in one document-order walk (the arena is already in document
//! order): a global running value per counter plus a per-element snapshot, so a
//! `counter(item)` reads the value the element held when it was incremented, as juice's
//! prototype-chained `counterProps` do.

use std::collections::HashMap;

use crate::apply::{Edit, custom_properties, resolve_var, substitute_vars, winners};
use crate::css::Rule;
use crate::dom::Tree;
use crate::options::InlineOptions;
use crate::selector::PseudoElement;

/// The synthesized boxes, as edits inserting at each matched element's content start
/// (`::before`) or content end (`::after`).
pub(crate) fn pseudo_edits(
    html: &str,
    rules: &[Rule],
    tree: &Tree<'_>,
    options: &InlineOptions,
) -> Vec<Edit> {
    // If no rule targets a pseudo-element there is nothing to do.
    if !rules.iter().any(|r| r.pseudo.is_some()) {
        return Vec::new();
    }

    // Custom-property maps for `var()` in content / styles — built regardless of
    // `resolveCSSVariables`, because juice resolves `var()` inside `content` either way.
    let custom_maps: Vec<Vec<(String, String)>> = tree
        .elements
        .iter()
        .enumerate()
        .map(|(index, element)| custom_properties(html, rules, tree, index, element, options))
        .collect();

    // The per-element counter snapshot, computed in one document-order pass.
    let counters = counter_snapshots(rules, tree);

    let mut edits = Vec::new();
    for (index, element_counters) in counters.iter().enumerate() {
        if !tree.elements[index].inlinable {
            continue;
        }
        for kind in [PseudoElement::Before, PseudoElement::After] {
            if let Some(edit) =
                pseudo_edit(rules, tree, index, kind, options, &custom_maps, element_counters)
            {
                edits.push(edit);
            }
        }
    }
    edits
}

/// One pseudo-element's edit, or `None` when no `::before`/`::after` rule with a
/// `content` matches this element (or, for `::after`, when the element has no close tag).
#[allow(clippy::too_many_arguments)]
fn pseudo_edit(
    rules: &[Rule],
    tree: &Tree<'_>,
    index: usize,
    kind: PseudoElement,
    options: &InlineOptions,
    custom_maps: &[Vec<(String, String)>],
    counters: &HashMap<String, i64>,
) -> Option<Edit> {
    let decls = pseudo_winners(rules, tree, index, kind);
    // juice only materializes a pseudo-element that carries a `content`.
    let content_decl = decls.iter().find(|d| d.property == "content")?;

    let at = match kind {
        PseudoElement::Before => tree.elements[index].content_start,
        // No close tag → nothing to append to (a void or unclosed element).
        PseudoElement::After => tree.elements[index].content_end?,
    };

    let content = parse_content(
        &content_decl.value,
        index,
        tree,
        custom_maps,
        counters,
    );

    // The box's style: every declaration except `content` (and `--*` under
    // resolveCSSVariables), with `var()` resolved.
    let mut style = String::new();
    for decl in &decls {
        if decl.property == "content" {
            continue;
        }
        if options.resolve_css_variables && decl.property.starts_with("--") {
            continue;
        }
        let value = if options.resolve_css_variables {
            substitute_vars(&decl.value, index, tree, custom_maps, 0)
        } else {
            decl.value.clone()
        };
        if !style.is_empty() {
            style.push(' ');
        }
        style.push_str(&format!("{}: {};", decl.property, value));
    }
    let style_attr = if style.is_empty() {
        String::new()
    } else {
        format!(" style=\"{}\"", escape_attr(&style))
    };

    let node = match content {
        Content::Img(url) => format!("<img src=\"{}\"{style_attr}>", escape_attr(&url)),
        Content::Text(text) => format!("<span{style_attr}>{}</span>", escape_text(&text)),
    };

    Some(Edit {
        range: at..at,
        text: node,
    })
}

/// The declarations of the pseudo-element of `kind` on element `index`: the winners of
/// every `::before`/`::after` rule of that kind whose base selector matches, ranked and
/// deduped exactly as [`crate::apply::winners`] ranks a real element's rules.
fn pseudo_winners(
    rules: &[Rule],
    tree: &Tree<'_>,
    index: usize,
    kind: PseudoElement,
) -> Vec<crate::css::Declaration> {
    let mut ranked = Vec::new();
    for (order, rule) in rules.iter().enumerate() {
        if rule.pseudo == Some(kind) && rule.selector.matches(tree, index) {
            for declaration in &rule.declarations {
                ranked.push((declaration.important, rule.specificity(), order, declaration));
            }
        }
    }
    ranked.sort_by_key(|(important, specificity, order, _)| (*important, *specificity, *order));

    let mut winners: Vec<crate::css::Declaration> = Vec::new();
    for (.., declaration) in ranked {
        match winners.iter_mut().find(|w| w.property == declaration.property) {
            Some(existing) => existing.clone_from(declaration),
            None => winners.push(declaration.clone()),
        }
    }
    winners
}

/// The parsed `content` of a pseudo-element: either text or, for a lone `url(...)`, an
/// image source.
enum Content {
    Text(String),
    Img(String),
}

/// juice's `parseContent`: `none`/`normal` → empty, a lone `url(...)` → image, otherwise
/// the quote-delimited tokens joined, with `attr()` / `var()` / `counter()` resolved.
fn parse_content(
    content: &str,
    index: usize,
    tree: &Tree<'_>,
    custom_maps: &[Vec<(String, String)>],
    counters: &HashMap<String, i64>,
) -> Content {
    let content = content.trim();
    if content == "none" || content == "normal" {
        return Content::Text(String::new());
    }
    if let Some(url) = lone_url(content) {
        return Content::Img(url);
    }

    let mut out = String::new();
    // Split on single/double quotes, as juice does: the odd segments are string literals,
    // the even ones function calls / whitespace.
    for token in content.split(['\'', '"']) {
        if token.is_empty() {
            continue;
        }
        if let Some(inner) = func_arg(token, "var") {
            let (name, fallback) = split_once_top(inner, ',');
            let resolved = resolve_var(index, name.trim(), tree, custom_maps)
                .or_else(|| fallback.map(|f| f.trim().to_owned()))
                .unwrap_or_default();
            out.push_str(strip_quotes(&resolved));
            continue;
        }
        if let Some(inner) = func_arg(token, "counter") {
            let (name, style) = split_once_top(inner, ',');
            if let Some(value) = counters.get(name.trim()) {
                out.push_str(&apply_counter_style(*value, style.map(str::trim)));
                continue;
            }
            // An undefined counter contributes nothing (juice leaves the token unmatched;
            // it then falls through to the literal branch, but a bare `counter(...)` token
            // has no useful literal form — mirror juice by emitting the raw token).
            out.push_str(token);
            continue;
        }
        if let Some(inner) = func_arg(token, "attr") {
            let name = inner.trim();
            if let Some(attr) = tree.elements[index].attr(name) {
                out.push_str(attr.value);
            }
            continue;
        }
        out.push_str(token);
    }
    // Naive unescape, as juice: drop backslashes.
    Content::Text(out.replace('\\', ""))
}

/// The whole value as a single `url(...)` (juice's image test), unwrapped and unquoted.
fn lone_url(content: &str) -> Option<String> {
    let inner = func_arg(content.trim(), "url")?;
    // Must be a *lone* url() — `func_arg` already anchored to the whole token.
    Some(strip_quotes(inner.trim()).to_owned())
}

/// If `token` (trimmed) is `name( … )`, return the inner argument text. Anchored: the
/// call must span the whole trimmed token.
fn func_arg<'a>(token: &'a str, name: &str) -> Option<&'a str> {
    let t = token.trim();
    let rest = t.strip_prefix(name).or_else(|| {
        // case-insensitive prefix
        (t.len() >= name.len() && t[..name.len()].eq_ignore_ascii_case(name))
            .then(|| &t[name.len()..])
    })?;
    let rest = rest.trim_start();
    let inner = rest.strip_prefix('(')?.strip_suffix(')')?;
    Some(inner)
}

/// Split `s` on the first top-level (unparenthesized, unquoted) occurrence of `sep`.
fn split_once_top(s: &str, sep: char) -> (&str, Option<&str>) {
    let bytes = s.as_bytes();
    let mut depth = 0i32;
    let mut quote: Option<u8> = None;
    for (i, &b) in bytes.iter().enumerate() {
        match quote {
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None => match b {
                b'"' | b'\'' => quote = Some(b),
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ if b == sep as u8 && depth == 0 => return (&s[..i], Some(&s[i + 1..])),
                _ => {}
            },
        }
    }
    (s, None)
}

/// Strip one layer of matching surrounding quotes.
fn strip_quotes(s: &str) -> &str {
    let s = s.trim();
    for q in ['"', '\''] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            return &s[1..s.len() - 1];
        }
    }
    s
}

/// juice's `applyCounterStyle`: the counter value rendered in the requested style
/// (`decimal` by default; roman / latin as juice supports them).
fn apply_counter_style(value: i64, style: Option<&str>) -> String {
    match style {
        Some("lower-roman") => romanize(value).to_lowercase(),
        Some("upper-roman") => romanize(value),
        Some("lower-latin" | "lower-alpha") => alphanumeric(value).to_lowercase(),
        Some("upper-latin" | "upper-alpha") => alphanumeric(value),
        _ => value.to_string(),
    }
}

/// juice's `numbers.romanize`.
fn romanize(num: i64) -> String {
    if num <= 0 {
        return num.to_string();
    }
    let key = [
        "", "C", "CC", "CCC", "CD", "D", "DC", "DCC", "DCCC", "CM", "", "X", "XX", "XXX", "XL", "L",
        "LX", "LXX", "LXXX", "XC", "", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX",
    ];
    let digits: Vec<u32> = num
        .to_string()
        .chars()
        .map(|c| c.to_digit(10).unwrap_or(0))
        .collect();
    let mut roman = String::new();
    let mut n = digits.clone();
    for i in (0..3).rev() {
        if let Some(&d) = n.pop().as_ref() {
            roman.insert_str(0, key[(d + i as u32 * 10) as usize]);
        }
    }
    // Thousands: one 'M' per remaining thousand.
    let thousands: String = n.iter().fold(String::new(), |mut acc, d| {
        acc.push_str(&d.to_string());
        acc
    });
    let m = thousands.parse::<usize>().unwrap_or(0);
    format!("{}{roman}", "M".repeat(m))
}

/// juice's `numbers.alphanumeric` (spreadsheet-style: 1→A, 26→Z, 27→AA).
fn alphanumeric(num: i64) -> String {
    if num <= 0 {
        return String::new();
    }
    let mut n = num;
    let mut s = String::new();
    while n > 0 {
        let t = (n - 1) % 26;
        s.insert(0, (b'A' + t as u8) as char);
        n = (n - t) / 26;
    }
    s
}

/// One document-order pass computing each element's counter snapshot: a global running
/// value per counter (juice's `counters`) plus a per-element map inheriting the parent's
/// (juice's prototype-chained `counterProps`).
fn counter_snapshots(rules: &[Rule], tree: &Tree<'_>) -> Vec<HashMap<String, i64>> {
    let mut global: HashMap<String, i64> = HashMap::new();
    let mut per_element: Vec<HashMap<String, i64>> = Vec::with_capacity(tree.elements.len());

    for index in 0..tree.elements.len() {
        // Inherit the parent's counters (prototype chain), then apply this element's own.
        let mut props = tree.elements[index]
            .parent
            .map(|p| per_element[p].clone())
            .unwrap_or_default();

        let decls = winners(rules, tree, index);
        // Resets first, then increments (mjml/CSS write them that way; a reset must be
        // seen before an increment on the same counter).
        for decl in decls.iter().filter(|d| d.property == "counter-reset") {
            reset_counters(&decl.value, &mut global, &mut props);
        }
        for decl in decls.iter().filter(|d| d.property == "counter-increment") {
            increment_counters(&decl.value, &mut global, &mut props);
        }
        per_element.push(props);
    }
    per_element
}

/// juice's `resetCounter`: `name [value]` pairs; a missing value resets to 0.
fn reset_counters(value: &str, global: &mut HashMap<String, i64>, props: &mut HashMap<String, i64>) {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    let mut j = 0;
    while j < tokens.len() {
        let counter = tokens[j].to_owned();
        let reset = tokens.get(j + 1).and_then(|t| t.parse::<i64>().ok());
        let v = reset.unwrap_or(0);
        global.insert(counter.clone(), v);
        props.insert(counter, v);
        j += if reset.is_some() { 2 } else { 1 };
    }
}

/// juice's `incrementCounter`: `name [value]` pairs; only counters already defined (via a
/// reset in scope) advance.
fn increment_counters(
    value: &str,
    global: &mut HashMap<String, i64>,
    props: &mut HashMap<String, i64>,
) {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    let mut j = 0;
    while j < tokens.len() {
        let counter = tokens[j];
        if !props.contains_key(counter) {
            j += 1;
            continue;
        }
        let incr = tokens.get(j + 1).and_then(|t| t.parse::<i64>().ok());
        let delta = incr.unwrap_or(1);
        let v = global.get(counter).copied().unwrap_or(0) + delta;
        global.insert(counter.to_owned(), v);
        props.insert(counter.to_owned(), v);
        j += if incr.is_some() { 2 } else { 1 };
    }
}

/// Escape a double-quoted attribute value.
fn escape_attr(value: &str) -> String {
    value.replace('&', "&amp;").replace('"', "&quot;")
}

/// Escape text content (what `$(el).text(...)` would emit).
fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
