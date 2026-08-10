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

    // Read out of the file rather than matched as a literal: a board is written
    // where it lands on the drawing sheet, so what is fixed is the angle and
    // the distance between the parts, not the numbers themselves.
    let placements: Vec<(f64, f64, f64)> = written
        .lines()
        .filter(|line| line.contains("(footprint "))
        .map(|line| {
            let at = line.rsplit("(at ").next().expect("a footprint is placed");
            let mut parts = at.trim_end_matches(')').split_whitespace();
            let mut next = || {
                parts
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(f64::NAN)
            };
            (next(), next(), next())
        })
        .collect();
    assert_eq!(placements.len(), 2, "both parts are placed:\n{written}");

    let (x1, y1, r1) = placements[0];
    let (x2, y2, r2) = placements[1];
    assert_eq!(r1, 0.0, "R1 is not turned:\n{written}");
    assert_eq!(r2, 90.0, "R2 is turned a quarter turn:\n{written}");
    assert!(
        (x2 - x1 - 20.0).abs() < 0.001 && (y2 - y1 - 10.0).abs() < 0.001,
        "the design puts R2 20mm right and 10mm below R1, and the file has \
         {}mm and {}mm:\n{written}",
        x2 - x1,
        y2 - y1
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

    let edges: Vec<(f64, f64, f64, f64)> = written
        .lines()
        .filter(|line| line.contains("(layer \"Edge.Cuts\")"))
        .map(|line| {
            let numbers: Vec<f64> = line
                .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
                .filter(|token| !token.is_empty())
                .filter_map(|token| token.parse().ok())
                .collect();
            (numbers[0], numbers[1], numbers[2], numbers[3])
        })
        .collect();
    assert_eq!(
        edges.len(),
        8,
        "the outline has eight corners, so it is eight lines, not four:\n{written}"
    );

    // The shape, read back where the design draws it: the board is written
    // onto the drawing sheet, so its corner is not at 0,0 in the file.
    let left = edges
        .iter()
        .flat_map(|(x1, _, x2, _)| [*x1, *x2])
        .fold(f64::MAX, f64::min);
    let top = edges
        .iter()
        .flat_map(|(_, y1, _, y2)| [*y1, *y2])
        .fold(f64::MAX, f64::min);
    let slot_wall = edges.iter().any(|(x1, y1, x2, y2)| {
        (x1 - left - 25.0).abs() < 0.001
            && (x2 - left - 25.0).abs() < 0.001
            && (y1 - top - 30.0).abs() < 0.001
            && (y2 - top - 10.0).abs() < 0.001
    });
    assert!(
        slot_wall,
        "the slot's wall is part of the board's edge:\n{written}"
    );
}
