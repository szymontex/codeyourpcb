//! The project is written in English, including its examples.
//!
//! `cargo test -p cypcb-cli --test the_project_speaks_one_language`
//!
//! Four tracked files carried Polish until 2026-08-27: `DESKTOP-SETUP.md`, the
//! `simple-psu` example board, the copy of it the viewer ships as a template,
//! and the user guide, which quoted the example and then explained in a note
//! that its comments were in another language. A design somebody opens from
//! the template panel is the first `.cypcb` most people read.
//!
//! Three things are deliberately out of scope. `.gsd/` and `.planning/` are
//! milestone records that quote the owner in his own language, which is
//! evidence rather than prose - translating it would destroy it - and
//! `docs/TRACKER.md` quotes him the same way. `viewer/svg-pcb` is a vendored
//! copy of somebody else's editor, down to an author's name and a Hershey font
//! table with accented glyph keys in it; a rule this project imposes on its own
//! writing does not reach into a dependency.

use std::path::{Path, PathBuf};

/// Letters no English text has.
/// Written as escapes rather than as the letters themselves, so this file can
/// be read by the rule it enforces.
const POLISH: [char; 18] = [
    '\u{105}', '\u{107}', '\u{119}', '\u{142}', '\u{144}', '\u{f3}', '\u{15b}', '\u{17a}',
    '\u{17c}', '\u{104}', '\u{106}', '\u{118}', '\u{141}', '\u{143}', '\u{d3}', '\u{15a}',
    '\u{179}', '\u{17b}',
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Every text file this repository ships, by the same rule a reader would use:
/// skip what a build wrote, skip the milestone records, read the rest.
fn shipped_text(root: &Path) -> Vec<PathBuf> {
    let skip_dir = [
        "target",
        "node_modules",
        ".git",
        ".gsd",
        ".planning",
        "dist",
        "pkg",
        "test-results",
        "playwright-report",
        // Vendored: somebody else's editor, carried whole.
        "svg-pcb",
    ];
    let text = [
        "rs", "ts", "js", "mjs", "md", "toml", "json", "html", "css", "cypcb", "sh", "bat", "yml",
        "yaml", "c", "h",
    ];
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries {
            let path = entry.expect("a directory entry").path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            if path.is_dir() {
                if !skip_dir.contains(&name.as_str()) {
                    stack.push(path);
                }
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| text.contains(&e))
            {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

#[test]
fn no_shipped_file_is_written_in_polish() {
    let root = repo_root();
    let tracker = root.join("docs").join("TRACKER.md");
    let files = shipped_text(&root);
    assert!(
        files.len() >= 200,
        "this repository has more than 200 text files and this run read {}",
        files.len()
    );

    let mut offenders: Vec<String> = Vec::new();
    for file in files {
        if file == tracker {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let Some(line) = text
            .lines()
            .find(|line| line.chars().any(|c| POLISH.contains(&c)))
        else {
            continue;
        };
        offenders.push(format!(
            "{}: {}",
            file.strip_prefix(&root).unwrap_or(&file).display(),
            line.trim()
        ));
    }

    assert!(
        offenders.is_empty(),
        "the project is written in English and these lines are not:\n{}",
        offenders.join("\n")
    );
}
