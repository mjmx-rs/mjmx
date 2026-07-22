//! A structure-aware CSS minifier, used only when
//! [`MinifyOptions::minify_css`](crate::MinifyOptions::minify_css) is on.
//!
//! html-minifier hands `minifyCSS` to `clean-css`, which *rewrites* stylesheets
//! (shortening colors, merging shorthands, reordering and deduplicating
//! declarations). This is **not** that. It parses the stylesheet with
//! [`cssparser`] — the Servo CSS Syntax Level 3 tokenizer/parser — and
//! re-serializes it, emitting only bytes that were in the source. Nothing is
//! reordered, merged, shortened or dropped except comments and whitespace that
//! CSS ignores.
//!
//! ## Why a parser
//!
//! The previous implementation was a lexical pass, and its own docs recorded the
//! limitation that motivated this one: it could not touch whitespace around `:`,
//! because `a :hover` (descendant) and `a:hover` (pseudo-class) are different
//! rules and a context-free scan cannot tell a selector from a declaration. A
//! parse gives that context, so `color: red` can collapse to `color:red` while
//! `a :hover` keeps its space.
//!
//! ## Structure
//!
//! [`minify_css`] runs a recursive rule walk:
//!
//! - the stylesheet is a rule list ([`StyleSheetParser`]);
//! - a qualified rule is `<selector-prelude> { <body> }`;
//! - an at-rule is `@name <prelude> ;` or `@name <prelude> { <body> }`;
//! - a body is a mixed declaration/rule list ([`RuleBodyParser`]), so CSS
//!   nesting and at-rules nested inside `@media`/`@supports`/`@layer` recurse
//!   through the same code;
//! - a declaration is `name : value`, `!important` included in the value.
//!
//! Every leaf — selector prelude, at-rule prelude, declaration name, declaration
//! value — is a **source slice**, never a token re-serialization. That slice is
//! then whitespace-collapsed by [`minify_fragment`], which walks it token by
//! token (recursing into `(`/`[`/`{` blocks) and copies each token's original
//! bytes. Strings, `url()` tokens, escapes, numbers and unit spellings therefore
//! survive byte-for-byte; only whitespace runs and comments between tokens are
//! candidates for removal.
//!
//! ## What is *not* minified, and why
//!
//! - **Custom properties** (`--x: …`) keep their value verbatim. Their grammar
//!   is `<declaration-value>`, which is deliberately permissive, and the value is
//!   observable through `var()` substitution; an empty value is re-emitted as a
//!   single space rather than nothing.
//! - **`:` in at-rule preludes** is only collapsed for `@media`, whose grammar
//!   cannot contain a selector. `@supports selector(.a :hover)` and `@scope (.a)
//!   to (.b :hover)` can, so their preludes keep every space around `:`.
//! - **Whitespace before `(`** is never removed anywhere: `and (min-width: …)`
//!   would retokenize as the function `and(`.
//! - **Whitespace around `+`, `-`, `*`, `/`** is never removed: it is significant
//!   in `calc()` and `*` is the universal selector.
//! - **Unquoted `url(…)`** is one token and is copied whole, interior whitespace
//!   included.
//! - **Comments** are dropped, but a comment between two tokens is replaced by a
//!   whitespace opportunity, so `a/**/b` never becomes `ab`.
//! - **Unparseable input** is re-emitted as its raw source slice: `cssparser`
//!   hands the offending text back with the error and this pass copies it
//!   through, so an IE hack like `*width:1px` or a truncated rule survives.
//!
//! Two things *are* dropped besides comments and whitespace, both of which CSS
//! itself ignores: the legacy `<!--`/`-->` (CDO/CDC) wrappers around a
//! stylesheet, which the CSS grammar skips at the top level, and the `;` that
//! ends the last declaration of a block.

use cssparser::{
    AtRuleParser, DeclarationParser, ParseError, Parser, ParserInput, ParserState,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, StyleSheetParser, Token,
    serialize_identifier,
};

use crate::CssDialect;

/// Minify a `<style>` body under the rules in the [module docs](self).
pub(crate) fn minify_css(css: &str, dialect: CssDialect) -> String {
    // `StyleSheetParser` swallows a leading `@charset` rule instead of yielding it,
    // so it is split off and re-emitted verbatim rather than silently lost.
    let (charset, rest) = split_charset(css);

    let mut input = ParserInput::new(rest);
    let mut parser = Parser::new(&mut input);
    let mut rules = RuleParser { dialect };

    let mut pieces = Vec::new();
    for result in StyleSheetParser::new(&mut parser, &mut rules) {
        match result {
            Ok(piece) => pieces.push(piece),
            // Top-level garbage: emit the source slice untouched.
            Err((_, raw)) => pieces.push(Piece::Rule(raw.trim().to_owned())),
        }
    }

    let mut out = String::with_capacity(css.len());
    out.push_str(charset);
    join(&mut out, pieces);
    out
}

/// One serialized item of a rule list.
enum Piece {
    /// A declaration (`name:value`), which needs a `;` before whatever follows it.
    Decl(String),
    /// A complete rule (`sel{…}`, `@name …;`, `@name …{…}`) — self-delimiting.
    Rule(String),
}

/// Concatenate `pieces`, inserting the `;` separators declarations need. No
/// trailing `;` is emitted: the closing `}` (or end of stylesheet) ends the last
/// declaration on its own.
fn join(out: &mut String, pieces: Vec<Piece>) {
    let mut pending_semicolon = false;
    for piece in pieces {
        let (text, is_decl) = match piece {
            Piece::Decl(text) => (text, true),
            Piece::Rule(text) => (text, false),
        };
        if text.is_empty() {
            continue;
        }
        if pending_semicolon && !out.ends_with(';') {
            out.push(';');
        }
        out.push_str(&text);
        pending_semicolon = is_decl;
    }
}

/// Split a leading `@charset "…";` off `css`, returning it and the remainder.
///
/// The at-rule is only recognized at the very start of the stylesheet, which is
/// the only place it is meaningful, and the scan skips over quoted strings so a
/// `;` inside the encoding name does not end it early.
fn split_charset(css: &str) -> (&str, &str) {
    let trimmed = css.trim_start();
    let offset = css.len() - trimmed.len();
    if trimmed.len() < 8 || !trimmed[..8].eq_ignore_ascii_case("@charset") {
        return ("", css);
    }
    let bytes = trimmed.as_bytes();
    let mut i = 8;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(_) if b == b'\\' => i += 1,
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None if b == b'"' || b == b'\'' => quote = Some(b),
            None if b == b';' => {
                let end = offset + i + 1;
                return (&css[offset..end], &css[end..]);
            }
            None => {}
        }
        i += 1;
    }
    ("", css)
}

// --- rule walk ---------------------------------------------------------------

/// The one parser used at every level: stylesheet, at-rule body, and rule body.
struct RuleParser {
    /// Which upstream minifier's output shape to reproduce.
    dialect: CssDialect,
}

/// A parsed at-rule prelude: the serialized `@name` plus its minified prelude.
struct AtPrelude {
    /// `@` followed by the (re-escaped) at-keyword.
    name: String,
    /// Everything between the at-keyword and the `;` or `{`, minified.
    prelude: String,
}

impl AtPrelude {
    /// `@name` joined to its prelude by a single space.
    ///
    /// The space is kept even where the tokenizer does not need it (`@media(…)`
    /// retokenizes fine): removing it before a `(` is the one collapse this pass
    /// refuses everywhere else, and one byte is not worth a special case.
    fn head(&self) -> String {
        let mut out = self.name.clone();
        if !self.prelude.is_empty() {
            out.push(' ');
            out.push_str(&self.prelude);
        }
        out
    }
}

impl<'i> AtRuleParser<'i> for RuleParser {
    type Prelude = AtPrelude;
    type AtRule = Piece;
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        name: cssparser::CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        let raw = drain(input);
        // `@media`'s prelude grammar has no selector production, so a `:` there is
        // always a media-feature separator and its spaces are insignificant. Every
        // other at-rule (`@supports selector(…)`, `@scope (…) to (…)`) may carry a
        // selector, where a leading space before `:` is a descendant combinator.
        let colon = name.eq_ignore_ascii_case("media");
        let mut at_name = String::with_capacity(name.len() + 1);
        at_name.push('@');
        // Re-escaped rather than copied, so a `@\6d edia` spelling stays valid.
        let _ = serialize_identifier(&name, &mut at_name);
        Ok(AtPrelude {
            name: at_name,
            prelude: minify_fragment(raw, Ctx::Prelude { colon }),
        })
    }

    fn rule_without_block(
        &mut self,
        prelude: Self::Prelude,
        _start: &ParserState,
    ) -> Result<Self::AtRule, ()> {
        Ok(Piece::Rule(format!("{};", prelude.head())))
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::AtRule, ParseError<'i, Self::Error>> {
        Ok(Piece::Rule(format!(
            "{}{{{}}}",
            prelude.head(),
            parse_body(input, self.dialect)
        )))
    }
}

impl<'i> QualifiedRuleParser<'i> for RuleParser {
    type Prelude = String;
    type QualifiedRule = Piece;
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        let prelude = drain(input);
        Ok(match self.dialect {
            // cssnano does not touch selector preludes: `.a, .b` keeps its space and
            // `.c   >   .d` keeps all three. Only the surrounding whitespace goes, and
            // that is the caller's `trim`.
            CssDialect::Cssnano => prelude.trim().to_owned(),
            CssDialect::CleanCss => minify_fragment(prelude, Ctx::Selector),
        })
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
        Ok(Piece::Rule(format!(
            "{prelude}{{{}}}",
            parse_body(input, self.dialect)
        )))
    }
}

impl<'i> DeclarationParser<'i> for RuleParser {
    type Declaration = Piece;
    type Error = ();

    fn parse_value<'t>(
        &mut self,
        name: cssparser::CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        start: &ParserState,
    ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
        // On entry the `:` has just been consumed, so the source from the start of
        // the declaration to here is `name`, any comments/whitespace, and the colon.
        let head = input.slice(start.position()..input.position()).trim_end();
        let head = head.strip_suffix(':').unwrap_or(head);
        // Run the name through the fragment pass too: it drops a `p/*x*/:v` comment
        // while copying the ident's own bytes (escapes included) verbatim.
        let property = minify_fragment(head, Ctx::Value);

        let raw = drain(input);
        let value = if name.starts_with("--") {
            // `<declaration-value>` is deliberately permissive and the text is
            // observable through `var()`, so it is kept exactly. An empty value is
            // legal but `--x:` is not universally accepted; emit `--x: `.
            if raw.is_empty() { " " } else { raw }.to_owned()
        } else {
            let value = minify_fragment(raw, Ctx::Value);
            match self.dialect {
                // cssnano closes the gap `!important` is written with; clean-css keeps
                // it. The keyword itself is never rewritten under either.
                CssDialect::Cssnano => value.replace(" !important", "!important"),
                CssDialect::CleanCss => value,
            }
        };
        Ok(Piece::Decl(format!("{property}:{value}")))
    }
}

impl<'i> RuleBodyItemParser<'i, Piece, ()> for RuleParser {
    fn parse_declarations(&self) -> bool {
        true
    }

    // Kept on so CSS nesting, `@media` bodies and `@keyframes` selectors all reach
    // the qualified-rule path; cssparser falls back to it when a declaration parse
    // fails, so an ident-led selector like `div {…}` is still handled.
    fn parse_qualified(&self) -> bool {
        true
    }
}

/// Minify a bare declaration list — the contents of a `style="…"` attribute.
///
/// Both upstreams run their CSS minifier over style attributes as well as `<style>`
/// bodies, which is mostly visible as the trailing `;` disappearing.
pub(crate) fn minify_declarations(css: &str, dialect: CssDialect) -> String {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    parse_body(&mut parser, dialect)
}

/// Serialize the `{ … }` body of a rule: a mixed list of declarations and nested
/// rules, with unparseable items copied through as raw source.
fn parse_body(input: &mut Parser<'_, '_>, dialect: CssDialect) -> String {
    let mut rules = RuleParser { dialect };
    let mut pieces = Vec::new();
    for result in RuleBodyParser::<_, Piece, ()>::new(input, &mut rules) {
        match result {
            Ok(piece) => pieces.push(piece),
            // Treated as a declaration so it keeps a `;` separator from its
            // neighbours; a redundant `;` is an ignored empty declaration.
            Err((_, raw)) => pieces.push(Piece::Decl(raw.trim().to_owned())),
        }
    }
    let mut out = String::new();
    join(&mut out, pieces);
    out
}

/// Consume everything the delimited `input` covers and return its source text.
///
/// `Parser::next` skips over any `{}`/`[]`/`()` block it does not descend into,
/// so this reaches the delimiter without losing the bytes in between.
fn drain<'i>(input: &mut Parser<'i, '_>) -> &'i str {
    let start = input.position();
    while input.next().is_ok() {}
    input.slice_from(start).trim()
}

// --- fragment (prelude / value) whitespace pass ------------------------------

/// Which grammar a fragment belongs to, which is what decides whether the
/// whitespace around a `:` is significant.
#[derive(Clone, Copy)]
enum Ctx {
    /// A qualified rule's selector list, where ` :hover` is a descendant
    /// combinator plus a pseudo-class and the space must stay.
    Selector,
    /// An at-rule prelude. `colon` is set only for at-rules whose grammar cannot
    /// contain a selector (see [`AtRuleParser::parse_prelude`]).
    Prelude { colon: bool },
    /// A declaration value (or property name), where every `:` belongs to the
    /// value — `progid:DXImageTransform…`, `url(https://…)` — and is left alone.
    Value,
    /// The inside of a selector's `[ … ]`, where `=` and the match-prefix delims
    /// (`^ $ * ~ |`) glue to their neighbours. Entered only from
    /// [`Ctx::Selector`], so a `[line-name]` inside a value stays a value.
    Attribute,
}

/// Collapse the insignificant whitespace and comments in one fragment, copying
/// every token's original bytes.
fn minify_fragment(src: &str, ctx: Ctx) -> String {
    let mut out = String::with_capacity(src.len());
    if src.is_empty() {
        return out;
    }
    let mut input = ParserInput::new(src);
    let mut parser = Parser::new(&mut input);
    walk(&mut parser, ctx, &mut out);
    out
}

/// Token walk behind [`minify_fragment`], recursing through blocks.
///
/// Blocks must be descended into with `parse_nested_block`: a `(`/`[`/`{` token
/// covers only its own delimiter, and skipping it would drop the entire block
/// body from the output.
fn walk(parser: &mut Parser<'_, '_>, ctx: Ctx, out: &mut String) {
    // Whether a whitespace/comment run has been seen since the last real token.
    let mut space = false;
    loop {
        let start = parser.position();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            // End of the fragment (or of this block): a trailing run is dropped.
            break;
        };
        let token = token.clone();
        let end = parser.position();
        let close = match token {
            Token::WhiteSpace(_) | Token::Comment(_) => {
                space = true;
                continue;
            }
            Token::Function(_) | Token::ParenthesisBlock => Some((')', ctx)),
            Token::SquareBracketBlock => Some((']', attribute_ctx(ctx))),
            Token::CurlyBracketBlock => Some(('}', ctx)),
            _ => None,
        };

        // For a block this is just the opening delimiter (`(`, `[`, `{`, `name(`);
        // for anything else it is the whole token, source bytes untouched.
        emit(out, parser.slice(start..end), space, ctx);
        space = false;

        if let Some((close, inner_ctx)) = close {
            let _ = parser.parse_nested_block(|inner| -> Result<(), ParseError<'_, ()>> {
                walk(inner, inner_ctx, out);
                Ok(())
            });
            out.push(close);
        }
    }
}

/// The context inside a `[ … ]` opened in `ctx`.
fn attribute_ctx(ctx: Ctx) -> Ctx {
    match ctx {
        Ctx::Selector => Ctx::Attribute,
        other => other,
    }
}

/// Append `text`, preceded by a single space if one was seen before it and the
/// token boundary does not already separate the two.
fn emit(out: &mut String, text: &str, space: bool, ctx: Ctx) {
    if space
        && let Some(prev) = out.chars().last()
        && let Some(next) = text.chars().next()
        && !drops_space(prev, next, ctx)
    {
        out.push(' ');
    }
    out.push_str(text);
}

/// Whether the whitespace between a token ending in `prev` and one starting with
/// `next` can be removed without changing how the two retokenize or parse.
///
/// Everything not listed keeps its space. In particular `+`, `-`, `*` and `/`
/// never lose one (`calc(1px + 2px)`, the universal selector), and nothing drops
/// a space *before* `(`, which would turn an ident into a function token.
fn drops_space(prev: char, next: char, ctx: Ctx) -> bool {
    // Separators that already delimit their neighbours in every CSS grammar.
    if prev == ',' || next == ',' || prev == ';' {
        return true;
    }
    if prev == '(' || next == ')' || prev == '[' || next == ']' {
        return true;
    }
    if prev == '{' || next == '{' || prev == '}' || next == '}' {
        return true;
    }
    match ctx {
        // Combinators. `~` covers both `a ~ b` and `[href ~= "x"]`; `+` is left
        // alone for the `calc()` reason above even though selectors are safe.
        Ctx::Selector => matches!(prev, '>' | '~') || matches!(next, '>' | '~'),
        Ctx::Prelude { colon } => colon && (prev == ':' || next == ':'),
        Ctx::Value => false,
        // The `i`/`s` case-sensitivity flag is an ident and keeps its space.
        Ctx::Attribute => {
            matches!(prev, '=' | '^' | '$' | '*' | '~' | '|')
                || matches!(next, '=' | '^' | '$' | '*' | '~' | '|')
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CssDialect;

    /// These cases predate the dialect split and describe clean-css's shape, which is
    /// the default; [`cssnano_dialect`] covers where the other one differs.
    fn minify_css(css: &str) -> String {
        super::minify_css(css, CssDialect::CleanCss)
    }

    #[test]
    fn collapses_whitespace_and_punctuation() {
        let input = "\n  .a {\n    width: 100%   !important;\n    max-width: 100%;\n  }\n";
        assert_eq!(
            minify_css(input),
            ".a{width:100% !important;max-width:100%}"
        );
    }

    #[test]
    fn drops_comments() {
        assert_eq!(minify_css("/* hi */ .a { color: red }"), ".a{color:red}");
    }

    #[test]
    fn a_comment_between_tokens_leaves_a_separator() {
        // Dropping the comment outright would fuse two idents into one.
        assert_eq!(minify_css(".a { margin: 0/**/auto }"), ".a{margin:0 auto}");
        assert_eq!(minify_css(".a /**/ .b { color: red }"), ".a .b{color:red}");
        assert_eq!(minify_css(".a { color/*x*/: red }"), ".a{color:red}");
    }

    // --- the point of the parse: `:` is context-sensitive ---------------------

    #[test]
    fn collapses_colon_in_declarations() {
        assert_eq!(minify_css(".a { color : red }"), ".a{color:red}");
    }

    #[test]
    fn keeps_descendant_space_before_pseudo_class() {
        // `.a :hover` is a descendant selector; collapsing the space would change it.
        assert_eq!(
            minify_css(".a :hover { color: red }"),
            ".a :hover{color:red}"
        );
        assert_eq!(minify_css(".a:hover { color: red }"), ".a:hover{color:red}");
        // …including after a functional pseudo-class.
        assert_eq!(
            minify_css(":not(.a) :hover { color: red }"),
            ":not(.a) :hover{color:red}"
        );
    }

    #[test]
    fn keeps_descendant_space_before_pseudo_element() {
        assert_eq!(
            minify_css("div ::before { color: red }"),
            "div ::before{color:red}"
        );
    }

    #[test]
    fn collapses_around_combinators_but_not_the_universal_selector() {
        assert_eq!(minify_css("a > b ~ c { color: red }"), "a>b~c{color:red}");
        // `a * b` and `a*b` are not the same selector.
        assert_eq!(minify_css("a * b { color: red }"), "a * b{color:red}");
    }

    #[test]
    fn keeps_colon_untouched_inside_a_value() {
        assert_eq!(
            minify_css(".a { filter: progid:DXImageTransform.Microsoft.gradient(x) }"),
            ".a{filter:progid:DXImageTransform.Microsoft.gradient(x)}"
        );
    }

    // --- preserved constructs -------------------------------------------------

    #[test]
    fn preserves_string_contents() {
        assert_eq!(
            minify_css(".a::before { content: \"x   y\" }"),
            ".a::before{content:\"x   y\"}"
        );
    }

    #[test]
    fn preserves_braces_and_semicolons_inside_strings() {
        let input = ".a::before { content: \"} ; {\" }";
        assert_eq!(minify_css(input), ".a::before{content:\"} ; {\"}");
    }

    #[test]
    fn preserves_urls_quoted_and_bare() {
        assert_eq!(
            minify_css(".a { background: url( \"a  b.png\" ) }"),
            ".a{background:url(\"a  b.png\")}"
        );
        // An unquoted url() is a single token and is copied whole, spaces included.
        assert_eq!(
            minify_css(".a { background: url( https://x/y.png ) }"),
            ".a{background:url( https://x/y.png )}"
        );
        assert_eq!(
            minify_css(".a { background: url(https://x/y.png) no-repeat }"),
            ".a{background:url(https://x/y.png) no-repeat}"
        );
    }

    #[test]
    fn preserves_important() {
        assert_eq!(
            minify_css(".a { color: red   !important }"),
            ".a{color:red !important}"
        );
    }

    #[test]
    fn preserves_custom_property_values_verbatim() {
        assert_eq!(
            minify_css(":root { --gap :  10px    20px ; color: red }"),
            ":root{--gap:10px    20px;color:red}"
        );
        // Permissive grammar: this is not a declaration list, just tokens.
        assert_eq!(
            minify_css(":root { --x: { a : b ; } }"),
            ":root{--x:{ a : b ; }}"
        );
        // An empty custom-property value keeps a space rather than becoming `--x:`.
        assert_eq!(minify_css(":root { --x: ; }"), ":root{--x: }");
        assert_eq!(
            minify_css(".a { width: var( --gap ) }"),
            ".a{width:var(--gap)}"
        );
    }

    #[test]
    fn keeps_calc_operator_spacing() {
        assert_eq!(
            minify_css(".a { width: calc( 100% - 10px ) }"),
            ".a{width:calc(100% - 10px)}"
        );
    }

    #[test]
    fn collapses_commas_in_selectors_and_values() {
        assert_eq!(
            minify_css(
                ".a , .b { font-family: Arial , sans-serif ; color: rgba( 0 , 0 , 0 , .5 ) }"
            ),
            ".a,.b{font-family:Arial,sans-serif;color:rgba(0,0,0,.5)}"
        );
    }

    #[test]
    fn collapses_attribute_selectors() {
        assert_eq!(
            minify_css("a[ href ^= \"x\" ] { color: red }"),
            "a[href^=\"x\"]{color:red}"
        );
    }

    // --- at-rules -------------------------------------------------------------

    #[test]
    fn handles_at_rules_with_and_without_blocks() {
        let input = "@media only screen and (min-width:480px) {\n  .b {\n    width: 50%;\n  }\n}";
        assert_eq!(
            minify_css(input),
            "@media only screen and (min-width:480px){.b{width:50%}}"
        );
        assert_eq!(minify_css("@import  url(a.css) ;"), "@import url(a.css);");
        assert_eq!(
            minify_css("@charset \"utf-8\";\n.a { color: red }"),
            "@charset \"utf-8\";.a{color:red}"
        );
    }

    #[test]
    fn collapses_media_feature_colons_only() {
        assert_eq!(
            minify_css("@media (min-width : 480px) { .a { color: red } }"),
            "@media (min-width:480px){.a{color:red}}"
        );
        // `@supports selector(…)` can hold a selector, so its `:` spacing stays.
        assert_eq!(
            minify_css("@supports selector(.a :hover) { .b { color: red } }"),
            "@supports selector(.a :hover){.b{color:red}}"
        );
    }

    #[test]
    fn keeps_the_space_before_a_prelude_parenthesis() {
        // `and(` would retokenize as a function.
        let out = minify_css("@media screen and ( max-width : 1px ) { .a { color: red } }");
        assert_eq!(out, "@media screen and (max-width:1px){.a{color:red}}");
    }

    #[test]
    fn handles_nested_at_rules() {
        let input =
            "@media screen {\n  @supports (display: grid) {\n    .a { color: red; }\n  }\n}";
        assert_eq!(
            minify_css(input),
            // `@supports` is not on the colon allow-list: its prelude may hold a
            // selector, so the space after `display:` stays.
            "@media screen{@supports (display: grid){.a{color:red}}}"
        );
    }

    #[test]
    fn handles_font_face_and_keyframes_bodies() {
        assert_eq!(
            minify_css("@font-face { font-family: \"X\" ; src: url(x.woff2) }"),
            "@font-face{font-family:\"X\";src:url(x.woff2)}"
        );
        // `from`/`to` are idents but are keyframe selectors, not declarations.
        assert_eq!(
            minify_css("@keyframes spin { from { opacity: 0 } 50% , to { opacity: 1 } }"),
            "@keyframes spin{from{opacity:0}50%,to{opacity:1}}"
        );
    }

    #[test]
    fn handles_css_nesting() {
        assert_eq!(
            minify_css(".a { color: red; & :hover { color: blue } }"),
            ".a{color:red;& :hover{color:blue}}"
        );
    }

    // --- malformed input ------------------------------------------------------

    #[test]
    fn passes_through_unparseable_declarations() {
        // The IE star hack is not a valid declaration; it must survive anyway.
        let out = minify_css(".a { *width: 1px; color: red }");
        assert!(out.contains("*width: 1px"), "{out}");
        assert!(out.contains("color:red"), "{out}");
    }

    #[test]
    fn closes_an_unterminated_block() {
        assert_eq!(minify_css(".a { color: red"), ".a{color:red}");
        assert_eq!(
            minify_css("@media screen { .a { color: red"),
            "@media screen{.a{color:red}}"
        );
    }

    #[test]
    fn drops_the_legacy_comment_wrapper() {
        // `<!--`/`-->` around a stylesheet are CDO/CDC tokens the CSS grammar
        // skips at the top level; they carry no style information.
        assert_eq!(minify_css("<!--\n.a { color: red }\n-->"), ".a{color:red}");
    }

    #[test]
    fn is_idempotent() {
        for input in [
            ".a :hover { color : red ; }",
            "@media only screen and (min-width : 480px) { .b { width: 50% } }",
            ":root { --x:  1px  2px ; }",
            ".a { background: url( https://x/y.png ) }",
            ".a { *width: 1px; color: red }",
        ] {
            let once = minify_css(input);
            assert_eq!(minify_css(&once), once, "{input:?}");
        }
    }

    #[test]
    fn tolerates_stray_delimiters() {
        assert_eq!(minify_css("}"), "}");
        assert_eq!(minify_css(""), "");
        assert_eq!(minify_css("   \n  "), "");
        assert_eq!(minify_css("/* only a comment */"), "");
    }

    #[test]
    fn never_invents_bytes_outside_the_source() {
        // Whatever the input, the output may only add structural `{};` characters.
        for input in [
            "@media{",
            ".a{;;;}",
            "a{b:c;;d:e}",
            "@unknown foo;",
            "@unknown foo { bar }",
            ".a { color: red } garbage",
            "'unterminated string",
            "url(unterminated",
        ] {
            let out = minify_css(input);
            for c in out.chars().filter(|c| !"{};".contains(*c)) {
                assert!(input.contains(c), "{input:?} -> {out:?} invented {c:?}");
            }
        }
    }

    /// cssnano's shape, as mjml 5.4.0 emits it.
    ///
    /// Every expectation here was read off `mjml@5.4.0`'s own minified output for the
    /// same stylesheet, not derived from cssnano's source.
    #[test]
    fn cssnano_dialect() {
        let nano = |css: &str| super::minify_css(css, CssDialect::Cssnano);

        // Selector preludes are left byte-for-byte: the comma keeps its space and
        // `.c   >   .d` keeps all three.
        assert_eq!(nano(".a, .b { color: red; }"), ".a, .b{color:red}");
        assert_eq!(nano(".c   >   .d { margin: 0 ; }"), ".c   >   .d{margin:0}");
        // ... where clean-css collapses both.
        assert_eq!(minify_css(".a, .b { color: red; }"), ".a,.b{color:red}");
        assert_eq!(minify_css(".c   >   .d { margin: 0 ; }"), ".c>.d{margin:0}");

        // The space before `!important` closes.
        assert_eq!(
            nano(".a { width: 100%   !important; }"),
            ".a{width:100%!important}"
        );
        assert_eq!(
            minify_css(".a { width: 100%   !important; }"),
            ".a{width:100% !important}"
        );

        // Commas *inside a value* lose their space under both.
        assert_eq!(
            nano(".a { font-family: Helvetica, Arial, sans-serif; }"),
            ".a{font-family:Helvetica,Arial,sans-serif}"
        );

        // Values are still never rewritten: `0px` does not become `0`.
        assert_eq!(nano(".a { margin: 0px auto; }"), ".a{margin:0px auto}");
    }
}
