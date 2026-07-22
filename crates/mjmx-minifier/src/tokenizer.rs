//! A small, verbatim-preserving HTML tokenizer for the minifier/beautifier.
//!
//! Unlike [`mjmx_html_compare`]'s tokenizer (which normalizes on the way in),
//! this one keeps the *exact source bytes* of every tag, comment, doctype, and
//! raw-text block. Only ordinary inter-tag text is left free for the serializer
//! to collapse. The token stream is a faithful, loss-free view of the input:
//! concatenating every token's source text reproduces the original document.
//!
//! It understands the HTML shapes mjmx emits: a leading `<!doctype html>`,
//! void/unclosed elements, raw-text elements (`<style>`/`<script>`/`<title>`/
//! `<textarea>`/`<pre>`), MSO conditional comments (`<!--[if mso | IE]>…<![endif]-->`
//! and downlevel-revealed `<!--[if !mso]><!-->` blocks), normal comments, and
//! single/double-quoted or valueless attributes. It builds no tree.

/// One lexical token. Every variant carries verbatim source text except
/// [`Token::Text`], whose whitespace the serializer is free to rewrite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Token {
    /// A declaration such as `<!doctype html>`, verbatim (including brackets).
    Doctype(String),
    /// A comment, verbatim including `<!--`/`-->` — covers MSO conditionals,
    /// whose internal structure is significant and must never be rewritten.
    Comment(String),
    /// A start tag, verbatim (including `<`, attributes, and `>`).
    Open {
        /// The exact source text of the tag, e.g. `<td align="left">`.
        text: String,
        /// The lower-cased tag name, e.g. `td`.
        tag: String,
        /// Whether this is a void element (`<br>`, `<img>`, `<meta>`, …).
        void: bool,
        /// Whether the tag was written self-closing (`<img … />`).
        self_closing: bool,
    },
    /// An end tag, verbatim, e.g. `</td>`.
    Close {
        /// The exact source text of the close tag.
        text: String,
        /// The lower-cased tag name.
        tag: String,
    },
    /// Ordinary character data between tags. Whitespace here is insignificant
    /// layout and may be collapsed or dropped by the serializer.
    Text(String),
    /// Verbatim inner content of a raw-text element (`<style>`, `<pre>`, …).
    /// Preserved byte-for-byte unless the serializer was explicitly asked to
    /// rewrite it (see `minify_css`).
    RawText {
        /// The exact inner source text of the element.
        text: String,
        /// The lower-cased name of the enclosing element, e.g. `style`. Kept so a
        /// serializer can treat `<style>` bodies differently from `<pre>` ones.
        tag: String,
    },
}

/// Elements whose content is captured verbatim rather than parsed as markup.
/// `pre` is included (significant inner whitespace) on top of the usual HTML
/// raw/escapable-text set.
fn is_raw_text(tag: &str) -> bool {
    matches!(tag, "style" | "script" | "title" | "textarea" | "pre")
}

/// HTML void elements: a start tag with no corresponding end tag.
fn is_void(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
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
    let needle: Vec<char> = tag.chars().collect();
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

/// Tokenize `input` into a verbatim [`Token`] stream.
pub(crate) fn tokenize(input: &str) -> Vec<Token> {
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < n {
        if chars[i] != '<' {
            // Text run up to the next tag.
            let start = i;
            while i < n && chars[i] != '<' {
                i += 1;
            }
            tokens.push(Token::Text(chars[start..i].iter().collect()));
            continue;
        }

        if matches_at(&chars, i, &['<', '!', '-', '-']) {
            // Comment — ends at the first `-->`. MSO conditionals such as
            // `<!--[if mso | IE]>…<![endif]-->` are a single comment.
            let end = match find_seq(&chars, i + 4, &['-', '-', '>']) {
                Some(e) => e + 3,
                None => n,
            };
            tokens.push(Token::Comment(chars[i..end].iter().collect()));
            i = end;
            continue;
        }

        if i + 1 < n && chars[i + 1] == '!' {
            // Declaration such as `<!doctype html>`.
            let end = find_seq(&chars, i, &['>']).map_or(n, |e| e + 1);
            tokens.push(Token::Doctype(chars[i..end].iter().collect()));
            i = end;
            continue;
        }

        if i + 1 < n && chars[i + 1] == '/' {
            // End tag.
            let end = find_seq(&chars, i + 2, &['>']).map_or(n, |e| e + 1);
            let text: String = chars[i..end].iter().collect();
            let tag = text
                .trim_start_matches("</")
                .trim_end_matches('>')
                .trim()
                .to_ascii_lowercase();
            tokens.push(Token::Close { text, tag });
            i = end;
            continue;
        }

        // Start tag.
        let tag_start = i;
        let mut j = i + 1;
        let name_start = j;
        while j < n && !is_name_end(chars[j]) {
            j += 1;
        }
        let tag: String = chars[name_start..j]
            .iter()
            .collect::<String>()
            .to_ascii_lowercase();
        let mut self_closing = false;

        // Skip over the rest of the tag, tracking quotes so a `>` inside an
        // attribute value does not terminate it early.
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
            while j < n
                && !chars[j].is_whitespace()
                && chars[j] != '='
                && chars[j] != '>'
                && chars[j] != '/'
            {
                j += 1;
            }
            while j < n && chars[j].is_whitespace() {
                j += 1;
            }
            if j < n && chars[j] == '=' {
                j += 1;
                while j < n && chars[j].is_whitespace() {
                    j += 1;
                }
                if j < n && (chars[j] == '"' || chars[j] == '\'') {
                    let quote = chars[j];
                    j += 1;
                    while j < n && chars[j] != quote {
                        j += 1;
                    }
                    if j < n {
                        j += 1; // closing quote
                    }
                } else {
                    while j < n && !chars[j].is_whitespace() && chars[j] != '>' {
                        j += 1;
                    }
                }
            }
        }

        let text: String = chars[tag_start..j].iter().collect();
        let void = is_void(&tag);
        tokens.push(Token::Open {
            text,
            tag: tag.clone(),
            void,
            self_closing,
        });
        i = j;

        // Raw-text element: swallow inner content verbatim; the close tag is then
        // emitted by the normal loop on the next iteration.
        if is_raw_text(&tag) && !self_closing {
            let close = find_raw_close(&chars, i, &tag).unwrap_or(n);
            if close > i {
                tokens.push(Token::RawText {
                    text: chars[i..close].iter().collect(),
                    tag: tag.clone(),
                });
            }
            i = close;
        }
    }

    tokens
}
