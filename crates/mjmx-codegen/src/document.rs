//! Document assembly: the crate entry point [`render_document`].
//!
//! Three steps: build the [`Semantic`] view (`<mj-head>` aggregation + attribute
//! resolution), render the body into a buffer (which side-effects the
//! [`RenderContext`], collecting responsive rules), then assemble the full HTML
//! document — doctype, `<html>`, the head built from the accumulated context and head
//! data, and the body wrapper.

use std::fmt::Write;
use std::ops::Range;

use mjmx_ast::{Document, Element};
use mjmx_diagnostics::Diagnostic;
use mjmx_semantic::{HtmlAttributeRule, Semantic};
use mjmx_sourcemap::SourceMap;
use mjmx_syntax::MjmlTag;

use crate::components::Body;
use crate::context::{RenderContext, RenderOptions};
use crate::custom::ComponentLookup;
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
    render_impl(document, options, false, None).map(|rendered| rendered.html)
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
    render_document_with_inline_css_and_components(document, options, None)
}

/// [`render_document_with_inline_css`] with a custom-component registry.
///
/// A registered tag the parser left as [`MjmlTag::Unknown`] renders through its
/// [`CustomComponent`](crate::CustomComponent); with `registry` = `None` this is exactly
/// [`render_document_with_inline_css`].
///
/// # Errors
///
/// As [`render_document`].
pub fn render_document_with_inline_css_and_components<'a>(
    document: &Document<'a>,
    options: &RenderOptions,
    registry: Option<&'a dyn ComponentLookup>,
) -> Result<(String, Vec<String>, Vec<HtmlAttributeRule>), Vec<Diagnostic>> {
    render_impl(document, options, false, registry)
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
    let rendered = render_impl(document, options, true, None)?;
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

/// Bytes reserved for everything the assembly pass writes around the rendered body:
/// the doctype, the `<html>` tag and the whole `<head>`.
///
/// The head is the only part whose size is unknown at that point (the body buffer is
/// already complete). Across the repository's fixtures a rendered head runs 1.3–5.8 KB,
/// median ~2 KB, so this covers all but the largest in the initial allocation and costs
/// one doubling when it does not.
pub(crate) const HEAD_CAPACITY: usize = 4 * 1024;

/// A starting capacity for the body buffer, estimated from the source length.
///
/// Rendered output runs 3.6–33× the MJML source across the fixtures (median 12.7×), the
/// spread being how much boilerplate a component expands into. Eight is deliberately
/// below the median: overshooting wastes memory on every render, while undershooting
/// costs at most a doubling or two on the tail. The point is only to skip the dozen
/// reallocate-and-copy rounds a buffer growing from zero would otherwise pay.
///
/// Clamped at 512 KiB so a pathologically large source cannot reserve tens of megabytes
/// up front; past that, amortized doubling is cheap enough.
fn body_capacity(document: &Document<'_>) -> usize {
    const MAX: usize = 512 * 1024;
    let source_len = document.span.end.saturating_sub(document.span.start) as usize;
    source_len.saturating_mul(8).clamp(HEAD_CAPACITY, MAX)
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
    registry: Option<&'a dyn ComponentLookup>,
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
    if let Some(registry) = registry {
        ctx.set_components(registry);
    }
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
    let mut body_buf = String::with_capacity(body_capacity(document));
    if let Some(body) = body_element(root) {
        crate::render::traced(body, &mut ctx, &mut body_buf, |ctx, buf| {
            Body::new(body).render(ctx, buf)
        })
        .map_err(|d| vec![d])?;
    }

    // Pass 2: assemble the document around the collected context. The body is already
    // rendered, so the only unknown left is the head, which `HEAD_CAPACITY` covers.
    let mut out = String::with_capacity(body_buf.len() + HEAD_CAPACITY);
    // mjml's `beforeDoctype`: `<mj-raw position="file-start">` siblings of `<mj-body>`
    // are emitted ahead of the doctype (joined by a newline), for XML prologs and the
    // like.
    let file_start = &ctx.semantic.head().file_start_raws;
    if !file_start.is_empty() {
        out.push_str(&file_start.join("\n"));
        out.push('\n');
    }
    out.push_str("<!doctype html>\n");
    let _ = write!(
        out,
        "<html lang=\"{lang}\" dir=\"{dir}\" xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:v=\"urn:schemas-microsoft-com:vml\" xmlns:o=\"urn:schemas-microsoft-com:office:office\">",
        lang = ctx.lang,
        dir = ctx.dir,
    );
    out.push_str(&render_head(&ctx, &title, &body_buf));
    // mjml's `setBackgroundColor` from <mj-body background-color> lands here, appended
    // after the always-present word-spacing declaration.
    let body_background = match &ctx.body_background_color {
        Some(color) => format!("background-color:{color};"),
        None => String::new(),
    };
    if ctx.options.compat.is_v5() {
        // mjml 5 generates <body> from mj-body rather than the global skeleton, so the
        // element's own `id` and `css-class` land here. The two spaces after `<body` are
        // upstream's: its template interpolates an `id` slot that is empty when unset,
        // and mjmx reproduces the byte rather than tidying it, so output stays diffable
        // against mjml's.
        let id = match &ctx.body_id {
            Some(id) => format!("id=\"{id}\" "),
            None => String::new(),
        };
        let class = match &ctx.body_css_class {
            Some(class) => format!("class=\"{class}\" "),
            None => String::new(),
        };
        out.push_str("<body  ");
        out.push_str(&id);
        out.push_str(&class);
        out.push_str("style=\"word-spacing:normal;");
        out.push_str(&body_background);
        out.push_str("\">");
    } else {
        out.push_str("<body style=\"word-spacing:normal;");
        out.push_str(&body_background);
        out.push_str("\">");
    }
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

/// The end of an Outlook conditional comment.
const END_MARKER: &str = "<![endif]-->";
/// The opening of the `[if mso | IE]` conditional [`merge_outlook_conditionals`] fuses.
const OPEN_MARKER: &str = "<!--[if mso | IE]>";

/// The next [`END_MARKER`] in `html` at or after `from`.
///
/// `str::find` with a multi-byte needle builds a fresh two-way searcher on every call,
/// and mjml-shaped output carries one end marker per section, column and ghost table —
/// so the setup cost was being paid dozens of times per document, and dominated this
/// pass. Searching instead for the marker's `[`, a byte that is rare in email HTML,
/// goes through the single-byte path (a SIMD `memchr` in `core`) and confirms the full
/// marker only at the handful of candidate positions.
fn find_endif(html: &str, from: usize) -> Option<usize> {
    /// Byte offset of `[` within `<![endif]-->`.
    const BRACKET: usize = 2;
    let bytes = html.as_bytes();
    let mut at = from;
    while let Some(offset) = html[at..].find('[') {
        let bracket = at + offset;
        // `[` is one byte, so `bracket + 1` is always a char boundary.
        at = bracket + 1;
        // `start >= from` keeps the result at or after where the caller asked to look:
        // the lookback reaches two bytes behind the `[`, which could otherwise straddle
        // the boundary and report a marker the caller has already passed.
        if let Some(start) = bracket.checked_sub(BRACKET)
            && start >= from
            && bytes[start..].starts_with(END_MARKER.as_bytes())
        {
            return Some(start);
        }
    }
    None
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
    let mut out = String::with_capacity(html.len());
    let mut deletions = Vec::new();
    // Start of the text not yet copied into `out`, and where to look for the next end
    // marker. They differ only across a deletion: everything between them is dropped.
    let mut copied = 0usize;
    let mut scan = 0usize;
    while let Some(pos) = find_endif(html, scan) {
        // Look past the `<![endif]-->` and any whitespace for a fused `[if mso | IE]`.
        let after = &html[pos + END_MARKER.len()..];
        let trimmed = after.trim_start();
        if let Some(fused) = trimmed.strip_prefix(OPEN_MARKER) {
            // Drop both the end marker and the following open marker (and the gap).
            out.push_str(&html[copied..pos]);
            let resume = html.len() - fused.len();
            deletions.push(pos..resume);
            copied = resume;
            scan = resume;
        } else {
            // Keep this end marker and continue scanning after it.
            scan = pos + END_MARKER.len();
        }
    }
    out.push_str(&html[copied..]);
    (out, deletions)
}

/// Find the `<mj-body>` child of the `<mjml>` root.
fn body_element<'a>(root: &'a Element<'a>) -> Option<&'a Element<'a>> {
    root.element_children().find(|el| el.tag == MjmlTag::MjBody)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scan must find a marker wherever it sits, including at offset 0 — where the
    /// `[` is at index 2 and the `checked_sub` guard is what keeps the lookback in range.
    #[test]
    fn find_endif_locates_markers_at_any_offset() {
        assert_eq!(find_endif("<![endif]-->", 0), Some(0));
        assert_eq!(find_endif("ab<![endif]-->", 0), Some(2));
        assert_eq!(find_endif("<![endif]--><![endif]-->", 1), Some(12));
        assert_eq!(find_endif("<![endif]-->", 1), None);
    }

    /// A `[` that is not part of a marker must not be mistaken for one — including one in
    /// the first two bytes, where the lookback would underflow.
    #[test]
    fn find_endif_ignores_brackets_that_are_not_markers() {
        assert_eq!(find_endif("[", 0), None);
        assert_eq!(find_endif("a[b", 0), None);
        assert_eq!(find_endif("<!--[if mso | IE]>", 0), None);
        assert_eq!(find_endif("[![endif]-->", 0), None);
        assert_eq!(find_endif("text [1] more [2]", 0), None);
    }

    /// Multi-byte text around a marker must not break the byte-wise lookback: the scan
    /// only ever indexes at an ASCII `[`, so the slice bounds stay on char boundaries.
    #[test]
    fn find_endif_handles_multibyte_text() {
        assert_eq!(
            find_endif("héllo — ☃<![endif]-->", 0),
            Some("héllo — ☃".len())
        );
        assert_eq!(find_endif("emoji 🎉 [not a marker]", 0), None);
    }

    /// Two `[if mso | IE]` blocks separated by only whitespace fuse into one, and the
    /// deleted range covers exactly the end marker, the gap and the open marker.
    #[test]
    fn adjacent_conditionals_fuse() {
        let html = "<a><!--[if mso | IE]>x<![endif]-->\n  <!--[if mso | IE]>y<![endif]--></a>";
        let (out, deletions) = merge_outlook_conditionals(html);
        assert_eq!(out, "<a><!--[if mso | IE]>xy<![endif]--></a>");
        assert_eq!(deletions.len(), 1);
        assert_eq!(
            &html[deletions[0].clone()],
            "<![endif]-->\n  <!--[if mso | IE]>"
        );
    }

    /// A block under any other condition is left alone, as are lone end markers.
    #[test]
    fn unrelated_conditionals_are_untouched() {
        for html in [
            "<!--[if mso]>x<![endif]--><!--[if mso]>y<![endif]-->",
            "<!--[if mso | IE]>x<![endif]--><p>text</p><!--[if mso | IE]>y<![endif]-->",
            "<p>no conditionals at all</p>",
        ] {
            let (out, deletions) = merge_outlook_conditionals(html);
            assert_eq!(out, html);
            assert!(deletions.is_empty());
        }
    }

    /// Three or more back-to-back blocks collapse pairwise into a single block, and every
    /// reported range is ascending and non-overlapping (the source map relies on that).
    #[test]
    fn runs_of_conditionals_collapse_with_ordered_deletions() {
        let html = "<!--[if mso | IE]>a<![endif]--><!--[if mso | IE]>b<![endif]-->\
                    <!--[if mso | IE]>c<![endif]-->";
        let (out, deletions) = merge_outlook_conditionals(html);
        assert_eq!(out, "<!--[if mso | IE]>abc<![endif]-->");
        assert_eq!(deletions.len(), 2);
        assert!(deletions.windows(2).all(|w| w[0].end <= w[1].start));
    }
}
