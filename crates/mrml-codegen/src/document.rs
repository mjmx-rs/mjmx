//! Document assembly: the crate entry point [`render_document`].
//!
//! Two passes: first render the body into a buffer (which side-effects the
//! [`RenderContext`], collecting fonts and responsive rules), then assemble the full
//! HTML document — doctype, `<html>`, the head built from the accumulated context,
//! and the body wrapper.

use mrml_ast::{Document, Element};
use mrml_diagnostics::Diagnostic;
use mrml_syntax::MjmlTag;

use crate::components::Body;
use crate::context::{RenderContext, RenderOptions};
use crate::head::render_head;
use crate::render::Render;

/// Render a parsed [`Document`] to a full HTML string.
///
/// Returns `Err` with every diagnostic collected if the document has no `<mjml>`
/// root or a component fails to render.
pub fn render_document(
    document: &Document<'_>,
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

    let mut ctx = RenderContext::new(options.clone());
    // mjml default: lang "und", dir "auto"; overridden by <mjml> attributes.
    if let Some(lang) = root.attr("lang") {
        ctx.lang = lang.to_string();
    }
    if let Some(dir) = root.attr("dir") {
        ctx.dir = dir.to_string();
    }
    let title = mj_title(root).unwrap_or_default();

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
    out.push_str(&render_head(&ctx, &title));
    out.push_str("<body style=\"word-spacing:normal;\">");
    out.push_str(&body_buf);
    out.push_str("</body></html>");
    Ok(out)
}

/// Find the `<mj-body>` child of the `<mjml>` root.
fn body_element<'a>(root: &'a Element<'a>) -> Option<&'a Element<'a>> {
    root.element_children().find(|el| el.tag == MjmlTag::MjBody)
}

/// The `<mj-title>` text, if a `<mj-head>` provides one.
fn mj_title(root: &Element<'_>) -> Option<String> {
    let head = root
        .element_children()
        .find(|el| el.tag == MjmlTag::MjHead)?;
    let title = head
        .element_children()
        .find(|el| el.tag == MjmlTag::MjTitle)?;
    let mut text = String::new();
    for child in title.children() {
        if let mrml_ast::Node::Text(t) = child {
            text.push_str(t.value.as_str());
        }
    }
    Some(text)
}
