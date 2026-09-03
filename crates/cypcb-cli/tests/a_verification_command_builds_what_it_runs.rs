//! A verification command builds what it runs.
//!
//! `cargo test -p cypcb-cli --test a_verification_command_builds_what_it_runs`
//!
//! A document that says "run this to check the claim" has to name a command
//! that cannot be older than the claim. `./target/release/cypcb` can: it is
//! only as new as the last `cargo build`, and this project has already paid
//! for that twice. Once a measurement was recorded from a binary three commits
//! old and read as two DRC paths disagreeing. Once, on 2026-09-03, the binary
//! in the container was five days behind and refused a `corner` the grammar
//! had accepted since August - while `docs/TRACKER.md` was telling a reader to
//! verify its central claims with exactly that path.
//!
//! So the live verification blocks go through `cargo run`, and this holds them
//! there.

use std::path::{Path, PathBuf};

/// The invocation that can be stale: a built binary run straight from `target`.
const STALE: &str = "./target/release/";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn read(path: &str) -> String {
    std::fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|error| panic!("{path}: {error}"))
}

/// Everything from the tracker's `## Verification` heading to the end.
fn verification_section() -> String {
    let tracker = read("docs/TRACKER.md");
    let start = tracker
        .find("\n## Verification\n")
        .expect("the tracker has a Verification section");
    tracker[start..].to_string()
}

#[test]
fn the_trackers_verification_block_runs_the_tree_it_is_in() {
    let section = verification_section();
    assert!(
        !section.contains(STALE),
        "the Verification block tells a reader to run a binary that may predate \
         the claim it verifies:\n{section}"
    );
}

#[test]
fn the_feature_matrix_runs_the_tree_it_is_in() {
    let matrix = read("docs/competition-feature-matrix.md");
    assert!(
        !matrix.contains(STALE),
        "a row of the matrix verifies itself with a binary that may predate it"
    );
}

#[test]
fn the_section_this_reads_is_the_short_one() {
    // The positive control, and the case that matters most. Both assertions
    // above are absences, and an absence is free if the slice is empty or the
    // heading moves. The tracker's history is full of the stale path - it
    // records commands as they were run, which is right - so the file must
    // still contain it while the section must not.
    let tracker = read("docs/TRACKER.md");
    let section = verification_section();

    assert!(
        tracker.contains(STALE),
        "the tracker's history has always quoted this path; if it no longer \
         does, this control has stopped controlling anything"
    );
    assert!(
        section.len() < tracker.len() / 4,
        "the Verification section is the tail of the file, not the file: \
         {} of {} bytes",
        section.len(),
        tracker.len()
    );
    assert!(
        section.contains("cargo run"),
        "and it has to actually carry the commands it is being checked for:\n{section}"
    );
}
