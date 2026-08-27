//! Every file a slice summary names is either in the tree or annotated.
//!
//! `cargo test -p cypcb-cli --test a_slice_names_files_that_are_there`
//!
//! `.gsd/milestones/*/slices/*/*-SUMMARY.md` ends with a `key_files:` list -
//! what that slice left behind. Two hundred and thirty-four names across the
//! five milestones, and thirteen of them were not there: M005's whole Web
//! Worker - `routing-worker.ts`, `worker-protocol.ts`, `parse-source.ts` and
//! the two E2E specs that were said to prove it - plus `variant-transform.ts`
//! and the panel it fed. The milestone's own validation ticks five criteria
//! citing those files, and `main.ts` still calls `auto_route_with_params`
//! straight from the main thread, which is the thing R201 forbids.
//!
//! So a name that is not a file has to say what happened to it, in a
//! `key_files_not_in_repo:` block beside the list. Never committed is one
//! answer, deleted by a named commit is another; the one thing a summary
//! cannot do is name a file and leave the reader to find out.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Every slice summary in the repository, in a fixed order.
fn summaries(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.join(".gsd").join("milestones")];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path
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

/// The list items under `key`, up to the next top-level key.
fn block<'a>(text: &'a str, key: &str) -> Vec<&'a str> {
    let mut rows = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if line.trim_end() == key {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        match line.strip_prefix("  - ") {
            Some(item) => rows.push(item.trim()),
            None => break,
        }
    }
    rows
}

/// A `key_files_not_in_repo:` entry: the path, then ` - `, then the reason.
fn annotation(item: &str) -> (&str, &str) {
    match item.split_once(" - ") {
        Some((path, why)) => (path.trim(), why.trim()),
        None => (item.trim(), ""),
    }
}

/// The path a `key_files:` item names. Some entries carry a parenthesised
/// note after the path - `viewer/src/renderer.ts (RenderDiag.highlightedNet
/// added)` - and the note is not part of the name.
fn named_path(item: &str) -> &str {
    item.split_whitespace().next().unwrap_or(item)
}

#[test]
fn a_name_in_key_files_is_a_file_or_says_why_not() {
    let root = repo_root();
    let mut claimed = 0usize;
    let mut unexplained: Vec<String> = Vec::new();

    for summary in summaries(&root) {
        let text = std::fs::read_to_string(&summary).expect("a summary is readable");
        let annotated: BTreeSet<&str> = block(&text, "key_files_not_in_repo:")
            .into_iter()
            .filter(|item| !annotation(item).1.is_empty())
            .map(|item| annotation(item).0)
            .collect();

        for item in block(&text, "key_files:") {
            let file = named_path(item);
            claimed += 1;
            if root.join(file).exists() || annotated.contains(file) {
                continue;
            }
            unexplained.push(format!(
                "{} names {file} and it is not in the tree",
                summary.strip_prefix(&root).unwrap_or(&summary).display()
            ));
        }
    }

    assert!(
        claimed >= 200,
        "the summaries name 234 files and this run found {claimed}, so the parser stopped reading"
    );
    assert!(
        unexplained.is_empty(),
        "a slice names a file nobody can open and nothing says what happened to it:\n{}",
        unexplained.join("\n")
    );
}

#[test]
fn an_annotation_is_about_a_file_that_is_really_gone() {
    // The other direction: an annotation left behind after the file came back
    // is the same lie the other way round, and so is one for a name the
    // summary does not list.
    let root = repo_root();
    let mut stale: Vec<String> = Vec::new();

    for summary in summaries(&root) {
        let text = std::fs::read_to_string(&summary).expect("a summary is readable");
        let listed: BTreeSet<&str> = block(&text, "key_files:")
            .into_iter()
            .map(named_path)
            .collect();
        let at = summary.strip_prefix(&root).unwrap_or(&summary).display();

        for item in block(&text, "key_files_not_in_repo:") {
            let (path, why) = annotation(item);
            if why.is_empty() {
                stale.push(format!("{at} says {path} is missing and does not say why"));
            }
            if root.join(path).exists() {
                stale.push(format!("{at} says {path} is missing and it is in the tree"));
            }
            if !listed.contains(path) {
                stale.push(format!(
                    "{at} annotates {path}, which its key_files does not name"
                ));
            }
        }
    }

    assert!(stale.is_empty(), "{}", stale.join("\n"));
}
