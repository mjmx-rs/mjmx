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

pub use mjmx_codegen::{Mapping, RenderContext, RenderOptions, SourceMap, Span};
pub use mjmx_diagnostics::{Diagnostic, Severity};
pub use mjmx_formatter::FormatOptions;

/// Render MJML `source` to HTML using the given [`RenderOptions`].
///
/// Returns every parse or render [`Diagnostic`] on failure.
///
/// This does **not** resolve `<mj-include>`: a bare string has no base directory to
/// resolve relative paths against. Use [`render_file`] to render a file on disk with
/// includes resolved.
pub fn render(source: &str, options: &RenderOptions) -> Result<String, Vec<Diagnostic>> {
    let allocator = Allocator::new();
    let parsed = mjmx_parser::parse(source, &allocator);
    if !parsed.errors.is_empty() {
        return Err(parsed.errors);
    }
    let html = mjmx_codegen::render_document(&parsed.document, options)?;
    Ok(post_process(html, options))
}

/// Apply the whitespace post-processors selected by `options`.
///
/// `minify` and `beautify` are mutually exclusive whitespace transforms; if a
/// caller sets both, **`minify` wins** (the more destructive, size-oriented
/// transform is assumed to be the intent). When neither is set the rendered HTML
/// is returned untouched — the default render path is unchanged.
fn post_process(html: String, options: &RenderOptions) -> String {
    if options.minify {
        mjmx_minifier::minify(&html)
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
/// # Errors
///
/// As [`render`], plus a diagnostic if `options` requests `minify` or `beautify`: those
/// are whitespace rewrites applied *after* codegen, and they would silently invalidate
/// every recorded offset. Render with a sourcemap first, then post-process if you must
/// (and discard the map).
pub fn render_with_sourcemap(
    source: &str,
    options: &RenderOptions,
) -> Result<(String, SourceMap), Vec<Diagnostic>> {
    if options.minify || options.beautify {
        return Err(vec![
            Diagnostic::error("sourcemap collection is incompatible with minify/beautify").with_help(
                "minify and beautify rewrite whitespace after codegen, which shifts every mapped offset; render with a sourcemap using the default whitespace options",
            ),
        ]);
    }
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
    render_file_with(path, options, &mjmx_transformer::CombinedLoader::new())
}

/// The shared body of [`render_file`] and `render_file_allow_http`:
/// read → parse → resolve includes with `loader` → render.
fn render_file_with(
    path: &std::path::Path,
    options: &RenderOptions,
    loader: &dyn mjmx_transformer::IncludeLoader,
) -> Result<String, Vec<Diagnostic>> {
    let source = std::fs::read_to_string(path).map_err(|err| {
        vec![Diagnostic::error(format!(
            "could not read '{}': {err}",
            path.display()
        ))]
    })?;

    let root_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));

    let allocator = Allocator::new();
    let parsed = mjmx_parser::parse(&source, &allocator);
    if !parsed.errors.is_empty() {
        return Err(parsed.errors);
    }

    let builder = mjmx_ast::AstBuilder::new(&allocator);
    let (document, include_diags) =
        mjmx_transformer::resolve_includes(&builder, &parsed.document, loader, root_dir);
    // Any error-severity include diagnostic blocks rendering (missing file, cycle).
    if include_diags.iter().any(|d| d.severity == Severity::Error) {
        return Err(include_diags);
    }

    let html = mjmx_codegen::render_document(&document, options)?;
    Ok(post_process(html, options))
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
