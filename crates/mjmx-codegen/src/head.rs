//! Head assembly.
//!
//! The `<head>` is emitted *after* the body pass, from what the body registered in
//! the [`RenderContext`]: font `@import`s (mj-text registered Ubuntu) and the
//! `.mj-column-per-*` responsive rules (mj-column registered them). The static
//! boilerplate (reset styles, mso/office settings, the outlook group-fix) is copied
//! verbatim from mjml 4.18.0's standard output.

use crate::context::RenderContext;

// The mjml reset stylesheet, verbatim (leading/trailing whitespace preserved so the
// whitespace-normalized golden comparison lines up with mjml's own formatting).
const RESET_CSS: &str = r#"
    #outlook a {
      padding: 0;
    }

    body {
      margin: 0;
      padding: 0;
      -webkit-text-size-adjust: 100%;
      -ms-text-size-adjust: 100%;
    }

    table,
    td {
      border-collapse: collapse;
      mso-table-lspace: 0pt;
      mso-table-rspace: 0pt;
    }

    img {
      border: 0;
      height: auto;
      line-height: 100%;
      outline: none;
      text-decoration: none;
      -ms-interpolation-mode: bicubic;
    }

    p {
      display: block;
      margin: 13px 0;
    }

  "#;

// The mso OfficeDocumentSettings block, verbatim.
const MSO_OFFICE: &str = r#"<!--[if mso]>
    <noscript>
    <xml>
    <o:OfficeDocumentSettings>
      <o:AllowPNG/>
      <o:PixelsPerInch>96</o:PixelsPerInch>
    </o:OfficeDocumentSettings>
    </xml>
    </noscript>
    <![endif]-->"#;

// The Outlook <= 11 group-fix, verbatim.
const MSO_GROUP_FIX: &str = r#"<!--[if lte mso 11]>
    <style type="text/css">
      .mj-outlook-group-fix { width:100% !important; }
    </style>
    <![endif]-->"#;

/// Assemble the `<head>` from the accumulated render context.
///
/// `body` is the already-rendered body markup; it is scanned so that a web font is
/// imported only when its family name is actually referenced, matching mjml's
/// font-collection behaviour.
pub fn render_head(ctx: &RenderContext, title: &str, body: &str) -> String {
    let mut h = String::new();
    h.push_str("<head>");
    h.push_str(&format!("<title>{title}</title>"));

    // X-UA-Compatible, guarded from Outlook.
    h.push_str("<!--[if !mso]><!-->");
    h.push_str("<meta http-equiv=\"X-UA-Compatible\" content=\"IE=edge\">");
    h.push_str("<!--<![endif]-->");
    h.push_str("<meta http-equiv=\"Content-Type\" content=\"text/html; charset=UTF-8\">");
    h.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">");

    // Reset stylesheet.
    h.push_str("<style type=\"text/css\">");
    h.push_str(RESET_CSS);
    h.push_str("</style>");

    h.push_str(MSO_OFFICE);
    h.push_str(MSO_GROUP_FIX);

    // Web fonts: a face from the head's font table (mjml built-ins + <mj-font>) is
    // imported only if its family name appears in the rendered body — or in the
    // `<mj-style inline="inline">` CSS, which mjml's `buildFontsTags(content,
    // inlineStyle, fonts)` scans as a second source (helpers/fonts.js:11). That bucket
    // is never emitted as a `<style>` tag, so it has to be checked separately.
    let inline_styles = &ctx.semantic.head().inline_styles;
    let fonts: Vec<_> = ctx
        .semantic
        .head()
        .fonts
        .iter()
        .filter(|f| {
            body.contains(f.name.as_ref())
                || inline_styles.iter().any(|s| s.contains(f.name.as_ref()))
        })
        .collect();
    if !fonts.is_empty() {
        h.push_str("<!--[if !mso]><!-->");
        for font in &fonts {
            h.push_str(&format!(
                "<link href=\"{}\" rel=\"stylesheet\" type=\"text/css\">",
                font.href
            ));
        }
        h.push_str("<style type=\"text/css\">");
        for font in &fonts {
            // Leading/trailing whitespace mirrors mjml so the normalized token
            // stream (`> @import ... </style>`) matches.
            h.push_str(&format!("\n    @import url({});\n  ", font.href));
        }
        h.push_str("</style>");
        h.push_str("<!--<![endif]-->");
    }

    // Responsive column rules (registered by mj-column / mj-group). mjml groups every
    // class rule inside a single `@media` block, separated by a blank line.
    let media_queries = ctx.media_queries();
    if !media_queries.is_empty() {
        h.push_str("<style type=\"text/css\">");
        h.push_str(&format!(
            "\n    @media only screen and (min-width:{bp}px) {{\n",
            bp = ctx.breakpoint
        ));
        for (i, m) in media_queries.iter().enumerate() {
            if i > 0 {
                h.push('\n');
            }
            h.push_str(&format!(
                "      .{class} {{\n        {body}\n      }}\n",
                class = m.class,
                body = m.body,
            ));
        }
        h.push_str("    }\n  ");
        h.push_str("</style>");

        h.push_str(&format!(
            "<style media=\"screen and (min-width:{bp}px)\">",
            bp = ctx.breakpoint
        ));
        for m in media_queries {
            h.push_str(&format!(
                "\n    .moz-text-html .{class} {{\n      {body}\n    }}\n  ",
                class = m.class,
                body = m.body,
            ));
        }
        h.push_str("</style>");

        // mjml's `printerSupport` (mediaQueries.js:34-42): a third block replaying the
        // *base* (unprefixed) rules under `@media only print`, right after the
        // Thunderbird block. Off by default, and — as upstream — only reachable when
        // there are column rules at all, since the whole helper returns '' otherwise.
        if ctx.options.printer_support {
            h.push_str("<style type=\"text/css\">");
            h.push_str("\n    @media only print {\n");
            for (i, m) in media_queries.iter().enumerate() {
                if i > 0 {
                    h.push('\n');
                }
                h.push_str(&format!(
                    "      .{class} {{\n        {body}\n      }}\n",
                    class = m.class,
                    body = m.body,
                ));
            }
            h.push_str("    }\n  ");
            h.push_str("</style>");
        }
    }

    // Component <style>s. mjml's `buildStyleFromComponents` concatenates *every*
    // registered component style into a single `<style type="text/css">` block, so two
    // components that both register head CSS (e.g. mj-navbar + mj-accordion) share one
    // tag rather than getting one each.
    if !ctx.styles().is_empty() {
        h.push_str("<style type=\"text/css\">");
        for style in ctx.styles() {
            h.push_str(style);
        }
        h.push_str("</style>");
    }

    // Document <mj-style> CSS, emitted verbatim (mjml's `buildStyleFromTags`, likewise
    // a single block). Only the *plain* bucket appears here: mjml feeds
    // `globalData.inlineStyle` to juice with `insertPreservedExtraCss:false`, so inline
    // CSS never reaches a `<style>` tag. Applying it to the markup is deferred to the
    // css-inline milestone.
    if !ctx.semantic.head().styles.is_empty() {
        h.push_str("<style type=\"text/css\">");
        for style in &ctx.semantic.head().styles {
            h.push_str(style);
        }
        h.push_str("</style>");
    }

    // mjml's `headRaw`: `<mj-raw>` bodies collected from `<mj-head>`, emitted verbatim
    // as the last thing in the `<head>` (joined by a newline, as mjml's skeleton does).
    let raws = &ctx.semantic.head().raws;
    for (i, raw) in raws.iter().enumerate() {
        if i > 0 {
            h.push('\n');
        }
        h.push_str(raw);
    }

    h.push_str("</head>");
    h
}
