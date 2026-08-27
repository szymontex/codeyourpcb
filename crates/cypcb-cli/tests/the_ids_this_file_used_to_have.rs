//! The requirements file was renumbered, and the old ids are still out there.
//!
//! `cargo test -p cypcb-cli --test the_ids_this_file_used_to_have`
//!
//! `4c7a49f` replaced 64 requirements under seven prefixes - `DESK`, `DOC`,
//! `EDIT`, `LIB`, `PLAT`, `UI`, `WEB` - with 21 R-numbered ones, and wrote no
//! mapping between the two. M001, M002 and M003 were signed off against the
//! old list, so their documents still name ids the file no longer has.
//!
//! `.gsd/REQUIREMENTS.md` now says so under "The ids this file used to have",
//! with two counts in it. This re-takes both from the documents themselves,
//! and holds the file to the one promise it can keep about them: a superseded
//! id is not a requirement this file declares.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn requirements(root: &Path) -> String {
    std::fs::read_to_string(root.join(".gsd").join("REQUIREMENTS.md"))
        .expect("the requirements file is readable")
}

/// The ids under `superseded_ids:`, several to a line.
fn superseded(text: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if line.trim_end() == "superseded_ids:" {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        match line.strip_prefix("  - ") {
            Some(item) => ids.extend(item.split_whitespace().map(str::to_string)),
            None => break,
        }
    }
    ids
}

/// A number the section publishes, read from its own line.
fn published(text: &str, label: &str) -> usize {
    let line = text
        .lines()
        .find(|line| line.starts_with(label))
        .unwrap_or_else(|| panic!("the section publishes a line starting `{label}`"));
    line.rsplit(':')
        .next()
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or_else(|| panic!("`{line}` ends with a number"))
}

/// Every document under `.gsd/` except the requirements file itself.
fn other_documents(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.join(".gsd")];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "md")
                && path.file_name().is_some_and(|n| n != "REQUIREMENTS.md")
            {
                found.push(path);
            }
        }
    }
    found
}

#[test]
fn the_two_counts_are_what_the_documents_say() {
    let root = repo_root();
    let text = requirements(&root);
    let ids = superseded(&text);
    let unique: BTreeSet<&String> = ids.iter().collect();
    assert_eq!(
        ids.len(),
        unique.len(),
        "the superseded list repeats an id: {ids:?}"
    );

    let claimed_total = published(&text, "- Superseded ids:");
    assert_eq!(
        ids.len(),
        claimed_total,
        "the section says {claimed_total} superseded ids and lists {}",
        ids.len()
    );

    let corpus: String = other_documents(&root)
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap_or_default())
        .collect();
    let still_named = ids.iter().filter(|id| corpus.contains(id.as_str())).count();
    let claimed_named = published(&text, "- Still named elsewhere under");
    assert_eq!(
        still_named, claimed_named,
        "the section says {claimed_named} of the old ids are still named and the documents name {still_named}"
    );
}

#[test]
fn a_superseded_id_is_not_a_requirement_this_file_declares() {
    let root = repo_root();
    let text = requirements(&root);
    let declared: BTreeSet<String> = text
        .lines()
        .filter_map(|line| line.strip_prefix("### "))
        .filter_map(|rest| rest.split_whitespace().next())
        .map(str::to_string)
        .collect();
    assert!(
        declared.len() >= 20,
        "the file declares 28 requirements and this run read {}",
        declared.len()
    );

    let clash: Vec<String> = superseded(&text)
        .into_iter()
        .filter(|id| declared.contains(id))
        .collect();
    assert!(
        clash.is_empty(),
        "an id listed as superseded is declared in this file: {clash:?}"
    );
}
