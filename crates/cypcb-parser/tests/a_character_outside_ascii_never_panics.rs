#![cfg(feature = "rust-parser")]
//! A character longer than a byte, anywhere in any design, is read or refused
//! - never a panic.
//!
//! `cargo test -p cypcb-parser --test a_character_outside_ascii_never_panics`
//!
//! Until 2026-09-25 `board x { size 10mm x 10mm — }` killed the process: the
//! tokenizer stepped one byte into the em dash, and its next slice of the
//! source landed inside the character. The CLI died with exit 134, and the
//! language server and the viewer run the same reader.
//!
//! Every design in the repository is read again with a character of two,
//! three and four bytes put into a sample of its positions. What each
//! insertion must do depends on where it lands, found here by a scan written
//! apart from the tokenizer:
//!
//! - in code, one error, at that spot, naming the character, its line and its
//!   column;
//! - in a string, no complaint about the character, because a name, a value
//!   or a description may be written in any language;
//! - in a comment, exactly the errors the file had before.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use cypcb_parser::{parse, ParseError};

/// Two, three and four bytes: a Polish letter, an em dash, an emoji.
const INSERTED: [char; 3] = ['\u{105}', '\u{2014}', '\u{1F600}'];

/// How many positions of each file are tried, at most.
const POSITIONS_PER_FILE: usize = 240;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the crate sits two levels below the repo root")
}

/// Every `.cypcb` under the folders that hold designs: examples, test
/// fixtures and the viewer's templates.
fn designs() -> Vec<PathBuf> {
    fn walk(dir: &Path, found: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, found);
            } else if path.extension().is_some_and(|ext| ext == "cypcb") {
                found.push(path);
            }
        }
    }
    let root = repo_root();
    let mut found = Vec::new();
    for dir in [
        "examples",
        "tests/fixtures",
        "viewer/public/templates",
        "viewer/e2e/fixtures",
    ] {
        walk(&root.join(dir), &mut found);
    }
    found.sort();
    found
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Place {
    Code,
    String,
    Comment,
}

/// Where a character put at each byte offset would land. Offsets that are
/// not character boundaries are left as `None`. The first region to claim an
/// offset keeps it: the newline that ends a line comment is scanned again as
/// code, but a character put before it is still in the comment.
fn places(source: &str) -> Vec<Option<Place>> {
    let bytes = source.as_bytes();
    let mut place = vec![None; bytes.len() + 1];
    let mut i = 0;
    let mark = |from: usize, to: usize, what: Place, place: &mut Vec<Option<Place>>| {
        for (at, slot) in place.iter_mut().enumerate().take(to + 1).skip(from) {
            if slot.is_none() && source.is_char_boundary(at) {
                *slot = Some(what);
            }
        }
    };
    while i < bytes.len() {
        if bytes[i] == b'"' {
            // Inside the quotes, up to and including the closing one: a
            // character put before it is still in the string.
            let end = bytes[i + 1..]
                .iter()
                .position(|&b| b == b'"')
                .map_or(bytes.len(), |p| i + 1 + p);
            mark(i, i, Place::Code, &mut place);
            mark(i + 1, end, Place::String, &mut place);
            i = end + 1;
        } else if bytes[i..].starts_with(b"//") {
            // Between the two slashes is code: it splits the marker.
            let end = bytes[i..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(bytes.len(), |p| i + p);
            mark(i, i + 1, Place::Code, &mut place);
            mark(i + 2, end, Place::Comment, &mut place);
            i = end;
        } else if bytes[i..].starts_with(b"/*") {
            let close = bytes[i + 2..]
                .windows(2)
                .position(|w| w == b"*/")
                .map_or(bytes.len(), |p| i + 2 + p);
            mark(i, i + 1, Place::Code, &mut place);
            mark(i + 2, close, Place::Comment, &mut place);
            i = (close + 2).min(bytes.len());
        } else {
            mark(i, i, Place::Code, &mut place);
            i += 1;
        }
    }
    if place[bytes.len()].is_none() {
        place[bytes.len()] = Some(Place::Code);
    }
    place
}

/// 1-based line and column, the column in characters.
fn line_and_column(source: &str, offset: usize) -> (usize, usize) {
    let before = &source[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    (line, column)
}

fn at(error: &ParseError) -> Option<usize> {
    use miette::Diagnostic;
    error
        .labels()
        .and_then(|mut labels| labels.next())
        .map(|label| label.offset())
}

#[test]
fn every_design_with_a_wide_character_anywhere_is_read_without_a_panic() {
    let designs = designs();
    assert!(
        designs.len() >= 30,
        "the corpus went missing: {} designs",
        designs.len()
    );

    let mut tried = [0usize; 3];
    let mut panics = Vec::new();
    let mut wrong = Vec::new();

    for path in &designs {
        let name = path
            .strip_prefix(repo_root())
            .unwrap()
            .display()
            .to_string();
        let source = std::fs::read_to_string(path).expect("a design is UTF-8 text");
        let before = parse(&source).errors.len();
        let places = places(&source);
        let boundaries: Vec<usize> = (0..=source.len())
            .filter(|&at| places[at].is_some())
            .collect();
        let step = (boundaries.len() / POSITIONS_PER_FILE).max(1);

        for (n, &offset) in boundaries.iter().step_by(step).enumerate() {
            let inserted = INSERTED[n % INSERTED.len()];
            let place = places[offset].unwrap();
            let mut text = source.clone();
            text.insert(offset, inserted);
            tried[place as usize] += 1;

            let Ok(result) = catch_unwind(AssertUnwindSafe(|| parse(&text))) else {
                panics.push(format!("{name}+{offset} {inserted:?}"));
                continue;
            };
            let about_it: Vec<&ParseError> = result
                .errors
                .iter()
                .filter(|e| matches!(e, ParseError::UnexpectedCharacter { .. }))
                .collect();

            let fine = match place {
                Place::Code => {
                    let (line, column) = line_and_column(&text, offset);
                    let here: Vec<_> = result
                        .errors
                        .iter()
                        .filter(|e| at(e) == Some(offset))
                        .collect();
                    here.len() == 1
                        && matches!(
                            here[0],
                            ParseError::UnexpectedCharacter { character, line: l, column: c, .. }
                                if *character == inserted && *l == line && *c == column
                        )
                }
                Place::String => about_it.is_empty(),
                Place::Comment => result.errors.len() == before,
            };
            if !fine {
                let said: Vec<String> = result.errors.iter().map(|e| e.to_string()).collect();
                wrong.push(format!("{name}+{offset} {place:?} {inserted:?}: {said:?}"));
            }
        }
    }

    eprintln!(
        "{} designs; insertions in code {}, in strings {}, in comments {}",
        designs.len(),
        tried[Place::Code as usize],
        tried[Place::String as usize],
        tried[Place::Comment as usize],
    );
    assert!(
        tried.iter().all(|&n| n > 0),
        "every kind of place was tried: {tried:?}"
    );
    assert!(
        panics.is_empty(),
        "{} panics: {:?}",
        panics.len(),
        &panics[..panics.len().min(10)]
    );
    assert!(
        wrong.is_empty(),
        "{} wrong: {:#?}",
        wrong.len(),
        &wrong[..wrong.len().min(10)]
    );
}

#[test]
fn the_board_that_used_to_panic_names_the_dash_and_where_it_is() {
    let result = parse("board x { size 10mm x 10mm \u{2014} }\n");
    let said: Vec<String> = result.errors.iter().map(|e| e.to_string()).collect();
    assert_eq!(
        said,
        vec!["unexpected character '\u{2014}' (U+2014) at line 1, column 28"]
    );
}

#[test]
fn the_column_counts_characters_not_bytes() {
    // Three two-byte letters before the stray one on its line: column 30
    // counted in characters, 33 counted in bytes.
    let source = "component R1 resistor \"\u{17c}\u{f3}\u{142}w\" \u{2014}\n";
    let said: Vec<String> = parse(source).errors.iter().map(|e| e.to_string()).collect();
    assert!(
        said.iter().any(|s| s.ends_with("at line 1, column 30")),
        "{said:?}"
    );
}

#[test]
fn a_net_name_outside_ascii_is_written_in_quotes_and_reads() {
    let source = "version 1\nnet \"Zasilanie_\u{105}\u{119}\" {\n    R1.1\n}\n";
    let result = parse(source);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}

/// The grammar's parser takes the same insertions without a panic. It slices
/// the source at node boundaries, which tree-sitter keeps on characters.
#[cfg(feature = "tree-sitter-parser")]
#[test]
fn the_grammar_takes_the_same_insertions_without_a_panic() {
    let mut panics = Vec::new();
    let mut tried = 0usize;
    for path in designs() {
        let source = std::fs::read_to_string(&path).expect("a design is UTF-8 text");
        let places = places(&source);
        let boundaries: Vec<usize> = (0..=source.len())
            .filter(|&at| places[at].is_some())
            .collect();
        let step = (boundaries.len() / POSITIONS_PER_FILE).max(1);
        for (n, &offset) in boundaries.iter().step_by(step).enumerate() {
            let mut text = source.clone();
            text.insert(offset, INSERTED[n % INSERTED.len()]);
            tried += 1;
            if catch_unwind(AssertUnwindSafe(|| cypcb_parser::tree_sitter_parse(&text))).is_err() {
                panics.push(format!("{}+{offset}", path.display()));
            }
        }
    }
    assert!(tried > 0);
    assert!(
        panics.is_empty(),
        "{} panics: {:?}",
        panics.len(),
        &panics[..panics.len().min(10)]
    );
}
