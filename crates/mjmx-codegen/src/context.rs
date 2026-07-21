//! Render state threaded through a render pass.
//!
//! [`RenderContext`] is the mutable accumulator that body components push their
//! *head requirements* into while they render. MJML's head (`@media` column rules,
//! component `<style>`s) is not known up front — it is discovered during the body
//! pass, then emitted afterwards. This is the two-pass model: render the body into a
//! buffer (side-effecting the context), then assemble the document head from what was
//! collected.
//!
//! The context also carries the [`Semantic`] view of the document, so components can
//! resolve attributes (`<mj-attributes>` / `mj-class` / defaults) and the head can
//! read aggregated `<mj-head>` metadata (title, preview, fonts, breakpoint).

use mjmx_semantic::Semantic;

/// Global options controlling a render.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// The responsive breakpoint, in pixels. Columns collapse to full width below
    /// this width. `// mjml default: 480px`. A document `<mj-breakpoint>` overrides it.
    pub breakpoint: u32,
    /// Pretty-print the output. Honored by the umbrella crate as a whitespace
    /// post-process (`mjmx_minifier::beautify`) after codegen produces the HTML;
    /// the codegen stage itself always emits the same compact-ish markup. If both
    /// `beautify` and `minify` are set, `minify` takes precedence.
    pub beautify: bool,
    /// Minify the output. Honored by the umbrella crate as a whitespace
    /// post-process (`mjmx_minifier::minify`) after codegen produces the HTML.
    /// Takes precedence over `beautify` when both are set.
    pub minify: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            // mjml default: 480px breakpoint.
            breakpoint: 480,
            beautify: false,
            minify: false,
        }
    }
}

/// A responsive column rule to register in the head, keyed by CSS class.
#[derive(Debug, Clone)]
pub struct MediaQuery {
    /// The CSS class the rule targets, e.g. `mj-column-per-100`.
    pub class: String,
    /// The declaration body, e.g. `width: 100% !important;\n        max-width: 100%;`.
    pub body: String,
}

/// Mutable render state accumulated during the body pass and consumed by the head.
///
/// Borrows the document arena for lifetime `'a` through the owned [`Semantic`] view.
#[derive(Debug)]
pub struct RenderContext<'a> {
    /// Global render options.
    pub options: RenderOptions,
    /// The active breakpoint (mirrors `options.breakpoint`, overridden by
    /// `<mj-breakpoint>`).
    pub breakpoint: u32,
    /// Document language (`<html lang>` / body `<div lang>`). `// mjml default: und`.
    pub lang: String,
    /// Document direction. `// mjml default: auto`.
    pub dir: String,
    /// The effective container (content-box) width, in pixels, available to the
    /// component currently rendering. Threaded body → wrapper → section → column →
    /// group so width math (column pixel widths, divider ghost tables) resolves against
    /// the real available width instead of a hard-coded constant. `// mjml default:
    /// 600px` (the `<mj-body>` width). Each layout container saves, narrows, and
    /// restores it around its children.
    pub container_width: u32,
    /// The `<mj-body background-color>`, once resolved by the body renderer. mjml's
    /// `setBackgroundColor` hands this to document assembly, which appends it to the
    /// `<body style>` (`word-spacing:normal;background-color:…;`). `None` when unset,
    /// which is mjml's default (no `background-color` default attribute on `mj-body`).
    pub body_background_color: Option<String>,
    /// Aggregated head data and the attribute resolver.
    pub semantic: Semantic<'a>,
    media_queries: Vec<MediaQuery>,
    styles: Vec<String>,
    hamburger_seq: u32,
    carousel_seq: u32,
}

impl<'a> RenderContext<'a> {
    /// Create a context from render options and the analysed [`Semantic`] view.
    #[must_use]
    pub fn new(options: RenderOptions, semantic: Semantic<'a>) -> Self {
        let breakpoint = options.breakpoint;
        Self {
            options,
            breakpoint,
            // mjml default: lang "und", dir "auto".
            lang: "und".to_string(),
            dir: "auto".to_string(),
            // mjml default: <mj-body width> is 600px.
            container_width: 600,
            body_background_color: None,
            semantic,
            media_queries: Vec::new(),
            styles: Vec::new(),
            hamburger_seq: 0,
            carousel_seq: 0,
        }
    }

    /// Mint a deterministic anchor id for an `<mj-navbar hamburger>` checkbox/label
    /// pair. mjml uses a per-navbar random 16-hex-digit token here
    /// (`genRandomHexString(16)`); that value is non-reproducible, so we emit a stable
    /// counter-derived id instead (the token is a purely internal anchor whose only
    /// requirement is that the `<input id>` and `<label for>` agree). The vendored
    /// hamburger golden has its random id pinned to the first value produced here.
    pub fn next_hamburger_id(&mut self) -> String {
        let id = format!("{:016x}", self.hamburger_seq);
        self.hamburger_seq += 1;
        id
    }

    /// Mint a deterministic 16-hex id for an `<mj-carousel>` radio group. mjml uses a
    /// per-carousel random token (`genRandomHexString(16)`) woven through both the head
    /// stylesheet and the radio/label markup; that value is non-reproducible, so we emit
    /// a stable counter-derived id instead. The vendored carousel golden has its random
    /// token pinned to the first value produced here.
    pub fn next_carousel_id(&mut self) -> String {
        let id = format!("{:016x}", self.carousel_seq);
        self.carousel_seq += 1;
        id
    }

    /// Register a responsive column rule (deduped by class, insertion order kept).
    pub fn add_media_query(&mut self, class: impl Into<String>, body: impl Into<String>) {
        let class = class.into();
        if self.media_queries.iter().any(|m| m.class == class) {
            return;
        }
        self.media_queries.push(MediaQuery {
            class,
            body: body.into(),
        });
    }

    /// Register a raw component `<style>` body (deduped, insertion order kept).
    pub fn add_style(&mut self, css: impl Into<String>) {
        let css = css.into();
        if self.styles.contains(&css) {
            return;
        }
        self.styles.push(css);
    }

    /// The registered responsive column rules, in insertion order.
    #[must_use]
    pub fn media_queries(&self) -> &[MediaQuery] {
        &self.media_queries
    }

    /// The registered raw component styles, in insertion order.
    #[must_use]
    pub fn styles(&self) -> &[String] {
        &self.styles
    }
}
