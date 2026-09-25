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
                // An absolute literal is not a fixture and `root.join` does not
                // make it one: joining an absolute path throws the base away, so
                // `/tmp/test_board.dsn` was tested for existence at `/tmp`, found
                // there whenever the case that writes it had run on this machine,
                // and reported as a fixture the repository does not carry. This
                // check went red on 2026-09-13 for that reason and had been green
                // on the same tree an hour earlier - **a check whose answer depends
                // on what is lying in a temporary directory is not measuring the
                // repository.** A case writing to a fixed absolute path is a
                // separate defect and belongs to a separate check.
                if literal.starts_with('/') {
                    continue;
                }
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

/// The directory names that make a path belong to a machine rather than to
/// this repository. Built rather than written, because this file is one of the
/// files the walk reads and a check that names what it is looking for finds
/// itself.
fn shared_directories() -> Vec<String> {
    ["tmp", "var", "home", "Users"]
        .iter()
        .map(|name| format!("/{name}/"))
        .collect()
}

/// Every `.rs` file under `crates/`, source and test alike.
fn every_rust_file(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if entry.file_name() == "target" {
                continue;
            }
            every_rust_file(&path, out);
        } else if path.extension().is_some_and(|kind| kind == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_source_names_a_path_that_belongs_to_a_machine() {
    let root = repo_root();
    let itself = root.join(file!());
    let shared = shared_directories();

    let mut files = Vec::new();
    every_rust_file(&root.join("crates"), &mut files);
    files.sort();

    let mut literals = 0usize;
    let mut offenders: Vec<String> = Vec::new();
    for path in &files {
        if path == &itself {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        for literal in string_literals(&source) {
            literals += 1;
            if shared
                .iter()
                .any(|prefix| literal.starts_with(prefix.as_str()))
            {
                offenders.push(format!(
                    "{}: {literal}",
                    path.strip_prefix(&root).unwrap_or(path).display()
                ));
            }
        }
    }

    eprintln!(
        "rust files read: {}; string literals in them: {literals}; naming a machine's own \
         directory: {}",
        files.len(),
        offenders.len()
    );

    assert!(
        literals >= 2000,
        "this walk read {literals} string literals out of {} files, which is too few to be \
         reading the crates. A clean answer from a walk that found nothing is the same clean \
         answer as a tree with nothing wrong in it.",
        files.len()
    );
    assert!(
        offenders.is_empty(),
        "a source here names a path that belongs to a machine rather than to this repository: \
         {offenders:#?}\n\
         \n  Two cases had one until 2026-09-13. A test writing to a fixed name under the \
         machine's temporary directory is written over by the next run of itself, and the file \
         it leaves behind is read by anything else that looks there - one of them was found by \
         the check above and reported as a fixture the repository does not carry, on a tree \
         that had been green an hour before. Use `std::env::temp_dir()` with something unique \
         in the name, or a path under `target/`."
    );
}

/// What each `temp_dir()` call in `source` is joined with, or `None` for a call
/// that is not joined at all. Comment lines are skipped: a doc may describe
/// the fixed name it replaced.
fn temp_dir_joins(source: &str) -> Vec<Option<String>> {
    let call = format!("{}()", "temp_dir");
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut joins = Vec::new();
    let mut rest = code.as_str();
    while let Some(at) = rest.find(&call) {
        rest = &rest[at + call.len()..];
        let Some(argument) = rest.trim_start().strip_prefix(".join(") else {
            joins.push(None);
            continue;
        };
        let mut depth = 1usize;
        let mut end = argument.len();
        for (i, c) in argument.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        joins.push(Some(argument[..end].to_string()));
    }
    joins
}

#[test]
fn no_test_writes_to_a_directory_another_run_shares() {
    let root = repo_root();
    let itself = root.join(file!());
    let mut files = Vec::new();
    every_rust_file(&root.join("crates"), &mut files);
    files.sort();

    let mut calls = 0usize;
    let mut scratch = 0usize;
    let mut offenders: Vec<String> = Vec::new();
    for path in &files {
        if path == &itself {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        scratch += source.matches("scratch_dir(").count();
        for join in temp_dir_joins(&source) {
            calls += 1;
            let unique = join
                .as_deref()
                .is_some_and(|arg| arg.contains("process::id()"));
            if !unique {
                offenders.push(format!(
                    "{}: {}",
                    path.strip_prefix(&root).unwrap_or(path).display(),
                    join.as_deref().unwrap_or("(not joined)")
                ));
            }
        }
    }

    eprintln!(
        "rust files read: {}; temp_dir() calls: {calls}; scratch_dir calls: {scratch}; \
         with a name another run shares: {}",
        files.len(),
        offenders.len()
    );

    assert!(
        calls >= 10 && scratch >= 100,
        "this walk found {calls} temp_dir() calls and {scratch} scratch_dir calls in {} files, \
         which is too few to be reading the crates",
        files.len()
    );
    assert!(
        offenders.is_empty(),
        "a test writes under the machine's temporary directory with a name that does not \
         carry the process id: {offenders:#?}\n\
         \n  Every run of the suite on the machine shares that directory, so the nightly gate \
         and a gate started from another checkout empty it under each other. \
         `saving_a_design_checks_it_again` ran out of time inside a full run that way. Use \
         `cypcb_fixtures::scratch_dir(tag)`, or put `std::process::id()` in the name where \
         that crate is not a dependency."
    );
}
