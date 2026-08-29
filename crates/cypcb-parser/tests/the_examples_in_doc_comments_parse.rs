//! Every example written in the language, in a doc comment, parses.
//!
//! `cargo test -p cypcb-parser --test the_examples_in_doc_comments_parse`
//!
//! A fenced ```cypcb block in a doc comment is a claim: this is what the
//! language looks like. Rust runs doctests for ```rust and ignores the rest,
//! so these were read by nobody - and a project whose grammar gains a word
//! most weeks is one where a five-line example goes stale quietly.
//!
//! Found by scanning the crates rather than by listing them here, so an
//! example added tomorrow is held to the same thing without anybody
//! remembering to add it.

use std::fs;
use std::path::{Path, PathBuf};

/// The repository root, from this crate's own directory.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Every `.rs` file under `crates/*/src`.
fn sources(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.join("crates")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // `target` is build output and `tests` is held by its own
                // suites; what is being read here is what ships as documentation.
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name != "target" && name != "tests" {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// One example: where it came from and what it says.
struct Example {
    file: PathBuf,
    line: usize,
    source: String,
}

/// Every fenced `cypcb` block in a file's comments, with its comment markers
/// taken off.
fn examples_in(path: &Path) -> Vec<Example> {
    let text = fs::read_to_string(path).unwrap_or_default();
    let mut out = Vec::new();
    let mut current: Option<(usize, Vec<String>)> = None;

    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim_start();
        let body = line
            .strip_prefix("//!")
            .or_else(|| line.strip_prefix("///"))
            .or_else(|| line.strip_prefix("//"));
        let Some(body) = body else {
            // A fence cannot span a gap in the comment block.
            current = None;
            continue;
        };
        let body = body.strip_prefix(' ').unwrap_or(body);

        match &mut current {
            None => {
                if body.trim_start().starts_with("```cypcb") {
                    current = Some((index + 2, Vec::new()));
                }
            }
            Some((_, lines)) => {
                if body.trim_start().starts_with("```") {
                    let (line, lines) = current.take().expect("a fence that opened");
                    out.push(Example {
                        file: path.to_path_buf(),
                        line,
                        source: lines.join("\n"),
                    });
                } else {
                    lines.push(body.to_string());
                }
            }
        }
    }
    out
}

#[test]
fn every_cypcb_example_in_a_doc_comment_parses() {
    let root = repo_root();
    let examples: Vec<Example> = sources(&root)
        .iter()
        .flat_map(|path| examples_in(path))
        .collect();

    // The guard on the guard: a scanner that found nothing would pass while
    // reading nothing, which is how a census in this project once reported a
    // clean sweep of an empty set.
    assert!(
        examples.len() >= 5,
        "the crates carry cypcb examples in their doc comments: found {}",
        examples.len()
    );

    let mut broken = Vec::new();
    for example in &examples {
        // A block that only shows one construct is not a file, and the parser
        // is asked about files. `version 1` is what a file starts with, so an
        // example that does not say it is given it - the example is about the
        // construct rather than about the header.
        let source = if example.source.contains("version 1") {
            example.source.clone()
        } else {
            format!("version 1\n\n{}", example.source)
        };
        let parsed = cypcb_parser::parse(&source);
        if !parsed.errors.is_empty() {
            broken.push(format!(
                "{}:{}: {}",
                example
                    .file
                    .strip_prefix(&root)
                    .unwrap_or(&example.file)
                    .display(),
                example.line,
                parsed.errors[0]
            ));
        }
    }

    assert!(
        broken.is_empty(),
        "{} of {} examples in doc comments do not parse:\n{}",
        broken.len(),
        examples.len(),
        broken.join("\n")
    );
}
