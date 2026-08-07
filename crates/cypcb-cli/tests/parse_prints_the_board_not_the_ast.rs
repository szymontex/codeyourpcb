//! `cypcb parse -o json` says "board model as JSON" in its own help text.
//!
//! It printed the AST instead, under both formats, with a comment blaming a
//! cargo problem that no longer exists - so the two options differed in name
//! only and neither did what one of them promised. The difference matters:
//! the AST is what the file says, the model is what the board is. Only the
//! model knows a component's footprint was found, what an import brought in,
//! or which net a pin ended up on.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb_binary() -> PathBuf {
    let mut path = std::env::current_exe().expect("the test binary has a path");
    path.pop();
    path.pop();
    path.push("cypcb");
    path
}

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples")
}

/// Boards that are there to show what a broken file looks like.
const MEANT_TO_FAIL_PARSING: &[&str] = &["invalid.cypcb", "unknown_keyword.cypcb"];

fn parse(file: &Path, format: &str) -> Option<serde_json::Value> {
    let output = Command::new(cypcb_binary())
        .arg("parse")
        .arg(file)
        .arg("-o")
        .arg(format)
        .output()
        .expect("cypcb runs");
    if !output.status.success() {
        return None;
    }
    Some(serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "{} -o {format} did not print JSON: {e}",
            file.file_name().unwrap_or_default().to_string_lossy()
        )
    }))
}

fn example_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(examples_dir())
        .expect("the examples directory is there")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "cypcb"))
        .filter(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            !MEANT_TO_FAIL_PARSING.contains(&name.as_ref())
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no examples found to check");
    files
}

#[test]
fn every_example_prints_a_board_model() {
    for file in example_files() {
        let name = file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let model = parse(&file, "json").unwrap_or_else(|| panic!("{name} failed to parse"));

        for key in ["board", "components", "nets", "traces", "vias", "zones"] {
            assert!(
                model.get(key).is_some(),
                "{name}: the model has no `{key}` - this is the AST again"
            );
        }
        assert!(
            model.get("definitions").is_none(),
            "{name}: `definitions` is the AST's top level, not the board's"
        );
    }
}

#[test]
fn the_two_formats_are_two_different_things() {
    // Guarding the exact defect: both arms of the match printed the AST, so
    // the two formats returned byte-identical output on every file.
    let file = examples_dir().join("blink.cypcb");
    let model = parse(&file, "json").expect("the model");
    let ast = parse(&file, "ast").expect("the AST");

    assert_ne!(model, ast, "-o json and -o ast printed the same thing");
    assert!(
        ast.get("definitions").is_some(),
        "the AST output should still be the AST"
    );
}

#[test]
fn the_model_knows_what_an_import_brought_in() {
    // The strongest thing the model has over the AST on this file: every
    // component's footprint is one the model could find, and all of them
    // arrived through `import`. The AST holds the import as a path.
    let file = examples_dir().join("v2-imports.cypcb");
    let model = parse(&file, "json").expect("the model");

    let components = model["components"].as_array().expect("components");
    assert_eq!(components.len(), 6, "the file places six parts");
    for component in components {
        assert_eq!(
            component["footprint_known"], true,
            "{} names footprint {} and nothing resolved it",
            component["refdes"], component["footprint"]
        );
        assert!(
            !component["pins"].as_array().expect("pins").is_empty(),
            "{} ended up on no net at all",
            component["refdes"]
        );
    }

    let nets = model["nets"].as_array().expect("nets");
    assert_eq!(
        nets.len(),
        7,
        "seven nets, counting the ones a module wires"
    );
}

#[test]
fn a_pour_reaches_the_model_with_its_net_and_outline() {
    let file = examples_dir().join("pour-island.cypcb");
    let model = parse(&file, "json").expect("the model");

    let zones = model["zones"].as_array().expect("zones");
    assert_eq!(zones.len(), 1, "the board draws one plane");
    let zone = &zones[0];
    assert_eq!(zone["kind"], "CopperPour");
    assert_eq!(zone["net"], "GND");
    assert!(
        zone["max_x_nm"].as_i64().expect("a number") > zone["min_x_nm"].as_i64().expect("a number"),
        "a plane with no width is not a plane"
    );
}

#[test]
fn the_same_file_prints_the_same_bytes_twice() {
    // The model is read out of an ECS, whose iteration order is not the order
    // things were spawned in. Without sorting, a diff of two runs is noise.
    let file = examples_dir().join("blink.cypcb");
    let first = parse(&file, "json").expect("the model");
    let second = parse(&file, "json").expect("the model");
    assert_eq!(first, second);
}
