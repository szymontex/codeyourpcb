//! The audit counts what is there.
//!
//! `cargo test -p cypcb-cli --test the_audit_counts_what_is_there`
//!
//! V9's "Measured on our side" paragraph is the audit's own foundation: every
//! row of the parity table is read against it. Two of its numbers had gone
//! stale by 2026-09-03. It said `cypcb --help` lists ten subcommands when it
//! lists twelve - `from-dxf` and `library` are wired in `main.rs`, with help
//! text, and neither was in the sentence. And it said the rules directory
//! holds twenty rules when it holds 37.
//!
//! Neither number is hard to check; nobody was checking. So they are counted
//! here from the binary and from the tree, and the paragraph is held to them.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// The paragraph the parity table is measured against.
fn measured_paragraph() -> String {
    let tracker = read("docs/TRACKER.md");
    let start = tracker
        .find("**Measured on our side")
        .expect("V9 states what it measured");
    let rest = &tracker[start..];
    let end = rest.find("\n\n").expect("the paragraph ends");
    rest[..end].to_string()
}

/// The count and the names the audit's sentence states, read from the
/// enumeration rather than from the paragraph around it.
fn stated_subcommands(paragraph: &str) -> (usize, BTreeSet<String>) {
    let after = paragraph
        .split("subcommands: ")
        .nth(1)
        .expect("the sentence enumerates them");
    let list = after.split('.').next().expect("the sentence ends");
    let count = paragraph
        .split("lists ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|word| word.parse::<usize>().ok())
        .expect("the sentence states how many");

    let names = list
        .split('`')
        .skip(1)
        .step_by(2)
        .map(|name| name.trim().to_string())
        .collect();
    (count, names)
}

/// Every subcommand `cypcb --help` prints, `help` included.
fn subcommands() -> BTreeSet<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("--help")
        .output()
        .expect("the binary runs");
    let help = String::from_utf8_lossy(&output.stdout).to_string();
    let block = help
        .split("Commands:")
        .nth(1)
        .and_then(|rest| rest.split("\nOptions:").next())
        .expect("the help has a command list");

    block
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let indented = line.len() > trimmed.len();
            let name = trimmed.split_whitespace().next()?;
            (indented && name.chars().all(|c| c.is_ascii_lowercase() || c == '-'))
                .then(|| name.to_string())
        })
        .collect()
}

#[test]
fn the_paragraph_names_every_subcommand_the_binary_has() {
    // The list, not the paragraph. The first version of this case looked for
    // each name anywhere in the paragraph and passed while `library` was
    // missing from the sentence, because the prose beside it explains that
    // `library` was one of the two left out. A mutation found that; the
    // assertion reads the enumeration itself now, and the count it states.
    let paragraph = measured_paragraph();
    let (stated, listed) = stated_subcommands(&paragraph);
    let actual = subcommands();

    assert_eq!(
        listed, actual,
        "the audit's own list disagrees with the binary: it names {listed:?}, \
         `cypcb --help` prints {actual:?}"
    );
    assert_eq!(
        stated,
        actual.len(),
        "the audit says {stated} subcommands and there are {}",
        actual.len()
    );
}

#[test]
fn the_paragraph_states_the_number_of_rules_the_registry_holds() {
    let registered = read("crates/cypcb-drc/src/lib.rs")
        .matches("Box::new(rules::")
        .count();
    let paragraph = measured_paragraph();

    assert!(
        paragraph.contains(&format!("holds {registered}\nrules"))
            || paragraph.contains(&format!("holds {registered} rules")),
        "the registry holds {registered} rules and the audit does not say so:\n{paragraph}"
    );
}

#[test]
fn the_paragraph_this_reads_is_the_paragraph_it_means() {
    // The control. Both cases above look for text, and a lookup that finds the
    // wrong slice - or an empty one - would pass by finding nothing to
    // contradict. So: the slice is a paragraph rather than the file, and it
    // carries the two things it is being asked about.
    let paragraph = measured_paragraph();
    let tracker = read("docs/TRACKER.md");

    assert!(
        paragraph.len() < tracker.len() / 50,
        "the slice is one paragraph, not the file: {} of {} bytes",
        paragraph.len(),
        tracker.len()
    );
    assert!(paragraph.contains("cypcb --help"), "{paragraph}");
    assert!(paragraph.contains("rules"), "{paragraph}");
    assert!(
        subcommands().len() > 5,
        "the help parse found {} commands, which is not a command list",
        subcommands().len()
    );
}
