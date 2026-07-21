//! `mjmx-dev-server` — a local live-preview server for a single `.mjml` file.
//!
//! [`serve`] (async) renders the file, serves a browser preview, watches the file
//! and its directory for changes, and hot-reloads the browser over a WebSocket on
//! every save. [`run`] is the synchronous front door: it builds a tokio runtime and
//! blocks on [`serve`], so callers (the CLI) need no tokio of their own. This is the
//! only crate in the workspace that depends on tokio — async stays isolated here.
//!
//! Wiring: a [`tokio::sync::broadcast`] channel connects a [`notify`]-based file
//! [`watcher`] to every open WebSocket. On a debounced change the watcher publishes
//! one notification; each `/livereload` socket forwards it as a `reload` frame; the
//! browser reloads the `/preview` iframe. `/preview` renders per request via
//! [`mjmx::render_file`], so includes resolve and every reload reflects disk.

mod page;
mod routes;
mod watcher;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use mjmx::RenderOptions;
use tokio::sync::broadcast;

pub use page::{error_page, render_preview};

/// Options controlling the dev server.
pub struct DevOptions {
    /// TCP port to bind on `127.0.0.1`. Defaults to `3000`.
    pub port: u16,
    /// Best-effort open the preview URL in the default browser on startup.
    pub open: bool,
    /// Render options applied to every preview (breakpoint, minify/beautify).
    pub render: RenderOptions,
}

impl Default for DevOptions {
    fn default() -> Self {
        Self {
            port: 3000,
            open: false,
            render: RenderOptions::default(),
        }
    }
}

/// The error type surfaced by [`serve`] / [`run`]: anything from binding the socket
/// to running the server.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// Run the dev server synchronously: build a tokio runtime and block on [`serve`].
///
/// This is the entry point for the CLI, which stays free of any direct tokio
/// dependency.
///
/// # Errors
///
/// Returns an error if the runtime cannot be built or the server fails to bind/run.
pub fn run(path: PathBuf, options: DevOptions) -> Result<(), Error> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(serve(path, options))
}

/// Serve a live preview of `path` until Ctrl-C.
///
/// Binds `127.0.0.1:<port>`, spawns the file [`watcher`], prints the URL, optionally
/// opens the browser, then serves until an interrupt triggers graceful shutdown.
///
/// # Errors
///
/// Returns an error if the socket cannot be bound or the server encounters a fatal
/// I/O error while running.
pub async fn serve(path: PathBuf, options: DevOptions) -> Result<(), Error> {
    let (reload_tx, _) = broadcast::channel::<()>(16);

    watcher::spawn(path.clone(), options.render.clone(), reload_tx.clone());

    let state = routes::AppState {
        path: Arc::new(path.clone()),
        options: options.render.clone(),
        reload_tx,
    };
    let app = routes::router(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], options.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let url = format!("http://{}/", listener.local_addr()?);

    println!("mjmx dev server running at {url}");
    println!(
        "watching {} — save to hot-reload the browser",
        path.display()
    );
    if options.open {
        open_browser(&url);
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Resolve when the process receives Ctrl-C, driving graceful shutdown.
async fn shutdown_signal() {
    if tokio::signal::ctrl_c().await.is_ok() {
        println!("\nmjmx dev: shutting down");
    }
}

/// Best-effort open `url` in the platform's default browser; failures are ignored.
fn open_browser(url: &str) {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(program).arg(url).spawn();
}
