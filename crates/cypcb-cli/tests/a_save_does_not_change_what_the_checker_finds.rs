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

/// Every example that reads at all, through the writer and back.
///
/// Three of them were compared when this file was written and the repository
/// has twenty-five. The two that exist to fail - `invalid.cypcb` and
/// `unknown_keyword.cypcb` - are skipped by the parse rather than by a list,
/// so a third one cannot join them quietly.
#[test]
fn every_example_survives_a_save() {
    let dir = std::env::temp_dir().join("cypcb-save-every-example");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");

    let mut files: Vec<PathBuf> = std::fs::read_dir(repo_root().join("examples"))
        .expect("the examples are there")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "cypcb"))
        .collect();
    files.sort();

    let mut compared = 0;
    let mut differed: Vec<String> = Vec::new();
    for file in files {
        let name = file
            .file_name()
            .expect("a name")
            .to_string_lossy()
            .to_string();
        let source = std::fs::read_to_string(&file).expect("a readable example");
        if !cypcb_parser::parse(&source).errors.is_empty() {
            continue;
        }
        let before = checked(&file);
        let after = checked(&saved(&name, &dir));
        compared += 1;
        if before != after {
            differed.push(format!("{name}: {before:?} became {after:?}"));
        }
    }

    assert!(
        compared > 20,
        "only {compared} examples reached the writer, so this proves little"
    );
    assert!(
        differed.is_empty(),
        "a save changed what the checker finds:\n{}",
        differed.join("\n")
    );
}

/// A board with copper on it, which none of the examples above carry much of.
#[test]
fn a_routed_board_survives_a_save() {
    let dir = std::env::temp_dir().join("cypcb-save-routed");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");

    let routed = dir.join("routed.cypcb");
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("route")
        .arg(repo_root().join("examples/routing-test.cypcb"))
        .arg("--in-house")
        .arg("--fast")
        .arg("-o")
        .arg(&routed)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "routing the example failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let copper = std::fs::read_to_string(&routed).expect("the routed design is there");
    assert!(
        copper.contains("trace "),
        "the point of this case is copper:\n{copper}"
    );

    let before = checked(&routed);
    let after = checked(&saved_from(&routed, "saved-routed.cypcb", &dir));
    assert_eq!(before, after, "a save changed a routed board");
}

/// The legend a fabricator prints has to survive a save.
///
/// `SilkClearanceRule` measures printed designators rather than a footprint's
/// own artwork, so this is not a rule going missing - it is the picture. The
/// writer dropped every `silk` shape of a footprint it wrote, and the silk
/// gerber is drawn from those shapes: a design saved through here exported a
/// different board than the one it came from, with no warning and nothing in
/// the checker to say so.
const MARKED: &str = r#"version 1

board marked {
    size 20mm x 20mm
    layers 2
}

footprint MARKED {
    courtyard 4mm x 4mm
    pad 1 rect at -1mm, 0mm size 1.2mm x 1.2mm
    pad 2 rect at 1mm, 0mm size 1.2mm x 1.2mm
    silk line -2mm, 1.4mm to 2mm, 1.4mm width 0.15mm
    silk circle -1.6mm, -1.4mm radius 0.25mm width 0.15mm
}

component U1 ic "MARKED" {
    value "marked"
    at 10mm, 10mm
}
"#;

/// The silkscreen gerber `export` writes for a board.
fn silk_gerber(board: &Path, into: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("export")
        .arg(board)
        .arg("-o")
        .arg(into)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "exporting failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let gerber = std::fs::read_dir(into.join("gerber"))
        .expect("the gerbers are there")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .find(|path| path.to_string_lossy().contains("F_SilkS"))
        .expect("a front silkscreen file");
    std::fs::read_to_string(gerber).expect("the gerber is readable")
}

#[test]
fn the_legend_a_footprint_draws_survives_a_save() {
    let dir = std::env::temp_dir().join("cypcb-save-keeps-silk");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");

    let original = dir.join("marked.cypcb");
    std::fs::write(&original, MARKED).expect("the fixture is writable");

    let saved = saved_from(&original, "saved-marked.cypcb", &dir);
    let text = std::fs::read_to_string(&saved).expect("the saved design is there");
    assert!(
        text.contains("silk line") && text.contains("silk circle"),
        "both shapes are written back:\n{text}"
    );

    // The half that matters: the fabricator gets the same artwork.
    let before = silk_gerber(&original, &dir.join("out-before"));
    let after = silk_gerber(&saved, &dir.join("out-after"));
    assert_eq!(
        before, after,
        "the silkscreen a fabricator prints is the same board's:\n{text}"
    );
}
