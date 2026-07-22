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
use mjmx_semantic::{HtmlAttributeRule, Semantic};
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
    render_impl(document, options, false).map(|rendered| rendered.html)
}

/// [`render_document`], also returning the two head buckets that codegen hands back as
/// *data* for a caller to apply to the assembled document.
///
/// The HTML is byte-identical to [`render_document`]'s. Neither bucket is emitted as
/// markup by codegen:
///
/// - the `<mj-style inline="inline">` CSS bodies (mjml passes them to juice as
///   `extraCss` with `insertPreservedExtraCss: false`);
/// - the `<mj-html-attributes>` rules (mjml's `globalData.htmlAttributes`, applied by a
///   cheerio pass over the body).
///
/// The caller — [`mjmx`](../mjmx/index.html) — applies the html-attributes rules and
/// then the inline CSS (mjml's order: cheerio before juice), before any whitespace
/// post-processing. Both empty means the HTML is final.
///
/// # Errors
///
/// As [`render_document`].
pub fn render_document_with_inline_css<'a>(
    document: &Document<'a>,
    options: &RenderOptions,
) -> Result<(String, Vec<String>, Vec<HtmlAttributeRule>), Vec<Diagnostic>> {
    render_impl(document, options, false)
        .map(|rendered| (rendered.html, rendered.inline_css, rendered.html_attributes))
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
/// As [`render_document`], plus a refusal when the render would be post-processed at
/// all — see [`sourcemap_refusal`].
pub fn render_document_with_sourcemap<'a>(
    document: &Document<'a>,
    options: &RenderOptions,
) -> Result<(String, SourceMap), Vec<Diagnostic>> {
    let rendered = render_impl(document, options, true)?;
    Ok((rendered.html, rendered.sourcemap.unwrap_or_default()))
}

/// Everything a render produces, before any post-processing.
struct Rendered {
    html: String,
    /// Present only on the [`render_document_with_sourcemap`] path.
    sourcemap: Option<SourceMap>,
    /// The `<mj-style inline="inline">` bodies, in source order.
    inline_css: Vec<String>,
    /// The `<mj-html-attributes>` rules, in source order.
    html_attributes: Vec<HtmlAttributeRule>,
}

/// Why a source map cannot be produced for this render, if it cannot.
///
/// Every reason is the same reason: something rewrites the HTML *after* codegen has
/// recorded its offsets, so every mapping would silently point at the wrong bytes. The
/// document-shaped checks cannot live at the API boundary, because whether CSS inlining
/// or the html-attributes pass runs is a property of the **document** (does it have an
/// inline `<mj-style>` / an `<mj-html-attributes>`?) and is only known once semantic
/// analysis has partitioned the head buckets.
fn sourcemap_refusal(
    options: &RenderOptions,
    inline_css: &[String],
    html_attributes: &[HtmlAttributeRule],
) -> Option<Diagnostic> {
    if options.minify || options.beautify {
        return Some(
            Diagnostic::error("sourcemap collection is incompatible with minify/beautify").with_help(
                "minify and beautify rewrite whitespace after codegen, which shifts every mapped offset; render with a sourcemap using the default whitespace options",
            ),
        );
    }
    if !inline_css.is_empty() {
        return Some(
            Diagnostic::error("sourcemap collection is incompatible with <mj-style inline=\"inline\">").with_help(
                "CSS inlining runs after codegen: it rewrites style attributes and inserts new ones, which shifts every mapped offset; remove the inline style block, or render without a sourcemap",
            ),
        );
    }
    if !html_attributes.is_empty() {
        return Some(
            Diagnostic::error("sourcemap collection is incompatible with <mj-html-attributes>").with_help(
                "the html-attributes pass runs after codegen: it inserts and rewrites attributes on matched elements, which shifts every mapped offset; remove the <mj-html-attributes> block, or render without a sourcemap",
            ),
        );
    }
    None
}

/// The shared body of [`render_document`], [`render_document_with_inline_css`] and
/// [`render_document_with_sourcemap`].
///
/// `sourcemap` gates collection only; every byte written into the output is produced by
/// the same code on both paths.
fn render_impl<'a>(
    document: &Document<'a>,
    options: &RenderOptions,
    sourcemap: bool,
) -> Result<Rendered, Vec<Diagnostic>> {
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

    // The two head buckets that leave codegen as data rather than markup.
    let inline_css = semantic.head().inline_styles.clone();
    let html_attributes = semantic.head().html_attributes.clone();
    if sourcemap && let Some(diagnostic) = sourcemap_refusal(options, &inline_css, &html_attributes)
    {
        return Err(vec![diagnostic]);
    }

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
    Ok(Rendered {
        html,
        sourcemap: map,
        inline_css,
        html_attributes,
    })
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
