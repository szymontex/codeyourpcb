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

use std::path::Path;

/// The keywords a `.cypcb` file can start a definition with.
const TOP_LEVEL: &[&str] = &[
    "version",
    "board",
    "component",
    "net",
    "netclass",
    "zone",
    "keepout",
    "trace",
    "footprint",
    "module",
    "use",
    "import",
    "assert",
    "outline",
];

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
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("docs/SYNTAX.md");
    let guide = std::fs::read_to_string(&path).expect("the syntax guide is in the repo");

    let blocks = code_blocks(&guide);
    assert!(
        blocks.len() > 20,
        "only {} blocks found - the extractor is broken, not the guide",
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

        if !TOP_LEVEL.contains(&keyword) {
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
    eprintln!("checked {checked} of {} blocks", blocks.len());
    for skip in &skipped {
        eprintln!("  not a top-level construct, so not parsed: {skip}");
    }

    assert!(
        failures.is_empty(),
        "the guide teaches shapes the parser rejects:\n{}",
        failures.join("\n")
    );
    assert!(
        checked >= 10,
        "only {checked} concrete examples were checked, which is too few to call this covered"
    );
}
