//! Every assertion a shipped example makes about itself holds.
//!
//! `cargo test -p cypcb-cli --test the_examples_keep_the_promises_they_make`
//!
//! `assert` is how a design states a claim the checker then measures. An
//! example that ships with a failing one teaches the reader that assertions
//! are decorative, and `examples/v2-constraints.cypcb` shipped with three of
//! them for months: first because `within` answered every question with "not
//! checked", then because the part it asks about stated nothing. Both are
//! fixed, and this is what stops a third reason arriving quietly.
//!
//! Run through the built CLI rather than through `sync_ast_to_world`, because
//! an example may `import` from another file and resolving that is the
//! command's job. The first draft of this test called the sync directly and
//! reported `v2-imports.cypcb` as broken when nothing was wrong with it.
//!
//! Only assertions are checked. An example with unrouted pins is a board
//! nobody routed, which is most of them and is not a broken promise;
//! `drc-test.cypcb` is deliberately faulty and says so in its name.

use std::path::{Path, PathBuf};
use std::process::Command;

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples")
}

#[test]
fn no_example_ships_a_failing_assertion() {
    let mut asserted = 0usize;
    let mut broken: Vec<String> = Vec::new();

    let mut entries: Vec<PathBuf> = std::fs::read_dir(examples())
        .expect("the examples directory is there")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension()? == "cypcb").then_some(path)
        })
        .collect();
    entries.sort();
    assert!(!entries.is_empty(), "no examples were found at all");

    for path in entries {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        let source = std::fs::read_to_string(&path).expect("the example is readable");
        if !source
            .lines()
            .any(|line| line.trim_start().starts_with("assert "))
        {
            continue;
        }
        asserted += 1;

        let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
            .arg("check")
            .arg(&path)
            .output()
            .expect("the CLI runs");
        let report = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        for line in report.lines() {
            if line.contains("assertion") && line.contains("at (") {
                broken.push(format!("{name}: {}", line.trim()));
            }
        }
    }

    assert!(
        broken.is_empty(),
        "an example makes a claim it does not keep:\n{}",
        broken.join("\n")
    );
    // The guard on the guard: a rename or a moved directory that left this
    // walking nothing would otherwise pass in silence.
    assert!(
        asserted >= 2,
        "expected several examples to carry an assert; found {asserted}"
    );
}
