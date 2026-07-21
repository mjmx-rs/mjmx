//! HTTP(S) include loading — **only compiled with the `http-loader` feature**.
//!
//! # Security
//!
//! A template that can name a URL is a template that can make your process issue
//! arbitrary outbound requests: an SSRF-shaped capability (internal metadata
//! endpoints, `localhost` admin ports, private subnets). Nothing here is reachable
//! by accident:
//!
//! 1. the code does not exist unless the crate is built with `--features http-loader`;
//! 2. even then, the caller must explicitly construct an [`HttpLoader`] or
//!    [`CombinedLoader`] and pass it to
//!    [`resolve_includes_from`](crate::resolve_includes_from).
//!
//! [`render_file`](../../mjmx/fn.render_file.html) always uses the filesystem
//! [`LocalLoader`](crate::LocalLoader) and never fetches. Treat MJML sources the way
//! you would treat any other executable input: enabling this for untrusted templates
//! means letting their authors choose what your server connects to.
//!
//! # Resolution
//!
//! - `path` that is itself `http://…` / `https://…` is fetched as given, whatever
//!   the base is — so a local document can pull in a remote partial.
//! - any other `path` inside a document that was itself loaded over HTTP resolves
//!   against that document's URL, RFC-3986 style: `/x` from the origin, `//h/x`
//!   protocol-relative, `./x` and `../x` relative to the containing directory.
//!
//! # Injecting a different transport
//!
//! [`HttpLoader`] talks to the network through the [`HttpFetch`] trait. The default
//! implementation is [`UreqFetch`]; tests (and callers who want caching, auth
//! headers, or an allow-list) supply their own via [`HttpLoader::with_fetch`].

use mjmx_diagnostics::Diagnostic;

use crate::loader::{IncludeBase, IncludeLoader, Loaded, LocalLoader};

/// Whether `path` is an absolute HTTP(S) URL.
#[must_use]
pub fn is_http_url(path: &str) -> bool {
    let lower = path.trim_start().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// The transport [`HttpLoader`] uses to turn a URL into text.
///
/// Implement this to inject a stub in tests, or to add caching, authentication, or
/// an allow-list in front of the real network.
pub trait HttpFetch: Send + Sync {
    /// Fetch `url` and return the response body as text.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message on any transport, status, or decoding
    /// failure; the caller wraps it in a [`Diagnostic`] anchored to the include.
    fn get(&self, url: &str) -> Result<String, String>;
}

/// The default [`HttpFetch`]: a blocking `ureq` agent.
#[derive(Debug)]
pub struct UreqFetch {
    agent: ureq::Agent,
}

impl UreqFetch {
    /// A fetcher with a 10-second global timeout per request.
    #[must_use]
    pub fn new() -> Self {
        Self::with_timeout(std::time::Duration::from_secs(10))
    }

    /// A fetcher whose requests time out after `timeout` (connect + read, total).
    #[must_use]
    pub fn with_timeout(timeout: std::time::Duration) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .build();
        Self {
            agent: config.into(),
        }
    }
}

impl Default for UreqFetch {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpFetch for UreqFetch {
    fn get(&self, url: &str) -> Result<String, String> {
        let mut response = self
            .agent
            .get(url)
            .call()
            .map_err(|err| format!("request failed: {err}"))?;
        response
            .body_mut()
            .read_to_string()
            .map_err(|err| format!("could not read response body: {err}"))
    }
}

/// An [`IncludeLoader`] that fetches `<mj-include>` targets over HTTP(S).
///
/// See the [module docs](self) for the security posture and resolution rules.
pub struct HttpLoader {
    fetch: Box<dyn HttpFetch>,
}

impl std::fmt::Debug for HttpLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpLoader").finish_non_exhaustive()
    }
}

impl HttpLoader {
    /// A loader backed by the default [`UreqFetch`] transport.
    #[must_use]
    pub fn new() -> Self {
        Self::with_fetch(Box::new(UreqFetch::new()))
    }

    /// A loader backed by a custom transport.
    #[must_use]
    pub fn with_fetch(fetch: Box<dyn HttpFetch>) -> Self {
        Self { fetch }
    }

    /// The absolute URL `path` names when included from `from`.
    fn target_url(path: &str, from: &IncludeBase) -> Result<String, Diagnostic> {
        if is_http_url(path) {
            return Ok(normalize_url(path.trim()));
        }
        match from.as_url() {
            Some(base) => join_url(base, path).map_err(Diagnostic::error),
            None => Err(Diagnostic::error(format!(
                "cannot fetch include '{path}' over HTTP: it is not an absolute URL and \
                 the including document is not itself a URL (base '{}')",
                from.display()
            ))),
        }
    }
}

impl Default for HttpLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl IncludeLoader for HttpLoader {
    fn load(&self, path: &str, from: &IncludeBase) -> Result<Loaded, Diagnostic> {
        let url = Self::target_url(path, from)?;
        let contents = self
            .fetch
            .get(&url)
            .map_err(|err| Diagnostic::error(format!("could not fetch include '{url}': {err}")))?;
        Ok(Loaded {
            contents,
            base: IncludeBase::Url(parent_url(&url)),
            // The normalized absolute URL is the cycle identity; `./a.mjml` and
            // `../dir/a.mjml` collapse to the same string via `join_url`.
            id: url,
        })
    }
}

/// An [`IncludeLoader`] that dispatches per include: URLs go out over the network,
/// everything else comes off the filesystem.
///
/// This is what lets one document mix both. The rule is:
///
/// | include `path` | base | loader |
/// | --- | --- | --- |
/// | `https://h/p.mjml` | anything | [`HttpLoader`] |
/// | `./p.mjml` | a directory | [`LocalLoader`] |
/// | `./p.mjml` | a URL (inside a fetched partial) | [`HttpLoader`] |
///
/// So a remote partial's own relative includes stay remote and can never reach back
/// into the local filesystem, and a local partial never silently goes to the network
/// unless it spells out a full URL.
pub struct CombinedLoader {
    local: LocalLoader,
    http: HttpLoader,
}

impl std::fmt::Debug for CombinedLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CombinedLoader").finish_non_exhaustive()
    }
}

impl CombinedLoader {
    /// A combined loader using the default HTTP transport.
    #[must_use]
    pub fn new() -> Self {
        Self::with_http(HttpLoader::new())
    }

    /// A combined loader using a specific [`HttpLoader`] (e.g. a stubbed transport).
    #[must_use]
    pub fn with_http(http: HttpLoader) -> Self {
        Self {
            local: LocalLoader,
            http,
        }
    }
}

impl Default for CombinedLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl IncludeLoader for CombinedLoader {
    fn load(&self, path: &str, from: &IncludeBase) -> Result<Loaded, Diagnostic> {
        if is_http_url(path) || from.as_url().is_some() {
            self.http.load(path, from)
        } else {
            self.local.load(path, from)
        }
    }
}

// ---------------------------------------------------------------------------
// URL arithmetic
//
// Deliberately hand-rolled rather than pulling in a `url` dependency: the feature
// only needs origin + path handling for http(s), and this keeps the optional
// dependency surface to the HTTP client alone.
// ---------------------------------------------------------------------------

/// Split an absolute URL into `(origin, rest)`, where origin is `scheme://authority`
/// and rest starts at the first `/` of the path (or is empty).
fn split_origin(url: &str) -> Option<(&str, &str)> {
    let after_scheme = url.find("://")? + 3;
    let rest_start = url[after_scheme..]
        .find('/')
        .map_or(url.len(), |i| after_scheme + i);
    Some((&url[..rest_start], &url[rest_start..]))
}

/// Drop the query and fragment from `path`, returning `(path, suffix)`.
fn split_suffix(path: &str) -> (&str, &str) {
    match path.find(['?', '#']) {
        Some(i) => (&path[..i], &path[i..]),
        None => (path, ""),
    }
}

/// Resolve `.` / `..` segments in an absolute path, RFC 3986 §5.2.4 style.
fn normalize_path(path: &str) -> String {
    let trailing_slash = path.ends_with('/');
    let mut out: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    let mut result = String::with_capacity(path.len());
    for segment in &out {
        result.push('/');
        result.push_str(segment);
    }
    if result.is_empty() || (trailing_slash && !result.ends_with('/')) {
        result.push('/');
    }
    result
}

/// Normalize an absolute URL's path component in place (origin untouched).
fn normalize_url(url: &str) -> String {
    let Some((origin, rest)) = split_origin(url) else {
        return url.to_string();
    };
    let (path, suffix) = split_suffix(rest);
    format!("{origin}{}{suffix}", normalize_path(path))
}

/// Resolve `path` (relative, root-relative, or protocol-relative) against the
/// directory URL `base`, returning an absolute normalized URL.
fn join_url(base: &str, path: &str) -> Result<String, String> {
    let Some((origin, base_rest)) = split_origin(base) else {
        return Err(format!("include base '{base}' is not an absolute URL"));
    };

    // Protocol-relative: `//host/x` borrows the base's scheme.
    if let Some(rest) = path.strip_prefix("//") {
        let scheme = origin.split("://").next().unwrap_or("https");
        return Ok(normalize_url(&format!("{scheme}://{rest}")));
    }

    let (base_path, _) = split_suffix(base_rest);
    let (path, suffix) = split_suffix(path);

    let joined = if path.starts_with('/') {
        path.to_string()
    } else {
        // `base_path` is directory-like; keep everything up to its last `/`.
        let dir = match base_path.rfind('/') {
            Some(i) => &base_path[..=i],
            None => "/",
        };
        format!("{dir}{path}")
    };
    Ok(format!("{origin}{}{suffix}", normalize_path(&joined)))
}

/// The directory URL containing `url` — everything up to and including the last `/`.
fn parent_url(url: &str) -> String {
    let Some((origin, rest)) = split_origin(url) else {
        return url.to_string();
    };
    let (path, _) = split_suffix(rest);
    match path.rfind('/') {
        Some(i) => format!("{origin}{}", &path[..=i]),
        None => format!("{origin}/"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_urls() {
        assert!(is_http_url("https://example.com/a.mjml"));
        assert!(is_http_url("HTTP://example.com/a.mjml"));
        assert!(!is_http_url("./a.mjml"));
        assert!(!is_http_url("/etc/passwd"));
        assert!(!is_http_url("file:///etc/passwd"));
    }

    #[test]
    fn joins_relative_paths() {
        let base = "https://h/a/b/";
        assert_eq!(join_url(base, "p.mjml").unwrap(), "https://h/a/b/p.mjml");
        assert_eq!(join_url(base, "./p.mjml").unwrap(), "https://h/a/b/p.mjml");
        assert_eq!(join_url(base, "../p.mjml").unwrap(), "https://h/a/p.mjml");
        assert_eq!(join_url(base, "../../p.mjml").unwrap(), "https://h/p.mjml");
        assert_eq!(join_url(base, "/p.mjml").unwrap(), "https://h/p.mjml");
        assert_eq!(
            join_url(base, "//other/p.mjml").unwrap(),
            "https://other/p.mjml"
        );
        assert_eq!(
            join_url(base, "c/p.mjml").unwrap(),
            "https://h/a/b/c/p.mjml"
        );
    }

    #[test]
    fn join_keeps_query_and_port() {
        assert_eq!(
            join_url("http://h:8080/a/", "p.mjml?v=2").unwrap(),
            "http://h:8080/a/p.mjml?v=2"
        );
    }

    #[test]
    fn different_spellings_share_one_id() {
        // Cycle detection depends on this: both spellings must normalize equal.
        let a = join_url("https://h/a/b/", "../b/p.mjml").unwrap();
        let b = join_url("https://h/a/b/", "./p.mjml").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn parent_of_a_document_url() {
        assert_eq!(parent_url("https://h/a/b/p.mjml"), "https://h/a/b/");
        assert_eq!(parent_url("https://h/p.mjml"), "https://h/");
        assert_eq!(parent_url("https://h"), "https://h/");
        assert_eq!(parent_url("https://h/a/p.mjml?x=1"), "https://h/a/");
    }

    #[test]
    fn absolute_url_wins_over_base() {
        let from = IncludeBase::dir("/proj");
        let url = HttpLoader::target_url("https://h/a.mjml", &from).unwrap();
        assert_eq!(url, "https://h/a.mjml");
    }

    #[test]
    fn relative_path_from_a_dir_base_is_rejected() {
        let from = IncludeBase::dir("/proj");
        assert!(HttpLoader::target_url("./a.mjml", &from).is_err());
    }
}
