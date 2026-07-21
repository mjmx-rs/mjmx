//! The axum router and its three request handlers.
//!
//! - `GET /` → the [`SHELL`](crate::page::SHELL) preview page.
//! - `GET /preview` → the current rendered email (or an error page), rendered fresh
//!   per request so every reload reflects the file on disk.
//! - `GET /livereload` → a WebSocket that forwards a `reload` frame every time the
//!   watcher reports a processed change.
//! - `GET /whence?offset=N` → which MJML source element produced byte `N` of the
//!   preview (click-to-source), answered from an [`mjmx::SourceMap`].

use std::path::PathBuf;
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::{Html, Response};
use axum::routing::get;
use mjmx::RenderOptions;
use tokio::sync::broadcast;

use crate::page;

/// Shared, cheaply-clonable state handed to every request handler.
#[derive(Clone)]
pub struct AppState {
    /// The `.mjml` file being previewed.
    pub path: Arc<PathBuf>,
    /// Render options (breakpoint, minify/beautify) applied to every preview.
    pub options: RenderOptions,
    /// Broadcast channel the watcher publishes reload notifications on; each
    /// WebSocket connection subscribes to a fresh receiver.
    pub reload_tx: broadcast::Sender<()>,
}

/// Build the router wired to `state`.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(shell))
        .route("/preview", get(preview))
        .route("/livereload", get(livereload))
        .route("/whence", get(whence))
        .with_state(state)
}

/// `GET /whence?offset=N` — which MJML source element produced byte `N` of the preview?
///
/// The click-to-source primitive: re-render the file collecting an
/// [`mjmx::SourceMap`], look the offset up, and answer with JSON —
/// `{"tag":"mj-text","source":{"start":…,"end":…},"output":{"start":…,"end":…},
/// "text":"<mj-text>…"}` — or `{"error":"…"}`.
///
/// Offsets are into the **unprocessed** render (`RenderOptions` defaults for whitespace),
/// which is what `/preview` serves unless `--beautify`/`--minify` is in play; those
/// rewrite whitespace after codegen and are refused rather than answered wrongly. This
/// route also does not resolve `<mj-include>` — a mapped span is an offset into *one*
/// source text, and the previewed file is that text.
async fn whence(
    State(state): State<AppState>,
    Query(params): Query<WhenceQuery>,
) -> Json<serde_json::Value> {
    Json(lookup_offset(&state, params.offset))
}

/// Query string of [`whence`].
#[derive(serde::Deserialize)]
struct WhenceQuery {
    /// Byte offset into the rendered HTML.
    offset: usize,
}

/// The body of [`whence`], factored out so it is testable without a live server.
fn lookup_offset(state: &AppState, offset: usize) -> serde_json::Value {
    if state.options.minify || state.options.beautify {
        return serde_json::json!({
            "error": "source lookup is unavailable while --minify/--beautify is active: \
                      whitespace post-processing shifts every output offset",
        });
    }
    let source = match std::fs::read_to_string(state.path.as_path()) {
        Ok(source) => source,
        Err(err) => return serde_json::json!({ "error": err.to_string() }),
    };
    let (_, map) = match mjmx::render_with_sourcemap(&source, &state.options) {
        Ok(pair) => pair,
        Err(diags) => {
            let messages: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
            return serde_json::json!({ "error": messages.join("; ") });
        }
    };
    match map.lookup(offset) {
        Some(m) => serde_json::json!({
            "tag": m.tag.as_str(),
            "source": { "start": m.source.start, "end": m.source.end },
            "output": { "start": m.output.start, "end": m.output.end },
            "text": &source[m.source.start as usize..m.source.end as usize],
        }),
        None => serde_json::json!({
            "error": format!("no MJML element covers output offset {offset}"),
        }),
    }
}

/// `GET /` — the static preview shell (iframe + live-reload client).
async fn shell() -> Html<&'static str> {
    Html(page::SHELL)
}

/// `GET /preview` — render the file right now and return the HTML (or an error page).
async fn preview(State(state): State<AppState>) -> Html<String> {
    Html(page::render_preview(&state.path, &state.options))
}

/// `GET /livereload` — upgrade to a WebSocket subscribed to reload notifications.
async fn livereload(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    let rx = state.reload_tx.subscribe();
    ws.on_upgrade(move |socket| handle_socket(socket, rx))
}

/// Pump reload notifications to one connected browser until it (or the channel) closes.
///
/// A `select!` races two futures: a reload from the broadcast channel (forwarded as a
/// `reload` text frame — including on `Lagged`, since a missed notification still
/// means "something changed, reload") and inbound socket traffic (we honor a `Close`
/// / stream-end by exiting; axum auto-responds to pings).
async fn handle_socket(mut socket: WebSocket, mut rx: broadcast::Receiver<()>) {
    loop {
        tokio::select! {
            reload = rx.recv() => {
                match reload {
                    Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {
                        let frame = Message::Text(Utf8Bytes::from_static("reload"));
                        if socket.send(frame).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    Some(Ok(_)) => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slice_state(reload_tx: broadcast::Sender<()>) -> AppState {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/slice.mjml");
        AppState {
            path: Arc::new(path),
            options: RenderOptions::default(),
            reload_tx,
        }
    }

    /// The click-to-source round trip: find "Hello world" in the preview HTML, ask
    /// `/whence` who wrote it, and check the answer is the fixture's `<mj-text>`.
    #[test]
    fn whence_maps_a_preview_offset_back_to_the_source_element() {
        let (reload_tx, _) = broadcast::channel(16);
        let state = slice_state(reload_tx);

        let html = page::render_preview(&state.path, &state.options);
        let offset = html.find("Hello world").expect("preview renders the text");

        let answer = lookup_offset(&state, offset);
        assert_eq!(answer["tag"], "mj-text");
        assert!(
            answer["text"].as_str().unwrap().starts_with("<mj-text"),
            "{answer}"
        );
        assert!(answer["text"].as_str().unwrap().contains("Hello world"));

        // An offset past the document maps to nothing, reported rather than panicking.
        let miss = lookup_offset(&state, html.len() + 1_000);
        assert!(miss["error"].is_string(), "{miss}");
    }

    /// Whitespace post-processing invalidates offsets, so the route refuses instead of
    /// answering wrongly.
    #[test]
    fn whence_refuses_when_whitespace_is_post_processed() {
        let (reload_tx, _) = broadcast::channel(16);
        let mut state = slice_state(reload_tx);
        state.options.beautify = true;
        let answer = lookup_offset(&state, 0);
        assert!(
            answer["error"].as_str().unwrap().contains("beautify"),
            "{answer}"
        );
    }

    /// Boot the real router on an ephemeral port and fetch `/preview` over a raw
    /// TCP HTTP/1.0 request (no HTTP-client dependency), asserting a 200 with the
    /// rendered email body.
    #[tokio::test]
    async fn preview_route_serves_rendered_html() {
        use std::io::{Read, Write};

        let (reload_tx, _) = broadcast::channel(16);
        let app = router(slice_state(reload_tx));

        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let body = tokio::task::spawn_blocking(move || {
            let mut stream = std::net::TcpStream::connect(addr).unwrap();
            stream
                .write_all(b"GET /preview HTTP/1.0\r\nHost: localhost\r\n\r\n")
                .unwrap();
            let mut buf = String::new();
            stream.read_to_string(&mut buf).unwrap();
            buf
        })
        .await
        .unwrap();

        assert!(body.contains("200"), "expected 200 status, got:\n{body}");
        assert!(body.contains("Hello world"));
        server.abort();
    }
}
