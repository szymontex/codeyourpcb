//! A design saved through this project's own writer is the same board.
//!
//! `cargo test -p cypcb-cli --test a_save_does_not_change_what_the_checker_finds`
//!
//! Three statements are flattened rather than written back: a `netclass` onto
//! its members, a `module` and its instances into the parts they place, an
//! `import` into whatever it brought. Each was called harmless on the strength
//! of how sync works, and reading is how the writer came to drop a
//! differential pair and every assertion without anybody noticing.
//!
//! So this asks the checker instead. Same board, same violations, per kind -
//! and on the two examples written to demonstrate modules and imports.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// What `check -o json` counted, by kind.
fn checked(board: &Path) -> BTreeMap<String, usize> {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg("-o")
        .arg("json")
        .arg(board)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stdout).to_string();
    let report: serde_json::Value = serde_json::from_str(said.trim())
        .unwrap_or_else(|error| panic!("check prints JSON: {error}\n{said}"));
    report["summary"]
        .as_object()
        .expect("a summary")
        .iter()
        .map(|(kind, count)| (kind.clone(), count.as_u64().expect("a count") as usize))
        .collect()
}

/// The same board, written back out by `board_as_dsl`.
fn saved(example: &str, into: &Path) -> PathBuf {
    saved_from(&repo_root().join("examples").join(example), example, into)
}

/// The same board as `source_path`, written back out under `name`.
fn saved_from(source_path: &Path, name: &str, into: &Path) -> PathBuf {
    let example = name;
    let source = std::fs::read_to_string(source_path).expect("the design is there");

    let parsed = cypcb_parser::parse(&source);
    assert!(
        parsed.errors.is_empty(),
        "{example} parses: {:?}",
        parsed.errors
    );

    // The same three steps `check` takes, imports included: an example that
    // imports another is exactly what this is here to measure.
    let mut import_errors = Vec::new();
    let ast = cypcb_parser::resolve_imports(&parsed.value, source_path, &mut import_errors);
    assert!(
        import_errors.is_empty(),
        "{example} imports: {import_errors:?}"
    );

    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&ast, &source, &mut world, &mut library);
    assert!(
        result.errors.is_empty(),
        "{example} syncs: {:?}",
        result.errors
    );

    let written = cypcb_world::dsl::board_as_dsl(&mut world);
    let out = into.join(example);
    std::fs::write(&out, &written).expect("the saved design is writable");
    out
}

#[test]
fn a_module_and_an_import_survive_a_save() {
    let dir = std::env::temp_dir().join("cypcb-save-is-the-same-board");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");

    for example in [
        "v2-modules.cypcb",
        "v2-imports.cypcb",
        "v2-constraints.cypcb",
    ] {
        let before = checked(&repo_root().join("examples").join(example));
        assert!(
            !before.is_empty(),
            "{example} is only worth comparing if the checker finds something: {before:?}"
        );
        let after = checked(&saved(example, &dir));
        assert_eq!(before, after, "{example} is a different board after a save");
    }
}

/// A value that is not a physical quantity has to stay a string.
///
/// `value "10k"` is a resistance nobody spelled with a unit and `value
/// "LDO-3V3"` is a part number; written bare, the first is a number followed
/// by an unknown unit and the second is not a number at all - a file this
/// project's own parser refuses. The rule that lets `10kohm` through has to
/// keep both of these quoted.
const STRINGS: &str = r#"version 1

board strings {
    size 40mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value "10k"
    at 10mm, 10mm
}

component U1 ic "SOT-23-5" {
    value "LDO-3V3"
    at 30mm, 10mm
}
"#;

#[test]
fn a_value_that_is_not_a_quantity_stays_a_string() {
    let dir = std::env::temp_dir().join("cypcb-save-keeps-strings");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");

    let original = dir.join("strings.cypcb");
    std::fs::write(&original, STRINGS).expect("the fixture is writable");
    let before = checked(&original);

    let saved = saved_from(&original, "saved.cypcb", &dir);
    let text = std::fs::read_to_string(&saved).expect("the saved design is there");
    assert!(
        text.contains("value \"10k\"") && text.contains("value \"LDO-3V3\""),
        "neither of these is a quantity:\n{text}"
    );

    // And the file still reads, which is what quoting is for: `checked` fails
    // loudly on a design the binary cannot parse.
    assert_eq!(checked(&saved), before);
}
