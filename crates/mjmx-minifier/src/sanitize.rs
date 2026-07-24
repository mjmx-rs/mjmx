//! Template-variable protection for the CSS-minify pass, mjml 5.4.0's
//! `sanitizeStyles` / `allowMixedSyntax`.
//!
//! A template author may leave engine syntax inside CSS — `color: {{brand}}`,
//! `{{prop}}: red`, or a whole `{{#if}}…{{/if}}` block — expecting it to survive
//! rendering untouched. It does, until a CSS pass tries to *parse* the stylesheet:
//! mjmx's [`minify_css`](crate::minify_css) hands the `<style>` body and every
//! `style="…"` attribute to `cssparser`, which sees `{{brand}}` as garbage and
//! mangles the rule (upstream, `postcss`/`cssnano` throws outright).
//!
//! mjml's fix — ported here faithfully — is to **mask** each template variable
//! with an inert, parseable placeholder before the CSS pass and **restore** it
//! after. Three placement contexts are distinguished exactly as mjml does:
//!
//! - **value** (`color: {{x}}`): the whole `{{x}}` becomes a bare ident
//!   `variable_temp_N`;
//! - **property** (`{{x}}: red`): becomes a custom property `--mj-prop-temp_N:`;
//! - **block** (`{{#if}}`, anywhere else): becomes a fake declaration
//!   `sanitizedI:inner;`.
//!
//! Two guards, also mjml's: an **unbalanced** delimiter count is a hard error, and
//! **mixed** block + value/property syntax in one document is an error unless
//! `allow_mixed` is set.
//!
//! This is an **opt-in** pass. The default render never calls it, so its output is
//! unchanged. It operates only on the two CSS-bearing scopes mjml's regexes target
//! — `style="…"` attribute values and `<style>…</style>` bodies — and, like mjml,
//! it is regex/string surgery over the serialized HTML rather than a DOM pass.

/// One template-delimiter pair, e.g. `{{` … `}}`. mjml's `templateSyntax` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Syntax {
    /// Opening delimiter, e.g. `{{`.
    pub prefix: String,
    /// Closing delimiter, e.g. `}}`.
    pub suffix: String,
}

/// mjml's default `templateSyntax`: Handlebars/Mustache `{{ }}` and `[[ ]]`.
#[must_use]
pub fn default_syntaxes() -> Vec<Syntax> {
    vec![
        Syntax {
            prefix: "{{".into(),
            suffix: "}}".into(),
        },
        Syntax {
            prefix: "[[".into(),
            suffix: "]]".into(),
        },
    ]
}

/// Which template-variable placements a document's CSS contains.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Detect {
    /// A variable in value position (`color: {{x}}`).
    pub is_value: bool,
    /// A variable in property position (`{{x}}: red`).
    pub is_property: bool,
    /// A variable in neither position — a control block (`{{#if}}`).
    pub is_block: bool,
}

impl Detect {
    /// Whether any template variable was found in CSS at all.
    #[must_use]
    pub fn any(self) -> bool {
        self.is_value || self.is_property || self.is_block
    }
}

/// Why [`sanitize`] refused a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SanitizeError {
    /// A delimiter pair whose opens and closes do not balance in CSS.
    Unbalanced {
        /// The offending pair, formatted `{{…}}`.
        pair: String,
        /// How many opening delimiters were counted.
        open: usize,
        /// How many closing delimiters were counted.
        close: usize,
    },
    /// Block syntax mixed with value/property syntax without `allow_mixed`.
    Mixed,
}

impl std::fmt::Display for SanitizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SanitizeError::Unbalanced { pair, open, close } => write!(
                f,
                "unbalanced template delimiters in CSS: {pair} ({open} open, {close} close)"
            ),
            SanitizeError::Mixed => write!(
                f,
                "mixed template syntax in CSS: use either property/value syntax \
                 (e.g. color: {{{{x}}}}) or block syntax (e.g. {{{{x}}}}), not both \
                 — or set allow_mixed_syntax"
            ),
        }
    }
}

impl std::error::Error for SanitizeError {}

/// The masking done to a document, and the maps needed to undo it. Opaque; hand it
/// straight back to [`restore`].
#[derive(Debug, Default)]
pub struct Sanitized {
    /// The masked HTML, safe to run the CSS pass over.
    pub content: String,
    /// Whether anything was actually masked (else `content` equals the input).
    pub did_sanitize: bool,
    value_map: Vec<(String, String)>,
    prop_map: Vec<(String, String)>,
    is_block: bool,
}

/// Detect which template-variable placements appear in `html`'s CSS scopes.
#[must_use]
pub fn detect(html: &str, syntaxes: &[Syntax]) -> Detect {
    let content = style_content(html);
    let mut d = Detect::default();
    for syn in syntaxes {
        let mut from = 0;
        while let Some((var_start, _, _, var_end)) =
            next_var(&content, from, &syn.prefix, &syn.suffix)
        {
            let before = &content[..var_start];
            let after = &content[var_end..];
            let value_ctx = value_context(before);
            let prop_ctx = after.trim_start().starts_with(':');
            if value_ctx {
                d.is_value = true;
            }
            if prop_ctx {
                d.is_property = true;
            }
            if !value_ctx && !prop_ctx {
                d.is_block = true;
            }
            from = var_end;
        }
    }
    d
}

/// Mask every template variable in `html`'s CSS scopes with a parseable placeholder.
///
/// Returns the masked HTML plus the data [`restore`] needs. A document with no
/// template variables comes back unchanged with `did_sanitize == false`.
///
/// # Errors
///
/// - [`SanitizeError::Unbalanced`] if a delimiter pair does not balance in CSS;
/// - [`SanitizeError::Mixed`] if block syntax coexists with value/property syntax
///   and `allow_mixed` is false.
pub fn sanitize(
    html: &str,
    syntaxes: &[Syntax],
    allow_mixed: bool,
) -> Result<Sanitized, SanitizeError> {
    let content_scan = style_content(html);
    for syn in syntaxes {
        let open = count_occurrences(&content_scan, &syn.prefix);
        let close = count_occurrences(&content_scan, &syn.suffix);
        if open != close {
            return Err(SanitizeError::Unbalanced {
                pair: format!("{}…{}", syn.prefix, syn.suffix),
                open,
                close,
            });
        }
    }

    let detected = detect(html, syntaxes);
    if !allow_mixed && detected.is_block && (detected.is_value || detected.is_property) {
        return Err(SanitizeError::Mixed);
    }

    let mut result = Sanitized {
        content: html.to_string(),
        is_block: detected.is_block,
        ..Sanitized::default()
    };

    // Order matches mjml: value, then property, then block. Each earlier pass
    // consumes its own `prefix…suffix` occurrences, so the block pass only ever
    // sees the genuine control blocks that remain.
    if detected.is_value {
        result.content = sanitize_value(&result.content, syntaxes, &mut result.value_map);
        result.did_sanitize = true;
    }
    if detected.is_property {
        result.content = sanitize_property(&result.content, syntaxes, &mut result.prop_map);
        result.did_sanitize = true;
    }
    if detected.is_block {
        result.content = sanitize_block(&result.content, syntaxes);
        result.did_sanitize = true;
    }
    Ok(result)
}

/// Undo the masking [`sanitize`] applied, restoring every original template
/// variable. `restore(sanitize(x).content-after-css-pass)` recovers the variables.
#[must_use]
pub fn restore(html: &str, sanitized: &Sanitized, syntaxes: &[Syntax]) -> String {
    let mut out = html.to_string();
    // Value placeholders are bare idents; restore by whole-word swap.
    for (temp, original) in &sanitized.value_map {
        out = replace_word(&out, temp, original);
    }
    // Property placeholders are `--mj-prop-temp_N:`; restore the `name:` shape.
    for (temp, original) in &sanitized.prop_map {
        out = out.replace(&format!("{temp}:"), &format!("{original}:"));
        // A CSS pass may have inserted a space: `--mj-prop-temp_0 :`.
        out = out.replace(&format!("{temp} :"), &format!("{original}:"));
    }
    if sanitized.is_block {
        out = restore_block(&out, syntaxes);
    }
    out
}

// ---- value context (`color: {{x}}`) ---------------------------------------

/// mjml's `sanitizeCssValueVariablesHtml`: replace each `: {{x}}` occurrence's
/// `{{x}}` with a bare ` variable_temp_N ` ident, collapsing surrounding space.
fn sanitize_value(html: &str, syntaxes: &[Syntax], map: &mut Vec<(String, String)>) -> String {
    let mut counter = 0usize;
    let out = map_style_attrs(html, |value| {
        mask_value_scope(value, syntaxes, map, &mut counter, true)
    });
    map_style_blocks(&out, |content| {
        mask_value_scope(content, syntaxes, map, &mut counter, false)
    })
}

fn mask_value_scope(
    scope: &str,
    syntaxes: &[Syntax],
    map: &mut Vec<(String, String)>,
    counter: &mut usize,
    collapse: bool,
) -> String {
    // Collect every value-position variable token (the raw `{{…}}` slice), in
    // source order, then swap each for a fresh placeholder.
    let mut tokens = value_tokens(scope, syntaxes);
    tokens.sort_by_key(|&(idx, _)| idx);
    let mut result = scope.to_string();
    for (_, token) in tokens {
        let temp = format!("variable_temp_{counter}");
        map.push((temp.clone(), token.clone()));
        result = replace_first(&result, &token, &format!(" {temp} "));
        *counter += 1;
    }
    if collapse {
        collapse_ws(&result).trim().to_string()
    } else {
        result
    }
}

/// The raw `{{…}}` slices of every variable in value position within `scope`.
fn value_tokens(scope: &str, syntaxes: &[Syntax]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for syn in syntaxes {
        let mut from = 0;
        while let Some((var_start, _, _, var_end)) = next_var(scope, from, &syn.prefix, &syn.suffix)
        {
            if value_context(&scope[..var_start]) {
                out.push((var_start, scope[var_start..var_end].to_string()));
            }
            from = var_end;
        }
    }
    out
}

// ---- property context (`{{x}}: red`) --------------------------------------

/// mjml's `sanitizeCssPropertyVariablesHtml`: replace each `{{x}} :` occurrence
/// (variable then colon) with `--mj-prop-temp_N:`.
fn sanitize_property(html: &str, syntaxes: &[Syntax], map: &mut Vec<(String, String)>) -> String {
    let mut counter = 0usize;
    let out = map_style_attrs(html, |value| {
        mask_property_scope(value, syntaxes, map, &mut counter, true)
    });
    map_style_blocks(&out, |content| {
        mask_property_scope(content, syntaxes, map, &mut counter, false)
    })
}

fn mask_property_scope(
    scope: &str,
    syntaxes: &[Syntax],
    map: &mut Vec<(String, String)>,
    counter: &mut usize,
    collapse: bool,
) -> String {
    // `(raw token including trailing " :", raw token only)`, in source order.
    let mut tokens = property_tokens(scope, syntaxes);
    tokens.sort_by_key(|(idx, _, _)| *idx);
    let mut result = scope.to_string();
    for (_, full, var_only) in tokens {
        let temp = format!("--mj-prop-temp_{counter}");
        map.push((temp.clone(), var_only));
        result = replace_first(&result, &full, &format!("{temp}:"));
        *counter += 1;
    }
    if collapse {
        collapse_ws(&result).trim().to_string()
    } else {
        result
    }
}

/// For each property-position variable: `(index, "{{x}}<ws>:", "{{x}}")`.
fn property_tokens(scope: &str, syntaxes: &[Syntax]) -> Vec<(usize, String, String)> {
    let mut out = Vec::new();
    for syn in syntaxes {
        let mut from = 0;
        while let Some((var_start, _, _, var_end)) = next_var(scope, from, &syn.prefix, &syn.suffix)
        {
            let after = &scope[var_end..];
            let ws = after.len() - after.trim_start().len();
            if after[ws..].starts_with(':') {
                let full_end = var_end + ws + 1; // include the ':'
                out.push((
                    var_start,
                    scope[var_start..full_end].to_string(),
                    scope[var_start..var_end].to_string(),
                ));
            }
            from = var_end;
        }
    }
    out
}

// ---- block context (`{{#if}}`) --------------------------------------------

/// mjml's `sanitizeInlineStyleAttributes` + `sanitizeStyleTagBlocks`: replace every
/// remaining `{{…}}` with a fake declaration `sanitizedI:inner;`.
fn sanitize_block(html: &str, syntaxes: &[Syntax]) -> String {
    let out = map_style_attrs(html, |value| mask_block_attr(value, syntaxes));
    map_style_blocks(&out, |content| mask_block_content(content, syntaxes))
}

fn mask_block_attr(value: &str, syntaxes: &[Syntax]) -> String {
    let mut result = value.to_string();
    for (idx, syn) in syntaxes.iter().enumerate() {
        result = replace_all_vars(&result, &syn.prefix, &syn.suffix, |lead, inner, trail| {
            format!("{lead}sanitized{idx}:{};{trail}", inner.trim())
        });
    }
    result.strip_suffix(';').unwrap_or(&result).to_string()
}

fn mask_block_content(content: &str, syntaxes: &[Syntax]) -> String {
    let mut result = content.to_string();
    for (idx, syn) in syntaxes.iter().enumerate() {
        result = replace_all_vars(&result, &syn.prefix, &syn.suffix, |lead, inner, trail| {
            format!("{lead}sanitized{idx}:{inner};{trail}")
        });
    }
    result
}

/// mjml's `restoreInlineStyleAttributes` + `restoreStyleTagBlocks`: turn every
/// `sanitizedI:inner;` (or trailing `sanitizedI:inner`) back into `prefix inner suffix`.
fn restore_block(html: &str, syntaxes: &[Syntax]) -> String {
    let mut out = String::with_capacity(html.len());
    let mut i = 0;
    while let Some(rel) = html[i..].find("sanitized") {
        let start = i + rel;
        let after = &html[start + "sanitized".len()..];
        // sanitized<digit>:
        let digit_len = after.chars().take_while(char::is_ascii_digit).count();
        if digit_len == 0 || !after[digit_len..].starts_with(':') {
            out.push_str(&html[i..start + "sanitized".len()]);
            i = start + "sanitized".len();
            continue;
        }
        let idx: usize = after[..digit_len].parse().unwrap_or(usize::MAX);
        let inner_start = start + "sanitized".len() + digit_len + 1; // past ':'
        // inner runs to the next ';' (or end of this scope's text).
        let inner_end = html[inner_start..]
            .find(';')
            .map_or(html.len(), |r| inner_start + r);
        match syntaxes.get(idx) {
            Some(syn) => {
                out.push_str(&html[i..start]);
                out.push_str(&syn.prefix);
                out.push_str(&html[inner_start..inner_end]);
                out.push_str(&syn.suffix);
                // Consume the trailing ';' if present.
                i = if inner_end < html.len() && html.as_bytes()[inner_end] == b';' {
                    inner_end + 1
                } else {
                    inner_end
                };
            }
            None => {
                out.push_str(&html[i..inner_start]);
                i = inner_start;
            }
        }
    }
    out.push_str(&html[i..]);
    out
}

// ---- shared scanning helpers ----------------------------------------------

/// The concatenated text of every `style="…"` value and `<style>…</style>` body,
/// joined by newlines — mjml's `styleContent`, the surface both detection and the
/// balance check read.
fn style_content(html: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for_each_style_attr(html, |value| parts.push(value));
    for_each_style_block(html, |content| parts.push(content));
    parts.join("\n")
}

/// The next shortest `prefix … suffix` in `s` at or after `from`, as byte indices
/// `(var_start, inner_start, inner_end, var_end)`.
fn next_var(
    s: &str,
    from: usize,
    prefix: &str,
    suffix: &str,
) -> Option<(usize, usize, usize, usize)> {
    let p = s[from..].find(prefix)? + from;
    let inner_start = p + prefix.len();
    let sfx = s[inner_start..].find(suffix)? + inner_start;
    Some((p, inner_start, sfx, sfx + suffix.len()))
}

/// Whether the text before a variable puts it in CSS value position: after the last
/// `;` or `{`, a `:` appears. mjml's `/:[^;{]*$/`.
fn value_context(before: &str) -> bool {
    let cut = before.rfind([';', '{']).map_or(0, |i| i + 1);
    before[cut..].contains(':')
}

/// Count non-overlapping occurrences of `needle` in `haystack`.
fn count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Replace the first literal occurrence of `needle` with `replacement`.
fn replace_first(s: &str, needle: &str, replacement: &str) -> String {
    match s.find(needle) {
        Some(i) => {
            let mut out = String::with_capacity(s.len() - needle.len() + replacement.len());
            out.push_str(&s[..i]);
            out.push_str(replacement);
            out.push_str(&s[i + needle.len()..]);
            out
        }
        None => s.to_string(),
    }
}

/// Replace every whole-word occurrence of `word` (bounded by non-word chars) with
/// `replacement`. mjml restores value placeholders with a `\bword\b` swap.
fn replace_word(s: &str, word: &str, replacement: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while let Some(rel) = s[i..].find(word) {
        let start = i + rel;
        let end = start + word.len();
        let before_ok = s[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !is_word_char(c));
        let after_ok = s[end..].chars().next().is_none_or(|c| !is_word_char(c));
        if before_ok && after_ok {
            out.push_str(&s[i..start]);
            out.push_str(replacement);
            i = end;
        } else {
            out.push_str(&s[i..end]);
            i = end;
        }
    }
    out.push_str(&s[i..]);
    out
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Replace every `(leading-ws)prefix(inner)suffix(trailing-ws)` in `s`, handing the
/// captured leading ws, raw inner, and trailing ws to `f`. mjml's block regexes
/// swallow the surrounding whitespace, so the callback owns re-emitting it.
fn replace_all_vars(
    s: &str,
    prefix: &str,
    suffix: &str,
    f: impl Fn(&str, &str, &str) -> String,
) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while let Some((var_start, inner_start, inner_end, var_end)) = next_var(s, i, prefix, suffix) {
        // Leading whitespace run ending at var_start.
        let lead_start = s[i..var_start]
            .rfind(|c: char| !c.is_whitespace())
            .map_or(i, |r| i + r + 1);
        // Trailing whitespace run starting at var_end.
        let trail_len = s[var_end..]
            .find(|c: char| !c.is_whitespace())
            .unwrap_or(s.len() - var_end);
        let trail_end = var_end + trail_len;
        out.push_str(&s[i..lead_start]);
        out.push_str(&f(
            &s[lead_start..var_start],
            &s[inner_start..inner_end],
            &s[var_end..trail_end],
        ));
        i = trail_end;
    }
    out.push_str(&s[i..]);
    out
}

/// Collapse every whitespace run to a single space (does not trim). mjml's
/// `.replace(/\s+/g, ' ')`.
fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_ws {
                out.push(' ');
                prev_ws = true;
            }
        } else {
            out.push(c);
            prev_ws = false;
        }
    }
    out
}

/// Visit every `style="…"` attribute value (read-only).
fn for_each_style_attr<'a>(html: &'a str, mut f: impl FnMut(&'a str)) {
    let mut i = 0;
    while let Some(rel) = html[i..].find("style=\"") {
        let val_start = i + rel + "style=\"".len();
        match html[val_start..].find('"') {
            Some(end_rel) => {
                let val_end = val_start + end_rel;
                f(&html[val_start..val_end]);
                i = val_end + 1;
            }
            None => break,
        }
    }
}

/// Visit every `<style…>…</style>` body (read-only).
fn for_each_style_block<'a>(html: &'a str, mut f: impl FnMut(&'a str)) {
    let mut i = 0;
    while let Some(rel) = html[i..].find("<style") {
        let tag_start = i + rel;
        let Some(gt_rel) = html[tag_start..].find('>') else {
            break;
        };
        let content_start = tag_start + gt_rel + 1;
        match html[content_start..].find("</style") {
            Some(close_rel) => {
                let content_end = content_start + close_rel;
                f(&html[content_start..content_end]);
                i = content_end + "</style".len();
            }
            None => break,
        }
    }
}

/// Rewrite every `style="…"` attribute value through `f`.
fn map_style_attrs(html: &str, mut f: impl FnMut(&str) -> String) -> String {
    let mut out = String::with_capacity(html.len());
    let mut i = 0;
    while let Some(rel) = html[i..].find("style=\"") {
        let head_end = i + rel + "style=\"".len();
        match html[head_end..].find('"') {
            Some(end_rel) => {
                let val_end = head_end + end_rel;
                out.push_str(&html[i..head_end]);
                out.push_str(&f(&html[head_end..val_end]));
                out.push('"');
                i = val_end + 1;
            }
            None => break,
        }
    }
    out.push_str(&html[i..]);
    out
}

/// Rewrite every `<style…>…</style>` body through `f`.
fn map_style_blocks(html: &str, mut f: impl FnMut(&str) -> String) -> String {
    let mut out = String::with_capacity(html.len());
    let mut i = 0;
    while let Some(rel) = html[i..].find("<style") {
        let tag_start = i + rel;
        let Some(gt_rel) = html[tag_start..].find('>') else {
            break;
        };
        let content_start = tag_start + gt_rel + 1;
        match html[content_start..].find("</style") {
            Some(close_rel) => {
                let content_end = content_start + close_rel;
                out.push_str(&html[i..content_start]);
                out.push_str(&f(&html[content_start..content_end]));
                i = content_end;
            }
            None => break,
        }
    }
    out.push_str(&html[i..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MinifyOptions, minify_with};

    /// The full pipeline the umbrella runs: sanitize → CSS-minify → restore. Every
    /// expected string here was verified byte-for-byte against real `mjml@5.4.0`
    /// output (`{ minify: true, sanitizeStyles: true, allowMixedSyntax: true }`).
    fn round_trip(style_body: &str) -> String {
        let html = format!("<style type=\"text/css\">{style_body}</style>");
        let syn = default_syntaxes();
        let s = sanitize(&html, &syn, true).unwrap();
        let min = minify_with(&s.content, &MinifyOptions::for_mjml5());
        restore(&min, &s, &syn)
            .replace("<style type=\"text/css\">", "")
            .replace("</style>", "")
    }

    #[test]
    fn value_variable_survives_css_minify() {
        // Note the `{{pad}} px` space: masking `{{pad}}` as ` variable_temp_1 `
        // splits it from `px`, and restore does not re-join — matches mjml exactly.
        assert_eq!(
            round_trip(".x { color: {{brandColor}}; padding: {{pad}}px }"),
            ".x{color:{{brandColor}};padding:{{pad}} px}"
        );
    }

    #[test]
    fn property_variable_survives_css_minify() {
        assert_eq!(round_trip(".y { {{prop}}: red }"), ".y{{{prop}}:red}");
    }

    #[test]
    fn block_variable_survives_css_minify() {
        assert_eq!(
            round_trip("{{#if dark}}.z{color:#fff}{{/if}}"),
            "{{#if dark}}.z{color:#fff}{{/if}}"
        );
    }

    #[test]
    fn bracket_syntax_survives_css_minify() {
        assert_eq!(round_trip(".b { color: [[c]] }"), ".b{color:[[c]]}");
    }

    #[test]
    fn sanitizes_style_attributes_too() {
        let html = r#"<div style="color:{{c}};width:10px">x</div>"#;
        let syn = default_syntaxes();
        let s = sanitize(html, &syn, false).unwrap();
        assert!(s.did_sanitize);
        // The variable is masked out of the content the CSS pass will see.
        assert!(!s.content.contains("{{c}}"), "{}", s.content);
        // Through the full pipeline (the masking collapses whitespace, which the
        // minify pass then normalises) the variable and the real declaration return.
        let min = minify_with(&s.content, &MinifyOptions::for_mjml5());
        let restored = restore(&min, &s, &syn);
        assert!(restored.contains("{{c}}"), "{restored}");
        assert!(restored.contains("width:10px"), "{restored}");
        assert!(!restored.contains("variable_temp"), "{restored}");
    }

    #[test]
    fn no_variables_is_a_no_op() {
        let html = "<style>.a{color:red}</style>";
        let s = sanitize(html, &default_syntaxes(), false).unwrap();
        assert!(!s.did_sanitize);
        assert_eq!(s.content, html);
    }

    #[test]
    fn detect_classifies_each_context() {
        let syn = default_syntaxes();
        assert!(detect("<style>.a{color:{{v}}}</style>", &syn).is_value);
        assert!(detect("<style>.a{ {{p}}:red }</style>", &syn).is_property);
        assert!(detect("<style>{{#if x}}.a{color:red}{{/if}}</style>", &syn).is_block);
        assert!(!detect("<style>.a{color:red}</style>", &syn).any());
    }

    #[test]
    fn unbalanced_delimiters_are_rejected() {
        // Two opens, one close.
        let err = sanitize(
            "<style>.a{width:{{w}}px;color:{{c}</style>",
            &default_syntaxes(),
            false,
        )
        .unwrap_err();
        assert!(matches!(err, SanitizeError::Unbalanced { .. }), "{err:?}");
    }

    #[test]
    fn mixed_syntax_is_rejected_unless_allowed() {
        let html = "<style>{{#if x}}.a{color:{{v}}}{{/if}}</style>";
        let syn = default_syntaxes();
        assert_eq!(
            sanitize(html, &syn, false).unwrap_err(),
            SanitizeError::Mixed
        );
        // allow_mixed lets it through.
        assert!(sanitize(html, &syn, true).is_ok());
    }

    #[test]
    fn custom_syntax_is_honoured() {
        let syn = vec![Syntax {
            prefix: "<#".into(),
            suffix: "#>".into(),
        }];
        assert_eq!(
            round_trip_with(".a { color: <# c #> }", &syn),
            ".a{color:<# c #>}"
        );
    }

    fn round_trip_with(style_body: &str, syn: &[Syntax]) -> String {
        let html = format!("<style type=\"text/css\">{style_body}</style>");
        let s = sanitize(&html, syn, true).unwrap();
        let min = minify_with(&s.content, &MinifyOptions::for_mjml5());
        restore(&min, &s, syn)
            .replace("<style type=\"text/css\">", "")
            .replace("</style>", "")
    }
}
