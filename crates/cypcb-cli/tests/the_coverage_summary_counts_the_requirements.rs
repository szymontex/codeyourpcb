//! `.gsd/REQUIREMENTS.md` counts itself, and the count has to be its own.
//!
//! `cargo test -p cypcb-cli --test the_coverage_summary_counts_the_requirements`
//!
//! The file ends with a Coverage Summary - active, validated, mapped - and
//! nothing checked those figures against the statuses above them. The same
//! kind of line in `.gsd/STATE.md` sat four figures out of date for three
//! weeks and could not be guarded at all, because that file is gitignored.
//! This one is in the repository, so it can be.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// How many requirements carry this status.
fn counted(requirements: &str, status: &str) -> usize {
    requirements
        .lines()
        .filter(|line| line.trim_end() == format!("- Status: {status}"))
        .count()
}

/// What the summary claims for a line like `- Active requirements: 12`.
fn claimed(requirements: &str, label: &str) -> usize {
    let summary = requirements
        .split_once("## Coverage Summary")
        .expect("the file ends with a coverage summary")
        .1;
    summary
        .lines()
        .find_map(|line| line.trim().strip_prefix(&format!("- {label}: ")))
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or_else(|| panic!("the summary states `{label}`:\n{summary}"))
}

#[test]
fn the_summary_is_the_statuses_above_it() {
    let requirements = std::fs::read_to_string(repo_root().join(".gsd/REQUIREMENTS.md"))
        .expect("the requirements are there");

    let active = counted(&requirements, "active");
    let validated = counted(&requirements, "validated");
    assert!(active > 0 && validated > 0, "both kinds exist");

    assert_eq!(
        claimed(&requirements, "Active requirements"),
        active,
        "the summary counts the requirements marked active"
    );
    assert_eq!(
        claimed(&requirements, "Validated"),
        validated,
        "and the ones marked validated"
    );
}
