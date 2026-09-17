//! A constant's value against the figure the prose above it states.
//!
//! `HEAD_CLEARANCE` in `crates/cypcb-world/src/footprint/mounting.rs` is the
//! case this file was written for: its doc comment says "2mm of radius covers
//! a washer for the sizes here" and the value below it is two million
//! nanometres. Both are right today, and nothing held them together - the
//! footprint census that checks a constructor's block against the literals it
//! passes reads constructors, and this is a constant.
//!
//! A figure in a doc comment is what a reader believes. When the value moves
//! and the sentence does not, the sentence is a lie in a place nobody looks.
//!
//! Scope, stated because it decides the answer: `Nm` constants under
//! `crates/*/src`. A constant whose prose states no figure is counted and
//! skipped - prose that makes no claim cannot contradict one.
//!
//! `cargo test -p cypcb-cli --test a_constant_agrees_with_the_figure_stated_above_it`

use std::path::{Path, PathBuf};

/// Constants of this type found by the walk, today 16.
const CONSTANTS_EXAMINED_FLOOR: usize = 15;

/// Of those, the ones whose prose states a figure in millimetres, today 5.
const STATING_A_FIGURE_FLOOR: usize = 4;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The millimetre value a constant is declared with, for the two ways this
/// workspace declares one: raw nanometres, and `from_mm` with the figure
/// already in millimetres.
fn declared_mm(line: &str) -> Option<f64> {
    let trimmed = line.trim();
    let trimmed = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
    let rest = trimmed.strip_prefix("const ")?;
    let (_name, rest) = rest.split_once(": Nm = ")?;
    if let Some(rest) = rest.strip_prefix("Nm::from_mm(") {
        let value = rest.split(')').next()?;
        return value.replace('_', "").trim().parse::<f64>().ok();
    }
    let rest = rest.strip_prefix("Nm(")?;
    let value = rest.split(')').next()?;
    let value = value.replace('_', "");
    // `Nm::MAX` and `Nm::MIN` are declared from `i64`, which is not a figure.
    value.trim().parse::<i64>().ok().map(|nm| nm as f64 / 1e6)
}

/// The name, for a message that names what it is talking about.
fn declared_name(line: &str) -> String {
    let trimmed = line.trim();
    let trimmed = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
    trimmed
        .strip_prefix("const ")
        .and_then(|rest| rest.split(':').next())
        .unwrap_or("?")
        .to_string()
}

/// Every figure the prose writes in millimetres.
///
/// `2mm` and `0.25 mm` both count; `2mil` and `2m` do not, and a figure
/// followed by a letter is part of a word rather than a measurement.
fn figures_in_mm(prose: &str) -> Vec<f64> {
    let bytes: Vec<char> = prose.chars().collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        if i > 0 && (bytes[i - 1].is_alphanumeric() || bytes[i - 1] == '.') {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == '.') {
            i += 1;
        }
        let number: String = bytes[start..i].iter().collect();
        let mut after = i;
        if after < bytes.len() && bytes[after] == ' ' {
            after += 1;
        }
        let unit: String = bytes[after..bytes.len().min(after + 3)].iter().collect();
        let is_mm = unit.starts_with("mm")
            && !unit
                .chars()
                .nth(2)
                .is_some_and(|c| c.is_alphanumeric() || c == '.');
        if is_mm {
            if let Ok(value) = number.trim_end_matches('.').parse::<f64>() {
                found.push(value);
            }
        }
    }
    found
}

#[test]
fn a_constant_agrees_with_the_figure_stated_above_it() {
    let root = repo_root();
    let mut files = Vec::new();
    for entry in std::fs::read_dir(root.join("crates")).expect("the crates directory is there") {
        let src = entry.expect("a readable entry").path().join("src");
        if src.is_dir() {
            rust_files(&src, &mut files);
        }
    }
    files.sort();
    assert!(
        files.len() > 100,
        "the walk found {} files, which is not this workspace",
        files.len()
    );

    let mut examined = 0usize;
    let mut stating = 0usize;
    let mut disagreeing = Vec::new();

    for path in &files {
        let text = std::fs::read_to_string(path).expect("a readable source file");
        let lines: Vec<&str> = text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            let Some(value_mm) = declared_mm(line) else {
                continue;
            };
            examined += 1;

            let mut prose = String::new();
            let mut above = index;
            while above > 0 {
                let candidate = lines[above - 1].trim();
                if let Some(doc) = candidate.strip_prefix("///") {
                    prose = format!("{doc} {prose}");
                } else if !candidate.starts_with("#[") {
                    break;
                }
                above -= 1;
            }

            let figures = figures_in_mm(&prose);
            if figures.is_empty() {
                continue;
            }
            stating += 1;

            if !figures.iter().any(|f| (f - value_mm).abs() < 1e-9) {
                disagreeing.push(format!(
                    "{}:{} {} is {}mm, and the prose above it states {:?}",
                    path.strip_prefix(&root).unwrap_or(path).display(),
                    index + 1,
                    declared_name(line),
                    value_mm,
                    figures
                ));
            }
        }
    }

    println!(
        "Nm constants examined: {examined} (floor {CONSTANTS_EXAMINED_FLOOR}); \
         stating a figure in the prose above them: {stating} (floor {STATING_A_FIGURE_FLOOR}); \
         disagreeing with it: {}",
        disagreeing.len()
    );

    assert!(
        examined >= CONSTANTS_EXAMINED_FLOOR,
        "the walk found {examined} constants of this type and the workspace had \
         {CONSTANTS_EXAMINED_FLOOR} when this was written - a search that stops finding \
         them passes for the wrong reason"
    );
    assert!(
        stating >= STATING_A_FIGURE_FLOOR,
        "{stating} of them state a figure in prose, against {STATING_A_FIGURE_FLOOR} when \
         this was written - the check reads the prose, so prose it cannot read is the \
         failure mode to catch"
    );
    assert!(
        disagreeing.is_empty(),
        "a constant's value and the sentence above it state different figures:\n  {}\
         \n  The sentence is what a reader believes and the value is what the board gets. \
         Move the sentence with the value, or say what the other figure is.",
        disagreeing.join("\n  ")
    );
}
