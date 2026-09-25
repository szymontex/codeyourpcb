//! No exception in a test is keyed by a line number.
//!
//! A guard that scans the source and lets some sites through has to name
//! those sites somehow. The viewer's inner-layer guard named one as a file and
//! a line, and an edit fifteen lines above it moved the site from 583 to 598:
//! the gate went red on a tree with nothing wrong in it, and the fix it asked
//! for was to type a new number. It is keyed by the function the site is in
//! now. This keeps the next guard from starting the same way.
//!
//! It reads every string literal in the test and source trees and fails on one
//! that is a whole `name.ext:NUMBER`. A message that says where a problem is
//! builds that shape at run time and is not a literal, so it is not caught.
//!
//! `cargo test -p cypcb-cli --test no_exception_is_keyed_by_a_line_number`

use std::path::{Path, PathBuf};

/// Files read, today 768; a walk that found nothing would pass.
const FILES_FLOOR: usize = 600;

const EXTENSIONS: [&str; 6] = ["rs", "ts", "tsx", "js", "sh", "toml"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn source_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if name == "node_modules" || name == "target" || name == "dist" || name == "pkg" {
            continue;
        }
        if path.is_dir() {
            source_files(&path, out);
        } else if path
            .extension()
            .is_some_and(|e| EXTENSIONS.iter().any(|x| e == *x))
        {
            out.push(path);
        }
    }
}

/// Whether a literal is a place given as a file and a line: `name.ext:12`.
fn is_file_and_line(literal: &str) -> bool {
    let Some((file, line)) = literal.rsplit_once(':') else {
        return false;
    };
    let Some((stem, ext)) = file.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty()
        && stem
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-./".contains(c))
        && EXTENSIONS.contains(&ext)
        && !line.is_empty()
        && line.chars().all(|c| c.is_ascii_digit())
}

/// The text between each pair of a quote character on one line.
fn literals(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for quote in ['"', '\''] {
        let mut parts = line.split(quote);
        parts.next();
        while let Some(inside) = parts.next() {
            out.push(inside);
            parts.next();
        }
    }
    out
}

#[test]
fn the_matcher_knows_a_file_and_line_from_a_file() {
    // Built rather than written, so this file does not trip its own scan.
    assert!(is_file_and_line(&format!("layers.{}:{}", "ts", 598)));
    assert!(is_file_and_line(&format!("src/main.{}:{}", "rs", 7)));
    assert!(!is_file_and_line("layers.ts"));
    assert!(!is_file_and_line("layers.ts#layerDepth"));
    assert!(!is_file_and_line("12:30"));
    assert_eq!(
        literals(&format!("  '{}': 'why',", format_args!("a.{}:{}", "ts", 3))),
        vec![format!("a.{}:{}", "ts", 3).as_str(), "why"]
    );
}

#[test]
fn no_exception_is_keyed_by_a_line_number() {
    let root = repo_root();
    let mut files = Vec::new();
    for dir in ["crates", "viewer/src", "viewer/e2e", "scripts"] {
        source_files(&root.join(dir), &mut files);
    }
    files.sort();

    let mut keyed_by_line = Vec::new();
    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            for literal in literals(line) {
                if is_file_and_line(literal) {
                    keyed_by_line.push(format!(
                        "{}:{} names a place by its line: {literal:?}",
                        path.strip_prefix(&root).unwrap_or(path).display(),
                        index + 1
                    ));
                }
            }
        }
    }

    println!(
        "files read: {} (floor {FILES_FLOOR}); literals naming a file and a line: {}",
        files.len(),
        keyed_by_line.len()
    );
    assert!(
        files.len() >= FILES_FLOOR,
        "the walk found {} files",
        files.len()
    );
    assert!(
        keyed_by_line.is_empty(),
        "key these by what the line is, not where it is:\n{}",
        keyed_by_line.join("\n")
    );
}
