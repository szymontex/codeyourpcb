//! A board written here has to be a board KiCad would read.
//!
//! `cargo test -p cypcb-kicad --test a_design_opens_in_kicad`
//!
//! There is no KiCad in this container, so the strongest check available is
//! this project's own importer - the one that has read the benchmark boards
//! for months. Write a design out, read it back, and compare the board that
//! comes out with the one that went in.
//!
//! That is not the same as "pcbnew opens it", and this file does not claim it
//! is. What it does prove is that the file is well-formed s-expression, that
//! the layers, nets, footprints, pads, segments and vias are where the reader
//! expects them, and that nothing was lost on the way out.

use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// A board with two parts, a net, a trace on each layer and a via joining them.
const DESIGN: &str = r#"version 1

board round_trip {
    size 40mm x 30mm
    layers 2
}

component R1 resistor "0402" {
    value "10k"
    at 10mm, 10mm
}

component R2 resistor "0402" {
    value "1k"
    at 30mm, 20mm
    rotate 90
}

net SIG {
    R1.2
    R2.1
}

trace SIG {
    from R1.2
    to R2.1
    layer Top
    width 0.3mm
}

trace SIG {
    via 20mm,15mm drill 0.3mm
}
"#;

fn board_from(source: &str) -> BoardWorld {
    let parsed = cypcb_parser::parse(source);
    assert!(
        parsed.errors.is_empty(),
        "the fixture parses: {:?}",
        parsed.errors
    );

    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(
        result.errors.is_empty(),
        "the fixture syncs: {:?}",
        result.errors
    );
    world
}

#[test]
fn the_board_that_comes_back_is_the_board_that_went_out() {
    let mut world = board_from(DESIGN);
    let written = cypcb_kicad::write_board(&mut world, "cypcb-test");

    let read = cypcb_kicad::pcb_parser::parse_kicad_pcb_str(&written)
        .expect("the importer reads what the writer wrote");

    assert_eq!(read.metadata.component_count, 2, "both parts came back");
    assert_eq!(read.metadata.net_count, 1, "the design's one net came back");
    assert_eq!(read.metadata.layer_count, 2);
    assert_eq!(
        read.metadata.board_size_mm,
        (40.0, 30.0),
        "the outline is the board the design describes"
    );
    assert_eq!(read.metadata.trace_segment_count, 1, "the trace came back");
    assert_eq!(read.metadata.via_count, 1, "the via came back");
}

#[test]
fn a_rotated_part_keeps_its_angle() {
    // R2 is turned a quarter turn. A footprint written without its rotation is
    // a part whose pads are in the wrong place, which is the failure this
    // project has already had once in the other direction - the importer's
    // origin handling.
    let mut world = board_from(DESIGN);
    let written = cypcb_kicad::write_board(&mut world, "cypcb-test");

    assert!(
        written.contains("(at 30 20 90)"),
        "R2 is placed at 30mm, 20mm turned 90 degrees:\n{written}"
    );
    assert!(
        written.contains("(at 10 10 0)"),
        "R1 is placed at 10mm, 10mm and not turned:\n{written}"
    );
}

#[test]
fn every_pad_says_which_net_it_is_on() {
    // A board whose pads carry no net is a board KiCad shows as entirely
    // unrouted, whatever copper is on it.
    let mut world = board_from(DESIGN);
    let written = cypcb_kicad::write_board(&mut world, "cypcb-test");

    let pads_on_sig = written.matches("(net 1 \"SIG\")").count();
    assert!(
        pads_on_sig >= 2,
        "R1.2 and R2.1 are both on SIG, and the trace and via name it too:\n{written}"
    );
}

#[test]
fn a_board_with_nothing_on_it_is_still_a_board() {
    // The empty case has to produce a file the reader accepts, or the first
    // thing a new user does - write a board, open it - fails.
    let mut world =
        board_from("version 1\n\nboard empty {\n    size 20mm x 20mm\n    layers 2\n}\n");
    let written = cypcb_kicad::write_board(&mut world, "cypcb-test");

    let read = cypcb_kicad::pcb_parser::parse_kicad_pcb_str(&written)
        .expect("an empty board is still readable");
    assert_eq!(read.metadata.component_count, 0);
    assert_eq!(read.metadata.board_size_mm, (20.0, 20.0));
}

#[test]
fn a_board_that_is_not_a_rectangle_is_written_as_the_shape_it_is() {
    // The Gerber exporter has honoured a declared outline all along. This
    // wrote the rectangle the board's `size` describes whatever shape it
    // really was, so the two files disagreed about the same board and the one
    // a KiCad user opened was the wrong one.
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("the crate sits two levels below the repo root")
            .join("examples/cutout.cypcb"),
    )
    .expect("the example is there");

    let mut world = board_from(&source);
    let written = cypcb_kicad::write_board(&mut world, "cypcb-test");

    let edges = written.matches("(layer \"Edge.Cuts\")").count();
    assert_eq!(
        edges, 8,
        "the outline has eight corners, so it is eight lines, not four:\n{written}"
    );
    assert!(
        written.contains("(start 25 30) (end 25 10)"),
        "the slot's wall is part of the board's edge:\n{written}"
    );
}
