//! The decision log is current where it says it is.
//!
//! `cargo test -p cypcb-cli --test the_decision_log_is_current_where_it_says_it_is`
//!
//! `.gsd/DECISIONS.md` is a record of what was decided, and prose about the
//! past is right to keep. What is not is the status section under it, which
//! says what is *true now*: one of those sections said fifteen DRC rules were
//! registered, written on 2026-08-06 and read again on 2026-09-05 with the
//! answer 37. It handed the reader the command that counts them, which is the
//! right shape, and still went stale, because a command nobody runs is a
//! number nobody checks.
//!
//! So the two counts those sections state are asked of the code here. Neither
//! case reads prose for its own sake: each takes the number out of the
//! document and the same number out of the thing it describes.

use std::path::{Path, PathBuf};

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

/// The document wraps its lines, so a sentence is only a sentence once its
/// whitespace is one space.
fn collapsed(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The number immediately before `phrase`, or a failure naming the phrase.
///
/// A scanner that finds nothing has to fail here rather than pass quietly:
/// rewording the sentence removes the guard, and removing a guard is the thing
/// this file exists to catch.
fn number_before(document: &str, phrase: &str) -> usize {
    let text = collapsed(document);
    let at = text.find(phrase).unwrap_or_else(|| {
        panic!(".gsd/DECISIONS.md no longer says `{phrase}`, so no number is held to the code")
    });
    let digits: String = text[..at]
        .trim_end()
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    digits
        .parse()
        .unwrap_or_else(|_| panic!(".gsd/DECISIONS.md states no number before `{phrase}`"))
}

#[test]
fn the_rule_count_it_states_is_the_number_registered() {
    let stated = number_before(&read(".gsd/DECISIONS.md"), "rules are registered as of");
    let registered = read("crates/cypcb-drc/src/lib.rs")
        .matches("Box::new(rules::")
        .count();

    assert_eq!(
        stated, registered,
        "the decision log says {stated} DRC rules are registered and \
         crates/cypcb-drc/src/lib.rs registers {registered}"
    );
}

#[test]
fn the_stage_count_it_states_is_the_number_the_gate_runs() {
    let stated = number_before(&read(".gsd/DECISIONS.md"), "stages as of");

    // Every stage announces itself as `echo "[i/total] name"`, which is also
    // what the reader of a gate run counts.
    let gate = read("scripts/quality-gate.sh");
    let headers: Vec<&str> = gate
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("echo \"["))
        .collect();

    for (index, header) in headers.iter().enumerate() {
        let inside = header
            .split_once('[')
            .and_then(|(_, rest)| rest.split_once(']'))
            .map(|(inside, _)| inside)
            .unwrap_or_else(|| panic!("a stage header without brackets: {header}"));
        let expected = format!("{}/{}", index + 1, headers.len());
        assert_eq!(
            inside, expected,
            "scripts/quality-gate.sh announces stage `[{inside}]` where the run \
             reaches `[{expected}]`"
        );
    }

    assert_eq!(
        stated,
        headers.len(),
        "the decision log says the gate runs {stated} stages and \
         scripts/quality-gate.sh runs {}",
        headers.len()
    );
}
