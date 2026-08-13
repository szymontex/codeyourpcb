//! `0.508mm required` - required by whom?
//!
//! `cargo test -p cypcb-cli --test whose_number_is_the_pad_land`
//!
//! D6 made `min_pad_size` load-bearing: it is the floor `PadLandRule` holds a
//! drilled pad to. Every preset carried one and only JLCPCB's came off a
//! capability page. OSH Park publishes an annular ring and no pad diameter,
//! PCBWay the same, and the IPC classes are a design standard rather than a
//! fabricator - so each of those numbers was `min_drill_size + 2 *
//! min_annular_ring` rounded, a derived figure sitting in a field that had
//! stopped being derived and refusing boards in a fab's name.
//!
//! The figure barely moves. What moves is whose it is.

use std::process::Command;

/// Two through-hole pads with a 0.4mm land around a 0.25mm hole: under every
/// preset's floor, published or derived.
const SMALL_LANDS: &str = r#"version 1

board t {
    size 20mm x 20mm
    layers 2
}

footprint TINY {
    pad 1 circle at 0mm, 0mm size 0.4mm x 0.4mm drill 0.25mm
    pad 2 circle at 3mm, 0mm size 0.4mm x 0.4mm drill 0.25mm
}

component J1 connector "TINY" {
    value "x"
    at 8mm, 10mm
}
"#;

fn check_against(preset: &str) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-land-{preset}"));
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, SMALL_LANDS).expect("the board is written");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["check", "--preset", preset])
        .arg(&board)
        .output()
        .expect("the binary runs");

    String::from_utf8_lossy(&output.stderr).to_string()
}

/// OSH Park publishes a 5mil ring on a 10mil drill and no pad diameter, so the
/// 0.508mm this tool holds a board to is this tool's arithmetic.
#[test]
fn a_derived_land_says_it_is_derived() {
    let report = check_against("oshpark");

    assert!(
        report.contains("0.508mm required"),
        "the fixture is supposed to break this rule:\n{report}"
    );
    assert!(
        report.contains("oshpark_2layer does not state a minimum pad size"),
        "a number this tool computed must not read as the fab's:\n{report}"
    );
    assert!(
        report.contains("this tool's own value, not the fab's"),
        "{report}"
    );
}

/// JLCPCB does publish one, and a published number gets no note - the same
/// distinction this command already draws for via diameter, silk clearance and
/// courtyard clearance.
#[test]
fn a_published_land_is_reported_without_a_note() {
    let report = check_against("jlcpcb");

    assert!(
        report.contains("0.500mm required"),
        "JLCPCB publishes 0.5mm:\n{report}"
    );
    assert!(
        !report.contains("does not state a minimum pad size"),
        "a number the fab published is the fab's:\n{report}"
    );
}
