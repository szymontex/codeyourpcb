//! What the fabricator receives has to be the board that was routed.
//!
//! Every check upstream of this file is about the model. These files are the
//! last thing between a design and a wrong board: a trace missing from the
//! copper layer is an open circuit, and a via missing from the drill file is a
//! hole nobody makes.

use cypcb_core::{Nm, Point};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::excellon::export_excellon;
use cypcb_export::gerber::copper::export_copper_layer;
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource, Via};
use cypcb_world::components::Layer;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

/// A net routed across both layers, joined by one via.
///
/// Built by hand rather than parsed, so this test fails for export reasons
/// only.
fn routed_board() -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
    let net = world.intern_net("SIG");

    let trace = |layer, from: (f64, f64), to: (f64, f64)| Trace {
        segments: vec![TraceSegment::new(
            Point::from_mm(from.0, from.1),
            Point::from_mm(to.0, to.1),
        )],
        width: Nm::from_mm(0.2),
        layer,
        net_id: net,
        locked: false,
        source: TraceSource::Autorouted,
    };

    world.spawn_entity((trace(Layer::TopCopper, (10.5, 10.0), (15.0, 10.0)), net));
    world.spawn_entity((trace(Layer::BottomCopper, (15.0, 10.0), (19.5, 10.0)), net));
    world.spawn_entity((
        Via {
            position: Point::from_mm(15.0, 10.0),
            drill: Nm::from_mm(0.4),
            outer_diameter: Nm::from_mm(0.8),
            start_layer: Layer::TopCopper,
            end_layer: Layer::BottomCopper,
            net_id: net,
            locked: false,
        },
        net,
    ));

    world
}

#[test]
fn each_copper_layer_carries_the_trace_that_belongs_to_it() {
    let mut world = routed_board();
    let library = FootprintLibrary::new();
    let format = CoordinateFormat::FORMAT_MM_2_6;

    let top =
        export_copper_layer(&mut world, &library, Layer::TopCopper, &format).expect("top copper");
    let bottom = export_copper_layer(&mut world, &library, Layer::BottomCopper, &format)
        .expect("bottom copper");

    // D02 moves the aperture, D01 draws to the next point: one segment is a
    // move to its start and a draw to its end. Coordinates in the 2.6 format
    // the header declares - 10.5mm is `10500000`, six implied decimals.
    assert!(
        top.contains("X10500000Y10000000D02*") && top.contains("X15000000Y10000000D01*"),
        "the top trace is missing from the top layer:\n{top}"
    );
    assert!(
        bottom.contains("X15000000Y10000000D02*") && bottom.contains("X19500000Y10000000D01*"),
        "the bottom trace is missing from the bottom layer:\n{bottom}"
    );

    // And neither layer carries the other's copper, which would be a short
    // through the board.
    assert!(
        !top.contains("X19.500000"),
        "the bottom trace leaked onto the top layer:\n{top}"
    );
    assert!(
        !bottom.contains("X10.500000"),
        "the top trace leaked onto the bottom layer:\n{bottom}"
    );
}

#[test]
fn the_via_becomes_a_hole_the_fabricator_will_drill() {
    let mut world = routed_board();
    let library = FootprintLibrary::new();

    let drill = export_excellon(&mut world, &library, &CoordinateFormat::FORMAT_MM_2_6, None)
        .expect("drill file");

    assert!(
        drill.contains("T1C0.400000"),
        "the tool has to be the via's drill diameter:\n{drill}"
    );
    assert!(
        drill.contains("X15.000000Y10.000000"),
        "the hole has to be where the via is:\n{drill}"
    );

    // Exactly one hole: this board has one via and no through-hole pads, so a
    // second coordinate line would mean something was invented.
    let holes = drill.lines().filter(|line| line.starts_with('X')).count();
    assert_eq!(holes, 1, "expected one hole:\n{drill}");

    assert!(drill.contains("M30"), "an Excellon file ends with M30");
}
