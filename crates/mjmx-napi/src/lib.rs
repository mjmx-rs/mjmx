//! `mjmx-napi` — Node.js (N-API) bindings for the [`mjmx`] compiler.
//!
//! This crate is a **thin translation layer**: every exported function converts
//! JavaScript values into the umbrella crate's Rust types, calls straight through,
//! and converts the result back. No rendering, parsing or linting logic lives here.
//!
//! The JavaScript surface (see `index.d.ts`) is:
//!
//! | JS | Rust |
//! | --- | --- |
//! | `render(source, options?)` | [`mjmx::render`] |
//! | `renderFile(path, options?)` | [`mjmx::render_file`] |
//! | `lint(source)` | [`mjmx::lint`] |
//! | `format(source, options?)` | [`mjmx::format`] |
//! | `version()` | this crate's `CARGO_PKG_VERSION` |
//!
//! `render`, `renderFile` and `format` throw a JS `Error` whose message aggregates
//! the diagnostics; `lint` never throws and returns the diagnostics as data.
//!
//! # Options
//!
//! The render options use mjml's own `mjml2html` option names (`keepComments`,
//! `validationLevel`, `filePath`, `fonts`) so the addon reads as a drop-in for JS
//! mjml. Unlike [`mjmx-wasm`](https://docs.rs/mjmx-wasm) this is a native addon with
//! filesystem access, so `filePath` is fully supported: it is the base directory
//! relative `<mj-include>` paths resolve against, for [`render`] as well as
//! [`render_file`].

use indexmap::IndexMap;
use mjmx::{
    Diagnostic, FontFace, FormatOptions, InlineOptions, MinifyOptions, MjmlCompat, PreserveTag,
    RenderOptions, Severity, ValidationLevel,
};
use napi::bindgen_prelude::*;
use napi_derive::napi;

/// JS-side render options, under mjml's own option names.
///
/// Every field is optional; omitted fields fall back to [`RenderOptions::default`],
/// whose defaults are mjml's.
#[napi(object)]
#[derive(Default)]
pub struct JsRenderOptions {
    /// Which mjml release to reproduce, `4` or `5` (default: 5). mjml 5 changed the
    /// rendered HTML, so pass `4` to keep the pre-5.x output.
    /// mjmx extension — a given mjml install *is* one version.
    pub mjml_version: Option<u8>,
    /// Responsive breakpoint in pixels; columns collapse below this width.
    /// mjmx extension — mjml hardcodes 480 and only `<mj-breakpoint>` changes it.
    pub breakpoint: Option<u32>,
    /// Pretty-print the rendered HTML. [mjml: `beautify`]
    pub beautify: Option<bool>,
    /// Minify the rendered HTML. Takes precedence over `beautify`. [mjml: `minify`]
    pub minify: Option<bool>,
    /// Keep MJML source comments in the output HTML (default: true).
    /// [mjml: `keepComments`]
    pub keep_comments: Option<bool>,
    /// `"soft"` (report and render anyway, the default), `"strict"` (throw on any
    /// error) or `"skip"` (do not validate). [mjml: `validationLevel`]
    pub validation_level: Option<String>,
    /// Directory (or file) relative `<mj-include>` paths resolve against; overrides the
    /// rendered file's own directory. [mjml: `filePath`]
    pub file_path: Option<String>,
    /// Web fonts as mjml's `{ name: url }` object, *replacing* the built-in table
    /// rather than extending it. Key order is preserved. [mjml: `fonts`]
    pub fonts: Option<IndexMap<String, String>>,
    /// How to minify, consulted only when `minify` is true. Omitted switches keep
    /// mjml's defaults. [mjml: `minifyOptions`]
    pub minify_options: Option<JsMinifyOptions>,
    /// Skip `<mj-include>` tags instead of resolving them; they are *dropped*, along
    /// with the content they would have pulled in. [mjml: `ignoreIncludes`]
    pub ignore_includes: Option<bool>,
    /// Also emit the responsive column rules inside an `@media only print` block.
    /// [mjml: `printerSupport`]
    pub printer_support: Option<bool>,
    /// How to inline `<mj-style inline="inline">`, consulted only when the document
    /// has an inline `<mj-style>` bucket. Omitted switches keep juice's defaults as
    /// mjml configures them. [mjml: `juiceOptions`]
    pub juice_options: Option<JsJuiceOptions>,
    /// Templating delimiters the CSS inliner must not look inside, keyed by name.
    /// These *add* to the always-active built-ins — EJS (`<% %>`) and Handlebars
    /// (`{{ }}`) — and a built-in name re-used here replaces it in place. Key order
    /// is preserved, since masking is order-sensitive.
    ///
    /// Per-call, unlike mjml: upstream writes these into juice's module-global
    /// `codeBlocks` object, so they leak into every later render in the process.
    /// [mjml: `juicePreserveTags`]
    pub juice_preserve_tags: Option<IndexMap<String, JsPreserveTag>>,
}

/// mjml's `juiceOptions`, restricted to what mjmx's hand-rolled inliner can honour.
///
/// mjmx has no juice, so this is not a pass-through object: each juice option was
/// classified, and these five are the ones that mean something here.
///
/// The rest are refused by the *TypeScript* interface in `index.d.ts`, which is
/// closed — not at runtime, because N-API decodes this object field by field and
/// drops an unknown key before any mjmx code runs. `mjmx-wasm`, which receives the
/// raw JS object, throws [`mjmx::unsupported_option`] instead; the same limitation
/// already applies to [`JsMinifyOptions`] here.
///
/// The reasons, in order: `extraCss`, `applyStyleTags`, `removeStyleTags` and
/// `insertPreservedExtraCss` describe an arrangement mjmx fixes before rendering;
/// `preserveMediaQueries`, `preserveFontFaces`, `preserveKeyFrames` and
/// `preservePseudos` are unreachable in mjml's own configuration; `xmlMode`, `url`
/// and `webResources` have no analogue; `resolveCSSVariables` and
/// `inlinePseudoElements` are real gaps that cannot be faked.
#[napi(object)]
#[derive(Default)]
pub struct JsJuiceOptions {
    /// Mirror a matched `width` declaration onto the `width` attribute of
    /// `table`/`td`/`th`/`img` (default: true).
    pub apply_width_attributes: Option<bool>,
    /// Mirror a matched `height` declaration onto the `height` attribute of
    /// `table`/`td`/`th`/`img` (default: true).
    pub apply_height_attributes: Option<bool>,
    /// Mirror `background-color`/`background-image`/`text-align`/`vertical-align` onto
    /// `bgcolor`/`background`/`align`/`valign` on `<table>` (default: true).
    pub apply_attributes_table_elements: Option<bool>,
    /// Keep the `!important` keyword on an inlined declaration (default: false, as in
    /// juice and therefore mjml). It still decides the cascade either way; attribute
    /// mirrors never carry it.
    pub preserve_important: Option<bool>,
    /// The attribute inline declarations are read from and written to
    /// (default: `"style"`).
    pub style_attribute_name: Option<String>,
}

impl From<JsJuiceOptions> for InlineOptions {
    fn from(value: JsJuiceOptions) -> Self {
        let defaults = Self::default();
        Self {
            apply_width_attributes: value
                .apply_width_attributes
                .unwrap_or(defaults.apply_width_attributes),
            apply_height_attributes: value
                .apply_height_attributes
                .unwrap_or(defaults.apply_height_attributes),
            apply_attributes_table_elements: value
                .apply_attributes_table_elements
                .unwrap_or(defaults.apply_attributes_table_elements),
            preserve_important: value
                .preserve_important
                .unwrap_or(defaults.preserve_important),
            style_attribute_name: value
                .style_attribute_name
                .unwrap_or(defaults.style_attribute_name),
            preserve_tags: defaults.preserve_tags,
        }
    }
}

/// The delimiters of one `juicePreserveTags` entry.
#[napi(object)]
pub struct JsPreserveTag {
    /// The opening delimiter, e.g. `"<#"`.
    pub start: String,
    /// The closing delimiter, e.g. `"</#"`.
    pub end: String,
}

/// mjml's `minifyOptions` — the four html-minifier switches mjml hardcodes.
///
/// Only these four exist: mjml never sets another one, and this crate implements
/// exactly what it exposes. `minifyCss` is spelled `minifyCSS` on the JS side, as mjml
/// spells it.
#[napi(object)]
#[derive(Default)]
pub struct JsMinifyOptions {
    /// Collapse insignificant whitespace (default: true).
    pub collapse_whitespace: Option<bool>,
    /// Minify CSS inside `<style>` (default: false). A conservative whitespace and
    /// comment pass, not html-minifier's full `clean-css` optimizer.
    #[napi(js_name = "minifyCSS")]
    pub minify_css: Option<bool>,
    /// Treat attribute names case-sensitively; `false` lower-cases them (default: true).
    pub case_sensitive: Option<bool>,
    /// Drop empty `class`/`id`/`style`/`title`/`lang`/`dir`, empty event handlers and
    /// `value=""` on `<input>` — html-minifier's exact list (default: true).
    pub remove_empty_attributes: Option<bool>,
}

/// Layer a JS `minifyOptions` object over the defaults of the mjml release being
/// reproduced, so an explicit switch wins but an omitted one follows the release.
fn to_minify_options(value: Option<JsMinifyOptions>, defaults: MinifyOptions) -> MinifyOptions {
    let Some(value) = value else {
        return defaults;
    };
    MinifyOptions {
        collapse_whitespace: value
            .collapse_whitespace
            .unwrap_or(defaults.collapse_whitespace),
        minify_css: value.minify_css.unwrap_or(defaults.minify_css),
        case_sensitive: value.case_sensitive.unwrap_or(defaults.case_sensitive),
        remove_empty_attributes: value
            .remove_empty_attributes
            .unwrap_or(defaults.remove_empty_attributes),
        // Not one of mjml's four switches; it comes from the release being reproduced.
        css_dialect: defaults.css_dialect,
    }
}

/// The minify defaults of the mjml release being reproduced.
fn minify_defaults(compat: MjmlCompat) -> MinifyOptions {
    if compat.is_v5() {
        MinifyOptions::for_mjml5()
    } else {
        MinifyOptions::default()
    }
}

/// Build the [`RenderOptions`] a JS options object describes; `None` is the defaults.
///
/// The error is a plain `String` rather than a napi [`Error`], for two reasons: it
/// keeps the conversion unit-testable outside a Node process (dropping an `Error` calls
/// into the N-API runtime), and the caller is what knows the right [`Status`].
fn to_render_options(
    options: Option<JsRenderOptions>,
) -> std::result::Result<RenderOptions, String> {
    let defaults = RenderOptions::default();
    let Some(options) = options else {
        return Ok(defaults);
    };
    let compat = options
        .mjml_version
        .map_or(Ok(defaults.compat), parse_mjml_version)?;
    Ok(RenderOptions {
        compat,
        breakpoint: options.breakpoint.unwrap_or(defaults.breakpoint),
        beautify: options.beautify.unwrap_or(defaults.beautify),
        minify: options.minify.unwrap_or(defaults.minify),
        keep_comments: options.keep_comments.unwrap_or(defaults.keep_comments),
        validation_level: options
            .validation_level
            .as_deref()
            .map_or(Ok(defaults.validation_level), parse_validation_level)?,
        file_path: options.file_path.map(std::path::PathBuf::from),
        fonts: options.fonts.map(to_font_faces),
        minify_options: to_minify_options(options.minify_options, minify_defaults(compat)),
        ignore_includes: options.ignore_includes.unwrap_or(defaults.ignore_includes),
        printer_support: options.printer_support.unwrap_or(defaults.printer_support),
        // `juicePreserveTags` adds to the built-ins rather than replacing them, so the
        // two mjml options fold into the single carrier `RenderOptions` holds.
        inline_options: InlineOptions::from(options.juice_options.unwrap_or_default())
            .with_preserve_tags(to_preserve_tags(options.juice_preserve_tags)?),
    })
}

/// Turn mjml's `{ name: { start, end } }` object into ordered [`PreserveTag`]s.
///
/// Order is kept because masking is sequential: an earlier tag's block hides any
/// delimiter a later one would have matched inside it.
fn to_preserve_tags(
    tags: Option<IndexMap<String, JsPreserveTag>>,
) -> std::result::Result<Vec<PreserveTag>, String> {
    tags.unwrap_or_default()
        .into_iter()
        .map(|(name, tag)| {
            if tag.start.is_empty() || tag.end.is_empty() {
                return Err(format!(
                    "mjmx: juicePreserveTags '{name}' needs a non-empty start and end"
                ));
            }
            Ok(PreserveTag::new(name, tag.start, tag.end))
        })
        .collect()
}

/// [`to_render_options`], with the failure turned into the JS `TypeError` the exported
/// functions throw.
fn render_options(options: Option<JsRenderOptions>) -> Result<RenderOptions> {
    to_render_options(options).map_err(|message| Error::new(Status::InvalidArg, message))
}

/// Parse one of mjml's `validationLevel` strings.
/// Map the bare mjml major version a caller passes onto the behaviour bundle.
///
/// An unknown major is an error rather than a fallback to the default: rendering some
/// version other than the one asked for is the failure this option exists to prevent.
fn parse_mjml_version(version: u8) -> std::result::Result<MjmlCompat, String> {
    match version {
        4 => Ok(MjmlCompat::V4),
        5 => Ok(MjmlCompat::V5),
        other => Err(format!(
            "mjmx: unknown mjmlVersion {other} (expected 4 or 5)"
        )),
    }
}

fn parse_validation_level(level: &str) -> std::result::Result<ValidationLevel, String> {
    match level {
        "soft" => Ok(ValidationLevel::Soft),
        "strict" => Ok(ValidationLevel::Strict),
        "skip" => Ok(ValidationLevel::Skip),
        other => Err(format!(
            "mjmx: unknown validationLevel '{other}' (expected soft, strict or skip)"
        )),
    }
}

/// Turn mjml's `fonts` object into the owned faces [`RenderOptions::fonts`] expects,
/// keeping the JS key order (which decides `<link>` order in the rendered head).
fn to_font_faces(fonts: IndexMap<String, String>) -> Vec<FontFace<'static>> {
    fonts
        .into_iter()
        .map(|(name, href)| FontFace::new(name, href))
        .collect()
}

/// JS-side format options: `{ indentWidth? }`.
#[napi(object)]
#[derive(Default)]
pub struct JsFormatOptions {
    /// Spaces per indentation level (default: 2).
    pub indent_width: Option<u32>,
}

impl From<Option<JsFormatOptions>> for FormatOptionsWrapper {
    fn from(value: Option<JsFormatOptions>) -> Self {
        let defaults = FormatOptions::default();
        let Some(options) = value else {
            return Self(defaults);
        };
        Self(FormatOptions {
            indent_width: options
                .indent_width
                .map_or(defaults.indent_width, |width| width as usize),
        })
    }
}

/// Newtype so the JS format options can be converted with a blanket [`From`] impl
/// (the orphan rule forbids `impl From<Option<JsFormatOptions>> for FormatOptions`).
struct FormatOptionsWrapper(FormatOptions);

/// A span-anchored label on a [`JsDiagnostic`].
#[napi(object)]
pub struct JsLabel {
    /// Start byte offset into the source (inclusive).
    pub start: u32,
    /// End byte offset into the source (exclusive).
    pub end: u32,
    /// The message describing the labeled region.
    pub message: String,
}

/// A diagnostic as plain JS data: `{ severity, message, labels, help? }`.
#[napi(object)]
pub struct JsDiagnostic {
    /// `"error"`, `"warning"` or `"advice"`.
    pub severity: String,
    /// The primary message.
    pub message: String,
    /// Span-anchored labels.
    pub labels: Vec<JsLabel>,
    /// Optional help text suggesting a fix.
    pub help: Option<String>,
}

/// Stringify a [`Severity`] into the JS-facing enum value.
fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Advice => "advice",
    }
}

/// Translate one Rust [`Diagnostic`] into its JS object form.
fn to_js_diagnostic(diagnostic: Diagnostic) -> JsDiagnostic {
    JsDiagnostic {
        severity: severity_name(diagnostic.severity).to_owned(),
        message: diagnostic.message,
        labels: diagnostic
            .labels
            .into_iter()
            .map(|label| JsLabel {
                start: label.span.start,
                end: label.span.end,
                message: label.message,
            })
            .collect(),
        help: diagnostic.help,
    }
}

/// Aggregate a diagnostic list into a single readable JS `Error`.
///
/// The message is the first diagnostic (severity-prefixed, with its help text if
/// any) plus a count of the remainder, e.g.
/// `mjmx: error: mj-column must contain content (+2 more diagnostics)`.
///
/// The structured diagnostics are not attached here — N-API's `Error` is a plain
/// status + reason pair, so the JS wrapper in `index.js` re-runs `lint()` on the
/// offending source and attaches the result as `err.diagnostics`.
fn to_js_error(diagnostics: &[Diagnostic]) -> Error {
    let Some(first) = diagnostics.first() else {
        return Error::new(Status::GenericFailure, "mjmx: unknown error".to_owned());
    };
    let mut message = format!("mjmx: {}: {}", severity_name(first.severity), first.message);
    if let Some(help) = &first.help {
        message.push_str(&format!(" (help: {help})"));
    }
    let rest = diagnostics.len() - 1;
    if rest > 0 {
        let plural = if rest == 1 { "" } else { "s" };
        message.push_str(&format!(" (+{rest} more diagnostic{plural})"));
    }
    Error::new(Status::GenericFailure, message)
}

/// Render MJML `source` to HTML. Throws if parsing or rendering produced errors.
///
/// Resolves `<mj-include>` only when `options.filePath` gives a base directory to
/// resolve relative paths against (mjml's `filePath`); use [`render_file`] to render a
/// file, which derives that base from the file itself.
#[napi]
pub fn render(source: String, options: Option<JsRenderOptions>) -> Result<String> {
    let options = render_options(options)?;
    mjmx::render(&source, &options).map_err(|diagnostics| to_js_error(&diagnostics))
}

/// Render the MJML file at `path` to HTML, resolving `<mj-include>` from disk.
#[napi]
pub fn render_file(path: String, options: Option<JsRenderOptions>) -> Result<String> {
    let options = render_options(options)?;
    mjmx::render_file(std::path::Path::new(&path), &options)
        .map_err(|diagnostics| to_js_error(&diagnostics))
}

/// Format MJML `source` into canonical MJML source text. Throws on parse errors.
#[napi]
pub fn format(source: String, options: Option<JsFormatOptions>) -> Result<String> {
    let FormatOptionsWrapper(options) = options.into();
    mjmx::format(&source, &options).map_err(|diagnostics| to_js_error(&diagnostics))
}

/// Lint MJML `source`, returning every diagnostic. Never throws.
#[napi]
pub fn lint(source: String) -> Vec<JsDiagnostic> {
    mjmx::lint(&source)
        .into_iter()
        .map(to_js_diagnostic)
        .collect()
}

/// The version of the `mjmx-napi` crate (matches the workspace version).
#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// Conversion tests for the JS options objects.
///
/// The structs are plain Rust once N-API has decoded them, so the JS-object →
/// [`RenderOptions`] mapping is testable without a Node process. The end-to-end JS
/// surface is covered by `test/smoke.mjs`.
#[cfg(test)]
mod tests {
    use super::*;

    /// The `RenderOptions` a JS options object converts to.
    fn convert(options: Option<JsRenderOptions>) -> std::result::Result<RenderOptions, String> {
        to_render_options(options)
    }

    #[test]
    fn omitted_options_are_mjmls_defaults() {
        for options in [None, Some(JsRenderOptions::default())] {
            let options = convert(options).unwrap();
            assert_eq!(options.breakpoint, 480);
            assert!(!options.beautify);
            assert!(!options.minify);
            assert!(options.keep_comments);
            assert_eq!(options.validation_level, ValidationLevel::Soft);
            assert_eq!(options.file_path, None);
            assert_eq!(options.fonts, None);
            assert!(!options.ignore_includes);
            assert!(!options.printer_support);
            // No `mjmlVersion`, so 5.4.0's minify defaults.
            assert_eq!(options.minify_options, MinifyOptions::for_mjml5());
        }
    }

    #[test]
    fn every_option_round_trips_under_its_mjml_name() {
        let mut fonts = IndexMap::new();
        fonts.insert("Zeta".to_owned(), "https://z.example/z.css".to_owned());
        fonts.insert("Alpha".to_owned(), "https://a.example/a.css".to_owned());

        let options = convert(Some(JsRenderOptions {
            mjml_version: Some(4),
            breakpoint: Some(600),
            beautify: Some(true),
            minify: Some(true),
            keep_comments: Some(false),
            validation_level: Some("strict".to_owned()),
            file_path: Some("./partials".to_owned()),
            fonts: Some(fonts),
            minify_options: Some(JsMinifyOptions {
                collapse_whitespace: Some(false),
                minify_css: Some(true),
                case_sensitive: Some(false),
                remove_empty_attributes: Some(true),
            }),
            ignore_includes: Some(true),
            printer_support: Some(true),
            juice_options: Some(JsJuiceOptions {
                apply_width_attributes: Some(false),
                preserve_important: Some(true),
                ..JsJuiceOptions::default()
            }),
            juice_preserve_tags: Some(IndexMap::from([(
                "myTag".to_owned(),
                JsPreserveTag {
                    start: "<#".to_owned(),
                    end: "</#".to_owned(),
                },
            )])),
        }))
        .unwrap();

        assert_eq!(options.compat, MjmlCompat::V4);
        assert_eq!(options.breakpoint, 600);
        assert!(options.beautify);
        assert!(options.minify);
        assert!(!options.keep_comments);
        assert_eq!(options.validation_level, ValidationLevel::Strict);
        assert_eq!(
            options.file_path,
            Some(std::path::PathBuf::from("./partials"))
        );
        // Key order survives the boundary: it decides `<link>` order in the head.
        assert_eq!(
            options.fonts.unwrap(),
            vec![
                FontFace::new("Zeta", "https://z.example/z.css"),
                FontFace::new("Alpha", "https://a.example/a.css"),
            ]
        );
        assert!(options.ignore_includes);
        assert!(options.printer_support);
        assert_eq!(
            options.minify_options,
            MinifyOptions {
                collapse_whitespace: false,
                minify_css: true,
                case_sensitive: false,
                remove_empty_attributes: true,
                // `mjmlVersion: 4` above, so the 4.18.0 minifier.
                css_dialect: mjmx::CssDialect::CleanCss,
            }
        );
        // `juiceOptions` names only what it changes; the rest keep juice's defaults.
        assert!(!options.inline_options.apply_width_attributes);
        assert!(options.inline_options.apply_height_attributes);
        assert!(options.inline_options.preserve_important);
        // `juicePreserveTags` adds to the built-ins, never replaces them.
        assert_eq!(
            options
                .inline_options
                .preserve_tags
                .iter()
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>(),
            ["EJS", "HBS", "myTag"]
        );
    }

    #[test]
    fn an_empty_preserve_tag_delimiter_is_rejected() {
        let err = convert(Some(JsRenderOptions {
            juice_preserve_tags: Some(IndexMap::from([(
                "bad".to_owned(),
                JsPreserveTag {
                    start: "<#".to_owned(),
                    end: String::new(),
                },
            )])),
            ..JsRenderOptions::default()
        }))
        .unwrap_err();
        assert!(err.contains("bad"), "{err}");
    }

    #[test]
    fn omitted_minify_switches_keep_their_defaults() {
        // mjml spreads `minifyOptions` over the defaults, so a partial object only
        // changes the keys it names.
        let options = convert(Some(JsRenderOptions {
            minify_options: Some(JsMinifyOptions {
                minify_css: Some(true),
                ..JsMinifyOptions::default()
            }),
            ..JsRenderOptions::default()
        }))
        .unwrap();
        let defaults = MinifyOptions::default();
        assert!(options.minify_options.minify_css);
        assert_eq!(
            options.minify_options.collapse_whitespace,
            defaults.collapse_whitespace
        );
        assert_eq!(
            options.minify_options.remove_empty_attributes,
            defaults.remove_empty_attributes
        );
    }

    #[test]
    fn validation_level_accepts_every_string_in_the_union() {
        for (name, expected) in [
            ("soft", ValidationLevel::Soft),
            ("strict", ValidationLevel::Strict),
            ("skip", ValidationLevel::Skip),
        ] {
            let options = convert(Some(JsRenderOptions {
                validation_level: Some(name.to_owned()),
                ..JsRenderOptions::default()
            }))
            .unwrap();
            assert_eq!(options.validation_level, expected, "for '{name}'");
        }
    }

    #[test]
    fn an_unknown_validation_level_is_rejected() {
        let err = convert(Some(JsRenderOptions {
            validation_level: Some("loud".to_owned()),
            ..JsRenderOptions::default()
        }))
        .unwrap_err();
        assert!(err.contains("loud"), "{err}");
    }

    #[test]
    fn an_unknown_mjml_version_is_rejected() {
        // Falling back to the default here would render 5.x output for a caller who
        // explicitly asked for something else.
        let err = convert(Some(JsRenderOptions {
            mjml_version: Some(3),
            ..JsRenderOptions::default()
        }))
        .unwrap_err();
        assert!(err.contains('3'), "{err}");
    }

    #[test]
    fn an_omitted_mjml_version_tracks_upstream_latest() {
        let options = convert(Some(JsRenderOptions::default())).unwrap();
        assert_eq!(options.compat, MjmlCompat::V5);
    }

    #[test]
    fn format_options_take_indent_width() {
        let FormatOptionsWrapper(options) = Some(JsFormatOptions {
            indent_width: Some(4),
        })
        .into();
        assert_eq!(options.indent_width, 4);

        let FormatOptionsWrapper(options) = None.into();
        assert_eq!(options.indent_width, FormatOptions::default().indent_width);
    }
}
