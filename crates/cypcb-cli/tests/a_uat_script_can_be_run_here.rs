//! A UAT script asks for things this repository has.
//!
//! `cargo test -p cypcb-cli --test a_uat_script_can_be_run_here`
//!
//! Forty-one `*-UAT.md` files under `.gsd/milestones/` tell a person what to
//! click and what to read back. Three of them ask for a debug surface the
//! viewer does not expose - `window.__routingWorker` and
//! `window.__triggerVariantRouting`, which no commit in this clone ever added,
//! and `window.__variantPanel`, whose panel was deleted on purpose by
//! `a9e8c7a` - and one of those three runs two Playwright suites that are in no
//! commit either. M005/S01's own smoke test states the verdict: "if the browser
//! freezes, the worker is NOT active and S01 is broken".
//!
//! So a UAT script names a file this repository has, a `#id` its viewer
//! carries and a `window.__surface` its code sets, or it says in its own
//! `not_in_this_repository:` block what happened to the ones it does not.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Every file name in the repository, ignoring what a build put there.
fn basenames(root: &Path) -> BTreeSet<String> {
    let skip = ["target", "node_modules", ".git", "dist", "test-results"];
    let mut names = BTreeSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries {
            let path = entry.expect("a directory entry").path();
            let Some(name) = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            if path.is_dir() {
                if !skip.contains(&name.as_str()) {
                    stack.push(path);
                }
            } else {
                names.insert(name);
            }
        }
    }
    names
}

/// Everything the viewer's own sources say, as one string to search.
fn viewer_source(root: &Path) -> String {
    let mut text =
        std::fs::read_to_string(root.join("viewer").join("index.html")).unwrap_or_default();
    let mut stack = vec![root.join("viewer").join("src")];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "ts") {
                text.push_str(&std::fs::read_to_string(&path).unwrap_or_default());
            }
        }
    }
    text
}

fn uat_scripts(root: &Path) -> Vec<PathBuf> {
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
                .is_some_and(|n| n.ends_with("-UAT.md"))
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// What a script asks for: file names and `#id`s written in backticks, and
/// `window.__surface` however it is written.
fn asked_for(text: &str) -> BTreeSet<String> {
    let extensions = [".ts", ".rs", ".cypcb", ".wasm", ".html"];
    let mut names = BTreeSet::new();

    let mut rest = text;
    while let Some(open) = rest.find('`') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find('`') else { break };
        let token = &rest[..close];
        rest = &rest[close + 1..];
        if token.contains(char::is_whitespace) || token.contains('(') {
            continue;
        }
        // `.cypcb` on its own is the language's extension and `*.spec.ts` is a
        // glob; neither is a file this repository could be asked for.
        if token.starts_with('.') || token.contains('*') {
            continue;
        }
        if extensions.iter().any(|ext| token.ends_with(ext)) {
            names.insert(token.rsplit('/').next().unwrap_or(token).to_string());
        } else if let Some(id) = token.strip_prefix('#') {
            if !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                names.insert(token.to_string());
            }
        }
    }

    let mut rest = text;
    while let Some(at) = rest.find("window.__") {
        rest = &rest[at + "window.".len()..];
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        names.insert(format!("window.{}", &rest[..end]));
        rest = &rest[end..];
    }

    names
}

/// Does this repository answer what the script asked for?
fn present(name: &str, files: &BTreeSet<String>, source: &str) -> bool {
    match name.strip_prefix("window.") {
        Some(surface) => source.contains(surface),
        None => match name.strip_prefix('#') {
            Some(id) => source.contains(id),
            None => files.contains(name),
        },
    }
}

/// The items of a `not_in_this_repository:` block: the name, then the reason.
fn excuses(text: &str) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if line.trim_end() == "not_in_this_repository:" {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        match line.strip_prefix("  - ") {
            Some(item) => match item.split_once(" - ") {
                Some((name, why)) => rows.push((name.trim().to_string(), why.trim().to_string())),
                None => rows.push((item.trim().to_string(), String::new())),
            },
            None => break,
        }
    }
    rows
}

#[test]
fn what_a_script_asks_for_is_here_or_says_why_not() {
    let root = repo_root();
    let files = basenames(&root);
    let source = viewer_source(&root);
    let scripts = uat_scripts(&root);
    assert!(
        scripts.len() >= 30,
        "there are 41 UAT scripts and this run found {}",
        scripts.len()
    );

    let mut asked = 0usize;
    let mut unexplained: Vec<String> = Vec::new();
    for script in &scripts {
        let text = std::fs::read_to_string(script).expect("a UAT script is readable");
        let excused: BTreeSet<String> = excuses(&text)
            .into_iter()
            .filter(|(_, why)| !why.is_empty())
            .map(|(name, _)| name)
            .collect();
        for name in asked_for(&text) {
            asked += 1;
            if present(&name, &files, &source) || excused.contains(&name) {
                continue;
            }
            unexplained.push(format!(
                "{} asks for {name} and this repository has no such thing",
                script.strip_prefix(&root).unwrap_or(script).display()
            ));
        }
    }

    assert!(
        asked >= 25,
        "the scripts ask for more than 25 named things and this run read {asked}"
    );
    assert!(
        unexplained.is_empty(),
        "a UAT script cannot be run here and does not say so:\n{}",
        unexplained.join("\n")
    );
}

#[test]
fn an_excuse_is_about_something_really_missing() {
    let root = repo_root();
    let files = basenames(&root);
    let source = viewer_source(&root);
    let mut stale: Vec<String> = Vec::new();

    for script in uat_scripts(&root) {
        let text = std::fs::read_to_string(&script).expect("a UAT script is readable");
        let at = script
            .strip_prefix(&root)
            .unwrap_or(script.as_path())
            .display()
            .to_string();
        let asks = asked_for(&text);
        for (name, why) in excuses(&text) {
            if why.is_empty() {
                stale.push(format!("{at} excuses {name} without saying why"));
            }
            if present(&name, &files, &source) {
                stale.push(format!("{at} excuses {name} and this repository has it"));
            }
            if !asks.contains(&name) {
                stale.push(format!("{at} excuses {name}, which it never asks for"));
            }
        }
    }

    assert!(stale.is_empty(), "{}", stale.join("\n"));
}
