//! The include-loader abstraction: how `<mj-include>` content is fetched.
//!
//! Resolution (where a path points) is intentionally separated from the AST
//! transform (what to do with the loaded content). Today only [`LocalLoader`]
//! (filesystem) exists; an `HttpLoader` can be added later behind a feature flag
//! without touching the transform pass, because both implement [`IncludeLoader`].

use std::path::{Path, PathBuf};

use mjmx_diagnostics::Diagnostic;

/// Resolves and reads the content referenced by an `<mj-include path="...">`.
///
/// Implementations map an include `path` (relative, in the local case) against the
/// directory of the file that contains the include (`from_dir`) and return the loaded
/// text together with the **directory of the resolved file**. That directory is the
/// `from_dir` for any nested includes discovered inside the loaded content, so
/// relative paths resolve correctly at every depth.
pub trait IncludeLoader {
    /// Resolve `path` relative to `from_dir` and load it.
    ///
    /// On success returns `(file_contents, dir_of_that_file)`. On failure returns a
    /// [`Diagnostic`] describing what went wrong (missing file, read error, …); the
    /// caller anchors it to the offending `<mj-include>` span.
    ///
    /// # Errors
    ///
    /// Returns a [`Diagnostic`] when the path cannot be resolved or read.
    fn load(&self, path: &str, from_dir: &Path) -> Result<(String, PathBuf), Diagnostic>;
}

/// An [`IncludeLoader`] that reads from the local filesystem.
///
/// `path` is joined onto `from_dir` and read as UTF-8. The returned directory is the
/// parent of the resolved file, so nested relative includes are resolved against the
/// including file's own location (matching mjml's behavior).
#[derive(Debug, Clone, Copy, Default)]
pub struct LocalLoader;

impl IncludeLoader for LocalLoader {
    fn load(&self, path: &str, from_dir: &Path) -> Result<(String, PathBuf), Diagnostic> {
        let full = from_dir.join(path);
        let contents = std::fs::read_to_string(&full).map_err(|err| {
            Diagnostic::error(format!(
                "could not read include '{}': {err}",
                full.display()
            ))
        })?;
        let dir = full
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| from_dir.to_path_buf());
        Ok((contents, dir))
    }
}
