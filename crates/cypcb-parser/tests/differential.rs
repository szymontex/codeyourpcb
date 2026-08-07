#![cfg(all(feature = "rust-parser", feature = "tree-sitter-parser"))]
//! The Rust reader is checked against the parser it will replace.
//!
//! `cargo test -p cypcb-parser --features rust-parser --test differential`
//!
//! Both readers fill the same `ast.rs` types, so the comparison is their JSON.
//! Spans are stripped before comparing: tree-sitter's node boundaries and a
//! hand-written reader's are not the same bytes - a rule that includes a
//! trailing keyword, say - and pinning them now would be pinning tree-sitter's
//! shape rather than the language's meaning. Spans are step 1b, and the LSP is
//! the thing that will demand them.
//!
//! The skip list is down to the two boards written to fail parsing, where the
//! open question is error parity rather than coverage: both readers reject
//! them, and whether they say the same thing about them is step 1b.

use std::path::{Path, PathBuf};

use cypcb_parser::{reader, tree_sitter_parse as parse};

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples")
}

/// Boards the reader does not claim, and why.
const NOT_YET: &[(&str, &str)] = &[
    ("invalid.cypcb", "written to fail parsing"),
    ("unknown_keyword.cypcb", "written to fail parsing"),
];

fn covered_examples() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(examples_dir())
        .expect("the examples directory is there")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "cypcb"))
        .filter(|path| {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            !NOT_YET.iter().any(|(skipped, _)| *skipped == name)
        })
        .collect();
    files.sort();
    files
}

/// The AST as JSON, with every `span` removed.
fn shape(ast: &cypcb_parser::SourceFile) -> serde_json::Value {
    let mut json = serde_json::to_value(ast).expect("the AST serializes");
    strip_spans(&mut json);
    json
}

fn strip_spans(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.remove("span");
            for (_, child) in map.iter_mut() {
                strip_spans(child);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                strip_spans(item);
            }
        }
        _ => {}
    }
}

#[test]
fn the_two_readers_agree_on_every_board_the_new_one_claims() {
    let files = covered_examples();
    assert!(
        files.len() >= 8,
        "the reader should cover most of the examples, got {}",
        files.len()
    );

    let mut differences = Vec::new();
    let mut definitions_compared = 0usize;
    for file in files {
        let name = file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let source = std::fs::read_to_string(&file).expect("the example is readable");

        let expected = parse(&source);
        let actual = reader::read(&source);

        if !actual.errors.is_empty() {
            differences.push(format!("{name}: the reader reported {:?}", actual.errors));
            continue;
        }
        definitions_compared += expected.value.definitions.len();
        let (expected, actual) = (shape(&expected.value), shape(&actual.value));
        if expected != actual {
            differences.push(format!(
                "{name}:\n  tree-sitter: {}\n  reader:      {}",
                serde_json::to_string(&expected).unwrap_or_default(),
                serde_json::to_string(&actual).unwrap_or_default(),
            ));
        }
    }

    assert!(
        differences.is_empty(),
        "the readers disagree:\n{}",
        differences.join("\n")
    );

    // Two readers that both return nothing agree perfectly. This is the guard
    // against that: the boards compared above carry real definitions.
    eprintln!("compared {definitions_compared} definitions across the covered boards");
    assert!(
        definitions_compared >= 40,
        "only {definitions_compared} definitions were compared - the test is not exercising much"
    );
}

#[test]
fn the_list_of_boards_it_does_not_claim_is_real() {
    // A skip list that names a file which no longer exists is a skip nobody
    // notices. Each entry has to be a board that is actually there.
    for (name, reason) in NOT_YET {
        assert!(
            examples_dir().join(name).exists(),
            "{name} is skipped for {reason} and does not exist"
        );
    }
}

#[test]
fn a_board_the_reader_covers_still_reports_what_it_cannot_read() {
    // The reader collects errors rather than stopping, so a bad line has to
    // show up rather than being skipped into silence.
    let result = reader::read("board {\n    layers 2\n}\n");
    assert!(
        !result.errors.is_empty(),
        "a board with no name is an error, got none"
    );
}
