//! The axum router and its three request handlers.
//!
//! - `GET /` → the [`SHELL`](crate::page::SHELL) preview page.
//! - `GET /preview` → the current rendered email (or an error page), rendered fresh
//!   per request so every reload reflects the file on disk.
//! - `GET /livereload` → a WebSocket that forwards a `reload` frame every time the
//!   watcher reports a processed change.

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade};
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
        .with_state(state)
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
