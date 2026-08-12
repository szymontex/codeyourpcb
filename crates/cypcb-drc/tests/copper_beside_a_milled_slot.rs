//! Another net's copper against a milled opening.
//!
//! `cargo test -p cypcb-drc --test copper_beside_a_milled_slot`
//!
//! D7, settled by the owner: `min_slot_clearance` is the routed-edge question
//! asked of a slot. The number lived in every fab preset since the tables were
//! written and nothing read it - the same shape as `min_paste_clearance`
//! before `PasteClearanceRule`.
//!
//! The objection that killed this rule the first time is what this file is
//! mostly about. A plated slot has its own annulus around it by construction,
//! so a rule that measures every piece of copper against every slot fires on
//! every slotted pad ever drawn. Two of the four tests below are that case,
//! and they are the ones that would fail on a naive implementation.

use cypcb_core::{Nm, Point};
use cypcb_drc::{run_drc, Preset, PresetRules, ViolationKind};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// A 20x20 board with one slotted part at `(x, y)`.
///
/// The slot is 2.4mm long and 1.0mm wide, so its capsule is a 1.4mm segment
/// with a 0.5mm radius: the bit centres sit 0.7mm either side of the pad and
/// the wall is 1.2mm from it along the length, 0.5mm across.
fn board_with_a_slot(x: f64, y: f64) -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board(
        "slotted".to_string(),
        (Nm::from_mm(20.0), Nm::from_mm(20.0)),
        2,
    );

    let mut library = FootprintLibrary::new();
    let base = library
        .get("0402")
        .expect("the library has an 0402")
        .clone();
    library.register_design(Footprint {
        name: "latch".to_string(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Oblong,
            position: Point::ORIGIN,
            size: (Nm::from_mm(3.2), Nm::from_mm(1.8)),
            drill: Some(Nm::from_mm(1.0)),
            slot: Some((Nm::from_mm(2.4), Nm::from_mm(1.0))),
            layers: vec![Layer::TopCopper, Layer::BottomCopper],
        }],
        ..base
    });
    world.set_footprints(library);

    world.spawn_component(
        RefDes::new("J1"),
        Value::new(""),
        Position::from_mm(x, y),
        Rotation::ZERO,
        FootprintRef::new("latch"),
        NetConnections::new(),
    );
    let library = world.footprints().clone();
    world.rebuild_spatial_index_from_library(&library);
    (world, library)
}

/// Put an 0402 at `(x, y)` - foreign copper with a net of its own.
fn add_a_chip(world: &mut BoardWorld, refdes: &str, x: f64, y: f64) {
    world.spawn_component(
        RefDes::new(refdes),
        Value::new("10k"),
        Position::from_mm(x, y),
        Rotation::ZERO,
        FootprintRef::new("0402"),
        NetConnections::new(),
    );
}

fn slot_faults(world: &mut BoardWorld, library: &FootprintLibrary) -> Vec<String> {
    // Rebuilt here rather than by the caller: an empty index makes every test
    // in this file pass vacuously, which is exactly what happened the first
    // time it was written.
    world.rebuild_spatial_index_from_library(library);
    assert!(
        world.spatial().iter().count() > 0,
        "the spatial index is empty, so nothing below measures anything"
    );
    run_drc(world, &Preset::JlcpcbStandard2Layer.rules())
        .violations
        .into_iter()
        .filter(|violation| violation.kind == ViolationKind::SlotClearance)
        .map(|violation| violation.message)
        .collect()
}

#[test]
fn a_slotted_part_alone_on_a_board_is_not_a_violation() {
    // The objection, stated as a test. The pad's own annulus wraps the slot by
    // construction - that is what a plated slot IS - so a rule that does not
    // skip the slot's own component reports every slotted part ever drawn.
    let (mut world, library) = board_with_a_slot(10.0, 10.0);
    let faults = slot_faults(&mut world, &library);
    assert!(
        faults.is_empty(),
        "a plated slot's own annulus is not foreign copper: {faults:?}"
    );
}

#[test]
fn two_slotted_parts_far_apart_are_not_violations_of_each_other() {
    // The same objection at one remove: skipping only the slot's own entity is
    // not enough if the geometry is wrong, because each part's annulus is
    // foreign to the other part's slot. 8mm apart, nothing to report.
    let (mut world, library) = board_with_a_slot(6.0, 10.0);
    world.spawn_component(
        RefDes::new("J2"),
        Value::new(""),
        Position::from_mm(14.0, 10.0),
        Rotation::ZERO,
        FootprintRef::new("latch"),
        NetConnections::new(),
    );
    let faults = slot_faults(&mut world, &library);
    assert!(faults.is_empty(), "8mm apart is clear: {faults:?}");
}

#[test]
fn another_parts_copper_beside_the_slot_is_reported() {
    // An 0402 is 1.0 x 0.5mm of body with pads either side, placed 1.0mm above
    // the slot's centre. The slot wall runs 0.5mm out across its width, so the
    // laminate between them is well under JLCPCB's 0.3mm.
    let (mut world, library) = board_with_a_slot(10.0, 10.0);
    add_a_chip(&mut world, "R1", 10.0, 10.6);

    let faults = slot_faults(&mut world, &library);
    assert_eq!(
        faults.len(),
        1,
        "one piece of foreign copper, one report: {faults:?}"
    );
    assert!(
        faults[0].contains("milled slot"),
        "the message names what it measured against: {}",
        faults[0]
    );
}

#[test]
fn a_body_over_the_slot_with_its_copper_clear_is_not_reported() {
    // A component sits in the spatial index as its **courtyard** - the
    // assembly keepout over the whole part body - and this rule is about
    // copper. A DIP-8's body reaches 1.3mm either side of its centre line
    // while its pad columns sit 3.81mm apart, so the plastic can overhang a
    // slot with every piece of copper well clear of it.
    //
    // Measured against the courtyard that reads as a violation the board does
    // not have. This is the fault this rule shipped with, found by reading it
    // rather than by a board failing, and written down before it was fixed.
    let (mut world, library) = board_with_a_slot(10.0, 10.0);
    world.spawn_component(
        RefDes::new("U1"),
        Value::new("NE555"),
        // The gap between the two pad columns straddles the slot: the body
        // covers it, the pads are 1.9mm either side of it.
        Position::from_mm(10.0, 10.0),
        Rotation::ZERO,
        FootprintRef::new("DIP-8"),
        NetConnections::new(),
    );

    let faults = slot_faults(&mut world, &library);
    assert!(
        faults.is_empty(),
        "a body over a slot is an assembly question, not a copper one: {faults:?}"
    );
}

#[test]
fn the_same_copper_moved_clear_is_not_reported() {
    // The control, and the half that decides whether the rule measures
    // anything at all: the identical part 3mm away passes. Without this, a
    // rule that reported everything would pass the test above.
    let (mut world, library) = board_with_a_slot(10.0, 10.0);
    add_a_chip(&mut world, "R1", 10.0, 14.0);

    let faults = slot_faults(&mut world, &library);
    assert!(faults.is_empty(), "3mm away is clear: {faults:?}");
}
