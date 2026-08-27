//! A milestone's requirement outcomes are the statuses that stuck.
//!
//! `cargo test -p cypcb-cli --test a_milestone_outcome_is_the_status_that_stuck`
//!
//! `.gsd/milestones/*/M0*-SUMMARY.md` closes with `requirement_outcomes:`, a
//! list of `from_status` -> `to_status` moves the milestone claims to have
//! made. Twenty-one such moves exist and ten of them never took: M005 records
//! all seven of R201 to R207 as `validated` while `.gsd/REQUIREMENTS.md` has
//! them `active` and no commit in this clone carries the Web Worker they rest
//! on, and M004 records R110, R113 and R115 as `validated` after `4b2d2c8`
//! moved them back for citing test suites that do not run.
//!
//! A summary is a record and stays written as it was. What it has to do is
//! agree with the requirements file, or name the requirement in its own
//! `outcomes_not_in_effect:` block with what happened instead.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// What each requirement says its own status is.
fn declared(root: &Path) -> BTreeMap<String, String> {
    let text = std::fs::read_to_string(root.join(".gsd").join("REQUIREMENTS.md"))
        .expect("the requirements file is readable");
    let mut statuses = BTreeMap::new();
    let mut at: Option<String> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("### ") {
            at = rest
                .split_whitespace()
                .next()
                .filter(|id| id.starts_with('R'))
                .map(str::to_string);
        } else if let Some(status) = line.strip_prefix("- Status: ") {
            if let Some(id) = at.take() {
                statuses.insert(id, status.trim().to_string());
            }
        }
    }
    statuses
}

fn milestone_summaries(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let milestones = root.join(".gsd").join("milestones");
    let Ok(entries) = std::fs::read_dir(&milestones) else {
        return found;
    };
    for entry in entries {
        let dir = entry.expect("a directory entry").path();
        if !dir.is_dir() {
            continue;
        }
        let Ok(inner) = std::fs::read_dir(&dir) else {
            continue;
        };
        for file in inner {
            let path = file.expect("a directory entry").path();
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with("-SUMMARY.md"))
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// The `- id: Rxxx` / `to_status: ...` pairs, in the order they are written.
fn outcomes(text: &str) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    let mut id: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("- id: ") {
            id = Some(rest.trim().to_string());
        } else if let Some(rest) = trimmed.strip_prefix("to_status: ") {
            if let Some(found) = id.take() {
                rows.push((found, rest.trim().to_string()));
            }
        }
    }
    rows
}

/// The items of an `outcomes_not_in_effect:` block: the id, then the reason.
fn excused(text: &str) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if line.trim_end() == "outcomes_not_in_effect:" {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        match line.strip_prefix("  - ") {
            Some(item) => match item.split_once(" - ") {
                Some((id, why)) => rows.push((id.trim().to_string(), why.trim().to_string())),
                None => rows.push((item.trim().to_string(), String::new())),
            },
            None => break,
        }
    }
    rows
}

#[test]
fn an_outcome_agrees_with_the_requirement_or_says_it_did_not_take() {
    let root = repo_root();
    let statuses = declared(&root);
    assert!(
        statuses.len() >= 20,
        "the requirements file declares 28 statuses and this run read {}",
        statuses.len()
    );

    let mut claimed = 0usize;
    let mut wrong: Vec<String> = Vec::new();
    for summary in milestone_summaries(&root) {
        let text = std::fs::read_to_string(&summary).expect("a milestone summary is readable");
        let excuses: Vec<(String, String)> = excused(&text)
            .into_iter()
            .filter(|(_, why)| !why.is_empty())
            .collect();
        let at = summary
            .strip_prefix(&root)
            .unwrap_or(summary.as_path())
            .display()
            .to_string();

        for (id, to) in outcomes(&text) {
            claimed += 1;
            let now = statuses.get(&id).map(String::as_str).unwrap_or("no status");
            if now == to || excuses.iter().any(|(excused_id, _)| excused_id == &id) {
                continue;
            }
            wrong.push(format!("{at} moved {id} to {to} and it is {now} today"));
        }
    }

    assert!(
        claimed >= 15,
        "the milestone summaries record 21 outcomes and this run read {claimed}"
    );
    assert!(
        wrong.is_empty(),
        "a milestone claims a status the requirement does not have:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn an_excused_outcome_is_one_that_really_disagrees() {
    let root = repo_root();
    let statuses = declared(&root);
    let mut stale: Vec<String> = Vec::new();

    for summary in milestone_summaries(&root) {
        let text = std::fs::read_to_string(&summary).expect("a milestone summary is readable");
        let at = summary
            .strip_prefix(&root)
            .unwrap_or(summary.as_path())
            .display()
            .to_string();
        let claimed = outcomes(&text);

        for (id, why) in excused(&text) {
            if why.is_empty() {
                stale.push(format!("{at} excuses {id} without saying why"));
            }
            match claimed.iter().find(|(claimed_id, _)| claimed_id == &id) {
                None => stale.push(format!("{at} excuses {id}, which it never claims to move")),
                Some((_, to)) => {
                    let now = statuses.get(&id).map(String::as_str).unwrap_or("no status");
                    if now == to {
                        stale.push(format!(
                            "{at} excuses {id} and the requirement really is {to}"
                        ));
                    }
                }
            }
        }
    }

    assert!(stale.is_empty(), "{}", stale.join("\n"));
}
