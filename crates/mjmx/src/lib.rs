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

pub use mjmx_codegen::{RenderContext, RenderOptions};
pub use mjmx_diagnostics::{Diagnostic, Severity};

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
    mjmx_codegen::render_document(&parsed.document, options)
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
    let (document, include_diags) = mjmx_transformer::resolve_includes(
        &builder,
        &parsed.document,
        &mjmx_transformer::LocalLoader,
        root_dir,
    );
    // Any error-severity include diagnostic blocks rendering (missing file, cycle).
    if include_diags.iter().any(|d| d.severity == Severity::Error) {
        return Err(include_diags);
    }

    mjmx_codegen::render_document(&document, options)
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
