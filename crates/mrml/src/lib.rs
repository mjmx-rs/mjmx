//! `mrml` — the umbrella crate: MJML source text into responsive HTML.
//!
//! This is the one-call front door over the pipeline. [`render`] allocates an arena,
//! parses the source with [`mrml_parser`], and — if parsing produced no errors —
//! renders the AST with [`mrml_codegen`]. Parse and render diagnostics are surfaced
//! as `Err(Vec<Diagnostic>)`.
//!
//! ```
//! let html = mrml::render_default(
//!     "<mjml><mj-body><mj-section><mj-column><mj-text>Hi</mj-text></mj-column></mj-section></mj-body></mjml>",
//! )
//! .unwrap();
//! assert!(html.contains("Hi"));
//! assert!(html.starts_with("<!doctype html>"));
//! ```

use mrml_allocator::Allocator;

pub use mrml_codegen::{RenderContext, RenderOptions};
pub use mrml_diagnostics::{Diagnostic, Severity};

/// Render MJML `source` to HTML using the given [`RenderOptions`].
///
/// Returns every parse or render [`Diagnostic`] on failure.
pub fn render(source: &str, options: &RenderOptions) -> Result<String, Vec<Diagnostic>> {
    let allocator = Allocator::new();
    let parsed = mrml_parser::parse(source, &allocator);
    if !parsed.errors.is_empty() {
        return Err(parsed.errors);
    }
    mrml_codegen::render_document(&parsed.document, options)
}

/// Render MJML `source` to HTML with [`RenderOptions::default`].
pub fn render_default(source: &str) -> Result<String, Vec<Diagnostic>> {
    render(source, &RenderOptions::default())
}

/// Validate MJML `source` structurally, returning every diagnostic.
///
/// This is purely advisory and never fails: it parses `source`, then runs the
/// [`mrml_linter`] rules. The result concatenates parse diagnostics (malformed
/// syntax) with linter diagnostics (structural rule violations), in that order.
/// Callers decide what to do with them — e.g. print and exit non-zero on any
/// [`Severity::Error`].
#[must_use]
pub fn lint(source: &str) -> Vec<Diagnostic> {
    let allocator = Allocator::new();
    let parsed = mrml_parser::parse(source, &allocator);
    let mut diagnostics = parsed.errors;
    diagnostics.extend(mrml_linter::lint(&parsed.document));
    diagnostics
}
