//! mjml 5.4.0's include *sandbox*, verified against real `mjml@5.4.0` output.
//!
//! Ground truth (`npx mjml@5.4.0`, `{ ignoreIncludes: false }`): an include that stays
//! inside the base directory resolves; one that escapes it — via `../`, an absolute
//! path, or a path that does not exist — is *denied*, replaced with the literal comment
//! `<!-- mj-include denied -->`, and rendering continues rather than erroring. An
//! `includePath` entry widens the allowlist by one more permitted root.
//!
//! The sandbox is v5-only; under v4 there is no allowlist (see `options.rs`).

use std::path::PathBuf;

use mjmx::RenderOptions;

/// A throwaway directory tree under the system temp dir, removed on drop.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("mjmx-sbx-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        Scratch { root }
    }

    /// Write `contents` to `rel` under the scratch root, creating parents, and return
    /// the absolute path.
    fn write(&self, rel: &str, contents: &str) -> PathBuf {
        let path = self.root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// v5 with includes opted back in (`Some(false)`), so the sandbox is exercised.
fn v5_allow() -> RenderOptions {
    RenderOptions {
        ignore_includes: Some(false),
        ..Default::default()
    }
}

#[test]
fn v5_sandbox_resolves_inside_base_and_denies_every_escape() {
    let scratch = Scratch::new("mix");
    scratch.write("proj/partials/ok.mjml", "<mj-text>INSIDE OK</mj-text>");
    scratch.write("secret.mjml", "<mj-text>SECRET</mj-text>");
    let main = scratch.write(
        "proj/main.mjml",
        "<mjml><mj-body><mj-section><mj-column>\
         <mj-include path=\"./partials/ok.mjml\" />\
         <mj-include path=\"../secret.mjml\" />\
         <mj-include path=\"/etc/hostname\" />\
         <mj-include path=\"./partials/missing.mjml\" />\
         </mj-column></mj-section></mj-body></mjml>",
    );

    let html = mjmx::render_file(&main, &v5_allow()).expect("renders despite denials");

    // The in-base partial resolves; the escapes do not.
    assert!(html.contains("INSIDE OK"), "in-base include should resolve");
    assert!(!html.contains("SECRET"), "`../` escape must be denied");
    // `../secret`, the absolute `/etc/hostname`, and the missing file are all denied —
    // exactly three, matching real mjml@5.4.0 (a missing file is denied, not read-failed,
    // because it cannot be canonicalized).
    assert_eq!(
        html.matches("<!-- mj-include denied -->").count(),
        3,
        "three escapes should each emit a denied comment:\n{html}"
    );
}

#[test]
fn v5_include_path_widens_the_allowlist() {
    let scratch = Scratch::new("roots");
    scratch.write(
        "layouts/header.mjml",
        "<mj-text>HEADER FROM LAYOUTS</mj-text>",
    );
    let main = scratch.write(
        "proj/main.mjml",
        "<mjml><mj-body><mj-section><mj-column>\
         <mj-include path=\"../layouts/header.mjml\" />\
         </mj-column></mj-section></mj-body></mjml>",
    );

    // Without the extra root, `../layouts` escapes the base and is denied.
    let denied = mjmx::render_file(&main, &v5_allow()).expect("renders");
    assert!(denied.contains("<!-- mj-include denied -->"));
    assert!(!denied.contains("HEADER FROM LAYOUTS"));

    // With `layouts` as an allowed root, the same include resolves.
    let allowed = mjmx::render_file(
        &main,
        &RenderOptions {
            ignore_includes: Some(false),
            include_path: vec![scratch.root.join("layouts")],
            ..Default::default()
        },
    )
    .expect("renders");
    assert!(allowed.contains("HEADER FROM LAYOUTS"));
    assert!(!allowed.contains("<!-- mj-include denied -->"));
}

#[test]
fn v5_sandbox_denies_percent_encoded_traversal() {
    // mjml fully-decodes percent-escapes before checking, so `%2e%2e%2f` (`../`) cannot
    // smuggle an escape past the allowlist. Verified against real mjml@5.4.0.
    let scratch = Scratch::new("enc");
    scratch.write("secret.mjml", "<mj-text>SECRET</mj-text>");
    let main = scratch.write(
        "proj/main.mjml",
        "<mjml><mj-body><mj-section><mj-column>\
         <mj-include path=\"%2e%2e%2fsecret.mjml\" />\
         </mj-column></mj-section></mj-body></mjml>",
    );

    let html = mjmx::render_file(&main, &v5_allow()).expect("renders");
    assert!(!html.contains("SECRET"), "encoded `../` must not escape");
    assert!(html.contains("<!-- mj-include denied -->"));
}

#[cfg(unix)]
#[test]
fn v5_sandbox_denies_a_symlink_escaping_the_base() {
    // A symlink that lives inside the base but points outside it must be denied:
    // containment is checked on the *canonicalized* path, which follows the link to its
    // real target. Verified against real mjml@5.4.0 (its `realpathSync` does the same).
    use std::os::unix::fs::symlink;

    let scratch = Scratch::new("symlink");
    scratch.write("outside.mjml", "<mj-text>SYMLINK TARGET SECRET</mj-text>");
    let main = scratch.write(
        "proj/main.mjml",
        "<mjml><mj-body><mj-section><mj-column>\
         <mj-include path=\"./link.mjml\" />\
         </mj-column></mj-section></mj-body></mjml>",
    );
    // proj/link.mjml -> ../outside.mjml (escapes the proj base).
    symlink("../outside.mjml", main.parent().unwrap().join("link.mjml")).unwrap();

    let html = mjmx::render_file(&main, &v5_allow()).expect("renders");
    assert!(
        !html.contains("SYMLINK TARGET SECRET"),
        "symlink escape denied"
    );
    assert!(html.contains("<!-- mj-include denied -->"));
}

#[test]
fn v4_has_no_sandbox_so_an_escape_still_resolves() {
    // Under v4 there is no allowlist: a `../` include that reads a real file resolves,
    // as it always did before 5.x. (v4 also resolves includes by default.)
    let scratch = Scratch::new("v4");
    scratch.write("secret.mjml", "<mj-text>V4 SEES SECRET</mj-text>");
    let main = scratch.write(
        "proj/main.mjml",
        "<mjml><mj-body><mj-section><mj-column>\
         <mj-include path=\"../secret.mjml\" />\
         </mj-column></mj-section></mj-body></mjml>",
    );

    let html = mjmx::render_file(
        &main,
        &RenderOptions {
            compat: mjmx::MjmlCompat::V4,
            ..Default::default()
        },
    )
    .expect("renders");
    assert!(html.contains("V4 SEES SECRET"), "v4 has no sandbox");
    assert!(!html.contains("<!-- mj-include denied -->"));
}
