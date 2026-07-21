//! A small, forgiving HTML tokenizer producing a flat event stream.
//!
//! It understands the exact HTML shapes emitted by mjml 4.18.0 (and this
//! project's renderer): a leading `<!doctype html>`, void/unclosed elements,
//! raw-text elements (`<style>`/`<script>`/`<title>`/`<textarea>`), MSO
//! conditional comments, normal comments, and single/double-quoted or valueless
//! attributes. It does not build a tree — nesting is recovered structurally by
//! the comparator, which walks the two flat streams in lock-step.

/// One lexical event. Void and self-closing elements produce a single
/// [`Event::Open`] with no matching [`Event::Close`] (there is no close tag in
/// the source), which is exactly how the comparator treats them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
    /// `<!doctype html>` — inner text with the leading `!` stripped.
    DocType(String),
    /// A start tag with its raw (un-normalized) attribute list, in source order.
    Open {
        tag: String,
        attrs: Vec<(String, String)>,
    },
    /// An end tag `</tag>`.
    Close { tag: String },
    /// Character data between tags (raw, un-normalized).
    Text(String),
    /// A comment's inner content (between `<!--` and `-->`), incl. MSO conditionals.
    Comment(String),
}

/// Elements whose content is captured verbatim as a single [`Event::Text`],
/// rather than being parsed as markup.
fn is_raw_text(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "style" | "script" | "title" | "textarea"
    )
}

fn matches_at(chars: &[char], at: usize, needle: &[char]) -> bool {
    at + needle.len() <= chars.len() && chars[at..at + needle.len()] == *needle
}

/// Index of the first occurrence of `needle` in `chars` at or after `from`.
fn find_seq(chars: &[char], from: usize, needle: &[char]) -> Option<usize> {
    let mut i = from;
    while i + needle.len() <= chars.len() {
        if chars[i..i + needle.len()] == *needle {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Index of the start (`<`) of the matching close tag `</tag>` for a raw-text
/// element, matched case-insensitively.
fn find_raw_close(chars: &[char], from: usize, tag: &str) -> Option<usize> {
    let needle: Vec<char> = tag.to_ascii_lowercase().chars().collect();
    let n = chars.len();
    let mut i = from;
    while i < n {
        if chars[i] == '<' && i + 1 < n && chars[i + 1] == '/' {
            let mut k = i + 2;
            let mut m = 0;
            while m < needle.len() && k < n && chars[k].to_ascii_lowercase() == needle[m] {
                k += 1;
                m += 1;
            }
            if m == needle.len() && (k >= n || chars[k].is_whitespace() || chars[k] == '>') {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn is_name_end(c: char) -> bool {
    c.is_whitespace() || c == '/' || c == '>'
}

/// Tokenize `input` into a flat [`Event`] stream.
pub(crate) fn tokenize(input: &str) -> Vec<Event> {
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    let mut events = Vec::new();
    let mut i = 0;

    while i < n {
        if chars[i] != '<' {
            // Text run up to the next tag.
            let start = i;
            while i < n && chars[i] != '<' {
                i += 1;
            }
            events.push(Event::Text(chars[start..i].iter().collect()));
            continue;
        }

        if matches_at(&chars, i, &['<', '!', '-', '-']) {
            // Comment — ends at the first `-->`. MSO conditionals such as
            // `<!--[if mso | IE]>...<![endif]-->` are a single comment.
            let start = i + 4;
            let (content_end, next) = match find_seq(&chars, start, &['-', '-', '>']) {
                Some(e) => (e, e + 3),
                None => (n, n),
            };
            events.push(Event::Comment(chars[start..content_end].iter().collect()));
            i = next;
            continue;
        }

        if i + 1 < n && chars[i + 1] == '!' {
            // Declaration such as `<!doctype html>`.
            let end = find_seq(&chars, i, &['>']).unwrap_or(n);
            let inner: String = chars[i + 2..end.min(n)].iter().collect();
            events.push(Event::DocType(inner));
            i = if end < n { end + 1 } else { n };
            continue;
        }

        if i + 1 < n && chars[i + 1] == '/' {
            // End tag.
            let start = i + 2;
            let end = find_seq(&chars, start, &['>']).unwrap_or(n);
            let name: String = chars[start..end].iter().collect();
            events.push(Event::Close {
                tag: name.trim().to_string(),
            });
            i = if end < n { end + 1 } else { n };
            continue;
        }

        // Start tag.
        let mut j = i + 1;
        let name_start = j;
        while j < n && !is_name_end(chars[j]) {
            j += 1;
        }
        let tag: String = chars[name_start..j].iter().collect();
        let mut attrs: Vec<(String, String)> = Vec::new();
        let mut self_closing = false;

        loop {
            while j < n && chars[j].is_whitespace() {
                j += 1;
            }
            if j >= n {
                break;
            }
            if chars[j] == '>' {
                j += 1;
                break;
            }
            if chars[j] == '/' {
                self_closing = true;
                j += 1;
                continue;
            }
            // Attribute name.
            let an_start = j;
            while j < n
                && !chars[j].is_whitespace()
                && chars[j] != '='
                && chars[j] != '>'
                && chars[j] != '/'
            {
                j += 1;
            }
            let aname: String = chars[an_start..j].iter().collect();
            while j < n && chars[j].is_whitespace() {
                j += 1;
            }
            let mut aval = String::new();
            if j < n && chars[j] == '=' {
                j += 1;
                while j < n && chars[j].is_whitespace() {
                    j += 1;
                }
                if j < n && (chars[j] == '"' || chars[j] == '\'') {
                    let quote = chars[j];
                    j += 1;
                    let vs = j;
                    while j < n && chars[j] != quote {
                        j += 1;
                    }
                    aval = chars[vs..j].iter().collect();
                    if j < n {
                        j += 1; // closing quote
                    }
                } else {
                    let vs = j;
                    while j < n && !chars[j].is_whitespace() && chars[j] != '>' {
                        j += 1;
                    }
                    aval = chars[vs..j].iter().collect();
                }
            }
            if !aname.is_empty() {
                attrs.push((aname, aval));
            }
        }

        events.push(Event::Open {
            tag: tag.clone(),
            attrs,
        });
        i = j;

        // Raw-text element: swallow inner content verbatim as one Text node; the
        // close tag is then emitted by the normal loop.
        if is_raw_text(&tag) && !self_closing {
            let close = find_raw_close(&chars, i, &tag).unwrap_or(n);
            events.push(Event::Text(chars[i..close].iter().collect()));
            i = close;
        }
    }

    events
}
