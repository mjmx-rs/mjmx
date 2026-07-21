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
pub fn render_head(ctx: &RenderContext, title: &str) -> String {
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

    // Fonts (present because mj-text used the Ubuntu stack). Guarded from Outlook.
    let fonts = ctx.fonts();
    if !fonts.is_empty() {
        h.push_str("<!--[if !mso]><!-->");
        for font in fonts {
            h.push_str(&format!(
                "<link href=\"{}\" rel=\"stylesheet\" type=\"text/css\">",
                font.href
            ));
        }
        h.push_str("<style type=\"text/css\">");
        for font in fonts {
            // Leading/trailing whitespace mirrors mjml so the normalized token
            // stream (`> @import ... </style>`) matches.
            h.push_str(&format!("\n    @import url({});\n  ", font.href));
        }
        h.push_str("</style>");
        h.push_str("<!--<![endif]-->");
    }

    // Responsive column rules (registered by mj-column).
    let media_queries = ctx.media_queries();
    if !media_queries.is_empty() {
        h.push_str("<style type=\"text/css\">");
        for m in media_queries {
            h.push_str(&format!(
                "\n    @media only screen and (min-width:{bp}px) {{\n      .{class} {{\n        {body}\n      }}\n    }}\n  ",
                bp = ctx.breakpoint,
                class = m.class,
                body = m.body,
            ));
        }
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
    }

    // Component <style>s (none in the M4 slice, but wired for extensibility).
    for style in ctx.styles() {
        h.push_str("<style type=\"text/css\">");
        h.push_str(style);
        h.push_str("</style>");
    }

    h.push_str("</head>");
    h
}
