//! Copper across a mounting hole has to be reported by something.
//!
//! `cargo test -p cypcb-drc --test copper_over_a_mounting_hole_is_reported`
//!
//! A mounting hole is a hole with no copper, so every rule that walks pad
//! copper walks straight past it. The courtyard rule stops a *part* being
//! placed on top of one; nothing stopped a *trace* being routed across one.
//!
//! The autorouter will not do it - the grid blocks the hole on every layer -
//! but the router is not the only way copper gets onto a board. A trace drawn
//! by hand in the viewer, a board imported from KiCad, or a zone poured over
//! the hole all arrive without the router's opinion, and the checker was the
//! only thing left to notice.
//!
//! What happens when it is not noticed: the drill cuts the trace, so the net
//! is open, and the copper it exposes at the hole wall touches the screw. A
//! metal standoff then ties that net to the chassis.
//!
//! The clearance is `min_edge_clearance`, because that is what a mounting hole
//! is - a board edge cut into the middle of the board. The same drill exposes
//! the same copper for the same reason, and the fabricator's number for it is
//! already in the rule set.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::DrcRule;
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, NetId, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// An M3 mounting hole: 3.2mm drilled, no copper anywhere.
fn mounting_hole() -> Footprint {
    let drill = Nm::from_mm(3.2);
    Footprint {
        name: "MOUNT-M3".to_string(),
        description: "M3 mounting hole".to_string(),
        pads: vec![PadDef {
            number: String::new(),
            shape: PadShape::Circle,
            position: Point::ORIGIN,
            size: (drill, drill),
            drill: Some(drill),
            slot: None,
            layers: Vec::new(),
            mask_margin: None,
        }],
        bounds: Rect::from_center_size(Point::ORIGIN, (drill, drill)),
        courtyard: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(7.2), Nm::from_mm(7.2))),
        silk: Vec::new(),
    }
}

/// A 20 x 20mm board with one M3 hole at its centre.
fn board_with_a_hole() -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);

    let mut library = FootprintLibrary::new();
    library.register(mounting_hole());

    world.spawn_component(
        RefDes::new("H1"),
        Value::new("M3"),
        Position::from_mm(10.0, 10.0),
        Rotation(0),
        FootprintRef::new("MOUNT-M3"),
        NetConnections::new(),
    );

    world.set_footprints(library.clone());
    (world, library)
}

/// A horizontal trace on the top layer at height `y`, crossing the board.
fn spawn_horizontal_trace(world: &mut BoardWorld, y: f64, layer: Layer) {
    let net = NetId::new(7);
    world.spawn_entity((
        Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(2.0, y),
                Point::from_mm(18.0, y),
            )],
            width: Nm::from_mm(0.2),
            layer,
            net_id: net,
            locked: false,
            source: TraceSource::Autorouted,
        },
        net,
    ));
}

#[test]
fn a_trace_straight_through_the_hole_is_reported() {
    let (mut world, library) = board_with_a_hole();
    // Dead across the middle of a 3.2mm hole at 10mm, 10mm.
    spawn_horizontal_trace(&mut world, 10.0, Layer::TopCopper);
    world.rebuild_spatial_index_from_library(&library);

    let violations = cypcb_drc::rules::MountingHoleClearanceRule
        .check(&mut world, &DesignRules::jlcpcb_2layer());

    assert_eq!(
        violations.len(),
        1,
        "a trace drawn across a 3.2mm hole: the drill cuts it open and the \
         screw touches the copper. Got: {violations:?}"
    );
    let message = format!("{:?}", violations[0]);
    assert!(
        message.contains("H1"),
        "the report has to name the hole so somebody can find it: {message}"
    );
}

#[test]
fn a_trace_on_the_other_layer_is_reported_too() {
    // The drill goes through the whole board, so the layer a trace is on
    // makes no difference. A rule that only walked the top copper would pass
    // the test above and still ship the defect.
    let (mut world, library) = board_with_a_hole();
    spawn_horizontal_trace(&mut world, 10.0, Layer::BottomCopper);
    world.rebuild_spatial_index_from_library(&library);

    let violations = cypcb_drc::rules::MountingHoleClearanceRule
        .check(&mut world, &DesignRules::jlcpcb_2layer());
    assert_eq!(
        violations.len(),
        1,
        "the hole is drilled through both layers: {violations:?}"
    );
}

#[test]
fn a_trace_that_clears_the_hole_is_not_reported() {
    // 4mm below the centre of a 3.2mm hole: the hole's edge is at 8.4mm, the
    // trace's own edge at 6.1mm, so 2.3mm of clearance. Well past the
    // 0.3mm JLCPCB edge clearance this rule measures with.
    let (mut world, library) = board_with_a_hole();
    spawn_horizontal_trace(&mut world, 6.0, Layer::TopCopper);
    world.rebuild_spatial_index_from_library(&library);

    let violations = cypcb_drc::rules::MountingHoleClearanceRule
        .check(&mut world, &DesignRules::jlcpcb_2layer());
    assert!(
        violations.is_empty(),
        "a trace 2.3mm clear of the hole is fine: {violations:?}"
    );
}

#[test]
fn a_trace_just_inside_the_clearance_is_reported_and_just_outside_is_not() {
    // The boundary, measured rather than assumed. The hole edge is 1.6mm from
    // its centre and the trace half-width is 0.1mm, so the copper gap for a
    // trace at height `y` is `(10 - y) - 1.6 - 0.1`. JLCPCB's edge clearance
    // is 0.3mm, so the gap closes at y = 8.0: 7.9 leaves 0.4mm and 8.1 leaves
    // 0.2mm. The first version of this test put both cases on the same side of
    // that pivot and read the rule as wrong when the arithmetic was.
    let rules = DesignRules::jlcpcb_2layer();
    assert_eq!(
        rules.min_edge_clearance,
        Nm::from_mm(0.3),
        "this test's arithmetic is written for a 0.3mm clearance"
    );

    let (mut world, library) = board_with_a_hole();
    spawn_horizontal_trace(&mut world, 7.9, Layer::TopCopper);
    world.rebuild_spatial_index_from_library(&library);
    let outside = cypcb_drc::rules::MountingHoleClearanceRule.check(&mut world, &rules);
    assert!(
        outside.is_empty(),
        "0.4mm of clearance is more than the 0.3mm asked for: {outside:?}"
    );

    let (mut world, library) = board_with_a_hole();
    spawn_horizontal_trace(&mut world, 8.1, Layer::TopCopper);
    world.rebuild_spatial_index_from_library(&library);
    let inside = cypcb_drc::rules::MountingHoleClearanceRule.check(&mut world, &rules);
    assert_eq!(
        inside.len(),
        1,
        "0.2mm of clearance is less than the 0.3mm asked for: {inside:?}"
    );
}

#[test]
fn a_plated_hole_is_not_measured_by_this_rule() {
    // A through-hole pad has copper by design - that is what plating is - and
    // measuring copper-to-hole on one would report every pin on the board.
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);

    let mut library = FootprintLibrary::new();
    let drill = Nm::from_mm(1.0);
    library.register(Footprint {
        name: "PIN".to_string(),
        description: "one plated pin".to_string(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Circle,
            position: Point::ORIGIN,
            size: (Nm::from_mm(1.7), Nm::from_mm(1.7)),
            drill: Some(drill),
            slot: None,
            layers: vec![Layer::TopCopper, Layer::BottomCopper],
            mask_margin: None,
        }],
        bounds: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(1.7), Nm::from_mm(1.7))),
        courtyard: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(2.2), Nm::from_mm(2.2))),
        silk: Vec::new(),
    });

    let mut nets = NetConnections::new();
    nets.add(cypcb_world::components::PinConnection::new(
        "1".to_string(),
        NetId::new(7),
    ));
    world.spawn_component(
        RefDes::new("J1"),
        Value::new("pin"),
        Position::from_mm(10.0, 10.0),
        Rotation(0),
        FootprintRef::new("PIN"),
        nets,
    );
    world.set_footprints(library.clone());

    // A trace running into the pin, which is how a pin is meant to be used.
    spawn_horizontal_trace(&mut world, 10.0, Layer::TopCopper);
    world.rebuild_spatial_index_from_library(&library);

    let violations = cypcb_drc::rules::MountingHoleClearanceRule
        .check(&mut world, &DesignRules::jlcpcb_2layer());
    assert!(
        violations.is_empty(),
        "a trace reaching a plated pin is a connection, not a violation: {violations:?}"
    );
}

#[test]
fn the_default_rule_set_reports_it_too() {
    // The rule existing is not the same as the rule running. `run_drc` is what
    // `cypcb check` calls and what the viewer shows, and a rule written but
    // never registered is a rule nobody is protected by.
    let (mut world, library) = board_with_a_hole();
    spawn_horizontal_trace(&mut world, 10.0, Layer::TopCopper);
    world.rebuild_spatial_index_from_library(&library);

    let result = cypcb_drc::run_drc(&mut world, &DesignRules::jlcpcb_2layer());
    let about_the_hole: Vec<_> = result
        .violations
        .iter()
        .filter(|violation| violation.message.contains("unplated hole"))
        .collect();

    assert_eq!(
        about_the_hole.len(),
        1,
        "the checker a user runs has to report a trace across a mounting hole. \
         All violations: {:?}",
        result.violations
    );
}

#[test]
fn a_board_whose_only_feature_is_a_mounting_hole_is_clean() {
    // The other half of a new rule: what it says about a board with nothing
    // wrong. Adding mounting holes gave every board that has one two false
    // violations per hole - an unconnected pin, because a hole has no pin, and
    // a zero annular ring, because a hole has no copper around it. A checker
    // that cries about correct boards is one people stop reading.
    let (mut world, library) = board_with_a_hole();
    world.rebuild_spatial_index_from_library(&library);

    let result = cypcb_drc::run_drc(&mut world, &DesignRules::jlcpcb_2layer());
    assert!(
        result.violations.is_empty(),
        "a board with one correctly placed mounting hole and nothing else: {:?}",
        result.violations
    );
}
