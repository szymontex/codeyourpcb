//! The refusal counts the houses rather than remembering how many there are.
//!
//! `cargo test -p cypcb-cli --test the_house_list_is_counted_not_remembered`
//!
//! `cypcb export --house nonsense` explains itself at length, and the sentence
//! held four hand-written copies of a list of two: "Two are written down",
//! "jlcpcb, pcbway", and both names again in "Export with `--house jlcpcb` or
//! `--house pcbway`". A third house would have made all four wrong at once -
//! the shape this project has spent a fortnight finding in its own files.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples")
        .join(name)
}

/// What the command says when it is handed a house it cannot cut files for.
fn refusal() -> String {
    let out = std::env::temp_dir().join("cypcb-house-refusal");
    let _ = std::fs::remove_dir_all(&out);
    let output = cypcb()
        .arg("export")
        .arg(example("blink.cypcb"))
        .arg("-o")
        .arg(&out)
        .arg("--house")
        .arg("nonsense")
        .output()
        .expect("the binary runs");
    assert!(
        !output.status.success(),
        "an unknown house cannot be exported for"
    );
    assert!(
        !out.exists(),
        "and nothing is written before the refusal: {}",
        out.display()
    );
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// The refusal as one line.
///
/// miette wraps to the terminal and draws a gutter, so a sentence that reads
/// whole to a person is broken by newlines and `|` characters in the bytes.
/// Matching against the rendered shape would test the renderer.
fn flattened(text: &str) -> String {
    text.replace('\u{2502}', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn every_house_the_crate_knows_is_named_in_the_refusal() {
    let said = flattened(&refusal());
    let houses: Vec<&str> = cypcb_export::presets::house_names().collect();

    assert!(!houses.is_empty(), "the crate knows at least one house");
    for house in &houses {
        assert!(
            said.contains(house),
            "`{house}` is a house this command can cut files for and the refusal does not \
             name it:\n{said}"
        );
        assert!(
            said.contains(&format!("--house {house}")),
            "and it has to show how to ask for it:\n{said}"
        );
    }

    // The count too, which was the word most likely to go stale: it read
    // "Two" while the list beside it could grow.
    assert!(
        said.contains(&format!("{} are written down", houses.len())),
        "the refusal counts the houses it names:\n{said}"
    );
}

#[test]
fn the_refusal_still_explains_the_other_list() {
    // The point of the message: `--preset` on `check` takes design rules and
    // knows more names, so a board can be checked against a house this
    // command cannot yet write files for. Losing that in a refactor would
    // leave a reader told no and nothing else.
    let said = flattened(&refusal());
    assert!(said.contains("cypcb check --preset"), "{said}");
    assert!(said.contains("oshpark"), "{said}");
}

#[test]
fn every_house_named_can_actually_be_exported_for() {
    // The other direction: a name in the list that does not resolve would be
    // advice to type something that fails.
    for house in cypcb_export::presets::house_names() {
        assert!(
            cypcb_export::presets::from_name(house).is_some(),
            "`{house}` is offered and does not resolve"
        );
    }
}
