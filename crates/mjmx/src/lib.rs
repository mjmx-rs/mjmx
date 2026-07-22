//! `mjmx` — the umbrella crate: MJML source text into responsive HTML.
//!
//! This is the one-call front door over the pipeline. [`render`] allocates an arena,
//! parses the source with [`mjmx_parser`], and — if parsing produced no errors —
//! renders the AST with [`mjmx_codegen`]. Parse and render diagnostics are surfaced
//! as `Err(Vec<Diagnostic>)`.
//!
//! ```
//! let html = mjmx::render_default(
//!     "<mjml><mj-body><mj-section><mj-column><mj-text>Hi</mj-text></mj-column></mj-section></mj-body></mjml>",
//! )
//! .unwrap();
//! assert!(html.contains("Hi"));
//! assert!(html.starts_with("<!doctype html>"));
//! ```

use mjmx_allocator::Allocator;

pub use mjmx_codegen::{
    FontFace, Mapping, RenderContext, RenderOptions, SourceMap, Span, ValidationLevel,
};
pub use mjmx_css_inline::{
    InlineOptions, PreserveTag, SUPPORTED_OPTIONS, refusal, unsupported_option,
};
pub use mjmx_diagnostics::{Diagnostic, Severity};
pub use mjmx_formatter::FormatOptions;
pub use mjmx_minifier::MinifyOptions;

use std::path::Path;

/// Render MJML `source` to HTML using the given [`RenderOptions`].
///
/// Returns every parse, validation or render [`Diagnostic`] on failure.
///
/// `<mj-include>` is resolved only when [`RenderOptions::file_path`] gives a base to
/// resolve relative paths against; with the default `None` a bare string renders with
/// its includes left untouched. Use [`render_file`] to render a file on disk, which
/// derives that base from the file itself.
///
/// Non-fatal diagnostics (a `soft` [`validation_level`](RenderOptions::validation_level)
/// finding, an advisory include note) are dropped here. Use [`render_with_diagnostics`]
/// to see them alongside the HTML.
pub fn render(source: &str, options: &RenderOptions) -> Result<String, Vec<Diagnostic>> {
    render_with_diagnostics(source, options).map(|(html, _)| html)
}

/// [`render`], also returning the diagnostics that did **not** stop the render.
///
/// This is mjml's own shape — `mjml2html` returns `{ html, errors }`, where `errors` is
/// what `validationLevel: 'soft'` found but rendered through anyway. `render` exists
/// unchanged for callers that only want the HTML.
///
/// # Errors
///
/// As [`render`]: parse errors, include-resolution errors, a `strict` validation
/// failure, or a render error.
pub fn render_with_diagnostics(
    source: &str,
    options: &RenderOptions,
) -> Result<(String, Vec<Diagnostic>), Vec<Diagnostic>> {
    render_source_with(source, options, &mjmx_transformer::LocalLoader)
}

/// [`render_with_diagnostics`] with the loader — and, for a file render, the include
/// base directory — chosen by the caller.
///
/// The whole pipeline lives here: parse → resolve `<mj-include>` (only when there is a
/// base) → validate at the requested [`ValidationLevel`] → render → post-process.
/// `base` overrides [`RenderOptions::file_path`], so a file render always has a base
/// even with the default options.
fn render_source(
    source: &str,
    options: &RenderOptions,
    loader: &dyn mjmx_transformer::IncludeLoader,
    base: Option<&Path>,
) -> Result<(String, Vec<Diagnostic>), Vec<Diagnostic>> {
    let allocator = Allocator::new();
    let parsed = mjmx_parser::parse(source, &allocator);
    if !parsed.errors.is_empty() {
        return Err(parsed.errors);
    }

    // Everything that was reported but did not stop the render.
    let mut soft = Vec::new();

    let builder = mjmx_ast::AstBuilder::new(&allocator);
    // mjml's `ignoreIncludes` short-circuits include handling entirely: its parser
    // drops every `<mj-include>` before the node exists, so nothing is resolved *and*
    // nothing is left behind. Checked before the base lookup, so it wins over both an
    // explicit `base` and `file_path`.
    let document = if options.ignore_includes {
        mjmx_transformer::strip_includes(&builder, &parsed.document)
    } else {
        match base.or_else(|| options.file_path.as_deref().map(include_base)) {
            Some(root_dir) => {
                let (document, include_diags) = mjmx_transformer::resolve_includes(
                    &builder,
                    &parsed.document,
                    loader,
                    root_dir,
                );
                // Any error-severity include diagnostic blocks rendering (missing file, cycle).
                if include_diags.iter().any(|d| d.severity == Severity::Error) {
                    return Err(include_diags);
                }
                soft.extend(include_diags);
                document
            }
            None => parsed.document,
        }
    };

    // mjml's `validationLevel`, applied to the post-include tree exactly as mjml
    // applies `MJMLValidator` after parsing.
    match options.validation_level {
        ValidationLevel::Skip => {}
        level => {
            let lint_diags = mjmx_linter::lint(&document);
            if level == ValidationLevel::Strict && has_errors(&lint_diags) {
                return Err(lint_diags);
            }
            soft.extend(lint_diags);
        }
    }

    let (html, inline_css, html_attributes) =
        mjmx_codegen::render_document_with_inline_css(&document, options)?;
    // mjml's order: the cheerio `htmlAttributes` pass runs *before* juice, so an
    // added attribute can be a selector target for the inliner that follows.
    let html = apply_html_attributes(html, &html_attributes);
    Ok((
        post_process(inline_css_into(html, &inline_css, options), options),
        soft,
    ))
}

/// Apply the `<mj-html-attributes>` rules to the rendered HTML, mjml's cheerio pass.
///
/// Runs on the assembled document, **before** CSS inlining and any whitespace
/// post-processing, matching mjml's `mjml-core` order (`globalData.htmlAttributes`
/// before `juice`). With no rules there is nothing to do and the HTML is returned
/// untouched — the default render path is unchanged, byte for byte.
fn apply_html_attributes(html: String, rules: &[mjmx_codegen::HtmlAttributeRule]) -> String {
    if rules.is_empty() {
        return html;
    }
    // Flatten the per-selector rules into the `(selector, name, value)` triples the
    // pass applies in order.
    let triples: Vec<mjmx_css_inline::HtmlAttribute<'_>> = rules
        .iter()
        .flat_map(|rule| {
            rule.attributes
                .iter()
                .map(move |(name, value)| mjmx_css_inline::HtmlAttribute {
                    selector: rule.selector.as_str(),
                    name: name.as_str(),
                    value: value.as_str(),
                })
        })
        .collect();
    mjmx_css_inline::apply_html_attributes(&html, &triples)
}

/// Apply the `<mj-style inline="inline">` bucket to the rendered HTML, mjml's juice pass.
///
/// Runs on the assembled document and **before** [`post_process`], matching mjml's order:
/// inlining reads the markup codegen produced, and minify/beautify then run over the
/// result. With an empty bucket there is nothing to do and the HTML is returned untouched
/// — the default render path is unchanged, byte for byte.
fn inline_css_into(html: String, inline_css: &[String], options: &RenderOptions) -> String {
    if inline_css.is_empty() {
        return html;
    }
    // Concatenated in source order, so a later `<mj-style inline="inline">` wins ties
    // against an earlier one exactly as it would inside a single stylesheet.
    mjmx_css_inline::inline_with(&html, &inline_css.join("\n"), &options.inline_options)
}

/// [`render_source`] with no caller-supplied base: the only base, if any, is
/// [`RenderOptions::file_path`].
fn render_source_with(
    source: &str,
    options: &RenderOptions,
    loader: &dyn mjmx_transformer::IncludeLoader,
) -> Result<(String, Vec<Diagnostic>), Vec<Diagnostic>> {
    render_source(source, options, loader, None)
}

/// The directory relative `<mj-include>` paths resolve against, given a
/// [`RenderOptions::file_path`]: the path itself when it is a directory, otherwise its
/// parent (mjml's `filePath` is documented as the *file* the source came from).
fn include_base(path: &Path) -> &Path {
    if path.is_dir() {
        return path;
    }
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

/// Whether any diagnostic is error-severity.
fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| d.severity == Severity::Error)
}

/// Apply the whitespace post-processors selected by `options`.
///
/// `minify` and `beautify` are mutually exclusive whitespace transforms; if a
/// caller sets both, **`minify` wins** (the more destructive, size-oriented
/// transform is assumed to be the intent). When neither is set the rendered HTML
/// is returned untouched — the default render path is unchanged.
fn post_process(html: String, options: &RenderOptions) -> String {
    if options.minify {
        // `minify_options` is mjml's `minifyOptions`: consulted only on this branch,
        // exactly as mjml only spreads it into the html-minifier call when `minify` is
        // truthy.
        mjmx_minifier::minify_with(&html, &options.minify_options)
    } else if options.beautify {
        mjmx_minifier::beautify(&html)
    } else {
        html
    }
}

/// Render MJML `source` to HTML **and** a [`SourceMap`] from output bytes back to the
/// MJML elements that produced them.
///
/// The HTML is byte-identical to [`render`]'s for the same input — collection only
/// observes buffer offsets, it never emits. Use [`SourceMap::lookup`] to answer "which
/// MJML element produced this bit of HTML?" (click-to-source, go-to-output, debugging a
/// surprising render) and [`SourceMap::mappings_for_source`] for the reverse.
///
/// ```
/// let source = "<mjml><mj-body><mj-section><mj-column>\
///               <mj-text>Hello world</mj-text></mj-column></mj-section></mj-body></mjml>";
/// let (html, map) = mjmx::render_with_sourcemap(source, &mjmx::RenderOptions::default()).unwrap();
/// let at = html.find("Hello world").unwrap();
/// let m = map.lookup(at).unwrap();
/// assert!(source[m.source.start as usize..m.source.end as usize].starts_with("<mj-text"));
/// ```
///
/// This is the bare codegen path: it neither validates
/// ([`validation_level`](RenderOptions::validation_level)) nor resolves `<mj-include>`
/// ([`file_path`](RenderOptions::file_path)), since both would move the source offsets
/// the map is expressed in. Every other option is honored.
///
/// # Errors
///
/// As [`render`], plus a refusal for any render whose HTML is rewritten after codegen,
/// which would silently invalidate every recorded offset:
///
/// - `options` requests `minify` or `beautify` — whitespace rewrites;
/// - the document has a `<mj-style inline="inline">` — CSS inlining rewrites `style`
///   attributes and inserts new ones. This one is a property of the *document*, not of
///   the options, so it is only detected once the source has been parsed and analyzed.
///
/// Render with a sourcemap first, then post-process if you must (and discard the map).
pub fn render_with_sourcemap(
    source: &str,
    options: &RenderOptions,
) -> Result<(String, SourceMap), Vec<Diagnostic>> {
    let allocator = Allocator::new();
    let parsed = mjmx_parser::parse(source, &allocator);
    if !parsed.errors.is_empty() {
        return Err(parsed.errors);
    }
    mjmx_codegen::render_document_with_sourcemap(&parsed.document, options)
}

/// Render MJML `source` to HTML with [`RenderOptions::default`].
pub fn render_default(source: &str) -> Result<String, Vec<Diagnostic>> {
    render(source, &RenderOptions::default())
}

/// Render an MJML file at `path` to HTML, resolving `<mj-include>` against the disk.
///
/// Unlike [`render`], this has a base directory (the file's parent), so relative
/// `<mj-include path="./partial.mjml">` references resolve via the filesystem
/// [`LocalLoader`](mjmx_transformer::LocalLoader). The flow is: read → parse →
/// [`resolve_includes`](mjmx_transformer::resolve_includes) → render.
///
/// Include-resolution diagnostics (missing files, cycles) are error-severity and are
/// returned as `Err` alongside any parse errors. String-input [`render`] cannot
/// resolve relative includes because it has no base directory; use this for files.
///
/// # Errors
///
/// Returns every diagnostic on failure: an I/O error reading `path`, parse errors,
/// include-resolution errors, or render errors.
pub fn render_file(
    path: &std::path::Path,
    options: &RenderOptions,
) -> Result<String, Vec<Diagnostic>> {
    render_file_with_diagnostics(path, options).map(|(html, _)| html)
}

/// [`render_file`], also returning the diagnostics that did **not** stop the render.
///
/// The file-input counterpart of [`render_with_diagnostics`], with the same
/// `{ html, errors }` shape mjml's `mjml2html` returns.
///
/// # Errors
///
/// As [`render_file`].
pub fn render_file_with_diagnostics(
    path: &std::path::Path,
    options: &RenderOptions,
) -> Result<(String, Vec<Diagnostic>), Vec<Diagnostic>> {
    render_file_with(path, options, &mjmx_transformer::LocalLoader)
}

/// Render an MJML file, resolving `<mj-include>` **over the network as well as the
/// disk**.
///
/// Only exists with the non-default `http-loader` feature. An include whose `path`
/// is an `http://` / `https://` URL is fetched; a relative path inside a fetched
/// partial resolves against that partial's URL; everything else comes off the disk
/// exactly as in [`render_file`].
///
/// # Security
///
/// This hands the template author control over what your process connects to — an
/// SSRF-shaped capability (cloud metadata endpoints, `localhost` admin ports,
/// private subnets). It is a separate function behind a non-default cargo feature
/// precisely so it cannot be reached by accident: [`render_file`] never fetches.
/// Do not call this on templates you do not trust. See [`mjmx_transformer::http`].
///
/// # Errors
///
/// As [`render_file`], plus any HTTP transport or status failure.
#[cfg(feature = "http-loader")]
pub fn render_file_allow_http(
    path: &std::path::Path,
    options: &RenderOptions,
) -> Result<String, Vec<Diagnostic>> {
    render_file_allow_http_with_diagnostics(path, options).map(|(html, _)| html)
}

/// `render_file_allow_http`, also returning the diagnostics that did **not** stop the
/// render — the network-enabled counterpart of [`render_file_with_diagnostics`].
///
/// # Errors
///
/// As `render_file_allow_http`.
#[cfg(feature = "http-loader")]
pub fn render_file_allow_http_with_diagnostics(
    path: &std::path::Path,
    options: &RenderOptions,
) -> Result<(String, Vec<Diagnostic>), Vec<Diagnostic>> {
    render_file_with(path, options, &mjmx_transformer::CombinedLoader::new())
}

/// The shared body of [`render_file`] and `render_file_allow_http`:
/// read → parse → resolve includes with `loader` → render.
fn render_file_with(
    path: &std::path::Path,
    options: &RenderOptions,
    loader: &dyn mjmx_transformer::IncludeLoader,
) -> Result<(String, Vec<Diagnostic>), Vec<Diagnostic>> {
    let source = std::fs::read_to_string(path).map_err(|err| {
        vec![Diagnostic::error(format!(
            "could not read '{}': {err}",
            path.display()
        ))]
    })?;

    // An explicit `file_path` wins over the directory derived from the input path,
    // matching mjml: `filePath` is what includes resolve against, whatever file the
    // source was actually read from.
    let derived = path.parent().unwrap_or_else(|| Path::new("."));
    let root_dir = options
        .file_path
        .as_deref()
        .map_or(derived, |explicit| include_base(explicit));

    render_source(&source, options, loader, Some(root_dir))
}

/// Format MJML `source` into canonical MJML source text.
///
/// Parses `source` and pretty-prints the resulting tree with [`mjmx_formatter`].
/// This is the programmatic front door for the `mjmx fmt` tool. Formatting is a
/// pure source-to-source transform: the output is idempotent under re-formatting,
/// reparse-equivalent to the input, and render-equivalent to it.
///
/// # Errors
///
/// Returns every parse [`Diagnostic`] if `source` is malformed; a well-formed
/// document always formats successfully.
pub fn format(source: &str, options: &FormatOptions) -> Result<String, Vec<Diagnostic>> {
    let allocator = Allocator::new();
    let parsed = mjmx_parser::parse(source, &allocator);
    if !parsed.errors.is_empty() {
        return Err(parsed.errors);
    }
    Ok(mjmx_formatter::format(&parsed.document, options))
}

/// Migrate MJML **v3** `source` to v4, returning migrated MJML *source*.
///
/// Parses `source`, runs the [`mjmx_transformer::migrate`] AST pass (a faithful port
/// of mjml's own `mjml-migrate`), and serializes the result back with
/// [`mjmx_formatter`] — so the output is canonical, valid v4 MJML that this
/// renderer accepts. Use [`migrate_with_notes`] to also see what changed.
///
/// Migration is meant to be run **once, on a v3 document**: some rules (unwrapping
/// `<mj-container>`, rebuilding `<mj-social>` children) are destructive by design,
/// matching mjml. See [`mjmx_transformer::migrate`] for the full rule table.
///
/// # Errors
///
/// Returns every parse [`Diagnostic`] if `source` is malformed.
pub fn migrate(source: &str) -> Result<String, Vec<Diagnostic>> {
    migrate_with_notes(source, &FormatOptions::default()).map(|(migrated, _)| migrated)
}

/// [`migrate`], plus the diagnostics describing every change made.
///
/// The notes are advisory ([`Severity::Advice`]) except for constructs that could
/// not be migrated (dropped v3-only tags, a `<mj-hero>` with no `<mj-hero-content>`),
/// which are [`Severity::Warning`]. Migration itself never fails once the document
/// parses.
///
/// # Errors
///
/// Returns every parse [`Diagnostic`] if `source` is malformed.
pub fn migrate_with_notes(
    source: &str,
    options: &FormatOptions,
) -> Result<(String, Vec<Diagnostic>), Vec<Diagnostic>> {
    let allocator = Allocator::new();
    let parsed = mjmx_parser::parse(source, &allocator);
    if !parsed.errors.is_empty() {
        return Err(parsed.errors);
    }
    let builder = mjmx_ast::AstBuilder::new(&allocator);
    let (document, notes) = mjmx_transformer::migrate(&builder, &parsed.document);
    Ok((mjmx_formatter::format(&document, options), notes))
}

/// Validate MJML `source` structurally, returning every diagnostic.
///
/// This is purely advisory and never fails: it parses `source`, then runs the
/// [`mjmx_linter`] rules. The result concatenates parse diagnostics (malformed
/// syntax) with linter diagnostics (structural rule violations), in that order.
/// Callers decide what to do with them — e.g. print and exit non-zero on any
/// [`Severity::Error`].
#[must_use]
pub fn lint(source: &str) -> Vec<Diagnostic> {
    let allocator = Allocator::new();
    let parsed = mjmx_parser::parse(source, &allocator);
    let mut diagnostics = parsed.errors;
    diagnostics.extend(mjmx_linter::lint(&parsed.document));
    diagnostics
}
