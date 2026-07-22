//! A conservative CSS whitespace/comment minifier, used only when
//! [`MinifyOptions::minify_css`](crate::MinifyOptions::minify_css) is on.
//!
//! html-minifier hands `minifyCSS` to `clean-css`, which parses the stylesheet and
//! rewrites values (shortening colors, merging shorthands, reordering declarations).
//! This is **not** that: it is a lexical pass that only removes bytes CSS ignores.
//!
//! Rules:
//! - `/* … */` comments are dropped.
//! - Whitespace runs collapse to one space.
//! - Whitespace is trimmed on both sides of `{`, `}` and `;`, and after `,`.
//! - A `;` immediately before `}` is dropped.
//! - Quoted strings (`'…'`, `"…"`) are copied byte-for-byte, so
//!   `content: "a   b"` survives intact.
//!
//! Whitespace around `:` is **kept**. In a declaration it is insignificant, but in a
//! selector `a :hover` (descendant) and `a:hover` (pseudo-class) are different rules,
//! and this pass does not know which context it is in. Keeping the space is always
//! correct; removing it is not.

/// Minify a `<style>` body under the rules in the [module docs](self).
pub(crate) fn minify_css(css: &str) -> String {
    let chars: Vec<char> = css.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(css.len());
    let mut i = 0;

    while i < n {
        let c = chars[i];

        // Comment: skip to the closing `*/` (or to the end, if unterminated).
        if c == '/' && i + 1 < n && chars[i + 1] == '*' {
            let mut j = i + 2;
            while j + 1 < n && !(chars[j] == '*' && chars[j + 1] == '/') {
                j += 1;
            }
            i = if j + 1 < n { j + 2 } else { n };
            continue;
        }

        // String: copied verbatim, quotes included.
        if c == '"' || c == '\'' {
            out.push(c);
            let mut j = i + 1;
            while j < n {
                out.push(chars[j]);
                if chars[j] == '\\' && j + 1 < n {
                    j += 1;
                    out.push(chars[j]);
                } else if chars[j] == c {
                    break;
                }
                j += 1;
            }
            i = j + 1;
            continue;
        }

        if c.is_whitespace() {
            // Collapse the run, then decide whether it survives at all: a space is
            // dropped next to a punctuation character that already separates tokens.
            let mut j = i;
            while j < n && chars[j].is_whitespace() {
                j += 1;
            }
            let next = chars.get(j).copied();
            let drop = matches!(next, Some('{' | '}' | ';') | None)
                || matches!(out.chars().last(), Some('{' | '}' | ';' | ',') | None);
            if !drop {
                out.push(' ');
            }
            i = j;
            continue;
        }

        if c == '}' {
            // The final `;` of a block is redundant.
            while out.ends_with(';') {
                out.pop();
            }
        }

        out.push(c);
        i += 1;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::minify_css;

    #[test]
    fn collapses_whitespace_and_punctuation() {
        let input = "\n  .a {\n    width: 100%   !important;\n    max-width: 100%;\n  }\n";
        assert_eq!(
            minify_css(input),
            ".a{width: 100% !important;max-width: 100%}"
        );
    }

    #[test]
    fn drops_comments() {
        assert_eq!(minify_css("/* hi */ .a { color: red }"), ".a{color: red}");
    }

    #[test]
    fn keeps_selector_whitespace_around_colon() {
        // `.a :hover` is a descendant selector; collapsing the space would change it.
        assert_eq!(
            minify_css(".a :hover { color: red }"),
            ".a :hover{color: red}"
        );
    }

    #[test]
    fn preserves_string_contents() {
        assert_eq!(
            minify_css(".a::before { content: \"x   y\" }"),
            ".a::before{content: \"x   y\"}"
        );
    }

    #[test]
    fn handles_nested_at_rules() {
        let input = "@media only screen and (min-width:480px) {\n  .b {\n    width: 50%;\n  }\n}";
        assert_eq!(
            minify_css(input),
            "@media only screen and (min-width:480px){.b{width: 50%}}"
        );
    }
}
