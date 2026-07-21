//! The `mjmx-language-server` binary: serve the MJML language protocol over stdio.
//!
//! Editors launch this with no arguments and speak LSP on stdin/stdout. See the
//! crate README for editor wiring.

fn main() {
    mjmx_language_server::run();
}
