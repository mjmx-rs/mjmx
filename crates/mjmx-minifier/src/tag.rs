//! Start-tag rewriting for the [`MinifyOptions`] switches that act on attributes:
//! `case_sensitive`, `remove_empty_attributes` and `minify_css` (which reaches
//! `style="…"` as well as `<style>`).
//!
//! The tokenizer keeps every tag's exact source bytes, and [`rewrite`] preserves that
//! guarantee: it returns `Some(new_text)` **only** when a switch actually applies to
//! this tag, and `None` — meaning "emit the original bytes" — otherwise. So a document
//! with no empty `class=""`/`style=""`, no upper-case attribute names and no
//! shortenable `style` value comes out byte-identical whatever the switches are set to.

use std::borrow::Cow;

use crate::{MinifyOptions, css};

/// One parsed attribute of a start tag.
struct Attr<'a> {
    /// The attribute name, exactly as written.
    name: &'a str,
    /// The value *including* its quotes (`"left"`, `'left'`) when the attribute had
    /// one, or `None` for a valueless attribute (`<input disabled>`).
    value: Option<&'a str>,
}

impl Attr<'_> {
    /// The value with its surrounding quotes stripped; `""` for a valueless attribute.
    fn unquoted(&self) -> &str {
        let Some(value) = self.value else { return "" };
        let bytes = value.as_bytes();
        if bytes.len() >= 2
            && (bytes[0] == b'"' || bytes[0] == b'\'')
            && bytes[0] == bytes[bytes.len() - 1]
        {
            &value[1..value.len() - 1]
        } else {
            value
        }
    }
}

/// Attribute names html-minifier will drop when their value is empty, on any element.
///
/// Ported verbatim from html-minifier's `reEmptyAttribute`
/// (`src/htmlminifier.js:483-485`). Note how narrow it is: it is *not* "every `on*`
/// handler" and *not* "every empty attribute" — `alt=""` and `href=""` are meaningful
/// and are kept. `value` on `<input>` is handled separately, as upstream does.
const EMPTY_REMOVABLE: &[&str] = &[
    "class",
    "id",
    "style",
    "title",
    "lang",
    "dir",
    "onfocus",
    "onblur",
    "onchange",
    "onclick",
    "ondblclick",
    "onmousedown",
    "onmouseup",
    "onmouseover",
    "onmousemove",
    "onmouseout",
    "onkeypress",
    "onkeydown",
    "onkeyup",
];

/// html-minifier's `canDeleteEmptyAttribute` (`src/htmlminifier.js:487-496`).
fn can_delete_empty(tag: &str, name: &str, value: &str) -> bool {
    if !value.trim().is_empty() {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    (tag == "input" && lower == "value") || EMPTY_REMOVABLE.contains(&lower.as_str())
}

/// Rewrite the start tag `text` (whose lower-cased name is `tag`) under `options`,
/// or `None` when nothing needs changing and the source bytes should be kept.
pub(crate) fn rewrite(text: &str, tag: &str, options: &MinifyOptions) -> Option<String> {
    if options.case_sensitive && !options.remove_empty_attributes && !options.minify_css {
        // No switch touches attributes.
        return None;
    }

    let (name, attrs, self_closing) = parse(text)?;

    let mut changed = false;
    let mut kept: Vec<(String, Option<Cow<'_, str>>)> = Vec::with_capacity(attrs.len());
    for attr in &attrs {
        if options.remove_empty_attributes && can_delete_empty(tag, attr.name, attr.unquoted()) {
            changed = true;
            continue;
        }
        let out_name = if options.case_sensitive {
            attr.name.to_owned()
        } else {
            let lowered = attr.name.to_ascii_lowercase();
            changed |= lowered != attr.name;
            lowered
        };
        // Both upstream minifiers run their CSS pass over `style` attributes too, not
        // only `<style>` bodies. A tag inside an Outlook conditional never reaches here
        // — it is comment text to the tokenizer — which is exactly where upstream's
        // parser stops as well.
        let out_value = match attr.value {
            Some(value) if options.minify_css && attr.name.eq_ignore_ascii_case("style") => {
                let minified = css::minify_declarations(attr.unquoted(), options.css_dialect);
                if minified == attr.unquoted() {
                    Some(Cow::Borrowed(value))
                } else {
                    changed = true;
                    Some(Cow::Owned(format!("\"{minified}\"")))
                }
            }
            other => other.map(Cow::Borrowed),
        };
        kept.push((out_name, out_value));
    }

    if !changed {
        return None;
    }

    let mut out = String::with_capacity(text.len());
    out.push('<');
    out.push_str(name);
    for (attr_name, value) in kept {
        out.push(' ');
        out.push_str(&attr_name);
        if let Some(value) = value {
            out.push('=');
            out.push_str(&value);
        }
    }
    if self_closing {
        out.push_str(" /");
    }
    out.push('>');
    Some(out)
}

/// Split a start tag's source into `(name, attributes, self_closing)`.
///
/// Returns `None` for anything that does not look like a start tag, so the caller
/// falls back to emitting the original bytes.
fn parse(text: &str) -> Option<(&str, Vec<Attr<'_>>, bool)> {
    let inner = text.strip_prefix('<')?.strip_suffix('>')?;
    let name_end = inner
        .find(|c: char| c.is_whitespace() || c == '/')
        .unwrap_or(inner.len());
    let name = &inner[..name_end];
    if name.is_empty() {
        return None;
    }

    let bytes = inner.as_bytes();
    let mut attrs = Vec::new();
    let mut self_closing = false;
    let mut i = name_end;

    while i < inner.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes[i] == b'/' {
            self_closing = true;
            i += 1;
            continue;
        }

        let start = i;
        while i < inner.len()
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'='
            && bytes[i] != b'/'
        {
            i += 1;
        }
        let attr_name = &inner[start..i];

        // Look past whitespace for a `=`; without one this is a valueless attribute.
        let mut probe = i;
        while probe < inner.len() && bytes[probe].is_ascii_whitespace() {
            probe += 1;
        }
        if probe >= inner.len() || bytes[probe] != b'=' {
            attrs.push(Attr {
                name: attr_name,
                value: None,
            });
            continue;
        }

        probe += 1;
        while probe < inner.len() && bytes[probe].is_ascii_whitespace() {
            probe += 1;
        }
        let value_start = probe;
        if probe < inner.len() && (bytes[probe] == b'"' || bytes[probe] == b'\'') {
            let quote = bytes[probe];
            probe += 1;
            while probe < inner.len() && bytes[probe] != quote {
                probe += 1;
            }
            if probe < inner.len() {
                probe += 1; // closing quote
            }
        } else {
            while probe < inner.len() && !bytes[probe].is_ascii_whitespace() && bytes[probe] != b'/'
            {
                probe += 1;
            }
        }
        attrs.push(Attr {
            name: attr_name,
            value: Some(&inner[value_start..probe]),
        });
        i = probe;
    }

    Some((name, attrs, self_closing))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(case_sensitive: bool, remove_empty_attributes: bool) -> MinifyOptions {
        MinifyOptions {
            case_sensitive,
            remove_empty_attributes,
            ..MinifyOptions::default()
        }
    }

    #[test]
    fn a_tag_with_nothing_to_change_keeps_its_bytes() {
        let text = r#"<td   align="left"  class="a">"#;
        assert_eq!(rewrite(text, "td", &opts(true, true)), None);
    }

    #[test]
    fn empty_class_and_style_are_dropped() {
        let text = r#"<div class="" style="" align="center">"#;
        assert_eq!(
            rewrite(text, "div", &opts(true, true)).unwrap(),
            r#"<div align="center">"#
        );
    }

    #[test]
    fn empty_alt_and_href_are_kept() {
        // Not in html-minifier's list: they carry meaning even when empty.
        let text = r#"<img alt="" href="">"#;
        assert_eq!(rewrite(text, "img", &opts(true, true)), None);
    }

    #[test]
    fn empty_input_value_is_dropped_but_only_on_input() {
        assert_eq!(
            rewrite(
                r#"<input type="checkbox" value="">"#,
                "input",
                &opts(true, true)
            )
            .unwrap(),
            r#"<input type="checkbox">"#
        );
        assert_eq!(
            rewrite(r#"<option value="">"#, "option", &opts(true, true)),
            None
        );
    }

    #[test]
    fn removal_is_off_when_the_switch_is_off() {
        assert_eq!(
            rewrite(r#"<div class="">"#, "div", &opts(true, false)),
            None
        );
    }

    #[test]
    fn attribute_names_lower_case_when_not_case_sensitive() {
        assert_eq!(
            rewrite(
                r#"<td vAlign="top" bgColor="white">"#,
                "td",
                &opts(false, false)
            )
            .unwrap(),
            r#"<td valign="top" bgcolor="white">"#
        );
    }

    #[test]
    fn self_closing_and_valueless_attributes_survive_a_rewrite() {
        assert_eq!(
            rewrite(
                r#"<img class="" disabled src="x" />"#,
                "img",
                &opts(true, true)
            )
            .unwrap(),
            r#"<img disabled src="x" />"#
        );
    }
}
