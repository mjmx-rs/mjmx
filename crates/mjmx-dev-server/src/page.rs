//! HTML the dev server hands to the browser.
//!
//! Three pieces: the [`SHELL`] page (an `<iframe>` around the preview plus the
//! live-reload client), [`render_preview`] (the current rendered email, or a
//! readable error page when the build fails), and [`error_page`] (turns a
//! `Vec<Diagnostic>` into that error page). All output is self-contained — no CDN,
//! no external assets — so the preview works fully offline.

use std::path::Path;

use mjmx::RenderOptions;
use mjmx_diagnostics::{Diagnostic, Severity};

/// The preview shell served at `GET /`.
///
/// It frames `/preview` in an `<iframe>` and runs an inline live-reload client that
/// opens a WebSocket to `/livereload`; on a `reload` message it reloads only the
/// iframe (re-fetching `/preview`) so the outer page — and the socket — survive. A
/// small banner reports connection/build status, and the socket auto-reconnects if
/// the server restarts.
pub const SHELL: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>mjmx dev</title>
<style>
  * { box-sizing: border-box; }
  html, body { margin: 0; height: 100%; }
  body { display: flex; flex-direction: column; font-family: system-ui, sans-serif; }
  #mjmx-banner {
    flex: 0 0 auto; padding: 6px 12px; font-size: 13px; color: #fff;
    background: #555; transition: background 0.2s;
  }
  #mjmx-banner.ok { background: #2e7d32; }
  #mjmx-banner.err { background: #c62828; }
  #mjmx-frame { flex: 1 1 auto; width: 100%; border: 0; }
</style>
</head>
<body>
<div id="mjmx-banner">mjmx dev — connecting…</div>
<iframe id="mjmx-frame" src="/preview" title="preview"></iframe>
<script>
(function () {
  var banner = document.getElementById('mjmx-banner');
  var frame = document.getElementById('mjmx-frame');
  function connect() {
    var proto = location.protocol === 'https:' ? 'wss' : 'ws';
    var ws = new WebSocket(proto + '://' + location.host + '/livereload');
    ws.onopen = function () { banner.textContent = 'mjmx dev — connected'; banner.className = 'ok'; };
    ws.onmessage = function (ev) {
      if (ev.data === 'reload') {
        banner.textContent = 'mjmx dev — reloading…'; banner.className = 'ok';
        try { frame.contentWindow.location.reload(); }
        catch (e) { frame.src = '/preview?t=' + Date.now(); }
      }
    };
    ws.onclose = function () {
      banner.textContent = 'mjmx dev — disconnected, retrying…'; banner.className = 'err';
      setTimeout(connect, 1000);
    };
    ws.onerror = function () { ws.close(); };
  }
  connect();
})();
</script>
</body>
</html>
"#;

/// Render the current preview HTML for `path`.
///
/// On success this is the rendered email document (served verbatim into the
/// iframe). On any parse / include / render failure it is [`error_page`] instead, so
/// the browser always shows something actionable rather than a blank frame.
pub fn render_preview(path: &Path, options: &RenderOptions) -> String {
    match mjmx::render_file(path, options) {
        Ok(html) => html,
        Err(diagnostics) => error_page(&diagnostics),
    }
}

/// Render a `Vec<Diagnostic>` into a standalone HTML error page.
///
/// Each diagnostic is listed with its severity, message, any span labels
/// (`start..end`), and help text. Everything is HTML-escaped.
pub fn error_page(diagnostics: &[Diagnostic]) -> String {
    let mut items = String::new();
    for diagnostic in diagnostics {
        items.push_str("<li class=\"diag ");
        items.push_str(severity_class(diagnostic.severity));
        items.push_str("\"><span class=\"sev\">");
        items.push_str(&escape(&diagnostic.severity.to_string()));
        items.push_str("</span> <span class=\"msg\">");
        items.push_str(&escape(&diagnostic.message));
        items.push_str("</span>");
        for label in &diagnostic.labels {
            items.push_str("<div class=\"span\">at ");
            items.push_str(&label.span.start.to_string());
            items.push_str("..");
            items.push_str(&label.span.end.to_string());
            items.push_str(": ");
            items.push_str(&escape(&label.message));
            items.push_str("</div>");
        }
        if let Some(help) = &diagnostic.help {
            items.push_str("<div class=\"help\">help: ");
            items.push_str(&escape(help));
            items.push_str("</div>");
        }
        items.push_str("</li>");
    }

    let error_count = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let heading = if error_count == 1 {
        "1 error".to_string()
    } else if error_count > 1 {
        format!("{error_count} errors")
    } else {
        format!("{} diagnostic(s)", diagnostics.len())
    };

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>mjmx — build failed</title>
<style>
  body {{ margin: 0; padding: 24px; font-family: ui-monospace, monospace; background: #1e1e1e; color: #eee; }}
  h1 {{ font-size: 18px; color: #ff6b6b; margin: 0 0 16px; }}
  ul {{ list-style: none; padding: 0; margin: 0; }}
  .diag {{ padding: 12px 14px; margin-bottom: 10px; border-left: 4px solid #888; background: #2a2a2a; border-radius: 4px; }}
  .diag.error {{ border-color: #ff6b6b; }}
  .diag.warning {{ border-color: #ffb74d; }}
  .diag.advice {{ border-color: #64b5f6; }}
  .sev {{ text-transform: uppercase; font-size: 11px; letter-spacing: 0.05em; opacity: 0.8; }}
  .msg {{ font-weight: 600; }}
  .span, .help {{ font-size: 13px; opacity: 0.85; margin-top: 4px; }}
  .help {{ color: #81c784; }}
</style>
</head>
<body>
<h1>mjmx build failed — {heading}</h1>
<ul>{items}</ul>
</body>
</html>
"#
    )
}

/// The CSS class for a severity, used to color the diagnostic's left border.
fn severity_class(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Advice => "advice",
    }
}

/// Escape the five HTML-significant characters so diagnostic text renders as text.
fn escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mjmx_span::Span;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    #[test]
    fn shell_contains_iframe_and_livereload_client() {
        assert!(SHELL.contains("<iframe"));
        assert!(SHELL.contains("src=\"/preview\""));
        assert!(SHELL.contains("/livereload"));
        assert!(SHELL.contains("new WebSocket"));
        assert!(SHELL.contains(".reload()"));
    }

    #[test]
    fn error_page_lists_messages_severity_and_span() {
        let diagnostics = vec![
            Diagnostic::error("unexpected closing tag </mj-foo>")
                .with_label(Span::new(4, 12), "opened here"),
            Diagnostic::warning("mj-column has no children"),
        ];
        let html = error_page(&diagnostics);
        assert!(html.contains("unexpected closing tag"));
        assert!(html.contains("mj-column has no children"));
        assert!(html.contains("error"));
        assert!(html.contains("warning"));
        assert!(html.contains("4..12"));
        assert!(html.contains("opened here"));
        // The `<` in the message must be escaped, not emitted raw.
        assert!(html.contains("&lt;/mj-foo&gt;"));
        assert!(html.contains("1 error"));
    }

    #[test]
    fn render_preview_renders_a_valid_fixture() {
        let html = render_preview(&fixture("slice.mjml"), &RenderOptions::default());
        assert!(html.contains("Hello world"));
        assert!(html.starts_with("<!doctype html>"));
    }

    #[test]
    fn render_preview_of_missing_file_is_an_error_page() {
        let html = render_preview(&fixture("does-not-exist.mjml"), &RenderOptions::default());
        assert!(html.contains("mjmx build failed"));
        assert!(html.contains("could not read"));
    }
}
