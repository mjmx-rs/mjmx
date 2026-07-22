//! Document assembly: the crate entry point [`render_document`].
//!
//! Three steps: build the [`Semantic`] view (`<mj-head>` aggregation + attribute
//! resolution), render the body into a buffer (which side-effects the
//! [`RenderContext`], collecting responsive rules), then assemble the full HTML
//! document — doctype, `<html>`, the head built from the accumulated context and head
//! data, and the body wrapper.

use std::ops::Range;

use mjmx_ast::{Document, Element};
use mjmx_diagnostics::Diagnostic;
use mjmx_semantic::Semantic;
use mjmx_sourcemap::SourceMap;
use mjmx_syntax::MjmlTag;

use crate::components::Body;
use crate::context::{RenderContext, RenderOptions};
use crate::head::render_head;
use crate::render::Render;

/// Render a parsed [`Document`] to a full HTML string.
///
/// Returns `Err` with every diagnostic collected if the document has no `<mjml>`
/// root or a component fails to render.
pub fn render_document<'a>(
    document: &Document<'a>,
    options: &RenderOptions,
) -> Result<String, Vec<Diagnostic>> {
    render_impl(document, options, false).map(|(html, _)| html)
}

/// Render a parsed [`Document`], also collecting an output → source [`SourceMap`].
///
/// The HTML is **byte-identical** to [`render_document`]'s: mapping collection only reads
/// buffer lengths around each component's write, it never emits anything. The returned map
/// answers "which MJML element produced this byte of HTML?" — see [`mjmx_sourcemap`].
///
/// Offsets in the map are into the returned HTML string, already rebased past both the
/// document-assembly splice and the `mergeOutlookConditionnals` post-process. Any
/// whitespace post-processing a caller applies afterwards (minify/beautify) invalidates
/// them.
///
/// # Errors
///
/// As [`render_document`].
pub fn render_document_with_sourcemap<'a>(
    document: &Document<'a>,
    options: &RenderOptions,
) -> Result<(String, SourceMap), Vec<Diagnostic>> {
    let (html, map) = render_impl(document, options, true)?;
    Ok((html, map.unwrap_or_default()))
}

/// The shared body of [`render_document`] and [`render_document_with_sourcemap`].
///
/// `sourcemap` gates collection only; every byte written into the output is produced by
/// the same code on both paths.
fn render_impl<'a>(
    document: &Document<'a>,
    options: &RenderOptions,
    sourcemap: bool,
) -> Result<(String, Option<SourceMap>), Vec<Diagnostic>> {
    let root = document.root_element().ok_or_else(|| {
        vec![
            Diagnostic::error("no <mjml> root element found")
                .with_help("an MJML document must contain a single top-level <mjml> element"),
        ]
    })?;
    if root.tag != MjmlTag::Mjml {
        return Err(vec![
            Diagnostic::error("top-level element is not <mjml>")
                .with_help("wrap the document in a single <mjml> element"),
        ]);
    }

    // Semantic analysis: aggregate <mj-head> and set up attribute resolution. Head
    // warnings are non-fatal and do not abort the render. A caller-supplied font table
    // replaces the built-in one before any <mj-font> is folded in.
    let (semantic, _diagnostics) = Semantic::build_with_fonts(document, options.fonts.as_deref());

    let mut ctx = RenderContext::new(options.clone(), semantic);
    if sourcemap {
        ctx.enable_sourcemap();
    }
    // A <mj-breakpoint> overrides the option-supplied breakpoint.
    if let Some(bp) = ctx.semantic.head().breakpoint {
        ctx.breakpoint = bp;
    }
    // mjml default: lang "und", dir "auto"; overridden by <mjml> attributes.
    if let Some(lang) = root.attr("lang") {
        ctx.lang = lang.to_string();
    }
    if let Some(dir) = root.attr("dir") {
        ctx.dir = dir.to_string();
    }
    let title = ctx.semantic.head().title.clone().unwrap_or_default();

    // Pass 1: render the body, collecting head requirements into `ctx`.
    let mut body_buf = String::new();
    if let Some(body) = body_element(root) {
        crate::render::traced(body, &mut ctx, &mut body_buf, |ctx, buf| {
            Body::new(body).render(ctx, buf)
        })
        .map_err(|d| vec![d])?;
    }

    // Pass 2: assemble the document around the collected context.
    let mut out = String::new();
    // mjml's `beforeDoctype`: `<mj-raw position="file-start">` siblings of `<mj-body>`
    // are emitted ahead of the doctype (joined by a newline), for XML prologs and the
    // like.
    let file_start = &ctx.semantic.head().file_start_raws;
    if !file_start.is_empty() {
        out.push_str(&file_start.join("\n"));
        out.push('\n');
    }
    out.push_str("<!doctype html>\n");
    out.push_str(&format!(
        "<html lang=\"{lang}\" dir=\"{dir}\" xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:v=\"urn:schemas-microsoft-com:vml\" xmlns:o=\"urn:schemas-microsoft-com:office:office\">",
        lang = ctx.lang,
        dir = ctx.dir,
    ));
    out.push_str(&render_head(&ctx, &title, &body_buf));
    // mjml's `setBackgroundColor` from <mj-body background-color> lands here, appended
    // after the always-present word-spacing declaration.
    let body_background = match &ctx.body_background_color {
        Some(color) => format!("background-color:{color};"),
        None => String::new(),
    };
    out.push_str(&format!(
        "<body style=\"word-spacing:normal;{body_background}\">"
    ));
    // Where the body buffer lands in the assembled document: mappings were recorded
    // against `body_buf`, so this is the delta that rebases them.
    let body_offset = out.len();
    out.push_str(&body_buf);
    out.push_str("</body></html>");

    let (html, deletions) = merge_outlook_conditionals(&out);
    let map = ctx.take_sourcemap().map(|mut builder| {
        builder.shift(body_offset);
        builder.remap_deletions(&deletions);
        builder.finish()
    });
    Ok((html, map))
}

/// Fuse adjacent Outlook conditional comments, mirroring mjml's
/// `mergeOutlookConditionnals` post-process: every `<![endif]-->` immediately followed
/// (across only whitespace) by `<!--[if mso | IE]>` is deleted, so two back-to-back
/// `[if mso | IE]` blocks collapse into one. Layout components (wrapper, group, nested
/// sections) emit their ghost tables as independent blocks and rely on this pass to
/// reproduce mjml's seams byte-for-byte. Blocks under other conditions (`[if mso]`,
/// `[if lte mso 11]`, `[if !mso]`) never match and are left intact.
///
/// Also returns every byte range it deleted, in input coordinates, ascending and
/// non-overlapping — the source map uses them to rebase recorded output offsets onto the
/// post-processed HTML. The returned string is exactly what the pass has always produced;
/// the ranges are pure bookkeeping.
fn merge_outlook_conditionals(html: &str) -> (String, Vec<Range<usize>>) {
    const END: &str = "<![endif]-->";
    const OPEN: &str = "<!--[if mso | IE]>";
    let mut out = String::with_capacity(html.len());
    let mut deletions = Vec::new();
    let mut rest = html;
    // Offset of `rest` within `html`, so deletions are reported in input coordinates.
    let mut base = 0usize;
    while let Some(pos) = rest.find(END) {
        // Look past the `<![endif]-->` and any whitespace for a fused `[if mso | IE]`.
        let after = &rest[pos + END.len()..];
        let trimmed = after.trim_start();
        if let Some(fused) = trimmed.strip_prefix(OPEN) {
            // Drop both the end marker and the following open marker (and the gap).
            out.push_str(&rest[..pos]);
            let consumed = rest.len() - fused.len();
            deletions.push(base + pos..base + consumed);
            base += consumed;
            rest = fused;
        } else {
            // Keep this end marker and continue scanning after it.
            out.push_str(&rest[..pos + END.len()]);
            base += pos + END.len();
            rest = after;
        }
    }
    out.push_str(rest);
    (out, deletions)
}

/// Find the `<mj-body>` child of the `<mjml>` root.
fn body_element<'a>(root: &'a Element<'a>) -> Option<&'a Element<'a>> {
    root.element_children().find(|el| el.tag == MjmlTag::MjBody)
}
