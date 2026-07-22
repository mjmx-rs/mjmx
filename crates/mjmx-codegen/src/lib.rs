//! The `mjmx` render engine: MJML AST into responsive, mjml-faithful HTML.
//!
//! This crate is the codegen stage of the pipeline. It walks an
//! [`mjmx_ast::Document`] and reproduces mjml 4.18.0's actual output — the
//! table-based layout, the mso/IE conditional-comment ghost tables, the media-query
//! column classes, and the standard head boilerplate.
//!
//! # Model
//!
//! Rendering is two-pass and oxc-`Gen`-shaped: every component implements
//! [`Render`], writing its HTML fragment into a shared buffer while pushing its head
//! requirements (fonts, responsive rules, styles) into a mutable [`RenderContext`].
//! [`render_document`] runs the body pass, then assembles the `<head>` from what was
//! collected, so the Ubuntu `@import` and the `.mj-column-per-100` media queries
//! appear precisely because mj-text and mj-column asked for them.
//!
//! ```
//! use mjmx_allocator::Allocator;
//! use mjmx_ast::{AstBuilder, MjmlTag, Span};
//! use mjmx_codegen::{render_document, RenderOptions};
//!
//! let alloc = Allocator::new();
//! let b = AstBuilder::new(&alloc);
//! let body = b.element(MjmlTag::MjBody, b.atom("mj-body"), b.vec(), b.vec(), Span::default());
//! let mjml = b.element(
//!     MjmlTag::Mjml,
//!     b.atom("mjml"),
//!     b.vec(),
//!     b.vec_from_iter([b.element_node(body)]),
//!     Span::default(),
//! );
//! let doc = b.document(b.vec_from_iter([b.element_node(mjml)]), Span::default());
//!
//! let html = render_document(&doc, &RenderOptions::default()).unwrap();
//! assert!(html.starts_with("<!doctype html>"));
//! ```

mod components;
mod context;
mod document;
mod head;
mod render;

pub use context::{MediaQuery, RenderContext, RenderOptions, ValidationLevel};
// Re-exported so a caller can build a `RenderOptions::fonts` table without depending on
// `mjmx-semantic` directly.
pub use document::{
    render_document, render_document_with_inline_css, render_document_with_sourcemap,
};
pub use mjmx_semantic::FontFace;
pub use mjmx_sourcemap::{Mapping, SourceMap, Span};
pub use render::{Render, RenderResult, render_element};
