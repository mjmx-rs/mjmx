//! Filesystem watcher that drives hot-reload.
//!
//! `notify` delivers change events on a synchronous callback, so the watch loop runs
//! on a dedicated OS thread rather than a tokio task. It watches the target file
//! *and* its parent directory (non-recursively) so that same-directory
//! `<mj-include>` partials — and atomic saves, which replace the file's inode — are
//! still observed.
//!
//! # Why rebuilds must not feed themselves
//!
//! Every rebuild re-reads the file via [`mjmx::render_file`], and so does every
//! `/preview` request. If any of that read activity is mistaken for a change, the
//! watcher retriggers itself and rebuilds forever at one per debounce window. Three
//! independent layers prevent that, in order of strictness:
//!
//! 1. **Event kind** ([`is_content_kind`]) — only real content mutations count:
//!    `Modify(Data)`, `Modify(Name)` (editors rename a temp file into place),
//!    `Create` and `Remove`. `Access`, `Modify(Metadata)` (atime/mtime touches) and
//!    the catch-all `EventKind::Any` are rejected; the inotify backend reports plenty
//!    of read-side and bookkeeping activity through those.
//! 2. **Path** ([`is_watched_path`]) — only the target file and sibling `.mjml`
//!    files; editor scratch files (`.foo.swp`, `foo~`, vim's `4913` probe, dotfiles)
//!    and `sed -i`'s unsuffixed temp files are ignored.
//! 3. **Build output** ([`ContentGuard`]) — the decisive one. Each candidate event
//!    renders once and hashes the *result*; the browser is only told to reload when
//!    that result differs from the last one shown. This makes the loop impossible
//!    regardless of which events a platform's backend emits, so it is kept even
//!    though layer 1 alone appears sufficient. Fingerprinting the render output
//!    rather than the entry file's bytes is what lets an edit to an
//!    `<mj-include>` partial reload the browser: the entry file is untouched, but
//!    the rendered HTML changes.
//!
//! Bursts are debounced: after the first event the channel is drained until it stays
//! quiet for [`DEBOUNCE`], then the burst is processed at most once.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Duration;

use mjmx::{RenderOptions, Severity};
use notify::event::ModifyKind;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use tokio::sync::broadcast;

/// How long the file must stay quiet before a burst of events is treated as done.
const DEBOUNCE: Duration = Duration::from_millis(150);

/// Spawn the watcher on a dedicated OS thread.
///
/// The thread owns the `notify` watcher for its whole lifetime and publishes reload
/// notifications on `reload_tx`. It exits (logging any setup error) if the watcher
/// cannot be created; a failed watch never brings the server down.
pub fn spawn(path: PathBuf, options: RenderOptions, reload_tx: broadcast::Sender<()>) {
    std::thread::spawn(move || {
        if let Err(err) = watch_loop(&path, &options, &reload_tx) {
            eprintln!("mjmx dev: watcher error: {err}");
        }
    });
}

/// The blocking watch loop; returns on watcher setup failure or channel disconnect.
fn watch_loop(
    path: &Path,
    options: &RenderOptions,
    reload_tx: &broadcast::Sender<()>,
) -> notify::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res| {
        // If the receiver is gone the loop has ended; dropping the event is fine.
        let _ = tx.send(res);
    })?;

    arm_file_watch(&mut watcher, path);
    // Also watch the parent directory: sibling `<mj-include>` partials live there,
    // and atomic saves land as a rename into the directory rather than a write to
    // the file we are watching.
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = watcher.watch(parent, RecursiveMode::NonRecursive);
        }
    }

    // What the browser already shows at startup is the baseline: the first reload
    // only happens once a build produces something different.
    let mut guard = ContentGuard::primed(path, options);

    while let Ok(first) = rx.recv() {
        let mut candidate = is_change_event(&first, path);
        while let Ok(event) = rx.recv_timeout(DEBOUNCE) {
            candidate |= is_change_event(&event, path);
        }
        if !candidate {
            continue;
        }

        // An atomic save replaces the inode, leaving our file watch pointed at a
        // deleted one; re-arm so direct writes keep being seen.
        arm_file_watch(&mut watcher, path);

        // The decisive check: rebuild, and stay silent unless the result actually
        // differs, so read-triggered or bookkeeping events cannot sustain a loop.
        let build = build(path, options);
        if !guard.accept(Some(build.fingerprint)) {
            continue;
        }

        println!("mjmx dev: {}", build.status);
        // A dropped notification only matters if there are no receivers (no browser
        // connected), in which case there is nothing to reload.
        let _ = reload_tx.send(());
    }

    Ok(())
}

/// (Re-)watch the target file, ignoring errors from a stale or absent watch.
fn arm_file_watch(watcher: &mut impl Watcher, path: &Path) {
    let _ = watcher.unwatch(path);
    let _ = watcher.watch(path, RecursiveMode::NonRecursive);
}

/// Remembers the last build's result so an unchanged result never reloads twice.
///
/// This is the layer that makes a self-feeding rebuild loop impossible: no matter how
/// many events a backend emits for reads, metadata or its own bookkeeping, a reload
/// only happens when the build output actually changes.
struct ContentGuard {
    /// Fingerprint of the last accepted build.
    last: Option<u64>,
}

impl ContentGuard {
    /// Seed the guard with the current build, without counting it as a change.
    fn primed(path: &Path, options: &RenderOptions) -> Self {
        Self {
            last: Some(build(path, options).fingerprint),
        }
    }

    /// The comparison core, split out so it can be tested without touching the disk.
    fn accept(&mut self, fingerprint: Option<u64>) -> bool {
        if fingerprint == self.last {
            return false;
        }
        self.last = fingerprint;
        true
    }
}

/// The outcome of one rebuild: what to log, and a fingerprint of what it produced.
struct Build {
    /// Hash of the rendered HTML, or of the diagnostics when the render failed.
    fingerprint: u64,
    /// The human-readable status line for this build.
    status: String,
}

/// Render the file, returning a status line and a fingerprint of the result.
///
/// The fingerprint covers the *rendered output*, so it changes when any input does —
/// including an `<mj-include>` partial that leaves the entry file untouched. Failed
/// builds are fingerprinted over their diagnostics (with a different domain tag), so
/// ok→error and error→different-error both reload, while the same error twice does
/// not.
fn build(path: &Path, options: &RenderOptions) -> Build {
    match mjmx::render_file(path, options) {
        Ok(html) => Build {
            fingerprint: fingerprint(b"ok", html.as_bytes()),
            status: "rebuilt ok".to_string(),
        },
        Err(diagnostics) => {
            let errors = diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .count();
            let mut text = String::new();
            for diagnostic in &diagnostics {
                text.push_str(&diagnostic.to_string());
                text.push('\n');
            }
            Build {
                fingerprint: fingerprint(b"err", text.as_bytes()),
                status: format!("rebuild failed ({errors} error(s))"),
            }
        }
    }
}

/// Hash `bytes` within a `domain`, so a success and a failure never collide.
fn fingerprint(domain: &[u8], bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    domain.hash(&mut hasher);
    bytes.hash(&mut hasher);
    hasher.finish()
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
/// Deliberately excludes `EventKind::Any`, `Access(..)`, `Modify(Metadata(..))` and
/// `Modify(Any)`: those cover reads and bookkeeping, which a rebuild's own file read
/// would otherwise regenerate forever.
fn is_content_kind(kind: EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Modify(ModifyKind::Data(_))
            | EventKind::Modify(ModifyKind::Name(_))
    )
}

/// Whether a path from an event should be acted on.
///
/// The target file itself, or any sibling `.mjml` (an `<mj-include>` partial).
/// Editor scratch files are rejected first so a save's temp churn stays quiet.
fn is_watched_path(path: &Path, target: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    // vim swapfiles (`.foo.swp`) and dotfiles, backup copies (`foo~`), and vim's
    // `4913` inotify probe file. `sed -i`'s `sedXXXXXX` temp file has no `.mjml`
    // extension and is rejected by the extension check below.
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
    use notify::event::{
        AccessKind, AccessMode, CreateKind, DataChange, MetadataKind, RemoveKind, RenameMode,
    };

    #[test]
    fn content_mutations_are_candidate_changes() {
        assert!(is_content_kind(EventKind::Modify(ModifyKind::Data(
            DataChange::Content
        ))));
        assert!(is_content_kind(EventKind::Modify(ModifyKind::Data(
            DataChange::Any
        ))));
        assert!(is_content_kind(EventKind::Modify(ModifyKind::Name(
            RenameMode::To
        ))));
        assert!(is_content_kind(EventKind::Create(CreateKind::File)));
        assert!(is_content_kind(EventKind::Remove(RemoveKind::File)));
    }

    /// Regression: reads, metadata touches and the catch-all `Any` must not count.
    /// Every rebuild and every `/preview` request reads the file, so accepting any of
    /// these lets the watcher feed itself in an endless rebuild loop.
    #[test]
    fn reads_metadata_and_any_are_not_changes() {
        assert!(!is_content_kind(EventKind::Any));
        assert!(!is_content_kind(EventKind::Other));
        assert!(!is_content_kind(EventKind::Access(AccessKind::Read)));
        assert!(!is_content_kind(EventKind::Access(AccessKind::Open(
            AccessMode::Read
        ))));
        assert!(!is_content_kind(EventKind::Access(AccessKind::Close(
            AccessMode::Read
        ))));
        assert!(!is_content_kind(EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::Any
        ))));
        assert!(!is_content_kind(EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::AccessTime
        ))));
        assert!(!is_content_kind(EventKind::Modify(ModifyKind::Any)));
    }

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/slice.mjml")
    }

    /// Regression for the self-feeding loop: the same result twice must report
    /// "no change" the second time, so a rebuild's own read can never cause another.
    #[test]
    fn content_guard_rejects_repeated_identical_results() {
        let mut guard = ContentGuard { last: None };
        assert!(guard.accept(Some(7)), "first sighting is a change");
        assert!(!guard.accept(Some(7)), "identical result must not reload");
        assert!(!guard.accept(Some(7)), "and must stay quiet forever");
    }

    #[test]
    fn content_guard_accepts_real_edits() {
        let mut guard = ContentGuard { last: Some(1) };
        assert!(guard.accept(Some(2)));
        assert!(!guard.accept(Some(2)));
        assert!(guard.accept(Some(3)));
    }

    /// The end-to-end loop guarantee: rebuilding an untouched file over and over
    /// yields the same fingerprint, so a primed guard never reports a change. This
    /// is what stops a rebuild's own file read from triggering the next rebuild.
    #[test]
    fn repeated_builds_of_an_untouched_file_never_report_a_change() {
        let path = fixture();
        let options = RenderOptions::default();
        let mut guard = ContentGuard::primed(&path, &options);
        for _ in 0..5 {
            let build = build(&path, &options);
            assert!(!guard.accept(Some(build.fingerprint)));
        }
    }

    #[test]
    fn build_reports_status_and_distinguishes_success_from_failure() {
        let options = RenderOptions::default();
        let ok = build(&fixture(), &options);
        assert_eq!(ok.status, "rebuilt ok");

        let missing = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/nope.mjml");
        let failed = build(&missing, &options);
        assert!(failed.status.starts_with("rebuild failed"));
        assert_ne!(ok.fingerprint, failed.fingerprint);
    }

    #[test]
    fn watches_target_and_sibling_mjml_partials() {
        let target = Path::new("/w/mail.mjml");
        assert!(is_watched_path(Path::new("/w/mail.mjml"), target));
        assert!(is_watched_path(Path::new("/w/header.mjml"), target));
        assert!(is_watched_path(Path::new("/w/Footer.MJML"), target));
    }

    #[test]
    fn ignores_editor_scratch_files_and_unrelated_paths() {
        let target = Path::new("/w/mail.mjml");
        assert!(!is_watched_path(Path::new("/w/.mail.mjml.swp"), target));
        assert!(!is_watched_path(Path::new("/w/mail.mjml~"), target));
        assert!(!is_watched_path(Path::new("/w/4913"), target));
        assert!(!is_watched_path(Path::new("/w/.hidden"), target));
        assert!(!is_watched_path(Path::new("/w/sedA1b2C3"), target));
        assert!(!is_watched_path(Path::new("/w/out.html"), target));
    }

    /// Kind and path must *both* match for an event to be a candidate.
    #[test]
    fn change_event_requires_both_kind_and_path() {
        let target = Path::new("/w/mail.mjml");
        let data = EventKind::Modify(ModifyKind::Data(DataChange::Content));

        let hit = Ok(Event::new(data).add_path(target.to_path_buf()));
        assert!(is_change_event(&hit, target));

        let wrong_path = Ok(Event::new(data).add_path(PathBuf::from("/w/out.html")));
        assert!(!is_change_event(&wrong_path, target));

        let wrong_kind =
            Ok(Event::new(EventKind::Access(AccessKind::Read)).add_path(target.to_path_buf()));
        assert!(!is_change_event(&wrong_kind, target));
    }
}
