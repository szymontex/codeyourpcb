//! A board KiCad itself saved, read by this project's importer.
//!
//! `cargo test -p cypcb-kicad --test a_board_kicad_actually_wrote`
//!
//! Every `.kicad_pcb` in this repository was written by hand. The six under
//! `tests/fixtures/benchmark/` even say `(generator "pcbnew")
//! (generator_version "8.0.0")` and carry, between them, zero `(uuid ...)`,
//! zero `fp_text`, zero `(attr ...)`, zero `(descr ...)` and zero
//! `(fp_line ...)` - none of what pcbnew writes on every footprint it saves.
//! So both halves of KiCad interoperation, the writer and the reader, were
//! only ever checked against this project's own idea of the format.
//!
//! `tests/fixtures/kicad10-slotted.kicad_pcb` is the first file here KiCad
//! wrote: `examples/slotted-connector.cypcb` exported, then handed to
//! `kicad-cli pcb upgrade` on KiCad 10.0.5, which re-saved it in KiCad's own
//! current format. It carries 19 uuids.
//!
//! It found two things on the first run, both of which had been shipped and
//! neither of which any test could see:
//!
//! 1. The version gate ended at 20250101 and this file says 20260206, so the
//!    importer refused a board KiCad had just written.
//! 2. Past the gate, `net_count` came back **0**. KiCad 10 dropped the
//!    numbered net table that every version since 5 carried, and writes the
//!    name alone on each pad: `(net "VBUS")`, not `(net 3 "VBUS")`. A board
//!    imported this way arrives with its copper unconnected.

use cypcb_kicad::pcb_parser::parse_kicad_pcb_str;

fn kicad10() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/kicad10-slotted.kicad_pcb");
    std::fs::read_to_string(&path).expect("the fixture is there")
}

#[test]
fn the_fixture_really_came_out_of_kicad() {
    // Guards the fixture rather than the code: a hand-written replacement
    // would make every other test in this file a closed loop again.
    let text = kicad10();

    assert!(text.contains("(version 20260206)"), "not a KiCad 10 file");
    assert!(
        text.matches("(uuid").count() >= 10,
        "pcbnew stamps a uuid on everything it saves, and this file has {}",
        text.matches("(uuid").count()
    );
}

#[test]
fn a_kicad_10_board_is_not_refused_for_its_version() {
    let read = parse_kicad_pcb_str(&kicad10())
        .expect("a board KiCad wrote is a board this reader accepts");

    assert_eq!(read.metadata.version, 20260206);
}

#[test]
fn the_nets_come_back_from_a_board_with_no_net_table() {
    // slotted-connector.cypcb declares VBUS and GND.
    let read = parse_kicad_pcb_str(&kicad10()).expect("the board reads");

    assert_eq!(
        read.metadata.net_count, 2,
        "KiCad 10 names its nets on the pads instead of in a table, and this \
         board has two"
    );
}

#[test]
fn the_board_that_comes_back_is_the_board_that_went_to_kicad() {
    let read = parse_kicad_pcb_str(&kicad10()).expect("the board reads");

    assert_eq!(read.metadata.component_count, 1, "J1");
    assert_eq!(read.metadata.layer_count, 2);
    assert_eq!(read.metadata.board_size_mm, (30.0, 20.0));
}
