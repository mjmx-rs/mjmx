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

use std::path::PathBuf;

use mjmx_css_inline::InlineOptions;
use mjmx_minifier::MinifyOptions;
use mjmx_semantic::{FontFace, Semantic};
use mjmx_sourcemap::SourceMapBuilder;

/// How hard a render insists the document validates. mjml's `validationLevel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ValidationLevel {
    /// Validate, report, but render anyway. mjml's default, and ours.
    #[default]
    Soft,
    /// Validate and fail the render if any error-severity diagnostic is produced
    /// (mjml throws a `ValidationError` here).
    Strict,
    /// Do not validate at all.
    Skip,
}

/// Which mjml release mjmx reproduces.
///
/// mjml 5.0.0 changed rendered output in ways that are not additive — the `<body>` tag
/// moved under `mj-body`, the empty `style=""` on the article `<div>` is dropped, the
/// minifier became htmlnano/cssnano — so "parity with mjml" is only well defined once a
/// release is named. This selects the whole behaviour bundle at once rather than exposing
/// a flag per change: upstream ships these together, and the invalid combinations of
/// per-feature toggles are not worth carrying.
///
/// [`V4`](Self::V4) is frozen: it is the 4.18.0 behaviour mjmx shipped before 5.x support,
/// and the existing golden fixtures are its regression gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MjmlCompat {
    /// Reproduce mjml 4.18.0.
    V4,
    /// Reproduce mjml 5.4.0. The default, tracking upstream's `latest`.
    #[default]
    V5,
}

impl MjmlCompat {
    /// Whether this is the 5.x behaviour bundle.
    ///
    /// Render sites branch on this rather than matching the enum, so adding a future
    /// `V6` does not silently give it 4.x behaviour at every call site.
    #[must_use]
    pub fn is_v5(self) -> bool {
        matches!(self, Self::V5)
    }
}

/// Global options controlling a render.
///
/// Each field notes whether it mirrors an option mjml accepts on `mjml2html` or is an
/// mjmx extension. Where 4.18.0 and 5.4.0 differ, [`compat`](RenderOptions::compat)
/// decides which is reproduced.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Which mjml release to reproduce. `// mjmx default: v5`.
    ///
    /// **mjmx extension.** mjml has no such option — a given mjml install *is* one
    /// version. mjmx carries both because a renderer that silently changes its output
    /// on upgrade is not usable in a pipeline that diffs HTML.
    pub compat: MjmlCompat,
    /// The responsive breakpoint, in pixels. Columns collapse to full width below
    /// this width. `// mjml default: 480px`. A document `<mj-breakpoint>` overrides it.
    ///
    /// **mjmx extension.** mjml has no such option: it hardcodes `globalData.breakpoint`
    /// to `480px` and only `<mj-breakpoint>` changes it.
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
    /// How to minify, when [`minify`](Self::minify) is on. `// mjml default: {}`.
    ///
    /// Matches mjml's `minifyOptions`, which is spread over the four html-minifier
    /// switches mjml hardcodes — so the defaults here (see [`MinifyOptions`]) are
    /// mjml's own. Like mjml, this is **only consulted when `minify` is set**; with
    /// `minify: false` it has no effect at all.
    pub minify_options: MinifyOptions,
    /// Keep MJML source comments in the output HTML. `// mjml default: true`.
    ///
    /// Matches mjml's `keepComments`. mjml's parser turns every comment into an
    /// implicit `<mj-raw>` child of the enclosing element, so a kept comment is emitted
    /// verbatim exactly where it stood; setting this to `false` drops it. Comments
    /// *inside* an ending tag (`<mj-text>`, `<mj-raw>`, …) are part of that tag's raw
    /// body and survive either way — mjml captures that body as a source substring.
    pub keep_comments: bool,
    /// How hard to insist the document validates. `// mjml default: soft`.
    ///
    /// Matches mjml's `validationLevel`. Honored by the umbrella crate, which owns the
    /// linter; codegen itself never validates.
    pub validation_level: ValidationLevel,
    /// The path relative `<mj-include>` paths resolve against. `// mjml default: "."`.
    ///
    /// Matches mjml's `filePath`. A directory is used as-is; a file contributes its
    /// parent directory. `None` — our default rather than mjml's `"."` — means a
    /// string render resolves no includes at all, which is what `mjmx::render` has
    /// always done. Honored by the umbrella crate, which owns include resolution.
    pub file_path: Option<PathBuf>,
    /// The web-font table, replacing mjml's built-ins. `// mjml default: the five
    /// built-in Google fonts`.
    ///
    /// Matches mjml's `fonts`, including its *replace, never merge* semantics: passing
    /// a table drops `Open Sans`/`Droid Sans`/`Lato`/`Roboto`/`Ubuntu`. `None` keeps the
    /// built-ins. A `<mj-font>` still extends (or overrides by name) whatever is seeded,
    /// and either way a face is only emitted when the body actually references it.
    pub fonts: Option<Vec<FontFace<'static>>>,
    /// Skip `<mj-include>` entirely instead of resolving it. `// mjml default: false`.
    ///
    /// Matches mjml's `ignoreIncludes`, including its *drop, do not keep* semantics:
    /// mjml's parser `return`s on an `mj-include` before the node is built
    /// (`mjml-parser-xml/src/index.js:238`), so the tag and everything it would have
    /// pulled in simply vanish from the tree. Honored by the umbrella crate, which owns
    /// include resolution; codegen never sees the tag either way.
    pub ignore_includes: bool,
    /// Also emit the responsive column rules inside an `@media only print` block.
    /// `// mjml default: false`.
    ///
    /// Matches mjml's `printerSupport` (undocumented in mjml's README, but read off
    /// `mjml-core/src/helpers/mediaQueries.js:13,34-42`): when set, a third `<style>`
    /// replaying the *base* media queries under `@media only print` follows the screen
    /// and Thunderbird blocks. Like upstream, it is emitted only when there are column
    /// rules to replay.
    pub printer_support: bool,
    /// How to inline `<mj-style inline="inline">`. `// mjml default: {}`.
    ///
    /// Carries **both** of mjml's juice-facing options: the struct itself is
    /// `juiceOptions`, and its [`preserve_tags`](InlineOptions::preserve_tags) field
    /// is `juicePreserveTags`. One carrier rather than two, because there is no juice
    /// here to hand two separate objects to — mjmx's inliner is hand-rolled, and
    /// [`InlineOptions`] is exactly the subset of juice's options that has a meaning
    /// for it. See `mjmx_css_inline::options` for the option-by-option triage and for
    /// the ones mjmx refuses rather than silently ignores.
    ///
    /// Only consulted when the document has a non-empty inline `<mj-style>` bucket;
    /// otherwise no inlining runs at all.
    pub inline_options: InlineOptions,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            // mjmx default: track upstream's `latest`, currently 5.4.0.
            compat: MjmlCompat::V5,
            // mjml default: 480px breakpoint.
            breakpoint: 480,
            beautify: false,
            minify: false,
            // mjml default: `minifyOptions: {}`, i.e. the minifier's switches
            // unchanged. Which minifier that is follows `compat`, so this tracks the
            // `V5` above. A caller that flips `compat` to `V4` on a `..Default::default()`
            // literal and also minifies should pass `MinifyOptions::default()` with it;
            // the CLI and both bindings pick the pair together from `--mjml-version`.
            minify_options: MinifyOptions::for_mjml5(),
            // mjml default: comments are kept.
            keep_comments: true,
            // mjml default: soft validation.
            validation_level: ValidationLevel::Soft,
            // mjml defaults `filePath` to "."; None means "resolve no includes".
            file_path: None,
            // mjml default: the built-in font table.
            fonts: None,
            // mjml default: includes are resolved.
            ignore_includes: false,
            // mjml default: no @media print block.
            printer_support: false,
            // mjml default: `juiceOptions: undefined` / `juicePreserveTags: undefined`,
            // i.e. juice's defaults as mjml configures them.
            inline_options: InlineOptions::default(),
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
    /// The `<mj-body css-class>`, once resolved by the body renderer.
    ///
    /// Only [`MjmlCompat::V5`] reads it: mjml 5 moved the class off the wrapper `<div>`
    /// and onto the `<body>` tag, which document assembly emits, so the value has to
    /// travel out of the body renderer the same way `background-color` already does.
    /// Under V4 the body renderer writes the class on the `<div>` itself and this stays
    /// `None`.
    pub body_css_class: Option<String>,
    /// The `<mj-body id>`, once resolved by the body renderer.
    ///
    /// mjml 5 added `id` to `mj-body` and emits it on the `<body>` tag; mjml 4 has no
    /// such attribute and ignores it. Like [`body_css_class`](Self::body_css_class),
    /// only V5 populates this.
    pub body_id: Option<String>,
    /// Aggregated head data and the attribute resolver.
    pub semantic: Semantic<'a>,
    media_queries: Vec<MediaQuery>,
    styles: Vec<String>,
    hamburger_seq: u32,
    carousel_seq: u32,
    /// Source-mapping collector. `None` — the default — means no mapping is recorded and
    /// the instrumentation in [`crate::render`] short-circuits to a plain call, so the
    /// ordinary render path pays nothing but one `Option` check per element. Enabling it
    /// never changes a byte of the produced HTML: mappings are read off `buf.len()`,
    /// never written into `buf`.
    pub(crate) sourcemap: Option<SourceMapBuilder>,
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
            body_css_class: None,
            body_id: None,
            semantic,
            media_queries: Vec::new(),
            styles: Vec::new(),
            hamburger_seq: 0,
            carousel_seq: 0,
            sourcemap: None,
        }
    }

    /// Turn on source-map collection for this render pass.
    ///
    /// Off by default. Collection is observation-only — it records `buf.len()` before and
    /// after each element writes — so the produced HTML is byte-identical either way.
    pub fn enable_sourcemap(&mut self) {
        self.sourcemap = Some(SourceMapBuilder::default());
    }

    /// Whether source-map collection is on.
    #[must_use]
    pub fn collects_sourcemap(&self) -> bool {
        self.sourcemap.is_some()
    }

    /// Take the collected [`SourceMapBuilder`], if collection was enabled.
    ///
    /// Offsets are relative to the **body buffer** the pass wrote into; document assembly
    /// rebases them onto the final HTML.
    pub fn take_sourcemap(&mut self) -> Option<SourceMapBuilder> {
        self.sourcemap.take()
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
