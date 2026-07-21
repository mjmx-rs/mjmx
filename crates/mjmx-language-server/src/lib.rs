//! A Language Server Protocol implementation for MJML.
//!
//! `mjmx-language-server` is a thin protocol surface over the rest of the `mjmx`
//! toolchain: it parses with [`mjmx_parser`], validates with [`mjmx_linter`],
//! pretty-prints with [`mjmx_formatter`], and walks the [`mjmx_ast`] tree for the
//! outline. It contains no MJML knowledge of its own beyond a built-in
//! documentation table ([`docs`], deliberately non-exhaustive).
//!
//! # Features
//!
//! | Request | Behaviour |
//! | --- | --- |
//! | `textDocument/publishDiagnostics` | parse errors + linter findings, on open/change/save |
//! | `textDocument/completion` | context-aware tags (from the linter's `allowed_children`) and attributes |
//! | `textDocument/hover` | component and attribute documentation |
//! | `textDocument/formatting` | one whole-document edit from `mjmx-formatter` |
//! | `textDocument/documentSymbol` | the nested element outline |
//!
//! # Layout
//!
//! - [`line_index`] — byte offset ↔ UTF-16 [`Position`](tower_lsp::lsp_types::Position).
//! - [`context`] — what the caret is sitting on, tolerant of half-typed markup.
//! - [`docs`] — the built-in component/attribute documentation table.
//! - [`analysis`] — the pure feature implementations, all synchronous.
//! - [`server`] — the `tower-lsp` adapter; the only async code here.
//!
//! Async and tokio stay confined to this crate: [`run`] is a **synchronous** front
//! door that builds its own runtime and serves the protocol over stdio, mirroring
//! [`mjmx_dev_server::run`](https://docs.rs/mjmx-dev-server).
//!
//! ```no_run
//! // The `mjmx-language-server` binary is exactly this:
//! mjmx_language_server::run();
//! ```

pub mod analysis;
pub mod context;
pub mod docs;
pub mod line_index;
pub mod server;

pub use server::Backend;

/// Serve the language protocol over stdin/stdout, blocking until the client exits.
///
/// Builds a tokio runtime internally so callers need none of their own.
///
/// # Panics
///
/// Panics if the tokio runtime cannot be created (e.g. the OS refuses to spawn
/// threads), which is unrecoverable for a server that has nothing else to do.
pub fn run() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build the tokio runtime");
    runtime.block_on(serve());
}

/// The async body of [`run`]: wire the backend to stdio and serve.
async fn serve() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = tower_lsp::LspService::new(Backend::new);
    tower_lsp::Server::new(stdin, stdout, socket)
        .serve(service)
        .await;
}
