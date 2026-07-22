//! Knobs on the inlining pass — mjmx's answer to mjml's `juiceOptions` and
//! `juicePreserveTags`.
//!
//! mjml forwards these two straight into [juice](https://github.com/Automattic/juice)
//! (`mjml-core/src/index.js:382-396`). mjmx has no juice: the inliner is
//! hand-rolled, so each juice option had to be classified rather than forwarded.
//! [`InlineOptions`] is exactly the set that survived that triage; everything
//! else is rejected at the CLI / wasm / napi boundary with an explanation, so
//! that no caller can pass an option that quietly does nothing.
//!
//! ## The classification, option by option
//!
//! **Implemented**, because the hand-rolled inliner does the same work juice does:
//! `applyWidthAttributes`, `applyHeightAttributes`, `applyAttributesTableElements`,
//! `preserveImportant`, `styleAttributeName`, and `codeBlocks` (spelled
//! `juicePreserveTags`, see [`PreserveTag`]).
//!
//! **Structurally owned by mjmx, not an option**: `extraCss`, `applyStyleTags`,
//! `removeStyleTags`, `insertPreservedExtraCss`. In mjml these four *configure*
//! the pass; in mjmx they *are* the pass. `mjmx_semantic` partitions
//! `<mj-style>` into an inline bucket and a `<style>` bucket before rendering,
//! and this crate is only ever handed the inline one — so `extraCss` is not a
//! setting, it is the `css` argument, and the other three describe an
//! arrangement that is already fixed upstream of here. Upstream lets a caller
//! override them only because `...juiceOptions` is spread *last*, after mjml's
//! own keys; overriding `extraCss` there deletes the document's inline styles
//! outright. mjmx declines to reproduce that.
//!
//! **Unreachable in mjml's configuration**: `preserveMediaQueries`,
//! `preserveFontFaces`, `preserveKeyFrames`, `preservePseudos`. All four are
//! read only by juice's `getPreservedText`, which runs only under
//! `insertPreservedExtraCss` or `removeStyleTags` — and mjml sets both to
//! `false`. They cannot change mjml's output, so they cannot change mjmx's.
//!
//! **No mjmx analogue**: `xmlMode` (a cheerio parser mode — mjmx has one
//! forgiving tokenizer and no XML mode), `url` and `webResources` (juice only
//! reads these in `juiceResources`, which mjml does not call; and this crate
//! performs no I/O).
//!
//! **Meaningful but disproportionate, deliberately not implemented**:
//! `resolveCSSVariables` and `inlinePseudoElements`. See the crate docs' *Known
//! gaps* section — both need machinery the inliner does not have (a custom-property
//! cascade with `var()` fallbacks; synthesizing elements from a parsed `content`
//! value), and neither can be faked. Note that mjmx's current behaviour equals
//! `resolveCSSVariables: false`, which is *not* juice's default.

/// A templating construct the inliner must not look inside — mjml's
/// `juicePreserveTags`, juice's `codeBlocks`.
///
/// Everything from `start` through the nearest following `end` is masked out
/// before the document is tokenized and restored afterwards, so template syntax
/// that happens to contain markup characters is neither parsed as markup nor
/// rewritten. Without this, `{{ "<div class='a'>" }}` parses as a real `<div>`
/// and an inliner will happily splice a `style` attribute into the middle of a
/// Handlebars string literal.
///
/// # Divergence from upstream
///
/// juice keeps its code blocks in a module-level `juice.codeBlocks` object, and
/// mjml *mutates* it — `juice.codeBlocks[key] = val` — once per `mjml2html`
/// call. The entries therefore leak into every later render in the same process,
/// including ones that passed no `juicePreserveTags` at all. mjmx's list is
/// per-render data with no global state anywhere: two concurrent renders cannot
/// see each other's tags.
///
/// A second, smaller divergence: juice interpolates `start` and `end` into a
/// `RegExp`, so they are regex source. Here they are literal strings, which is
/// what the option's documented examples (`"<#"`, `"</#"`) actually mean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreserveTag {
    /// The key this tag was registered under. Purely an identity, as in juice:
    /// re-using a built-in name replaces that entry in place.
    pub name: String,
    /// The opening delimiter, e.g. `{{`.
    pub start: String,
    /// The closing delimiter, e.g. `}}`.
    pub end: String,
}

impl PreserveTag {
    /// A tag named `name` running from `start` to `end`.
    pub fn new(name: impl Into<String>, start: impl Into<String>, end: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            start: start.into(),
            end: end.into(),
        }
    }

    /// The two tags juice masks on every call, whether or not anything was
    /// registered (`juice/lib/cheerio.js`).
    #[must_use]
    pub fn defaults() -> Vec<Self> {
        vec![Self::new("EJS", "<%", "%>"), Self::new("HBS", "{{", "}}")]
    }
}

/// How to inline — mjml's `juiceOptions`, minus everything that has no meaning
/// for a hand-rolled inliner. See the [module docs](self) for the full triage.
///
/// The defaults are juice's own defaults *as mjml configures them*, so
/// `InlineOptions::default()` reproduces mjml 4.18.0's behaviour exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineOptions {
    /// Mirror a matched `width` declaration onto the `width` attribute of
    /// `table`/`td`/`th`/`img`. `// juice default: true`.
    pub apply_width_attributes: bool,
    /// Mirror a matched `height` declaration onto the `height` attribute of
    /// `table`/`td`/`th`/`img`. `// juice default: true`.
    pub apply_height_attributes: bool,
    /// Mirror `background-color`/`background-image`/`text-align`/`vertical-align`
    /// onto `bgcolor`/`background`/`align`/`valign`. `// juice default: true`.
    pub apply_attributes_table_elements: bool,
    /// Keep the `!important` keyword on a declaration that carries it.
    /// `// juice default: false`.
    ///
    /// With the default, `!important` still decides the cascade — it evicts the
    /// element's pre-existing declaration — but the keyword itself is dropped from
    /// the value, because nothing is left for it to out-rank. Turning this on
    /// writes `padding: 0 !important` instead of `padding: 0`. Attribute mirrors
    /// never carry the keyword either way.
    pub preserve_important: bool,
    /// The attribute inline declarations are read from and written to.
    /// `// juice default: "style"`.
    pub style_attribute_name: String,
    /// Templating constructs to mask out before tokenizing. Defaults to juice's
    /// built-ins (EJS `<% %>`, Handlebars `{{ }}`); mjml's `juicePreserveTags`
    /// adds to them via [`InlineOptions::add_preserve_tags`].
    pub preserve_tags: Vec<PreserveTag>,
}

impl Default for InlineOptions {
    fn default() -> Self {
        Self {
            apply_width_attributes: true,
            apply_height_attributes: true,
            apply_attributes_table_elements: true,
            // juice's own default is falsy, and mjml does not override it — which
            // is why mjml's output never carries `!important` from an inlined rule.
            preserve_important: false,
            style_attribute_name: "style".to_owned(),
            preserve_tags: PreserveTag::defaults(),
        }
    }
}

impl InlineOptions {
    /// Register `tags` on top of whatever is already there — mjml's
    /// `juicePreserveTags`, which *adds to* juice's built-ins rather than
    /// replacing them.
    ///
    /// A tag whose name matches an existing one replaces it **in place**, keeping
    /// the masking order; a new name is appended. That mirrors what assigning into
    /// juice's `codeBlocks` object does, without the global object.
    pub fn add_preserve_tags(&mut self, tags: impl IntoIterator<Item = PreserveTag>) {
        for tag in tags {
            match self.preserve_tags.iter_mut().find(|t| t.name == tag.name) {
                Some(existing) => *existing = tag,
                None => self.preserve_tags.push(tag),
            }
        }
    }

    /// [`add_preserve_tags`](Self::add_preserve_tags), by value, for building an
    /// options struct in one expression.
    #[must_use]
    pub fn with_preserve_tags(mut self, tags: impl IntoIterator<Item = PreserveTag>) -> Self {
        self.add_preserve_tags(tags);
        self
    }
}

/// The `juiceOptions` keys mjmx implements, under juice's own spelling.
///
/// Every binding checks against this list rather than keeping its own copy, so
/// the CLI, wasm and napi cannot drift apart on what is accepted.
pub const SUPPORTED_OPTIONS: &[&str] = &[
    "applyWidthAttributes",
    "applyHeightAttributes",
    "applyAttributesTableElements",
    "preserveImportant",
    "styleAttributeName",
];

/// Why mjmx refuses `key`, for a juice option it deliberately does not implement.
///
/// `None` means the key is not one of juice's options at all. Nothing here is
/// *ignored*: a caller who passes one of these gets an error, because an option
/// that is accepted and then does nothing is worse than one that is rejected.
#[must_use]
pub fn refusal(key: &str) -> Option<&'static str> {
    Some(match key {
        "extraCss" => {
            "in mjmx this is not a setting but the CSS itself — the `<mj-style \
             inline=\"inline\">` bucket `mjmx_semantic` collects. mjml lets you \
             override it only because `...juiceOptions` is spread last, which \
             silently deletes the document's own inline styles"
        }
        "applyStyleTags" | "removeStyleTags" | "insertPreservedExtraCss" => {
            "mjmx partitions `<mj-style>` into an inline bucket and a `<style>` \
             bucket before rendering, so the inliner never sees a `<style>` tag \
             and never emits one. mjml fixes all three of these to `false` for \
             the same reason"
        }
        "preserveMediaQueries" | "preserveFontFaces" | "preserveKeyFrames" | "preservePseudos" => {
            "unreachable: juice reads these only from `getPreservedText`, which \
             runs under `insertPreservedExtraCss` or `removeStyleTags`, and mjml \
             sets both to `false`"
        }
        "resolveCSSVariables" => {
            "not implemented: resolving `var()` needs a custom-property cascade \
             with fallback parsing. mjmx writes `var(--x)` through verbatim and \
             keeps the `--x` declaration, i.e. it behaves as `false`"
        }
        "inlinePseudoElements" => {
            "not implemented: mjmx drops `::before`/`::after` rules rather than \
             materializing them into elements, which would need `content` parsing \
             (strings, `url()`, `attr()`, counters)"
        }
        "xmlMode" => "a cheerio parser mode; mjmx has one forgiving HTML tokenizer and no XML mode",
        "url" | "webResources" => {
            "juice reads these only in `juiceResources`, which mjml does not call; \
             mjmx's inliner performs no I/O"
        }
        "codeBlocks" => "spell it `juicePreserveTags`, as mjml does",
        _ => return None,
    })
}

/// The error a binding reports for a `juiceOptions` key it will not accept.
#[must_use]
pub fn unsupported_option(key: &str) -> String {
    match refusal(key) {
        Some(reason) => format!("juiceOptions: `{key}` is not supported — {reason}"),
        None => format!(
            "juiceOptions: unknown option `{key}` (supported: {})",
            SUPPORTED_OPTIONS.join(", ")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_supported_key_is_accepted_rather_than_refused() {
        for key in SUPPORTED_OPTIONS {
            assert_eq!(refusal(key), None, "{key}");
        }
    }

    #[test]
    fn every_refused_key_says_why() {
        for key in [
            "extraCss",
            "applyStyleTags",
            "removeStyleTags",
            "insertPreservedExtraCss",
            "preserveMediaQueries",
            "preserveFontFaces",
            "preserveKeyFrames",
            "preservePseudos",
            "resolveCSSVariables",
            "inlinePseudoElements",
            "xmlMode",
            "url",
            "webResources",
            "codeBlocks",
        ] {
            let message = unsupported_option(key);
            assert!(refusal(key).is_some(), "{key}");
            assert!(message.contains(key), "{message}");
            assert!(!message.contains("unknown option"), "{message}");
        }
    }

    #[test]
    fn a_key_juice_never_had_is_merely_unknown() {
        let message = unsupported_option("beautify");
        assert!(message.contains("unknown option"), "{message}");
        assert!(message.contains("applyWidthAttributes"), "{message}");
    }

    #[test]
    fn the_defaults_are_mjmls_configuration_of_juice() {
        let options = InlineOptions::default();
        assert!(options.apply_width_attributes);
        assert!(options.apply_height_attributes);
        assert!(options.apply_attributes_table_elements);
        assert!(!options.preserve_important);
        assert_eq!(options.style_attribute_name, "style");
        assert_eq!(
            options
                .preserve_tags
                .iter()
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>(),
            ["EJS", "HBS"]
        );
    }

    #[test]
    fn preserve_tags_add_to_the_defaults() {
        let options =
            InlineOptions::default().with_preserve_tags([PreserveTag::new("myTag", "<#", "</#")]);
        assert_eq!(
            options
                .preserve_tags
                .iter()
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>(),
            ["EJS", "HBS", "myTag"]
        );
    }

    #[test]
    fn a_reused_name_replaces_in_place() {
        let options =
            InlineOptions::default().with_preserve_tags([PreserveTag::new("HBS", "[[", "]]")]);
        assert_eq!(
            options.preserve_tags,
            [
                PreserveTag::new("EJS", "<%", "%>"),
                PreserveTag::new("HBS", "[[", "]]"),
            ]
        );
    }

    #[test]
    fn nothing_leaks_between_two_option_values() {
        // The upstream bug this exists to avoid: juice's `codeBlocks` is a module
        // global, so mjml's `juice.codeBlocks[key] = val` outlives the call.
        let first =
            InlineOptions::default().with_preserve_tags([PreserveTag::new("myTag", "<#", "</#")]);
        let second = InlineOptions::default();
        assert_eq!(first.preserve_tags.len(), 3);
        assert_eq!(second.preserve_tags, PreserveTag::defaults());
    }
}
