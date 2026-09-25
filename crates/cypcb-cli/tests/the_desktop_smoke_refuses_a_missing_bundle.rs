//! The desktop smoke test refuses to run without `viewer/dist`.
//!
//! `cargo test -p cypcb-cli --test the_desktop_smoke_refuses_a_missing_bundle`
//!
//! Until 2026-09-23 `scripts/desktop-smoke.sh` photographed a debug build that
//! loads `devUrl`, so with a dev server on port 4321 it passed whatever this
//! tree's bundle looked like, or whether it existed at all. The script now
//! builds the binary with the bundle embedded, and Tauri refuses that build
//! when `viewer/dist` is missing. This case holds the script's own check, which
//! comes first: a missing or empty bundle is an error, not a skip, before
//! anything is built or started.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root is two directories above this crate")
}

/// A scratch tree holding only the smoke script, so `viewer/dist` is whatever
/// the case puts there.
fn tree_with_the_script(name: &str) -> cypcb_fixtures::ScratchDir {
    let root = cypcb_fixtures::scratch_dir(&format!("cypcb-smoke-{name}-{}", std::process::id()));
    std::fs::create_dir_all(root.join("scripts")).expect("the scratch tree is writable");
    std::fs::copy(
        repo_root().join("scripts/desktop-smoke.sh"),
        root.join("scripts/desktop-smoke.sh"),
    )
    .expect("the smoke script is there");
    root
}

fn run_smoke(root: &Path) -> (Option<i32>, String) {
    run_smoke_on(root, None)
}

fn run_smoke_on(root: &Path, ws_port: Option<u16>) -> (Option<i32>, String) {
    let mut command = Command::new("bash");
    command.arg(root.join("scripts/desktop-smoke.sh"));
    if let Some(port) = ws_port {
        command.env("CYPCB_SMOKE_WS_PORT", port.to_string());
    }
    let output = command.output().expect("bash runs the smoke script");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.code(), text)
}

fn assert_refused(root: &Path) {
    let (code, text) = run_smoke(root);
    assert_eq!(
        code,
        Some(1),
        "the smoke did not fail without a bundle:\n{text}"
    );
    assert!(
        text.contains("viewer/dist is empty"),
        "the smoke failed without naming the bundle:\n{text}"
    );
    assert!(
        !text.contains("[SKIP]") && !text.contains("[0/2]"),
        "the smoke got past the bundle check before refusing:\n{text}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn no_bundle_is_an_error() {
    assert_refused(&tree_with_the_script("no-bundle"));
}

#[test]
fn an_empty_bundle_is_an_error() {
    let root = tree_with_the_script("empty-bundle");
    std::fs::create_dir_all(root.join("viewer/dist")).expect("the scratch tree is writable");
    assert_refused(&root);
}

/// Somebody else on the smoke's port is an error, before anything is built.
///
/// Until 2026-09-25 the script connected to the port before the build and
/// bound it after, and a failed bind was swallowed. Two smokes started
/// together both saw it free, and the second app dialled the first run's
/// listener. The port is now bound before the build and held; this case holds
/// it first and expects the refusal.
#[test]
fn a_taken_port_is_an_error() {
    let root = tree_with_the_script("taken-port");
    std::fs::create_dir_all(root.join("viewer/dist")).expect("the scratch tree is writable");
    std::fs::write(root.join("viewer/dist").join("index.html"), "<html></html>")
        .expect("the scratch tree is writable");
    let holder = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port to hold");
    let port = holder.local_addr().expect("a bound address").port();

    let (code, text) = run_smoke_on(&root, Some(port));
    drop(holder);
    let _ = std::fs::remove_dir_all(&root);

    assert_eq!(
        code,
        Some(1),
        "the smoke did not fail on a taken port:\n{text}"
    );
    assert!(
        text.contains(&format!("something already listens on {port}")),
        "the smoke failed without naming the port:\n{text}"
    );
    assert!(
        !text.contains("[SKIP]") && !text.contains("[0/2]"),
        "the smoke got past the port before refusing:\n{text}"
    );
}
