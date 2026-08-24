//! `check` and `score` count one board the same way, and say which count is which.
//!
//! `cargo test -p cypcb-cli --test the_two_commands_count_one_board`
//!
//! Two commands print DRC numbers and they build their rules differently:
//! `check` runs `preset.rules()`, `score` runs
//! `DesignRules::from_constraints(&preset.constraints())`. They have already
//! differed by a factor of six once, and `from_constraints` has dropped fields
//! before, so nothing about them agreeing is structural - it is a fact that has
//! to be measured, on a board that violates enough to tell them apart.
//!
//! The other half is what the two published names mean. `check` prints
//! **rows**: one per pair of copper features the clearance rule measured, which
//! is what every ratchet, noise band and sweep table in this project is a count
//! of. `score` prints `clearance_contacts`: one per *pair of things in contact*,
//! however many rows describe it. On the board below that is 6 against 3, and a
//! fixture where the two happen to be equal would let either name drift into
//! the other without a test noticing.

use std::path::PathBuf;
use std::process::Command;

use cypcb_drc::{clearance_contacts, run_drc, Preset, PresetRules, ViolationKind};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// Three parts on a line, with a trace stepping between two of them.
///
/// The same board `check_counts_what_the_rules_count` uses: it violates
/// clearance in more rows than contacts, which is what makes the two names
/// distinguishable.
const BOARD: &str = r#"version 1

board grouping {
    size 30mm x 30mm
    layers 2
}

component R1 resistor "0402" {
    value "10k"
    at 5mm, 15mm
}

component R2 resistor "0402" {
    value "10k"
    at 25mm, 15mm
}

component R3 resistor "0402" {
    value "10k"
    at 15mm, 15mm
}

net SIG {
    R1.1
    R2.1
}

trace SIG {
    from R1.1
    via 14mm, 15mm
    via 16mm, 15mm
    to R2.1
    layer Top
    width 0.2mm
}
"#;

fn fixture() -> PathBuf {
    let dir = std::env::temp_dir().join("cypcb-two-commands");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, BOARD).expect("the fixture is writable");
    board
}

/// Rows, clearance rows and contacts, as the library counts them.
fn what_the_rules_say() -> (usize, usize, usize) {
    let parsed = cypcb_parser::parse(BOARD);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);

    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, BOARD, &mut world, &mut library);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    world.rebuild_spatial_index_from_library(&library);

    let preset = Preset::from_name("jlcpcb").expect("a known preset");
    let report = run_drc(&mut world, &preset.rules());
    let clearance = report
        .violations
        .iter()
        .filter(|v| v.kind == ViolationKind::Clearance)
        .count();
    (
        report.violations.len(),
        clearance,
        clearance_contacts(&report.violations),
    )
}

fn run(command: &str, board: &PathBuf) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg(command)
        .arg(board)
        .output()
        .expect("the binary runs");
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

#[test]
fn the_two_commands_agree_on_how_many_rules_this_board_breaks() {
    let (rows, clearance_rows, contacts) = what_the_rules_say();
    assert!(
        rows > clearance_rows && clearance_rows > contacts && contacts > 0,
        "the fixture has to tell rows, clearance rows and contacts apart: \
         {rows} / {clearance_rows} / {contacts}"
    );

    let board = fixture();
    let checked = run("check", &board);
    let scored = run("score", &board);

    assert!(
        checked.contains(&format!("{rows} DRC violation(s)")),
        "`check` has to report the {rows} rows the rules found:\n{checked}"
    );

    // The object, out of a stream that also carries the line naming how many
    // traces were scored.
    let start = scored.find('{').expect("`score` prints an object");
    let end = scored.rfind('}').expect("`score` prints an object");
    let json: serde_json::Value =
        serde_json::from_str(&scored[start..=end]).expect("`score` prints JSON");

    assert_eq!(
        json["drc_violations"].as_u64(),
        Some(rows as u64),
        "`score` builds its rules with `from_constraints` and `check` with \
         `preset.rules()`, and on this board they have to break the same number \
         of them:\n{scored}"
    );
    assert_eq!(
        json["clearance_contacts"].as_u64(),
        Some(contacts as u64),
        "`clearance_contacts` is a count of contacts, not of rows - {contacts} \
         here, where the rows are {clearance_rows}:\n{scored}"
    );
    assert!(
        checked.contains(&format!("clearance: {clearance_rows}")),
        "and `check`'s per-kind tally is the row count, {clearance_rows}, \
         which is the other name:\n{checked}"
    );
}
