//! A footprint is the size its doc block says.
//!
//! `cargo test -p cypcb-world --test a_footprint_is_the_size_its_doc_block_says`
//!
//! Every constructor under `src/footprint` carries a block a person reads
//! before they pick the part:
//!
//! ```text
//! /// - Pitch: 1.27mm
//! /// - Pad: 1.5mm x 0.6mm
//! /// - Row span: 5.4mm
//! /// - Body: 5.0mm x 4.0mm
//! ```
//!
//! and passes the same figures to the builder a few lines below as
//! `Nm::from_mm(...)`. Nothing held the two together: the census in
//! `scripts/claims-in-comments.sh` counts these among the figures no test
//! names, and a doc block is exactly the kind of prose that keeps its old
//! number when the call gets a new one.
//!
//! Only those labels are read. A doc comment says plenty of other things with
//! a millimetre in them - a pin-1 coordinate the builder computes, a KiCad
//! filename - and a check that read those would be noise rather than a gate.
//!
//! One allowance, and it is named rather than general: a row span is often
//! stored as its half, because that is what a pad position needs.
//! `sot23_5` writes `let half_span = Nm::from_mm(1.2); // row_span / 2 = 2.4 / 2`
//! against a block that states `Row span: 2.4mm`, and both are right. So a
//! `Row span` figure passes if the builder is given it or half of it, and no
//! other label gets that.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const LABELS: [&str; 5] = ["Pitch", "Pad", "Pad size", "Row span", "Body"];

fn footprint_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/footprint")
}

/// Every `<number>mm` in a line, as the number in front of the unit.
fn millimetres(line: &str) -> Vec<f64> {
    let bytes: Vec<char> = line.chars().collect();
    let mut found = Vec::new();
    let mut index = 0;
    while index + 1 < bytes.len() {
        if bytes[index] == 'm' && bytes[index + 1] == 'm' {
            let mut start = index;
            while start > 0 && (bytes[start - 1].is_ascii_digit() || bytes[start - 1] == '.') {
                start -= 1;
            }
            let number: String = bytes[start..index].iter().collect();
            if let Ok(value) = number.parse::<f64>() {
                found.push(value);
            }
            index += 2;
        } else {
            index += 1;
        }
    }
    found
}

/// Every `from_mm(<number>)` in a function body.
fn from_mm_literals(body: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for (_, rest) in body
        .match_indices("from_mm(")
        .map(|(at, _)| (at, &body[at + 8..]))
    {
        let number: String = rest
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        if let Ok(value) = number.parse::<f64>() {
            found.insert(format!("{value}"));
        }
    }
    found
}

/// One constructor: what its doc block states, and what its body is given.
struct Constructor {
    name: String,
    stated: Vec<(String, f64)>,
    built: BTreeSet<String>,
}

/// The doc figures and the body literals of every `pub fn` in one file.
fn constructors(source: &str) -> Vec<Constructor> {
    let lines: Vec<&str> = source.lines().collect();
    let mut pending: Vec<(String, f64)> = Vec::new();
    let mut out = Vec::new();

    for (at, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if let Some(item) = trimmed.strip_prefix("/// - ") {
            if let Some((label, _)) = item.split_once(':') {
                let label = label.trim();
                if LABELS.contains(&label) {
                    pending.extend(
                        millimetres(item)
                            .into_iter()
                            .map(|figure| (label.to_string(), figure)),
                    );
                }
            }
            continue;
        }
        if let Some(signature) = line.strip_prefix("pub fn ") {
            let name = signature
                .split(['(', '<'])
                .next()
                .unwrap_or(signature)
                .to_string();
            let mut body = String::new();
            for next in &lines[at..] {
                body.push_str(next);
                body.push('\n');
                if *next == "}" {
                    break;
                }
            }
            if !pending.is_empty() {
                out.push(Constructor {
                    name,
                    stated: std::mem::take(&mut pending),
                    built: from_mm_literals(&body),
                });
            }
            pending.clear();
        }
    }
    out
}

#[test]
fn every_figure_a_doc_block_states_is_a_figure_the_builder_is_given() {
    let mut checked_functions = 0;
    let mut checked_figures = 0;

    let mut files: Vec<PathBuf> = std::fs::read_dir(footprint_dir())
        .expect("the footprint modules are beside this crate's source")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|kind| kind == "rs"))
        .collect();
    files.sort();

    for path in files {
        let source = std::fs::read_to_string(&path).expect("a module this crate compiles");
        let file = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        for Constructor {
            name,
            stated,
            built,
        } in constructors(&source)
        {
            checked_functions += 1;
            for (label, figure) in stated {
                checked_figures += 1;
                let given = built.contains(&format!("{figure}"))
                    || (label == "Row span" && built.contains(&format!("{}", figure / 2.0)));
                assert!(
                    given,
                    "{file}: {name}'s doc block states {label}: {figure}mm and the \
                     builder is given {built:?}"
                );
            }
        }
    }

    println!("{checked_functions} constructors read, {checked_figures} figures held");

    // A parser that recognises nothing passes every assertion it never makes.
    assert!(
        checked_functions >= 10,
        "only {checked_functions} constructors had a doc block, so the reader is broken"
    );
    assert!(
        checked_figures >= 25,
        "only {checked_figures} figures were read, so the reader is broken"
    );
}
