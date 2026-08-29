//! Every example in the syntax guide has to be something the parser accepts.
//!
//! `docs/SYNTAX.md` is where a person learns the language. An example that no
//! longer parses teaches a shape the tool rejects, and the reader has no way
//! to tell whether they typed it wrong or the document is out of date. The
//! language grew a good deal - `use`, `import`, `assert`, `netclass`,
//! `outline`, `silk`, typed values - and nothing checked the guide against the
//! grammar afterwards.
//!
//! Most blocks in the guide are fragments rather than whole files, so each is
//! wrapped in the smallest document that can hold it. Fragments that are not
//! top-level constructs - a lone `at 10mm, 5mm`, a `width 0.5mm` - cannot be
//! wrapped without inventing a context, so they are counted and named rather
//! than quietly skipped.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// The keywords a `.cypcb` file can start a definition with, out of the
/// generated grammar.
///
/// This was a list in this file and it was two words short: `flex` and
/// `region` landed in the language and nothing here noticed, so every example
/// in the guide that opened with either was quietly counted as "not a
/// top-level construct" and never parsed. A list in a file is a second place
/// to forget, which is the reason the sibling guard reads the grammar.
fn top_level_keywords() -> BTreeSet<String> {
    let grammar: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("crates/cypcb-parser/grammar/src/grammar.json"))
            .expect("the generated grammar is committed"),
    )
    .expect("the generated grammar is JSON");

    let rules = &grammar["rules"];
    let mut keywords = BTreeSet::new();
    for member in rules["_definition"]["members"]
        .as_array()
        .expect("_definition is a choice")
    {
        let name = member["name"].as_str().expect("a choice of symbols");
        collect_leading_strings(&rules[name], &mut keywords);
    }
    keywords
}

/// The string literals a rule can begin with.
fn collect_leading_strings(rule: &serde_json::Value, out: &mut BTreeSet<String>) {
    match rule["type"].as_str() {
        Some("STRING") => {
            out.insert(rule["value"].as_str().unwrap_or_default().to_string());
        }
        Some("SEQ") => {
            if let Some(first) = rule["members"].as_array().and_then(|m| m.first()) {
                collect_leading_strings(first, out);
            }
        }
        Some("CHOICE") => {
            for member in rule["members"].as_array().into_iter().flatten() {
                collect_leading_strings(member, out);
            }
        }
        _ => {
            if !rule["content"].is_null() {
                collect_leading_strings(&rule["content"], out);
            }
        }
    }
}

fn code_blocks(markdown: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;

    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            match current.take() {
                Some(body) => blocks.push(body),
                None => {
                    // A tagged block that is not the language - shell, say.
                    let tag = line.trim().trim_start_matches('`').trim();
                    if tag.is_empty() || tag == "cypcb" {
                        current = Some(String::new());
                    }
                }
            }
        } else if let Some(body) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }

    blocks
}

#[test]
fn every_top_level_example_in_the_guide_parses() {
    check_document("docs/SYNTAX.md", 20, 10);
}

/// The keywords come from the grammar, and this is what says so.
///
/// A list in this file would pass every assertion above while quietly skipping
/// the constructs it had not heard of - which is what it did for `flex` and
/// `region`, and what let a broken example sit in the guide for a week. Each
/// member of `_definition` contributes at least one leading word, so a set
/// smaller than that count is a set built from something other than the
/// grammar.
#[test]
fn the_keywords_come_from_the_grammar_rather_than_from_a_list() {
    let grammar: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("crates/cypcb-parser/grammar/src/grammar.json"))
            .expect("the generated grammar is committed"),
    )
    .expect("the generated grammar is JSON");
    let members = grammar["rules"]["_definition"]["members"]
        .as_array()
        .expect("_definition is a choice")
        .len();

    let keywords = top_level_keywords();
    assert!(
        keywords.len() >= members,
        "the grammar offers {members} kinds of definition and this found {} words: {keywords:?}",
        keywords.len()
    );
}

/// Every other document in `docs/`, swept for examples in the language.
///
/// Measured while this was written: today there are **none**. The seven other
/// documents carry twenty fenced blocks between them and every one is shell,
/// printed output, a table or Rust - `architecture.md` explains the crates
/// rather than teaching the language. The guess that started this sweep was
/// that they opened blocks with `board` and `trace`; they do not, and the
/// number below says so rather than the sentence.
///
/// The sweep stays because the next example somebody writes in one of them is
/// held to the grammar without anybody remembering this file exists.
///
/// `TRACKER.md` is left out on purpose - it is a log of what happened,
/// quoting fragments and printed output by the hundred, and holding a diary
/// to the grammar would be reading it as something it does not claim to be.
#[test]
fn every_top_level_example_in_the_other_documents_parses() {
    let mut read = 0usize;
    let mut seen = 0usize;
    let mut text = 0usize;
    let mut checked = 0usize;
    for entry in std::fs::read_dir(repo_root().join("docs")).expect("docs is in the repo") {
        let path = entry.expect("a directory entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.ends_with(".md") || name == "SYNTAX.md" || name == "TRACKER.md" {
            continue;
        }
        let (blocks, parsed, bytes) = check_document(&format!("docs/{name}"), 0, 0);
        seen += blocks;
        checked += parsed;
        text += bytes;
        read += 1;
    }
    assert!(
        read >= 4,
        "the docs directory carries documents to read: found {read}"
    );
    // The loop really read them: twenty blocks and thousands of characters of
    // block text between the seven documents. The character count is what a
    // faked block count cannot produce - a loop that skipped the files
    // reports zero of it. What this may not assert is that any block is an
    // example in the language: none are today, and a test demanding otherwise
    // would be demanding the documents change.
    assert!(
        seen >= 15,
        "the other documents carry fenced blocks: only {seen} were read across {read} of them"
    );
    assert!(
        text > 500,
        "and those blocks have text in them: {text} characters across {read} documents"
    );
    eprintln!("docs other than the guide: {seen} blocks read, {checked} of them in the language");
}

/// Every example in one document, parsed or named as skipped. Returns how many
/// were parsed, so a caller can hold a whole directory to a total.
fn check_document(
    relative: &str,
    least_blocks: usize,
    least_checked: usize,
) -> (usize, usize, usize) {
    let top_level = top_level_keywords();
    let path = repo_root().join(relative);
    let guide =
        std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("{relative} is in the repo"));

    let blocks = code_blocks(&guide);
    assert!(
        blocks.len() >= least_blocks,
        "only {} blocks found in {relative} - the extractor is broken, not the document",
        blocks.len()
    );

    let mut checked = 0usize;
    let mut skipped: Vec<String> = Vec::new();
    let mut failures: Vec<String> = Vec::new();

    for block in &blocks {
        let first = block
            .lines()
            .find(|line| !line.trim().is_empty() && !line.trim_start().starts_with("//"))
            .unwrap_or("")
            .trim();
        let keyword = first.split_whitespace().next().unwrap_or("");

        if !top_level.contains(keyword) {
            skipped.push(first.chars().take(48).collect());
            continue;
        }

        // A template rather than an example. `<name>` is how this guide writes
        // "put your own here" and `{ ... }` is how it elides a body; neither is
        // language, and parsing them would only prove that angle brackets are
        // not identifiers.
        if block.contains('<') || block.contains("...") {
            skipped.push(format!(
                "template: {}",
                first.chars().take(40).collect::<String>()
            ));
            continue;
        }

        // A message the tool prints rather than a definition. The warning
        // about a board size with no unit opens with the word `board` too, and
        // the only definitions that open no block are `version`, `import` and
        // `use`.
        if !block.contains('{') && !matches!(keyword, "version" | "import" | "use") {
            skipped.push(format!(
                "printed output: {}",
                first.chars().take(40).collect::<String>()
            ));
            continue;
        }

        // Blocks the guide presents as wrong on purpose.
        if block.contains("ERROR") || block.contains("Missing") {
            skipped.push(format!(
                "shown as invalid: {}",
                first.chars().take(40).collect::<String>()
            ));
            continue;
        }

        // The smallest document that can hold a fragment. A block that brings
        // its own `version` is already one.
        let document = if keyword == "version" {
            block.clone()
        } else {
            format!(
                "version 1\n\nboard guide {{\n    size 80mm x 80mm\n    layers 2\n}}\n\n{block}"
            )
        };

        let parsed = cypcb_parser::parse(&document);
        if !parsed.errors.is_empty() {
            failures.push(format!("{first:?}: {:?}", parsed.errors));
        }
        checked += 1;
    }

    // No silent coverage: say what was read and what was not.
    eprintln!("{relative}: checked {checked} of {} blocks", blocks.len());
    for skip in &skipped {
        eprintln!("  not a top-level construct, so not parsed: {skip}");
    }

    assert!(
        failures.is_empty(),
        "{relative} teaches shapes the parser rejects:\n{}",
        failures.join("\n")
    );
    assert!(
        checked >= least_checked,
        "only {checked} concrete examples were checked in {relative}, which is too few to call it covered"
    );
    (
        blocks.len(),
        checked,
        blocks.iter().map(|block| block.len()).sum(),
    )
}
