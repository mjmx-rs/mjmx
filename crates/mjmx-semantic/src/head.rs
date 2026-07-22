//! Aggregated `<mj-head>` metadata: [`HeadData`] and the font table.
//!
//! Everything under `<mj-head>` that influences the document — the responsive
//! [`breakpoint`](HeadData::breakpoint), the `<title>` / preheader text, the font
//! import table, and raw `<mj-style>` CSS — is collected here once during
//! [`Semantic::build`](crate::Semantic::build), then read back by codegen when it
//! assembles the `<head>`.

use std::borrow::Cow;

/// A font the document may depend on: a family name and the stylesheet URL to import.
///
/// The table starts with mjml's built-in web fonts — or with a caller-supplied table,
/// which *replaces* them wholesale — and is extended (or overridden, by name) by each
/// `<mj-font>` in the head. Whether a given face is actually emitted is decided later by
/// codegen: a face is imported only if its [`name`](FontFace::name) appears in the
/// rendered body.
///
/// Both strings are [`Cow`]s: a `<mj-font>` borrows the document arena for `'a`, while a
/// face handed in by a caller (who has no arena) owns its strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFace<'a> {
    /// The font-family name (dedup key and the substring codegen scans for), e.g. `Ubuntu`.
    pub name: Cow<'a, str>,
    /// The stylesheet URL to `<link>` / `@import`.
    pub href: Cow<'a, str>,
}

impl<'a> FontFace<'a> {
    /// A face from any pair of string-likes — a borrowed arena `&'a str`, or owned
    /// `String`s from a caller-supplied font table.
    #[must_use]
    pub fn new(name: impl Into<Cow<'a, str>>, href: impl Into<Cow<'a, str>>) -> Self {
        Self {
            name: name.into(),
            href: href.into(),
        }
    }

    /// The same face with both strings owned, so it can be re-seeded into a head with a
    /// different (shorter or longer) arena lifetime.
    #[must_use]
    pub fn to_owned_face(&self) -> FontFace<'static> {
        FontFace {
            name: Cow::Owned(self.name.clone().into_owned()),
            href: Cow::Owned(self.href.clone().into_owned()),
        }
    }
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
    /// Raw *plain* `<mj-style>` CSS bodies, in source order, for verbatim head emission.
    ///
    /// mjml keeps two disjoint buckets (`globalData.style` / `globalData.inlineStyle`);
    /// this is the former — the only one that becomes a `<style>` tag.
    pub styles: Vec<String>,
    /// Raw `<mj-style inline="inline">` CSS bodies, in source order.
    ///
    /// mjml's `globalData.inlineStyle`: **never** emitted as a `<style>` tag (juice runs
    /// with `insertPreservedExtraCss:false`); it exists only to be fed to the CSS inliner
    /// as `extraCss`, and to be scanned by the web-font detector.
    pub inline_styles: Vec<String>,
    /// `<mj-raw>` bodies found under `<mj-head>`, in source order. mjml's `headRaw`:
    /// they are injected verbatim at the very end of the document `<head>`.
    pub raws: Vec<String>,
    /// `<mj-raw position="file-start">` bodies found as *direct children of `<mjml>`*
    /// (mjml's `mjOutsideRaws`). They are emitted before the doctype.
    pub file_start_raws: Vec<String>,
}

impl<'a> HeadData<'a> {
    /// A head pre-seeded with mjml's built-in fonts and no other metadata.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::with_fonts(None)
    }

    /// A head pre-seeded with `fonts`, or with mjml's built-ins when `fonts` is `None`.
    ///
    /// A caller-supplied table **replaces** the built-ins rather than extending them,
    /// matching mjml: its `fonts` option is a plain default value for `globalData.fonts`,
    /// so passing one drops `Open Sans` & co. entirely.
    #[must_use]
    pub fn with_fonts(fonts: Option<&[FontFace<'_>]>) -> Self {
        let fonts = match fonts {
            Some(supplied) => supplied.iter().map(FontFace::to_owned_face).collect(),
            None => DEFAULT_FONTS
                .iter()
                .map(|&(name, href)| FontFace::new(name, href))
                .collect(),
        };
        Self {
            breakpoint: None,
            title: None,
            preview: None,
            fonts,
            styles: Vec::new(),
            inline_styles: Vec::new(),
            raws: Vec::new(),
            file_start_raws: Vec::new(),
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
            existing.href = Cow::Borrowed(href);
        } else {
            self.fonts.push(FontFace::new(name, href));
        }
    }

    /// Whether any `<mj-style inline="inline">` was collected.
    #[must_use]
    pub fn has_inline_styles(&self) -> bool {
        !self.inline_styles.is_empty()
    }
}

impl Default for HeadData<'_> {
    fn default() -> Self {
        Self::with_defaults()
    }
}
