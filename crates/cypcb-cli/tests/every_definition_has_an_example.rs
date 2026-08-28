//! Every definition the language has is in an example.
//!
//! `cargo test -p cypcb-cli --test every_definition_has_an_example`
//!
//! A construct with no example is a construct nobody runs. That is where this
//! project keeps finding its defects: the via that produced no copper, the
//! neck the differential test had never been asked about, the blind span with
//! no board to place it on - each of them was a shape the language had and no
//! file used.
//!
//! Two were left when this was written. **`diffpair`** names two nets as one
//! signal and `DiffPairSkewRule` measures the copper between them against the
//! fab's length-match tolerance; **`keepout`** states an area copper may not
//! enter, and `to-kicad` has a branch that writes it as a KiCad rule area.
//! Neither appeared in a single example, so `examples/usb-diff-pair.cypcb` and
//! `examples/keepout.cypcb` were written.
//!
//! The list of definitions is read out of `grammar.js` rather than kept here,
//! so a construct added to the language fails this test until it has a file
//! somebody can copy.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// The keyword each top-level definition in the grammar opens with.
fn definitions() -> Vec<String> {
    let grammar =
        std::fs::read_to_string(repo_root().join("crates/cypcb-parser/grammar/grammar.js"))
            .expect("the grammar is in the repo");

    let list = grammar
        .split("_definition: $ => choice(")
        .nth(1)
        .expect("the grammar names its top-level definitions in one place")
        .split("),")
        .next()
        .expect("that list ends");

    // The zone block is four keywords in one rule - `zone`, `keepout`, `flex`
    // and `region` - so the list of definitions does not name three of them. A
    // census that reads only the choice above would have called this file
    // complete while `keepout` was in no example, which is exactly what it did
    // the first time it was run.
    let kinds = grammar
        .split("field('kind', choice(")
        .nth(1)
        .expect("the zone block chooses between its four words")
        .split(')')
        .next()
        .expect("that choice ends");
    let zone_words: Vec<String> = kinds
        .split(',')
        .map(|word| word.trim().trim_matches('\'').to_string())
        .filter(|word| !word.is_empty())
        .collect();
    assert_eq!(
        zone_words.len(),
        4,
        "four words open a zone block: {zone_words:?}"
    );

    let mut found: Vec<String> = zone_words;
    found.extend(
        list.lines()
            .filter_map(|line| line.trim().strip_prefix("$."))
            .filter_map(|rest| rest.split(',').next())
            .map(|rule| {
                // `board_definition` -> `board`, `import_statement` -> `import`,
                // and the one that is neither: placing a module is `use`.
                let rule = rule.trim();
                match rule {
                    "module_instance" => "use".to_string(),
                    _ => rule
                        .trim_end_matches("_definition")
                        .trim_end_matches("_statement")
                        .to_string(),
                }
            })
            .filter(|word: &String| word != "zone"),
    );
    found
}

#[test]
fn every_definition_the_grammar_has_appears_in_some_example() {
    let definitions = definitions();
    assert!(
        definitions.len() >= 13,
        "thirteen definitions were in the grammar when this was written: {definitions:?}"
    );

    let dir = repo_root().join("examples");
    let mut sources: Vec<(String, String)> = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("the examples are there") {
        let path = entry.expect("an entry").path();
        if path.extension().is_some_and(|ext| ext == "cypcb") {
            let name = path
                .file_name()
                .expect("a file has a name")
                .to_string_lossy()
                .to_string();
            sources.push((
                name,
                std::fs::read_to_string(&path).expect("an example is readable"),
            ));
        }
    }
    assert!(sources.len() > 10, "the examples directory went missing");

    for keyword in &definitions {
        let opens_with = |source: &str| {
            source.lines().any(|line| {
                let line = line.trim_start();
                line.starts_with(&format!("{keyword} "))
                    || line.starts_with(&format!("{keyword} {{"))
            })
        };
        assert!(
            sources.iter().any(|(_, source)| opens_with(source)),
            "`{keyword}` is in the language and in no example: a construct \
             nobody runs is where this project keeps finding its defects"
        );
    }
}

/// The property lists the parser publishes, block by block.
///
/// Every `unknown_property("block", &["a", "b"])` in the reader is a promise:
/// mistype something inside that block and this is what it takes. A property
/// on one of those lists with no board using it is the same gap the language's
/// own constructs had - `diffpair` and `keepout` were in no example until
/// 2026-08-25, and neither was ever run by anything a person could copy.
fn published_properties() -> Vec<(String, Vec<String>)> {
    let reader = std::fs::read_to_string(repo_root().join("crates/cypcb-parser/src/reader.rs"))
        .expect("the reader is in the repo");

    let mut found: Vec<(String, Vec<String>)> = Vec::new();
    for chunk in reader.split("unknown_property(").skip(1) {
        // One statement at a time: the call ends at `)`, and reading past it
        // takes the next hundred lines of the reader as property names.
        let Some((statement, _)) = chunk.split_once(");") else {
            continue;
        };
        let Some((head, _)) = statement.split_once(']') else {
            continue;
        };
        // The stackup's list is a named constant rather than a literal, and
        // its words have their own guard in the syntax guide's tests. A chunk
        // with no `&[` in it is that call, and running past it reads the next
        // hundred lines of the reader as property names.
        if !head.contains("&[") {
            continue;
        }
        let mut names = head.split('"').skip(1).step_by(2);
        let Some(block) = names.next() else { continue };
        let props: Vec<String> = names.map(str::to_string).collect();
        if props.is_empty() {
            continue;
        }
        found.push((block.to_string(), props));
    }
    found
}

#[test]
fn every_property_a_block_takes_is_in_some_example() {
    let reader = std::fs::read_to_string(repo_root().join("crates/cypcb-parser/src/reader.rs"))
        .expect("the reader is in the repo");
    let dir = repo_root().join("examples");
    let mut sources = String::new();
    for entry in std::fs::read_dir(&dir).expect("the examples are there") {
        let path = entry.expect("an entry").path();
        if path.extension().is_some_and(|ext| ext == "cypcb") {
            sources.push_str(&std::fs::read_to_string(&path).expect("an example is readable"));
            sources.push('\n');
        }
    }

    let blocks = published_properties();
    assert!(
        blocks.len() >= 6,
        "six blocks published a literal property list when this was written: \
         {:?}",
        blocks.iter().map(|(b, _)| b).collect::<Vec<_>>()
    );
    let named_constant = reader
        .matches("unknown_property(\"stackup\", STACKUP_WORDS)")
        .count();
    assert_eq!(
        named_constant, 2,
        "the stackup is the one block whose list is a constant, and it is \
         passed twice; a third call or a second such block would go unread here"
    );

    for (block, props) in &blocks {
        for property in props {
            // `pin.<N> = <NET>` is a form rather than a word; what a board has
            // to show is the `pin`.
            let word = property.split('.').next().unwrap_or(property);
            let used = if block == "net constraint" {
                // These live between brackets, and the same words are trace
                // properties - a `width` on a trace says nothing about
                // whether a net has ever stated one.
                sources
                    .lines()
                    .filter(|line| line.contains('[') && line.contains(']'))
                    .any(|line| line.contains(&format!("{word} ")))
            } else {
                sources
                    .lines()
                    .any(|line| line.trim_start().starts_with(&format!("{word} ")))
            };
            assert!(
                used,
                "`{block}` takes `{word}` and no example states one: a property \
                 nobody has written down is a property nobody has run"
            );
        }
    }
}
