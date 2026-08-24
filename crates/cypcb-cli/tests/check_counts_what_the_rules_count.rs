//! The numbers `cypcb check` prints are the numbers the rules found.
//!
//! `cargo test -p cypcb-cli --test check_counts_what_the_rules_count`
//!
//! Every published figure in this project - the ratchets, the noise bands,
//! `docs/routing.md`, every sweep table - is a count of DRC rows. The command
//! is where that count reaches a person, and its summary loop groups clearance
//! rows by contact for *display*: `check.rs` says in as many words that the
//! header and the per-kind tally stay row counts, because a display change that
//! quietly moved them would be a re-baseline pretending to be a tidy-up.
//!
//! Nothing held it to that. This does: it asks the binary and asks the rules,
//! on one board that violates both counts, and requires the same answer.
//!
//! It also pins the checker's own board-building. `check` reads a `.cypcb` and
//! runs the rules against it; `sync_ast_to_world` rebuilds the spatial index
//! from the footprint library on its way out (`cypcb-world/src/sync.rs`), which
//! is what puts the pads in the structure the clearance rule searches. A path
//! that stopped doing that would report fewer violations than the rules find,
//! and fewer is the direction that matters - a board that passes is sent to a
//! fabricator.

use std::path::PathBuf;
use std::process::Command;

use cypcb_drc::{run_drc, Preset, PresetRules};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// Three parts on a line, with a trace stepping between two of them.
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
    let dir = std::env::temp_dir().join("cypcb-check-counts");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, BOARD).expect("the fixture is writable");
    board
}

/// What the rules say about that board, built the way every other path builds
/// it.
fn what_the_rules_say() -> (usize, usize) {
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
        .filter(|v| v.kind == cypcb_drc::ViolationKind::Clearance)
        .count();
    (report.violations.len(), clearance)
}

#[test]
fn the_command_and_the_rules_agree_on_the_same_board() {
    let (total, clearance) = what_the_rules_say();
    assert!(
        total > 0 && clearance > 0,
        "the fixture has to violate something"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(fixture())
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);

    assert!(
        said.contains(&format!("{total} DRC violation(s)")),
        "the rules find {total} and the command has to report {total}:\n{said}"
    );
    assert!(
        said.contains(&format!("clearance: {clearance}")),
        "the rules find {clearance} clearance rows and the command has to report \
         {clearance}:\n{said}"
    );
}
