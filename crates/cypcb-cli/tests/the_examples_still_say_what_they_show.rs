//! Every board in `examples/` is checked, including the ones written to fail.
//!
//! `cargo test` never opened this directory. Eighteen files sit there as the
//! first thing a reader tries, two of them exist to demonstrate a parse error
//! and two to demonstrate a DRC fault, and a change that broke any of them
//! would have been found by a user rather than by the suite.
//!
//! The list of files is not written down here on purpose: the directory is
//! walked, so an example added tomorrow is covered without anybody remembering
//! to add it.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb_binary() -> PathBuf {
    let mut path = std::env::current_exe().expect("the test binary has a path");
    path.pop();
    path.pop();
    path.push("cypcb");
    path
}

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples")
}

fn example_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(examples_dir())
        .expect("the examples directory is there")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "cypcb"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no examples found to check");
    files
}

/// Boards that are there to show what a broken file looks like.
const MEANT_TO_FAIL_PARSING: &[&str] = &["invalid.cypcb", "unknown_keyword.cypcb"];

fn run(args: &[&std::ffi::OsStr]) -> (bool, String) {
    let output = Command::new(cypcb_binary())
        .args(args)
        .output()
        .expect("cypcb runs");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.success(), text)
}

#[test]
fn every_example_parses_except_the_ones_that_teach_a_parse_error() {
    let mut broken: Vec<String> = Vec::new();

    for file in example_files() {
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .expect("a file name")
            .to_string();
        let meant_to_fail = MEANT_TO_FAIL_PARSING.contains(&name.as_str());
        let (ok, text) = run(&["parse".as_ref(), file.as_os_str()]);

        match (ok, meant_to_fail) {
            (false, false) => broken.push(format!("{name}: stopped parsing\n{text}")),
            (true, true) => broken.push(format!(
                "{name}: parses now, and it is in the list of files that demonstrate a parse error"
            )),
            _ => {}
        }
    }

    assert!(
        broken.is_empty(),
        "examples no longer say what they show:\n{}",
        broken.join("\n")
    );
}

#[test]
fn the_board_written_to_show_a_pour_island_still_shows_one() {
    let file = examples_dir().join("pour-island.cypcb");
    let (ok, text) = run(&["check".as_ref(), file.as_os_str()]);

    assert!(!ok, "a board with a fault has to fail the check");
    assert!(
        text.contains("pour-island"),
        "the example exists to demonstrate an orphaned plane, got:\n{text}"
    );
}

#[test]
fn the_board_written_to_show_drc_faults_still_shows_them() {
    // drc-test.cypcb is the checker's own demonstration board. What it must
    // keep doing is fail; the exact count moves whenever a rule is refined,
    // and pinning it here would make every rule change look like a break.
    let file = examples_dir().join("drc-test.cypcb");
    let (ok, text) = run(&["check".as_ref(), file.as_os_str()]);

    assert!(!ok, "the DRC demonstration board has to fail the check");
    assert!(
        text.contains("DRC violation"),
        "and it has to say why, got:\n{text}"
    );
}
