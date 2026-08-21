//! Every command an example tells the reader to run, runs.
//!
//! `cargo test -p cypcb-cli --test the_examples_run_the_commands_they_print`
//!
//! Several examples carry a header saying how to try them:
//!
//! ```text
//! // Check it with:
//! //   cypcb check examples/cutout.cypcb
//! ```
//!
//! Nothing ran them. A command line in a comment is a promise to the reader
//! and it rots like any other: a renamed flag, a moved file or a subcommand
//! that grew a required argument all leave it printing something that does not
//! work, and the file it is in still parses.
//!
//! What counts as failure here is narrow on purpose. `cypcb check` on a board
//! nobody routed exits 1 with unrouted pins, and that is the command working.
//! What is refused is the command not being usable at all: a clap error, a
//! parse error, a missing file, a panic.
//!
//! One rule beyond that: an advertised `export` has to carry `--dry-run`. This
//! test runs from the repository root and a real export writes a directory of
//! Gerbers into it.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// The commands one example's comments tell the reader to run.
///
/// A line counts when it is a comment, names `cypcb`, and carries an argument
/// ending in `.cypcb`. That last part is what separates a command from prose
/// about one - several examples say things like "`cypcb check` does NOT check
/// this board for mains safety", which is a sentence and not an instruction.
fn advertised(source: &str) -> Vec<Vec<String>> {
    let mut found = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("//") {
            continue;
        }
        let text = trimmed.trim_start_matches('/').replace('`', " ");
        let Some(at) = text.find("cypcb ") else {
            continue;
        };
        let words: Vec<String> = text[at + "cypcb ".len()..]
            .split_whitespace()
            .map(str::to_string)
            .collect();
        if !words.iter().any(|word| word.ends_with(".cypcb")) {
            continue;
        }
        found.push(words);
    }
    found
}

#[test]
fn every_command_an_example_prints_is_one_that_runs() {
    let root = repo_root();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(root.join("examples"))
        .expect("the examples directory is there")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension()? == "cypcb").then_some(path)
        })
        .collect();
    entries.sort();

    let mut ran = 0usize;
    let mut broken: Vec<String> = Vec::new();

    for path in entries {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        let source = std::fs::read_to_string(&path).expect("the example is readable");

        for words in advertised(&source) {
            if words.first().map(String::as_str) == Some("export")
                && !words.iter().any(|word| word == "--dry-run")
            {
                broken.push(format!(
                    "{name}: advertises `cypcb {}` with no --dry-run, and this test runs from the repository root",
                    words.join(" ")
                ));
                continue;
            }

            ran += 1;
            let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
                .args(&words)
                .current_dir(&root)
                .output()
                .unwrap_or_else(|error| panic!("{name}: the CLI would not start: {error}"));
            let report = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );

            // A board with violations is the command working. A command that
            // cannot be used is not.
            let unusable = report.contains("cypcb::parse::")
                || report.lines().any(|line| line.starts_with("error:"))
                || report.contains("Usage:")
                || output.status.code().is_none();
            if unusable {
                broken.push(format!(
                    "{name}: `cypcb {}` does not run:\n{}",
                    words.join(" "),
                    report.lines().take(4).collect::<Vec<_>>().join("\n")
                ));
            }
        }
    }

    assert!(
        broken.is_empty(),
        "an example prints a command that does not work:\n{}",
        broken.join("\n\n")
    );
    // The guard on the guard. If the header format changes or the directory
    // moves, this walks nothing and passes in silence.
    assert!(
        ran >= 4,
        "expected several examples to advertise a command; ran {ran}"
    );
}
