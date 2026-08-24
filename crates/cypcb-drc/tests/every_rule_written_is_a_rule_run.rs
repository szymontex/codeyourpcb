//! A rule nobody registered checks nothing.
//!
//! `cargo test -p cypcb-drc --test every_rule_written_is_a_rule_run`
//!
//! `run_drc` builds its checkers from a hand-written list, and twice a rule
//! has been written, tested and left out of it. `UnroutedPinRule` carries the
//! date it was finally added in a comment beside its own registration;
//! `ViaSpanRule` was measured against the benchmark set with and without it
//! and the difference was 56 violations nobody was being told about. Both had
//! passing unit tests the whole time, because a rule's own test constructs the
//! rule and calls `check` - which is exactly the step `run_drc` was skipping.
//!
//! So this reads the source rather than the behaviour: every `pub struct
//! ...Rule` under `src/rules/` has to appear in the list `run_drc` builds.
//! There is no reflection in Rust to ask a crate what types it declares, and a
//! test that constructs each rule by name would be the same hand-written list
//! with the same hole in it.

use std::fs;
use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// The identifier that follows `marker`, taken while it stays an identifier.
fn names_after(haystack: &str, marker: &str) -> Vec<String> {
    let mut found = Vec::new();
    for piece in haystack.split(marker).skip(1) {
        let name: String = piece
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.ends_with("Rule") {
            found.push(name);
        }
    }
    found.sort();
    found.dedup();
    found
}

/// Every rule type declared under `src/rules/`.
fn rules_written() -> Vec<String> {
    let dir = crate_root().join("src/rules");
    let mut names = Vec::new();
    for entry in fs::read_dir(&dir).expect("the rules directory is readable") {
        let path = entry.expect("a directory entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("a rule module is readable");
        names.extend(names_after(&source, "pub struct "));
    }
    names.sort();
    names.dedup();
    names
}

/// Every rule type `run_drc` puts in its checker list.
fn rules_run() -> Vec<String> {
    let source =
        fs::read_to_string(crate_root().join("src/lib.rs")).expect("the crate root is readable");
    names_after(&source, "Box::new(rules::")
}

#[test]
fn every_rule_written_is_a_rule_run() {
    let written = rules_written();
    let run = rules_run();

    // A parse that found nothing would make the comparison below pass while
    // proving nothing. Thirty rules ship today; this is a floor, not a census,
    // so adding the thirty-first does not fail here.
    assert!(
        written.len() >= 30,
        "only {} rule types were found under src/rules, which means the reader is broken rather than the crate: {written:?}",
        written.len()
    );

    let missing: Vec<&String> = written.iter().filter(|name| !run.contains(name)).collect();
    assert!(
        missing.is_empty(),
        "written and never run, so they report nothing on any board: {missing:?}"
    );
}

#[test]
fn the_checker_list_names_each_rule_once() {
    // The other way a hand-written list goes wrong: the same rule twice, which
    // doubles every violation it finds and moves every published count.
    let source =
        fs::read_to_string(crate_root().join("src/lib.rs")).expect("the crate root is readable");
    let occurrences = source.split("Box::new(rules::").count() - 1;
    let distinct = rules_run().len();

    assert_eq!(
        occurrences, distinct,
        "the checker list has {occurrences} entries and {distinct} distinct rules in it"
    );
}
