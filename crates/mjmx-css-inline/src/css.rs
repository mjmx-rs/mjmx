//! Stylesheet parsing: CSS text into the flat list of inlinable rules.
//!
//! [`cssparser`] — already a workspace dependency for the minifier's `minifyCSS`
//! pass — does the tokenizing and the rule/declaration framing; this module only
//! decides what survives.
//!
//! **At-rules do not survive.** `@media`, `@font-face`, `@keyframes` and
//! everything nested inside them are dropped whole. An inlined declaration has
//! no way to carry a condition — there is no `style` attribute that applies only
//! below 480px — so mjml's pass discards them rather than pretending, and they
//! are not re-emitted as a `<style>` tag either (that bucket is the *other*
//! `<mj-style>`). Nested (CSS Nesting) qualified rules are likewise dropped.
//!
//! Declaration values are kept as their **source slice**, trimmed: colors, units
//! and `url()` spellings all reach the `style` attribute exactly as written.

use cssparser::{
    AtRuleParser, DeclarationParser, ParseError, Parser, ParserInput, ParserState,
    QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, StyleSheetParser,
};

use crate::selector::{PseudoElement, Selector, Specificity};

/// One declaration of a rule.
#[derive(Debug, Clone)]
pub(crate) struct Declaration {
    /// The lower-cased property name. CSS property names are ASCII
    /// case-insensitive, and every `style` attribute mjmx renders is lower-case,
    /// so folding here is what makes the two comparable.
    pub(crate) property: String,
    /// The value, verbatim, with any `!important` stripped off.
    pub(crate) value: String,
    /// Whether the declaration was marked `!important`.
    pub(crate) important: bool,
}

/// One qualified rule, flattened to a single selector: a rule whose prelude was
/// a selector list becomes one [`Rule`] per selector, all sharing the same
/// declarations, exactly as the cascade treats them.
#[derive(Debug)]
pub(crate) struct Rule {
    pub(crate) selector: Selector,
    pub(crate) declarations: Vec<Declaration>,
    /// The pseudo-element this rule targets (`::before`/`::after`), if any. A pseudo
    /// rule never inlines onto a real element — it feeds the `pseudo` pass, and only
    /// when `inlinePseudoElements` is on. `None` is an ordinary rule.
    pub(crate) pseudo: Option<PseudoElement>,
}

impl Rule {
    pub(crate) fn specificity(&self) -> Specificity {
        self.selector.specificity
    }
}

/// Parse `css` into the rules that can be inlined, in source order.
pub(crate) fn parse_stylesheet(css: &str) -> Vec<Rule> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let mut rules = RuleParser;

    // A rule cssparser could not frame is dropped: with no selector and no
    // declarations there is nothing to inline, and unlike the minifier this pass
    // has no obligation to reproduce the source text.
    StyleSheetParser::new(&mut parser, &mut rules)
        .flatten()
        .flatten()
        .collect()
}

/// The stylesheet-level parser: qualified rules become [`Rule`]s, at-rules are
/// consumed and discarded.
struct RuleParser;

impl<'i> AtRuleParser<'i> for RuleParser {
    type Prelude = ();
    type AtRule = Vec<Rule>;
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        _name: cssparser::CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        drain(input);
        Ok(())
    }

    fn rule_without_block(
        &mut self,
        (): Self::Prelude,
        _start: &ParserState,
    ) -> Result<Vec<Rule>, ()> {
        Ok(Vec::new())
    }

    fn parse_block<'t>(
        &mut self,
        (): Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::AtRule, ParseError<'i, Self::Error>> {
        drain(input);
        Ok(Vec::new())
    }
}

impl<'i> QualifiedRuleParser<'i> for RuleParser {
    type Prelude = Vec<(Selector, Option<PseudoElement>)>;
    type QualifiedRule = Vec<Rule>;
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        // `Selector::parse` returning `None` drops that one selector of the list
        // and keeps its siblings, which is how `a, a:hover { … }` inlines for `a`.
        // A trailing `::before`/`::after` is split off first and carried as `pseudo`.
        Ok(crate::selector::split_list(drain(input))
            .into_iter()
            .filter_map(|sel| {
                let (base, pseudo) = crate::selector::strip_pseudo_element(sel);
                Selector::parse(base).map(|selector| (selector, pseudo))
            })
            .collect())
    }

    fn parse_block<'t>(
        &mut self,
        prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
        let declarations = parse_declarations(input);
        if declarations.is_empty() {
            return Ok(Vec::new());
        }
        Ok(prelude
            .into_iter()
            .map(|(selector, pseudo)| Rule {
                selector,
                declarations: declarations.clone(),
                pseudo,
            })
            .collect())
    }
}

/// The rule-body parser. Only declarations are wanted here, so the at-rule and
/// qualified-rule hooks keep [`cssparser`]'s default (erroring) implementations
/// and their items are dropped: an at-rule nested in a declaration block is
/// meaningless, and a nested qualified rule is CSS Nesting, whose relative
/// selectors this pass does not resolve.
struct BodyParser;

impl<'i> AtRuleParser<'i> for BodyParser {
    type Prelude = ();
    type AtRule = Declaration;
    type Error = ();
}

impl<'i> QualifiedRuleParser<'i> for BodyParser {
    type Prelude = ();
    type QualifiedRule = Declaration;
    type Error = ();
}

impl<'i> DeclarationParser<'i> for BodyParser {
    type Declaration = Declaration;
    type Error = ();

    fn parse_value<'t>(
        &mut self,
        name: cssparser::CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _start: &ParserState,
    ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
        let (value, important) = split_important(drain(input));
        Ok(Declaration {
            property: name.to_ascii_lowercase(),
            value: value.to_owned(),
            important,
        })
    }
}

impl<'i> RuleBodyItemParser<'i, Declaration, ()> for BodyParser {
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        false
    }
}

/// Collect the declarations of a `{ … }` body, dropping anything unparseable.
fn parse_declarations(input: &mut Parser<'_, '_>) -> Vec<Declaration> {
    let mut body = BodyParser;
    RuleBodyParser::<_, Declaration, ()>::new(input, &mut body)
        .filter_map(Result::ok)
        .collect()
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

/// Split a trailing `!important` off a declaration value.
fn split_important(value: &str) -> (&str, bool) {
    const IMPORTANT: &str = "important";
    let head = value.trim_end();
    let Some(rest) = head
        .len()
        .checked_sub(IMPORTANT.len())
        .and_then(|at| head.get(at..))
        .filter(|tail| tail.eq_ignore_ascii_case(IMPORTANT))
        .map(|_| &head[..head.len() - IMPORTANT.len()])
    else {
        return (head, false);
    };
    match rest.trim_end().strip_suffix('!') {
        Some(value) => (value.trim_end(), true),
        None => (head, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn properties(css: &str) -> Vec<String> {
        parse_stylesheet(css)
            .iter()
            .flat_map(|r| r.declarations.iter())
            .map(|d| {
                format!(
                    "{}={}{}",
                    d.property,
                    d.value,
                    if d.important { "!" } else { "" }
                )
            })
            .collect()
    }

    #[test]
    fn at_rules_are_dropped_whole() {
        let css = "\
            .a { color: red; }\
            @media only screen and (max-width: 480px) { .a { color: blue; } }\
            @font-face { font-family: x; src: url(y); }\
            .b { color: green; }";
        assert_eq!(properties(css), ["color=red", "color=green"]);
    }

    #[test]
    fn a_selector_list_becomes_one_rule_per_selector() {
        let rules = parse_stylesheet(".a, .b span { color: red; }");
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].specificity(), (0, 1, 0));
        assert_eq!(rules[1].specificity(), (0, 1, 1));
    }

    #[test]
    fn an_ignored_pseudo_drops_only_its_own_selector() {
        let rules = parse_stylesheet(".a, .a:hover { color: red; }");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].specificity(), (0, 1, 0));
    }

    #[test]
    fn important_is_recognised_and_stripped() {
        assert_eq!(
            properties(".a { padding: 0 !important; margin: 0 ! IMPORTANT; color: red; }"),
            ["padding=0!", "margin=0!", "color=red"]
        );
    }

    #[test]
    fn values_keep_their_source_spelling() {
        assert_eq!(
            properties(".a { BORDER: 2px  solid  #00FF00; background: url( a.png ); }"),
            ["border=2px  solid  #00FF00", "background=url( a.png )"]
        );
    }
}
