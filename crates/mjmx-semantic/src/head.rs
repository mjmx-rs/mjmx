//! Aggregated `<mj-head>` metadata: [`HeadData`] and the font table.
//!
//! Everything under `<mj-head>` that influences the document — the responsive
//! [`breakpoint`](HeadData::breakpoint), the `<title>` / preheader text, the font
//! import table, and raw `<mj-style>` CSS — is collected here once during
//! [`Semantic::build`](crate::Semantic::build), then read back by codegen when it
//! assembles the `<head>`.

/// A font the document may depend on: a family name and the stylesheet URL to import.
///
/// The table starts with mjml's built-in web fonts and is extended (or overridden,
/// by name) by each `<mj-font>` in the head. Whether a given face is actually emitted
/// is decided later by codegen: a face is imported only if its [`name`](FontFace::name)
/// appears in the rendered body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFace<'a> {
    /// The font-family name (dedup key and the substring codegen scans for), e.g. `Ubuntu`.
    pub name: &'a str,
    /// The stylesheet URL to `<link>` / `@import`.
    pub href: &'a str,
}

/// mjml 4.18.0's built-in web-font table, in its canonical insertion order.
const DEFAULT_FONTS: &[(&str, &str)] = &[
    (
        "Open Sans",
        "https://fonts.googleapis.com/css?family=Open+Sans:300,400,500,700",
    ),
    (
        "Droid Sans",
        "https://fonts.googleapis.com/css?family=Droid+Sans:300,400,500,700",
    ),
    (
        "Lato",
        "https://fonts.googleapis.com/css?family=Lato:300,400,500,700",
    ),
    (
        "Roboto",
        "https://fonts.googleapis.com/css?family=Roboto:300,400,500,700",
    ),
    (
        "Ubuntu",
        "https://fonts.googleapis.com/css?family=Ubuntu:300,400,500,700",
    ),
];

/// mjml's default responsive breakpoint, in pixels.
pub const DEFAULT_BREAKPOINT: u32 = 480;

/// Everything aggregated from `<mj-head>`.
#[derive(Debug, Clone)]
pub struct HeadData<'a> {
    /// The responsive breakpoint from `<mj-breakpoint>`, if one was given.
    ///
    /// `None` means "not specified" so callers can fall back to their own default
    /// (mjml's default is [`DEFAULT_BREAKPOINT`], 480px); see [`HeadData::breakpoint`].
    pub breakpoint: Option<u32>,
    /// The `<mj-title>` text, if present.
    pub title: Option<String>,
    /// The `<mj-preview>` preheader text, if present.
    pub preview: Option<String>,
    /// The font table: mjml's built-in fonts followed by any `<mj-font>` additions.
    pub fonts: Vec<FontFace<'a>>,
    /// Raw `<mj-style>` CSS bodies, in source order, for verbatim head emission.
    pub styles: Vec<String>,
    /// `<mj-raw>` bodies found under `<mj-head>`, in source order. mjml's `headRaw`:
    /// they are injected verbatim at the very end of the document `<head>`.
    pub raws: Vec<String>,
    /// `<mj-raw position="file-start">` bodies found as *direct children of `<mjml>`*
    /// (mjml's `mjOutsideRaws`). They are emitted before the doctype.
    pub file_start_raws: Vec<String>,
    /// Whether any `<mj-style inline>` was seen. Injection is deferred to the
    /// css-inline milestone; this only records that inlining was requested.
    pub has_inline_styles: bool,
}

impl<'a> HeadData<'a> {
    /// A head pre-seeded with mjml's built-in fonts and no other metadata.
    #[must_use]
    pub fn with_defaults() -> Self {
        let fonts = DEFAULT_FONTS
            .iter()
            .map(|&(name, href)| FontFace { name, href })
            .collect();
        Self {
            breakpoint: None,
            title: None,
            preview: None,
            fonts,
            styles: Vec::new(),
            raws: Vec::new(),
            file_start_raws: Vec::new(),
            has_inline_styles: false,
        }
    }

    /// The effective breakpoint in pixels, falling back to [`DEFAULT_BREAKPOINT`].
    #[must_use]
    pub fn breakpoint(&self) -> u32 {
        self.breakpoint.unwrap_or(DEFAULT_BREAKPOINT)
    }

    /// Add (or, by name, override the href of) a `<mj-font>` entry.
    ///
    /// A new name is appended after the built-ins, preserving insertion order; a name
    /// that already exists (including a built-in) has its href replaced in place.
    pub fn add_font(&mut self, name: &'a str, href: &'a str) {
        if let Some(existing) = self.fonts.iter_mut().find(|f| f.name == name) {
            existing.href = href;
        } else {
            self.fonts.push(FontFace { name, href });
        }
    }

    /// Whether any `<mj-style inline>` was collected.
    #[must_use]
    pub fn has_inline_styles(&self) -> bool {
        self.has_inline_styles
    }
}

impl Default for HeadData<'_> {
    fn default() -> Self {
        Self::with_defaults()
    }
}
