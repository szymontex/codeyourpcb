//! The block above a footprint, against the numbers below it.
//!
//! Each constructor in `crates/cypcb-world/src/footprint/` carries a list of
//! dimensions in its doc comment - pitch, pad, row span, body - and then passes
//! the same figures to the builder a few lines further down. The two are
//! written by hand and nothing has ever compared them. **A comment carrying a
//! number is a claim**, and this is the claim a person reads first: somebody
//! choosing a footprint reads the block, not the call.
//!
//! The parse is restricted to those four labels. A doc comment here also lists
//! the parts a footprint is used for and quotes standards, and a figure from
//! that prose in the comparison would make the check noisy enough to be turned
//! off.
//!
//! **Two shapes the first run found, and neither was a wrong figure.** A square
//! body is stated twice in the block and written once in the code, and a row
//! span is stated whole in the block and passed halved, because the builders
//! here take a half-span. So the comparison is over distinct values, and the row
//! span accepts half of what the block says. Both are narrow and both are named:
//! nothing else in this directory is expressed as a fraction of what its block
//! states.
//!
//! **What this cannot catch, said so it is not mistaken for more:** distinct
//! values are a set, so a block whose pitch and row span were swapped against a
//! call that passes both still matches. Positional comparison would need the
//! argument order of every builder in this directory encoded here, and that
//! constant is the thing that goes stale. What it does catch is the case that
//! has actually happened to documents in this repository - a figure changed on
//! one side and left on the other.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The labels whose figures are the footprint's dimensions.
const DIMENSION_LABELS: &[&str] = &["Pitch:", "Pad:", "Row span:", "Body:"];

/// Below this the parse has stopped finding constructors rather than found the
/// directory shrunk. Thirteen carry a block today, across three files.
const BLOCKS_FLOOR: usize = 13;

/// Constructors whose doc comment is a sentence rather than a block, and states
/// a figure in it anyway. The four mounting holes each say their drill in the
/// prose above the call: "M2 mounting hole, 2.2mm drill." Somebody picking a
/// screw size reads that sentence, not the call under it.
const SENTENCES_FLOOR: usize = 4;

fn footprint_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/footprint")
}

/// Every `<number>mm` in a line, in nanometres so the comparison is exact.
fn millimetres(line: &str) -> Vec<i64> {
    let mut found = Vec::new();
    let bytes = line.as_bytes();
    let mut at = 0;
    while let Some(rel) = line[at..].find("mm") {
        let end = at + rel;
        let mut start = end;
        while start > 0 {
            let c = bytes[start - 1] as char;
            if c.is_ascii_digit() || c == '.' {
                start -= 1;
            } else {
                break;
            }
        }
        if start < end {
            if let Ok(value) = line[start..end].parse::<f64>() {
                found.push((value * 1_000_000.0).round() as i64);
            }
        }
        at = end + 2;
    }
    found
}

/// Every `Nm::from_mm(<number>)` in a block of code, in nanometres.
fn built_with(body: &str) -> Vec<i64> {
    let mut found = Vec::new();
    for piece in body.split("Nm::from_mm(").skip(1) {
        let Some(end) = piece.find(')') else { continue };
        if let Ok(value) = piece[..end].trim().parse::<f64>() {
            found.push((value * 1_000_000.0).round() as i64);
        }
    }
    found
}

fn millimetres_of(value: i64) -> String {
    format!("{}mm", value as f64 / 1_000_000.0)
}

#[test]
fn a_footprint_block_states_the_numbers_it_is_built_with() {
    let dir = footprint_dir();
    let mut sources: Vec<(String, String)> = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("the footprint directory is there") {
        let path = entry.expect("a directory entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("a file name")
            .to_string();
        sources.push((
            name,
            std::fs::read_to_string(&path).expect("the file reads"),
        ));
    }
    sources.sort();

    let mut blocks = 0usize;
    let mut figures = 0usize;
    let mut sentences = 0usize;
    let mut in_sentences = 0usize;
    let mut missing: Vec<String> = Vec::new();

    for (file, source) in &sources {
        let lines: Vec<&str> = source.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            if !line.starts_with("pub fn ") || !line.contains("-> Footprint") {
                continue;
            }
            let what = line
                .trim_start_matches("pub fn ")
                .split('(')
                .next()
                .unwrap_or(line);

            // The doc comment immediately above: the four labels, and any
            // figure the prose states when there is no block at all.
            let mut stated: Vec<(bool, i64)> = Vec::new();
            let mut in_prose: Vec<i64> = Vec::new();
            for above in lines[..index].iter().rev() {
                let above = above.trim();
                if above.starts_with("///") {
                    let text = above.trim_start_matches("///").trim();
                    if let Some(rest) = text.strip_prefix("- ") {
                        if DIMENSION_LABELS.iter().any(|label| rest.starts_with(label)) {
                            let halved = rest.starts_with("Row span:");
                            stated.extend(millimetres(rest).into_iter().map(|mm| (halved, mm)));
                        }
                    } else {
                        in_prose.extend(millimetres(text));
                    }
                    continue;
                }
                if above.starts_with("#[") || above.is_empty() {
                    continue;
                }
                break;
            }
            if stated.is_empty() && in_prose.is_empty() {
                continue;
            }

            // The constructor's own body: to the first line that closes it.
            let mut body = String::new();
            for below in &lines[index + 1..] {
                if below.starts_with('}') {
                    break;
                }
                body.push_str(below);
                body.push('\n');
            }

            let available: BTreeSet<i64> = built_with(&body).into_iter().collect();

            if stated.is_empty() {
                // A body that builds no figure at all is not a part being
                // described - `mirrored_to_bottom` explains the flip with "a
                // pad at +1mm ends up at -1mm", which is an illustration and
                // not a dimension anybody could build with.
                if available.is_empty() {
                    continue;
                }
                // **A sentence carrying a figure is a block with no labels.**
                // Nothing halves here: prose states what the call builds, or it
                // is describing a different part.
                sentences += 1;
                in_sentences += in_prose.len();
                for value in &in_prose {
                    if !available.contains(value) {
                        missing.push(format!(
                            "{file}: the sentence above {what} states {} and the call below it \
                             builds with no such figure",
                            millimetres_of(*value)
                        ));
                    }
                }
                continue;
            }

            blocks += 1;
            figures += stated.len();
            for (halved, value) in &stated {
                // A row span is stated whole and built from its half.
                let found =
                    available.contains(value) || (*halved && available.contains(&(value / 2)));
                if !found {
                    missing.push(format!(
                        "{file}: the block above {what} states {} and the call below it builds \
                         with neither that figure nor half of it",
                        millimetres_of(*value)
                    ));
                }
            }
        }
    }

    eprintln!(
        "footprint constructors carrying a dimension block: {blocks}; figures in those blocks: \
         {figures}; stating a figure in prose instead: {sentences} carrying {in_sentences}; \
         not passed by the call below: {}",
        missing.len()
    );

    assert!(
        sentences >= SENTENCES_FLOOR,
        "this check read {sentences} constructors stating a figure in prose, below the floor of \
         {SENTENCES_FLOOR}. A sentence loses its figure quietly - the word stays and the number \
         goes - so if one really was rewritten, lower the floor in the same commit."
    );

    assert!(
        blocks >= BLOCKS_FLOOR,
        "this check found {blocks} constructors with a dimension block and expected at least \
         {BLOCKS_FLOOR}. Either the blocks were rewritten into another shape, or the parse is \
         finding nothing and every figure below agreed by not being compared."
    );
    assert!(
        missing.is_empty(),
        "a footprint's block states a dimension its own constructor does not build with: \
         {missing:#?}\n\
         \n  The block is what a person reads when they pick a footprint, and the call is what \
         the board gets. When they disagree the block is the one that is believed, because it is \
         the one written in words."
    );
}
