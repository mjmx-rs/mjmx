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

/// The sandbox verdict for a candidate `<mj-include>` path — mjml 5.4.0's include model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IncludeVerdict {
    /// The path resolves inside an allowed root. Carries the resolved (lexical) path,
    /// which the read-failure comment echoes.
    Allowed(PathBuf),
    /// The path is rejected: a NUL byte, an absolute/drive-letter/UNC path, traversal
    /// outside every allowed root, or a target that does not exist. Replaced with an
    /// `<!-- mj-include denied -->` comment rather than read.
    Denied,
}

/// Apply mjml 5.4.0's include sandbox to `path`, resolved against `from_dir`, with
/// `roots` as extra allowed directories (mjml's `includePath`).
///
/// Rejects, in order: a NUL byte; an absolute, Windows drive-letter (`C:`), or UNC
/// (`\\`/`//`) path; then anything that — after fully decoding percent-escapes so
/// `%2e%2e%2f` cannot smuggle `../` through — canonicalizes to a location outside every
/// allowed root (the base plus `roots`). Containment is checked on the canonicalized
/// paths, so `..` and symlinks cannot escape. A non-existent target fails to canonicalize
/// and is therefore denied, exactly as mjml's `realpathSync`-based check denies it.
pub(crate) fn sandbox_check(path: &str, from_dir: &Path, roots: &[PathBuf]) -> IncludeVerdict {
    let decoded = fully_decode(path);
    if decoded.contains('\0') || is_dangerous_shape(&decoded) {
        return IncludeVerdict::Denied;
    }
    let candidate = from_dir.join(&decoded);
    let Ok(real_target) = candidate.canonicalize() else {
        return IncludeVerdict::Denied;
    };
    let allowed = std::iter::once(from_dir.to_path_buf())
        .chain(roots.iter().cloned())
        .any(|root| {
            root.canonicalize()
                .ok()
                .and_then(|real_root| real_target.strip_prefix(&real_root).ok())
                .is_some_and(|rel| !rel.as_os_str().is_empty())
        });
    if allowed {
        IncludeVerdict::Allowed(candidate)
    } else {
        IncludeVerdict::Denied
    }
}

/// Whether `path` is an absolute, drive-letter (`C:`), or UNC (`\\` / `//`) path —
/// shapes mjml rejects outright, before the allowlist check.
fn is_dangerous_shape(path: &str) -> bool {
    if Path::new(path).is_absolute() {
        return true;
    }
    let bytes = path.as_bytes();
    // A Windows drive letter (`C:`) is not "absolute" on unix, so it is checked apart.
    let has_drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    let is_unc = path.starts_with("\\\\") || path.starts_with("//");
    has_drive || is_unc
}

/// Decode percent-escapes repeatedly, mirroring mjml's `fullyDecode` (up to ten
/// `decodeURIComponent` passes), so a multiply-encoded `../` is unmasked before the
/// sandbox checks see it. Stops early when a pass changes nothing or yields invalid UTF-8.
pub(crate) fn fully_decode(input: &str) -> String {
    let mut current = input.to_string();
    for _ in 0..10 {
        match percent_decode_once(&current) {
            Some(decoded) if decoded != current => current = decoded,
            _ => break,
        }
    }
    current
}

/// One percent-decoding pass. Returns `None` when the decoded bytes are not valid UTF-8
/// (mjml's `decodeURIComponent` would throw; the caller stops decoding).
fn percent_decode_once(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).ok()
}
