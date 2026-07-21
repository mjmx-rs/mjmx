//! Render state threaded through a render pass.
//!
//! [`RenderContext`] is the mutable accumulator that body components push their
//! *head requirements* into while they render. MJML's head (`@media` column rules,
//! font `@import`s, component `<style>`s) is not known up front — it is discovered
//! during the body pass, then emitted afterwards. This is the two-pass model: render
//! the body into a buffer (side-effecting the context), then assemble the document
//! head from what was collected.

/// Global options controlling a render.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// The responsive breakpoint, in pixels. Columns collapse to full width below
    /// this width. `// mjml default: 480px`.
    pub breakpoint: u32,
    /// Pretty-print the output. **Not yet honored** — accepted for forward
    /// compatibility with the CLI. Output is currently always compact-ish.
    pub beautify: bool,
    /// Minify the output. **Not yet honored** — accepted for forward compatibility
    /// with the CLI.
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

/// A font the document depends on: a family name and the stylesheet URL to import.
#[derive(Debug, Clone)]
pub struct Font {
    /// The font-family name (dedup key), e.g. `Ubuntu`.
    pub name: String,
    /// The stylesheet URL to `<link>` / `@import`.
    pub href: String,
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
#[derive(Debug)]
pub struct RenderContext {
    /// Global render options.
    pub options: RenderOptions,
    /// The active breakpoint (mirrors `options.breakpoint`).
    pub breakpoint: u32,
    /// Document language (`<html lang>` / body `<div lang>`). `// mjml default: und`.
    pub lang: String,
    /// Document direction. `// mjml default: auto`.
    pub dir: String,
    fonts: Vec<Font>,
    media_queries: Vec<MediaQuery>,
    styles: Vec<String>,
}

impl RenderContext {
    /// Create a context from render options.
    #[must_use]
    pub fn new(options: RenderOptions) -> Self {
        let breakpoint = options.breakpoint;
        Self {
            options,
            breakpoint,
            // mjml default: lang "und", dir "auto".
            lang: "und".to_string(),
            dir: "auto".to_string(),
            fonts: Vec::new(),
            media_queries: Vec::new(),
            styles: Vec::new(),
        }
    }

    /// Register a font dependency (deduped by family name, insertion order kept).
    pub fn add_font(&mut self, name: impl Into<String>, href: impl Into<String>) {
        let name = name.into();
        if self.fonts.iter().any(|f| f.name == name) {
            return;
        }
        self.fonts.push(Font {
            name,
            href: href.into(),
        });
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

    /// The registered fonts, in insertion order.
    #[must_use]
    pub fn fonts(&self) -> &[Font] {
        &self.fonts
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
