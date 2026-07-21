//! Document assembly: the crate entry point [`render_document`].
//!
//! Three steps: build the [`Semantic`] view (`<mj-head>` aggregation + attribute
//! resolution), render the body into a buffer (which side-effects the
//! [`RenderContext`], collecting responsive rules), then assemble the full HTML
//! document — doctype, `<html>`, the head built from the accumulated context and head
//! data, and the body wrapper.

use mjmx_ast::{Document, Element};
use mjmx_diagnostics::Diagnostic;
use mjmx_semantic::Semantic;
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
    // warnings are non-fatal and do not abort the render.
    let (semantic, _diagnostics) = Semantic::build(document);

    let mut ctx = RenderContext::new(options.clone(), semantic);
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
        Body::new(body)
            .render(&mut ctx, &mut body_buf)
            .map_err(|d| vec![d])?;
    }

    // Pass 2: assemble the document around the collected context.
    let mut out = String::new();
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
    out.push_str(&body_buf);
    out.push_str("</body></html>");
    Ok(merge_outlook_conditionals(&out))
}

/// Fuse adjacent Outlook conditional comments, mirroring mjml's
/// `mergeOutlookConditionnals` post-process: every `<![endif]-->` immediately followed
/// (across only whitespace) by `<!--[if mso | IE]>` is deleted, so two back-to-back
/// `[if mso | IE]` blocks collapse into one. Layout components (wrapper, group, nested
/// sections) emit their ghost tables as independent blocks and rely on this pass to
/// reproduce mjml's seams byte-for-byte. Blocks under other conditions (`[if mso]`,
/// `[if lte mso 11]`, `[if !mso]`) never match and are left intact.
fn merge_outlook_conditionals(html: &str) -> String {
    const END: &str = "<![endif]-->";
    const OPEN: &str = "<!--[if mso | IE]>";
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find(END) {
        // Look past the `<![endif]-->` and any whitespace for a fused `[if mso | IE]`.
        let after = &rest[pos + END.len()..];
        let trimmed = after.trim_start();
        if let Some(fused) = trimmed.strip_prefix(OPEN) {
            // Drop both the end marker and the following open marker (and the gap).
            out.push_str(&rest[..pos]);
            rest = fused;
        } else {
            // Keep this end marker and continue scanning after it.
            out.push_str(&rest[..pos + END.len()]);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// Find the `<mj-body>` child of the `<mjml>` root.
fn body_element<'a>(root: &'a Element<'a>) -> Option<&'a Element<'a>> {
    root.element_children().find(|el| el.tag == MjmlTag::MjBody)
}
