//! The README installs what the build runs.
//!
//! `cargo test -p cypcb-cli --test the_readme_installs_what_the_build_runs`
//!
//! The Quick Start said `cargo install wasm-pack` and named it a prerequisite.
//! Nothing in this repository has run wasm-pack since 2026-08-08:
//! `viewer/build-wasm.sh` runs cargo, `wasm-bindgen` and `wasm-opt` as three
//! steps of its own, and says at the top why. The two tools it does need were
//! not named anywhere a person setting up would read, so a first build failed
//! twice on tools the front page never mentioned.
//!
//! The rule is the general one rather than a grep for wasm-pack: **every tool
//! the README tells you to install has to be a tool something in this
//! repository runs.** A page that sends somebody to install what nothing uses
//! is wrong in the way that costs an afternoon.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Installed to install the others, so nothing here runs it. The README
/// recommends it because `cargo install wasm-bindgen-cli` builds from source
/// and `cargo binstall` fetches the release binary.
const BOOTSTRAP: [&str; 1] = ["cargo-binstall"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root is two directories above this crate")
}

fn read(path: &str) -> String {
    std::fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|error| panic!("{path}: {error}"))
}

/// The crate name after every `cargo install` or `cargo binstall` in the text.
fn installs(readme: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for line in readme.lines() {
        let line = line.trim_start();
        for opening in ["cargo install ", "cargo binstall "] {
            if let Some(rest) = line.strip_prefix(opening) {
                if let Some(name) = rest.split_whitespace().next() {
                    found.insert(name.to_string());
                }
            }
        }
    }
    found
}

/// What the crate calls the binary it installs.
fn binary_of(crate_name: &str) -> &str {
    crate_name.strip_suffix("-cli").unwrap_or(crate_name)
}

/// The words a script actually runs, with its comments dropped.
///
/// The first version of this case searched the scripts as plain text and its
/// mutation walked straight past: `build-wasm.sh` explains in a comment why it
/// stopped using wasm-pack, so a README that told you to install wasm-pack
/// matched that comment and passed. A grep for a name answers whether the name
/// is written down, never whether anything runs it - which is the same mistake
/// K011 records, made again in the case written to catch it.
fn words_scripts_run(text: &str) -> BTreeSet<String> {
    let mut words = BTreeSet::new();
    for line in text.lines() {
        let code = line.split('#').next().unwrap_or("");
        for word in code.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_')) {
            if !word.is_empty() {
                words.insert(word.to_string());
            }
        }
    }
    words
}

/// Every shell script this repository ships, as one text.
fn scripts() -> String {
    let mut text = read("viewer/build-wasm.sh");
    let dir = repo_root().join("scripts");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("the scripts directory is at the workspace root")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|kind| kind == "sh"))
        .collect();
    paths.sort();
    for path in paths {
        text.push('\n');
        text.push_str(&std::fs::read_to_string(&path).expect("a script this repository ships"));
    }
    text
}

#[test]
fn every_tool_the_readme_installs_is_run_by_something_here() {
    let named = installs(&read("README.md"));

    // A reader that finds nothing agrees with every README ever written.
    assert!(
        !named.is_empty(),
        "no install command was read out of README.md, so the reader is broken"
    );

    let run = words_scripts_run(&scripts());
    for crate_name in &named {
        if BOOTSTRAP.contains(&crate_name.as_str()) {
            continue;
        }
        let binary = binary_of(crate_name);
        assert!(
            run.contains(binary),
            "README.md tells the reader to install {crate_name}, and no script \
             in this repository runs {binary}"
        );
    }
}
