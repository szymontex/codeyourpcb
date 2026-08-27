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

/// One row of the traceability table: id, status cell, primary owner.
fn table_rows(requirements: &str) -> Vec<(String, String, String)> {
    requirements
        .lines()
        .filter(|line| line.starts_with("| R"))
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            (cells.len() > 4).then(|| {
                (
                    cells[1].to_string(),
                    cells[3].to_string(),
                    cells[4].to_string(),
                )
            })
        })
        .collect()
}

/// The status a requirement declares for itself.
fn declared(requirements: &str, id: &str) -> String {
    let block = requirements
        .split(&format!("### {id} "))
        .nth(1)
        .unwrap_or_else(|| panic!("{id} has an entry of its own"));
    block
        .lines()
        .find_map(|line| line.trim().strip_prefix("- Status: "))
        .unwrap_or_else(|| panic!("{id} states a status"))
        .trim()
        .to_string()
}

/// The table repeats every status, and a repetition drifts.
///
/// R110, R113 and R115 were moved back to active on 2026-08-08 - they cited
/// E2E suites that do not run - and the table went on calling them validated
/// until 2026-08-27. The status was corrected where it is declared and not
/// where it is repeated.
#[test]
fn the_table_says_what_each_requirement_says() {
    let requirements = std::fs::read_to_string(repo_root().join(".gsd/REQUIREMENTS.md"))
        .expect("the requirements are there");

    let rows = table_rows(&requirements);
    assert!(rows.len() > 20, "the table lists every requirement");

    let mut wrong: Vec<String> = Vec::new();
    for (id, cell, _) in &rows {
        let own = declared(&requirements, id);
        if &own != cell {
            wrong.push(format!("{id}: the table says {cell}, the entry says {own}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "the table repeats a status it no longer has:\n{}",
        wrong.join("\n")
    );
}

/// And the summary's mapped count is the table's own owners.
#[test]
fn the_mapped_count_is_the_owners_in_the_table() {
    let requirements = std::fs::read_to_string(repo_root().join(".gsd/REQUIREMENTS.md"))
        .expect("the requirements are there");

    let mapped = table_rows(&requirements)
        .into_iter()
        .filter(|(_, _, owner)| owner.starts_with('M'))
        .count();
    assert!(mapped > 0, "some requirement is owned by a slice");

    let summary = requirements
        .split_once("## Coverage Summary")
        .expect("the file ends with a coverage summary")
        .1;
    assert!(
        summary.contains(&format!("- Mapped to slices: {mapped}")),
        "the summary counts {mapped} requirements owned by a slice:\n{summary}"
    );
}
