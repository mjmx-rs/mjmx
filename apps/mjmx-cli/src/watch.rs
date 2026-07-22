//! `mjmx <file> --watch`: recompile the file on every change.
//!
//! This is mjml's `-w`/`--watch` (`mjml-cli/src/commands/watchFiles.js`) on the
//! render path. Two properties come straight from mjml 4.18.0's observed behaviour:
//!
//! - **Watch output is always a file, never stdout.** With `-o` it writes there;
//!   without `-o` mjml writes `<stem>.html` — the input's *basename* with `.mjml`
//!   swapped for `.html` — into the **current directory** (not beside the source).
//!   [`derive_output_path`] reproduces that.
//! - Watching requires a real file input; there is nothing to watch on stdin, so the
//!   caller rejects `--watch` without a file (and clap rejects `--watch --stdin`).
//!
//! The watch loop is a deliberately minimal copy of the one that drives `mjmx dev`
//! (`mjmx-dev-server/src/watcher.rs`): same three guards against a rebuild feeding
//! itself — event *kind* ([`is_content_kind`]), event *path* ([`is_watched_path`])
//! and re-arming the file watch after an atomic save. It is copied rather than
//! shared so the dev server's tokio/broadcast machinery stays out of the render path.
//! The output HTML is never a `.mjml`, so writing it can never match
//! [`is_watched_path`] and cannot retrigger the loop — no output fingerprint needed.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::Duration;

use mjmx::{RenderOptions, Severity};
use notify::event::ModifyKind;
use notify::{Event, EventKind, RecursiveMode, Watcher};

/// How long the file must stay quiet before a burst of events is treated as done.
const DEBOUNCE: Duration = Duration::from_millis(150);

/// Where a watch rebuild writes its HTML; mjml's watch never writes to stdout.
pub enum OutputTarget {
    /// The explicit `-o`/`--output` path, written to verbatim.
    File(PathBuf),
    /// No `-o`: mjml's `<stem>.html` in the current directory (see [`derive_output_path`]).
    Derived(PathBuf),
}

impl OutputTarget {
    /// The path this target writes to.
    fn path(&self) -> &Path {
        match self {
            Self::File(path) | Self::Derived(path) => path,
        }
    }
}

/// mjml's derived watch-output name: the input's basename with `.mjml` → `.html`,
/// resolved in the **current directory** rather than beside the source.
///
/// mjml computes `replaceExtension(stripPath(input))`: `stripPath` keeps only the
/// basename, so `sub/foo.mjml` becomes `foo.html` in the process's cwd. A basename
/// with no `.mjml` extension just gains `.html`.
pub fn derive_output_path(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("output");
    PathBuf::from(format!("{stem}.html"))
}

/// Run the watch loop: build once, then rebuild on every change until Ctrl-C.
pub fn run(input: &Path, output: OutputTarget, options: &RenderOptions) -> ExitCode {
    // An initial build so the output file exists before the first edit; the status
    // line makes what happened (and where it landed) explicit.
    eprintln!("mjmx: {}", compile(input, output.path(), options));
    eprintln!("mjmx: watching {} (Ctrl-C to stop)", input.display());

    if let Err(err) = watch_loop(input, &output, options) {
        eprintln!("mjmx: watch error: {err}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Render `input` and write the HTML to `output`, returning a one-line status.
///
/// A failed render prints its diagnostics to stderr and keeps the watcher alive, as
/// mjml does — an error in one save should not end the session.
fn compile(input: &Path, output: &Path, options: &RenderOptions) -> String {
    match mjmx::render_file(input, options) {
        Ok(html) => match std::fs::write(output, &html) {
            Ok(()) => format!("compiled {} -> {}", input.display(), output.display()),
            Err(err) => format!(
                "compiled {} but could not write {}: {err}",
                input.display(),
                output.display()
            ),
        },
        Err(diagnostics) => {
            for diagnostic in &diagnostics {
                eprintln!("{diagnostic}");
            }
            let errors = diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .count();
            format!("failed to compile {} ({errors} error(s))", input.display())
        }
    }
}

/// The blocking watch loop; returns on watcher setup failure or channel disconnect.
fn watch_loop(input: &Path, output: &OutputTarget, options: &RenderOptions) -> notify::Result<()> {
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res| {
        // If the receiver is gone the loop has ended; dropping the event is fine.
        let _ = tx.send(res);
    })?;

    arm_file_watch(&mut watcher, input);
    // Also watch the parent directory: same-directory `<mj-include>` partials live
    // there, and atomic saves land as a rename into the directory rather than a
    // write to the file we are watching.
    if let Some(parent) = input.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = watcher.watch(parent, RecursiveMode::NonRecursive);
        }
    }

    while let Ok(first) = rx.recv() {
        let mut candidate = is_change_event(&first, input);
        while let Ok(event) = rx.recv_timeout(DEBOUNCE) {
            candidate |= is_change_event(&event, input);
        }
        if !candidate {
            continue;
        }

        // An atomic save replaces the inode, leaving our file watch pointed at a
        // deleted one; re-arm so direct writes keep being seen.
        arm_file_watch(&mut watcher, input);

        eprintln!("mjmx: {}", compile(input, output.path(), options));
    }

    Ok(())
}

/// (Re-)watch the target file, ignoring errors from a stale or absent watch.
fn arm_file_watch(watcher: &mut impl Watcher, path: &Path) {
    let _ = watcher.unwatch(path);
    let _ = watcher.watch(path, RecursiveMode::NonRecursive);
}

/// Whether an event is a candidate change: right kind *and* a path we care about.
fn is_change_event(event: &notify::Result<Event>, target: &Path) -> bool {
    let Ok(event) = event else {
        return false;
    };
    is_content_kind(event.kind) && event.paths.iter().any(|path| is_watched_path(path, target))
}

/// Whether an event kind represents a real content mutation.
///
/// Excludes `Access(..)`, `Modify(Metadata(..))`, `Modify(Any)` and the catch-all
/// `Any`/`Other`: those cover reads and bookkeeping, which the rebuild's own file
/// read would otherwise regenerate forever.
fn is_content_kind(kind: EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Modify(ModifyKind::Data(_))
            | EventKind::Modify(ModifyKind::Name(_))
    )
}

/// Whether a path from an event should be acted on: the target file, or any sibling
/// `.mjml` (an `<mj-include>` partial). Editor scratch files are rejected first.
///
/// The rendered `.html` output is deliberately *not* matched, so writing it can
/// never retrigger the loop.
fn is_watched_path(path: &Path, target: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    // vim swapfiles (`.foo.swp`) and dotfiles, backup copies (`foo~`), and vim's
    // `4913` inotify probe file. `sed -i`'s temp file has no `.mjml` extension and
    // is rejected by the extension check below.
    if name.starts_with('.') || name.ends_with('~') || name == "4913" {
        return false;
    }

    if target.file_name().is_some_and(|target| target == name) {
        return true;
    }

    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mjml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, CreateKind, DataChange, MetadataKind, RemoveKind, RenameMode};

    #[test]
    fn derive_output_uses_basename_html_in_cwd() {
        // Basename only, `.mjml` -> `.html`, resolved in the cwd (no directory).
        assert_eq!(
            derive_output_path(Path::new("foo.mjml")),
            PathBuf::from("foo.html")
        );
        assert_eq!(
            derive_output_path(Path::new("sub/foo.mjml")),
            PathBuf::from("foo.html")
        );
        assert_eq!(
            derive_output_path(Path::new("/a/b/mail.mjml")),
            PathBuf::from("mail.html")
        );
        // A basename without `.mjml` still gains `.html`.
        assert_eq!(
            derive_output_path(Path::new("plain")),
            PathBuf::from("plain.html")
        );
    }

    #[test]
    fn content_mutations_are_changes_reads_and_metadata_are_not() {
        assert!(is_content_kind(EventKind::Modify(ModifyKind::Data(
            DataChange::Content
        ))));
        assert!(is_content_kind(EventKind::Modify(ModifyKind::Name(
            RenameMode::To
        ))));
        assert!(is_content_kind(EventKind::Create(CreateKind::File)));
        assert!(is_content_kind(EventKind::Remove(RemoveKind::File)));

        assert!(!is_content_kind(EventKind::Any));
        assert!(!is_content_kind(EventKind::Access(AccessKind::Read)));
        assert!(!is_content_kind(EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::AccessTime
        ))));
        assert!(!is_content_kind(EventKind::Modify(ModifyKind::Any)));
    }

    #[test]
    fn watches_target_and_sibling_partials_but_not_the_html_output() {
        let target = Path::new("/w/mail.mjml");
        assert!(is_watched_path(Path::new("/w/mail.mjml"), target));
        assert!(is_watched_path(Path::new("/w/header.mjml"), target));
        assert!(is_watched_path(Path::new("/w/Footer.MJML"), target));

        // The rendered output and editor scratch files must never retrigger.
        assert!(!is_watched_path(Path::new("/w/mail.html"), target));
        assert!(!is_watched_path(Path::new("/w/.mail.mjml.swp"), target));
        assert!(!is_watched_path(Path::new("/w/mail.mjml~"), target));
        assert!(!is_watched_path(Path::new("/w/4913"), target));
    }

    #[test]
    fn change_event_requires_both_kind_and_path() {
        let target = Path::new("/w/mail.mjml");
        let data = EventKind::Modify(ModifyKind::Data(DataChange::Content));

        let hit = Ok(Event::new(data).add_path(target.to_path_buf()));
        assert!(is_change_event(&hit, target));

        // Right kind, but the html output is not a watched path.
        let wrong_path = Ok(Event::new(data).add_path(PathBuf::from("/w/mail.html")));
        assert!(!is_change_event(&wrong_path, target));

        // Watched path, but a read is not a content change.
        let wrong_kind =
            Ok(Event::new(EventKind::Access(AccessKind::Read)).add_path(target.to_path_buf()));
        assert!(!is_change_event(&wrong_kind, target));
    }
}
