//! The architecture document's countable claims, counted.
//!
//! `cargo test -p cypcb-cli --test the_architecture_counts_what_is_there`
//!
//! `docs/architecture.md` said fourteen crates when there were eighteen and
//! thirteen exported files when there were fourteen - both written once and
//! read many times, and both the kind of number a command answers in a second.
//! The document states each beside the command now, and this runs the
//! commands.
//!
//! The WASM size is not here: it is a build artifact whose bytes move with the
//! toolchain, so the document carries a measured figure and the command that
//! gives it rather than a promise a test would have to keep.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn architecture() -> String {
    std::fs::read_to_string(repo_root().join("docs/architecture.md"))
        .expect("the architecture document is there")
}

#[test]
fn the_crate_count_is_the_workspace_it_describes() {
    let crates = std::fs::read_dir(repo_root().join("crates"))
        .expect("the crates are there")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .count();
    // `members = ["crates/*", "src-tauri"]`, so the desktop crate is the one
    // that does not live under `crates/`.
    let members = crates + 1;

    let doc = architecture();
    assert!(
        doc.contains(&format!("**{members}** Rust crates")),
        "the document has to say {members} crates:\n{}",
        doc.lines()
            .filter(|line| line.contains("Rust crates"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn the_export_file_count_is_what_export_writes() {
    let dir = std::env::temp_dir().join("cypcb-architecture-count");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("export")
        .arg("--dry-run")
        .arg("-o")
        .arg(&dir)
        .arg("examples/blink.cypcb")
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    assert!(output.status.success(), "the dry run failed");
    let listed = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    assert!(listed > 5, "a dry run lists the files it would write");

    let doc = architecture();
    assert!(
        doc.contains(&format!("-> **{listed}** on")),
        "the document has to say {listed} files:\n{}",
        doc.lines()
            .filter(|line| line.contains("wc -l"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
