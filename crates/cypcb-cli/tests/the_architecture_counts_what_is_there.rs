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

/// The routing canon states this count as well, in its own words, and the copy
/// a reader meets first is the one that reads as current for longest.
fn routing_canon() -> String {
    std::fs::read_to_string(repo_root().join("docs/ROUTING-CANON.md"))
        .expect("the routing canon is there")
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

    // The dry run is what the document is checked against, and until now it was
    // also the only thing this test read. A test named after what export writes
    // opens what export wrote: the same board through a real run, into the same
    // directory, counted on disk. Without it the document and the listing could
    // agree perfectly while the command wrote something else.
    let written = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("export")
        .arg("-o")
        .arg(&dir)
        .arg("examples/blink.cypcb")
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    assert!(written.status.success(), "the export failed");
    assert_eq!(
        files_under(&dir),
        listed,
        "the dry run listed {listed} files and the run wrote {}",
        files_under(&dir)
    );

    let doc = architecture();
    assert!(
        doc.contains(&format!("-> **{listed}** on")),
        "the document has to say {listed} files:\n{}",
        doc.lines()
            .filter(|line| line.contains("wc -l"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    // **Two surfaces, one run.** The canon says this count in its own sentence,
    // and until 2026-09-16 nothing read that sentence: the day the export
    // writes a fifteenth file, the assertion above would have demanded a repair
    // in one document and left the other reading fourteen. A number is allowed
    // to stand in two places when the same run holds both copies.
    let canon = routing_canon();
    let said = format!("**{listed} listed, {listed} written.**");
    assert!(
        canon.contains(&said),
        "the routing canon has to say {said}, and it says:\n{}",
        canon
            .lines()
            .filter(|line| line.contains("listed,"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Every file under a directory, at any depth - an export writes into
/// subdirectories and a count of the top level would miss most of them.
fn files_under(dir: &std::path::Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                files_under(&path)
            } else {
                1
            }
        })
        .sum()
}
