//! Every construct the language has is shown in the guide.
//!
//! `cargo test -p cypcb-parser --test the_guide_shows_every_construct`
//!
//! `the_syntax_guide_parses.rs` holds the guide to the grammar in one
//! direction: nothing it shows may be something the parser rejects. This is
//! the other direction, and it is the one that had drifted - `docs/SYNTAX.md`
//! documented none of the v2 constructs and three v1 ones. Measured on
//! 2026-08-09, before the sections were written: `assert`, `netclass` and
//! `outline` never appeared at the start of a line in any example, and neither
//! did `import`, so a reader had no way to learn they existed.
//!
//! The list of constructs comes from the generated grammar rather than from a
//! list in this file, because a list in this file is a second place to forget.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// The literal keywords a top-level definition can start with.
///
/// Read out of `grammar/src/grammar.json`: the members of `_definition`, and
/// for each the first string literal its rule requires. `zone_definition`
/// starts with a choice of two words, so both come back.
fn top_level_keywords() -> BTreeSet<String> {
    let grammar: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("crates/cypcb-parser/grammar/src/grammar.json"))
            .expect("the generated grammar is committed"),
    )
    .expect("the generated grammar is JSON");

    let rules = &grammar["rules"];
    let members = rules["_definition"]["members"]
        .as_array()
        .expect("_definition is a choice");

    let mut keywords = BTreeSet::new();
    for member in members {
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
        Some("FIELD")
        | Some("ALIAS")
        | Some("PREC")
        | Some("PREC_LEFT")
        | Some("PREC_RIGHT")
        | Some("REPEAT")
        | Some("REPEAT1")
        | Some("TOKEN")
        | Some("IMMEDIATE_TOKEN") => {
            collect_leading_strings(&rule["content"], out);
        }
        _ => {}
    }
}

/// The first word of every line inside a fenced example in the guide.
fn words_the_guide_shows() -> BTreeSet<String> {
    let guide = std::fs::read_to_string(repo_root().join("docs/SYNTAX.md"))
        .expect("the syntax guide is there");

    let mut shown = BTreeSet::new();
    let mut inside = false;
    for line in guide.lines() {
        if line.trim_start().starts_with("```") {
            inside = !inside;
            continue;
        }
        if !inside {
            continue;
        }
        if let Some(word) = line.split_whitespace().next() {
            shown.insert(word.to_string());
        }
    }
    shown
}

#[test]
fn the_guide_shows_every_top_level_construct() {
    let keywords = top_level_keywords();
    assert!(
        keywords.len() >= 12,
        "the grammar reader found almost nothing, so this test proves nothing: {keywords:?}"
    );

    let shown = words_the_guide_shows();
    let missing: Vec<&String> = keywords.iter().filter(|k| !shown.contains(*k)).collect();

    assert!(
        missing.is_empty(),
        "docs/SYNTAX.md never shows these in an example: {missing:?}"
    );
}
