//! A connector hanging off the board edge is refused, and the arithmetic is
//! the file's own.
//!
//! `cargo test -p cypcb-cli --test a_connector_hanging_off_the_edge_is_refused`
//!
//! `routing-diagnostic.kicad_pcb` is refused for edge clearance, and until the
//! spatial index was filled where a board is read nobody could see it: the
//! rule walks that index and the index was empty, so the board read as clean.
//!
//! The verdict is the board's fault and not this reader's, which is the
//! question the tracker asked. The file's own numbers:
//!
//! - `(gr_rect (start 0 0) (end 15 15))` on `Edge.Cuts` - a 15mm square
//! - `J1` placed `(at 8.0 12.0)`
//! - its pad 2 at `(at 0 2.54)`, 1.7mm across, so its copper runs to
//!   y = 12.0 + 2.54 + 0.85 = **15.39mm**, past the edge
//! - its pad 3 at `(at 0 5.08)`, whose whole hole sits at y = 17.08mm, off the
//!   board altogether
//!
//! One `edge-clearance` report, because that rule names the component once
//! however many of its pads are out - at the pad that is furthest out, pad 3;
//! two `hole-to-edge` reports, because that one names each hole.
//!
//! The coordinate in that report used to be the courtyard's centre, because
//! the rule measured the box a component sits in the index as. A courtyard is
//! not copper, so a part whose body overhangs the edge while its pads stay
//! well inside was refused for copper it does not have. It measures pads now,
//! which is why the case below names 17.080mm and not 14.540mm.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::rules::EdgeClearanceRule;
use cypcb_drc::{DesignRules, DrcRule};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root is two directories above this crate")
}

const BOARD: &str = "tests/fixtures/kicad-tools/tests/fixtures/routing-diagnostic.kicad_pcb";

#[test]
fn the_board_is_the_square_its_own_edge_cuts_draw() {
    // The verdict below rests on the outline being read as the file states it.
    // A ring read a fraction wide would excuse copper that is over the edge,
    // and a ring read narrow would refuse copper that is not.
    let parsed = cypcb_kicad::parse_kicad_pcb(&repo_root().join(BOARD)).expect("the board reads");
    let (size, _) = parsed
        .world
        .board_info()
        .expect("the board states its own size");
    assert_eq!(size.width, cypcb_core::Nm::from_mm(15.0));
    assert_eq!(size.height, cypcb_core::Nm::from_mm(15.0));
}

#[test]
fn the_checker_names_the_connector_that_is_over_the_edge() {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(repo_root().join(BOARD))
        .output()
        .expect("the binary runs");
    // The report goes to stdout and the failing exit code to the shell; both
    // streams are read here so the case cannot pass on an empty one.
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!report.trim().is_empty(), "the checker printed nothing");

    assert!(
        report.contains("edge-clearance at (8.000mm, 17.080mm): J1"),
        "the connector over the edge is not named:\n{report}"
    );
    // The pad's centre is 0.46mm from the edge and its copper is 0.85mm wide
    // either way, so there is no gap left at all.
    assert!(
        report.contains("Edge clearance violation: 0.00mm actual, 0.30mm required"),
        "the gap the checker measured is not reported:\n{report}"
    );
    assert!(
        report.contains("edge-clearance: 1"),
        "the summary does not carry the edge verdict:\n{report}"
    );
    // Both of the connector's outer holes are off the board, and that rule
    // names each one.
    assert!(
        report.contains("hole-to-edge: 2"),
        "the two holes over the edge are not both reported:\n{report}"
    );
}

/// A 10mm board with one pad whose copper stops `gap_mm` short of the right
/// edge.
fn board_with_pad_near_the_edge(gap_mm: f64) -> BoardWorld {
    board_with_pad_and_body(gap_mm, 0.5)
}

/// The same, with a body of the given half-width around the pad.
fn board_with_pad_and_body(gap_mm: f64, courtyard_half_mm: f64) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("e".to_string(), (Nm::from_mm(10.0), Nm::from_mm(10.0)), 2);

    let mut library = FootprintLibrary::new();
    library.register(Footprint {
        name: "square".to_string(),
        description: String::new(),
        bounds: Rect::from_points(Point::from_mm(-0.3, -0.3), Point::from_mm(0.3, 0.3)),
        courtyard: Rect::from_points(
            Point::from_mm(-courtyard_half_mm, -courtyard_half_mm),
            Point::from_mm(courtyard_half_mm, courtyard_half_mm),
        ),
        silk: Vec::new(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Rect,
            position: Point::ORIGIN,
            size: (Nm::from_mm(0.6), Nm::from_mm(0.6)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
        }],
    });

    world.spawn_component(
        RefDes::new("R1"),
        Value::new(""),
        // Copper reaches 0.3mm past the centre, so the centre goes that much
        // further in than the gap being asked for.
        Position::from_mm(10.0 - gap_mm - 0.3, 5.0),
        Rotation::ZERO,
        FootprintRef::new("square"),
        NetConnections::new(),
    );
    world.set_footprints(library.clone());
    world.rebuild_spatial_index_from_library(&library);
    world
}

#[test]
fn copper_inside_the_board_but_too_near_its_edge_is_refused() {
    // The case above is copper that is over the edge, where any threshold at
    // all reports it. This is the threshold itself: JLCPCB asks for 0.3mm, so
    // a pad 0.2mm from the edge is refused and one 0.4mm away is not, and both
    // are inside the board.
    let rules = DesignRules::jlcpcb_2layer();

    let mut too_close = board_with_pad_near_the_edge(0.2);
    assert_eq!(
        EdgeClearanceRule.check(&mut too_close, &rules).len(),
        1,
        "copper 0.2mm from the edge is not refused against a 0.3mm rule"
    );

    let mut clear = board_with_pad_near_the_edge(0.4);
    assert!(
        EdgeClearanceRule.check(&mut clear, &rules).is_empty(),
        "copper 0.4mm from the edge is refused against a 0.3mm rule"
    );
}

#[test]
fn a_body_hanging_over_the_edge_is_not_copper_over_the_edge() {
    // The defect this rule carried until 2026-08-31. A component sits in the
    // spatial index as its courtyard - the assembly keepout that covers the
    // part body - and the rule measured that box. So a connector whose plastic
    // overhangs the board while its pads stay 0.4mm inside was refused for
    // copper it does not have.
    //
    // Here the body reaches 0.5mm past the edge and the copper stops 0.4mm
    // short of it, against a 0.3mm rule.
    let mut world = board_with_pad_and_body(0.4, 1.2);
    let violations = EdgeClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());
    assert!(
        violations.is_empty(),
        "a part whose body overhangs the edge was refused for copper that is \
         0.4mm inside it: {violations:?}"
    );
}
