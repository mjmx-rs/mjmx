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

use mjmx::{Diagnostic, FormatOptions, RenderOptions, Severity};
use napi::bindgen_prelude::*;
use napi_derive::napi;

/// JS-side render options: `{ breakpoint?, beautify?, minify? }`.
///
/// Every field is optional; omitted fields fall back to [`RenderOptions::default`].
#[napi(object)]
#[derive(Default)]
pub struct JsRenderOptions {
    /// Responsive breakpoint in pixels (mjml default: 480).
    pub breakpoint: Option<u32>,
    /// Pretty-print the rendered HTML.
    pub beautify: Option<bool>,
    /// Minify the rendered HTML. Takes precedence over `beautify`.
    pub minify: Option<bool>,
}

impl From<Option<JsRenderOptions>> for RenderOptionsWrapper {
    fn from(value: Option<JsRenderOptions>) -> Self {
        let defaults = RenderOptions::default();
        let Some(options) = value else {
            return Self(defaults);
        };
        Self(RenderOptions {
            breakpoint: options.breakpoint.unwrap_or(defaults.breakpoint),
            beautify: options.beautify.unwrap_or(defaults.beautify),
            minify: options.minify.unwrap_or(defaults.minify),
        })
    }
}

/// Newtype so the JS options object can be converted with a blanket [`From`] impl
/// (the orphan rule forbids `impl From<Option<JsRenderOptions>> for RenderOptions`).
struct RenderOptionsWrapper(RenderOptions);

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

/// Newtype mirroring [`RenderOptionsWrapper`] for [`FormatOptions`].
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
/// Does **not** resolve `<mj-include>` (a bare string has no base directory); use
/// [`render_file`] for that.
#[napi]
pub fn render(source: String, options: Option<JsRenderOptions>) -> Result<String> {
    let RenderOptionsWrapper(options) = options.into();
    mjmx::render(&source, &options).map_err(|diagnostics| to_js_error(&diagnostics))
}

/// Render the MJML file at `path` to HTML, resolving `<mj-include>` from disk.
#[napi]
pub fn render_file(path: String, options: Option<JsRenderOptions>) -> Result<String> {
    let RenderOptionsWrapper(options) = options.into();
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
