//! Nothing in this workspace is written and never called.
//!
//! `cargo test -p cypcb-cli --test no_crate_is_written_and_never_called`
//!
//! D3 asked what to tidy and was closed on 2026-08-2x with "there is nothing
//! to delete": `cypcb-calc` has three callers, `cypcb-platform` one, and
//! `cypcb-watcher` got one when `cypcb watch` shipped. That measurement named
//! the four crates the question had been asked about and stopped there, so it
//! could not find the one nobody had asked about.
//!
//! This counts every crate rather than four of them. A crate is called when
//! another crate names it in its manifest, and it is a program when it states
//! a `[[bin]]`; anything that is neither is code nobody runs, and the list of
//! those is written down here so that a new one fails this test rather than
//! joining a category nobody is counting.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Every manifest in the workspace: the crates, and the desktop shell.
fn manifests() -> Vec<(String, String)> {
    let mut found = Vec::new();
    let crates = workspace().join("crates");
    for entry in std::fs::read_dir(&crates).expect("the crates directory exists") {
        let path = entry.expect("a directory entry").path();
        let manifest = path.join("Cargo.toml");
        if manifest.exists() {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .expect("a crate name")
                .to_string();
            let text = std::fs::read_to_string(&manifest).expect("the manifest is readable");
            found.push((name, text));
        }
    }
    let desktop = workspace().join("src-tauri").join("Cargo.toml");
    if desktop.exists() {
        found.push((
            "src-tauri".to_string(),
            std::fs::read_to_string(&desktop).expect("the manifest is readable"),
        ));
    }
    found
}

/// The crates nothing else names in a manifest and which state no program of
/// their own.
fn uncalled() -> BTreeSet<String> {
    let manifests = manifests();
    let mut found = BTreeSet::new();
    for (name, own) in &manifests {
        if name == "src-tauri" {
            continue;
        }
        let called = manifests
            .iter()
            .any(|(other, text)| other != name && text.contains(name.as_str()));
        let is_program = own.contains("[[bin]]");
        if !called && !is_program {
            found.insert(name.clone());
        }
    }
    found
}

#[test]
fn the_crates_nobody_calls_are_the_ones_written_down_here() {
    // `cypcb-library` is 3751 lines and 41 passing tests behind a SQLite
    // dependency, and nothing in this workspace mentions it: not a manifest,
    // not a `use`. It builds and its tests pass, which is why it survived a
    // question that only looked at four other crates.
    //
    // It is listed rather than deleted because deleting a crate is the owner's
    // call, and listed rather than ignored because a category nobody counts is
    // how it got here.
    let expected: BTreeSet<String> = ["cypcb-library".to_string()].into_iter().collect();
    assert_eq!(
        uncalled(),
        expected,
        "a crate that is neither called nor a program is either new work \
         nothing reaches yet or work nothing will ever reach again - and \
         either way this list has to say which"
    );
}

#[test]
fn the_binaries_are_the_ones_that_state_a_program() {
    // The other two crates with no dependants are programs: `cypcb-cli` is the
    // tool this project is, and `cypcb-lsp` is the language server the editor
    // starts. Neither is uncalled - they are called by a person.
    let manifests = manifests();
    let programs: BTreeSet<String> = manifests
        .iter()
        .filter(|(name, text)| name != "src-tauri" && text.contains("[[bin]]"))
        .map(|(name, _)| name.clone())
        .collect();

    assert!(
        programs.contains("cypcb-cli"),
        "the tool states a program: {programs:?}"
    );
    assert!(
        programs.contains("cypcb-lsp"),
        "and so does the language server: {programs:?}"
    );
}
