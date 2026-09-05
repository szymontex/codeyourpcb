//! A test reads only what the repository has.
//!
//! `cargo test -p cypcb-cli --test a_test_reads_only_what_the_repository_has`
//!
//! On 2026-09-05 a case was written that read `.gsd/STATE.md`. It passed here,
//! survived its own mutation, went green through `./scripts/quality-gate.sh`,
//! and would have failed on any fresh clone, because that file is ignored at
//! `.gitignore` line 85. `git add` refusing the document is the only reason it
//! was caught. Nothing in the gate looks at this: every stage runs against a
//! working tree that already has the file.
//!
//! So the question is asked of git rather than of the disk. A path a test reads
//! must be a path the repository carries.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root is two directories above this crate")
}

/// Everything git is tracking, asked once.
fn tracked() -> HashSet<String> {
    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(repo_root())
        .output()
        .expect("git ls-files: the suite runs from a checkout, so git must answer");
    assert!(
        output.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .collect()
}

/// String literals in Rust source, skipping char literals and escapes.
///
/// The first version of this split on `'"'` and took every other piece, which
/// the char literal in that very expression shifted by one - so the check read
/// the gaps between literals instead of the literals, and its own mutation
/// walked straight past it.
fn string_literals(source: &str) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' => {
                let mut j = i + 1;
                j += if bytes.get(j) == Some(&b'\\') { 2 } else { 1 };
                i = if bytes.get(j) == Some(&b'\'') {
                    j + 1
                } else {
                    i + 1
                };
            }
            b'"' => {
                let start = i + 1;
                let mut j = start;
                while j < bytes.len() && bytes[j] != b'"' {
                    j += if bytes[j] == b'\\' { 2 } else { 1 };
                }
                if j <= bytes.len() {
                    out.push(source[start..j.min(bytes.len())].to_owned());
                }
                i = j + 1;
            }
            _ => i += 1,
        }
    }
    out
}

/// Every string literal in the test sources that names a file now on disk.
fn paths_the_tests_read() -> Vec<(String, String)> {
    let root = repo_root();
    let mut found = Vec::new();
    let crates = std::fs::read_dir(root.join("crates")).expect("crates/ is readable");
    for entry in crates.flatten() {
        let tests = entry.path().join("tests");
        if !tests.is_dir() {
            continue;
        }
        for case in std::fs::read_dir(&tests).into_iter().flatten().flatten() {
            let path = case.path();
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap_or_default();
            let name = path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .display()
                .to_string();
            for literal in string_literals(&source) {
                let literal = literal.as_str();
                let looks_like_a_path = literal.contains('/')
                    && !literal.contains(char::is_whitespace)
                    && !literal.contains('\\')
                    && literal
                        .split('/')
                        .next_back()
                        .is_some_and(|f| f.contains('.'));
                // git lists a file once, from the root and with no prefix.
                // A case reaches its fixture either way: `./scripts/x.sh` from
                // the root, or `../../tests/fixtures/y` from its own crate.
                let literal = literal.strip_prefix("./").unwrap_or(literal);
                let literal = literal.strip_prefix("../../").unwrap_or(literal);
                if looks_like_a_path && root.join(literal).is_file() {
                    found.push((name.clone(), literal.to_owned()));
                }
            }
        }
    }
    found
}

#[test]
fn no_case_reads_a_file_the_repository_does_not_carry() {
    let tracked = tracked();
    let read = paths_the_tests_read();

    for (case, path) in &read {
        assert!(
            tracked.contains(path),
            "{case} reads {path}, which git is not tracking - the case passes here and fails on a clone"
        );
    }

    assert!(
        read.len() >= 15,
        "only {} paths were recognised, so this case is not reading the sources it thinks it is",
        read.len()
    );
}
