//! The include-loader abstraction: how `<mj-include>` content is fetched.
//!
//! Resolution (where a path points) is intentionally separated from the AST
//! transform (what to do with the loaded content). [`LocalLoader`] reads the
//! filesystem and is always available; [`HttpLoader`](crate::HttpLoader) and
//! [`CombinedLoader`](crate::CombinedLoader) fetch over the network and only exist
//! behind the `http-loader` cargo feature.

use std::path::{Path, PathBuf};

use mjmx_diagnostics::Diagnostic;

/// Where an include path is resolved *from*.
///
/// A base is "directory-like": the location the including document lives in, not
/// the document itself. [`IncludeBase::Dir`] is a filesystem directory;
/// [`IncludeBase::Url`] is a base URL that always ends in `/` (see
/// [`IncludeBase::url`]), so `partial.mjml` next to `https://h/a/doc.mjml`
/// resolves against `https://h/a/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncludeBase {
    /// A local filesystem directory.
    Dir(PathBuf),
    /// A base URL, normalized to end with `/`.
    Url(String),
}

impl IncludeBase {
    /// A filesystem base for `dir`.
    #[must_use]
    pub fn dir(dir: impl Into<PathBuf>) -> Self {
        IncludeBase::Dir(dir.into())
    }

    /// A URL base, normalized to end with `/`.
    ///
    /// Pass the *directory* URL (`https://h/a/`); a trailing slash is appended when
    /// missing so that later joins behave predictably.
    #[must_use]
    pub fn url(url: impl Into<String>) -> Self {
        let mut url = url.into();
        if !url.ends_with('/') {
            url.push('/');
        }
        IncludeBase::Url(url)
    }

    /// The filesystem directory, or `None` when this is a URL base.
    #[must_use]
    pub fn as_dir(&self) -> Option<&Path> {
        match self {
            IncludeBase::Dir(dir) => Some(dir),
            IncludeBase::Url(_) => None,
        }
    }

    /// The base URL, or `None` when this is a filesystem base.
    #[must_use]
    pub fn as_url(&self) -> Option<&str> {
        match self {
            IncludeBase::Url(url) => Some(url),
            IncludeBase::Dir(_) => None,
        }
    }

    /// A human-readable rendering, for diagnostics.
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            IncludeBase::Dir(dir) => dir.display().to_string(),
            IncludeBase::Url(url) => url.clone(),
        }
    }
}

/// One successfully loaded include.
#[derive(Debug, Clone)]
pub struct Loaded {
    /// The file's text.
    pub contents: String,
    /// The base that nested includes inside `contents` resolve against — the
    /// directory (or directory URL) of the loaded file itself.
    pub base: IncludeBase,
    /// A canonical identity for the loaded resource, used for cycle detection.
    ///
    /// Two include paths that name the same resource must produce the same `id`
    /// (e.g. `./a.mjml` and `../dir/a.mjml`), and two different resources must not.
    pub id: String,
}

/// Resolves and reads the content referenced by an `<mj-include path="...">`.
///
/// Implementations map an include `path` against the base of the file that
/// contains the include, and return the text plus the base for any nested includes
/// found inside it, so relative paths resolve correctly at every depth.
pub trait IncludeLoader {
    /// Resolve `path` relative to `from` and load it.
    ///
    /// # Errors
    ///
    /// Returns a [`Diagnostic`] when the path cannot be resolved or read; the
    /// caller anchors it to the offending `<mj-include>` span.
    fn load(&self, path: &str, from: &IncludeBase) -> Result<Loaded, Diagnostic>;
}

/// An [`IncludeLoader`] that reads from the local filesystem.
///
/// `path` is joined onto the base directory and read as UTF-8. The returned base is
/// the parent of the resolved file, so nested relative includes resolve against the
/// including file's own location (matching mjml's behavior). A URL base is rejected:
/// this loader never touches the network.
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalLoader;

impl IncludeLoader for LocalLoader {
    fn load(&self, path: &str, from: &IncludeBase) -> Result<Loaded, Diagnostic> {
        let Some(from_dir) = from.as_dir() else {
            return Err(Diagnostic::error(format!(
                "cannot resolve local include '{path}' against the URL base '{}': \
                 the filesystem loader does not fetch over the network",
                from.display()
            )));
        };
        let full = from_dir.join(path);
        let contents = std::fs::read_to_string(&full).map_err(|err| {
            Diagnostic::error(format!(
                "could not read include '{}': {err}",
                full.display()
            ))
        })?;
        // Canonicalize when possible so `./a.mjml` and `../dir/a.mjml` share one id.
        let id = full.canonicalize().unwrap_or_else(|_| full.clone());
        let dir = full
            .parent()
            .map_or_else(|| from_dir.to_path_buf(), Path::to_path_buf);
        Ok(Loaded {
            contents,
            base: IncludeBase::Dir(dir),
            id: id.display().to_string(),
        })
    }
}
