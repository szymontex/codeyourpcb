//! A body over a mounting hole is not copper over a mounting hole.
//!
//! `cargo test -p cypcb-cli --test a_body_over_a_mounting_hole_is_not_copper`
//!
//! `MountingHoleClearanceRule` measures copper against an unplated hole: the
//! drill cuts whatever it passes through, and the screw head touches what is
//! left. A component sits in the spatial index as its **courtyard** - the
//! assembly keepout over the part body - and the rule measured that box, so a
//! part whose plastic reaches over the hole while its pads stay well clear was
//! refused for copper the drill never touches.
//!
//! The same defect the edge rule carried one commit ago, and the same fix:
//! `component_pads`, which `ClearanceRule` and `slot-clearance` already share.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::rules::MountingHoleClearanceRule;
use cypcb_drc::{DesignRules, DrcRule};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// A part whose body is `body_half_mm` across and whose single pad is 0.6mm
/// across, placed so the pad's copper stops `pad_gap_mm` from the mounting
/// hole at the middle of the board.
fn board_with_hole_and_part(pad_gap_mm: f64, body_half_mm: f64) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("m".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);

    let mut library = FootprintLibrary::new();
    // An M3 mounting hole: drilled, unplated, no copper of its own.
    library.register(Footprint {
        name: "mount".to_string(),
        description: String::new(),
        bounds: Rect::from_points(Point::from_mm(-1.6, -1.6), Point::from_mm(1.6, 1.6)),
        courtyard: Rect::from_points(Point::from_mm(-1.6, -1.6), Point::from_mm(1.6, 1.6)),
        silk: Vec::new(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Circle,
            position: Point::ORIGIN,
            size: (Nm::from_mm(3.2), Nm::from_mm(3.2)),
            drill: Some(Nm::from_mm(3.2)),
            slot: None,
            // No copper layer at all is what makes a hole unplated.
            layers: Vec::new(),
            mask_margin: None,
        }],
    });
    library.register(Footprint {
        name: "part".to_string(),
        description: String::new(),
        bounds: Rect::from_points(Point::from_mm(-0.3, -0.3), Point::from_mm(0.3, 0.3)),
        courtyard: Rect::from_points(
            Point::from_mm(-body_half_mm, -body_half_mm),
            Point::from_mm(body_half_mm, body_half_mm),
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
        RefDes::new("H1"),
        Value::new(""),
        Position::from_mm(10.0, 10.0),
        Rotation::ZERO,
        FootprintRef::new("mount"),
        NetConnections::new(),
    );
    // The hole's radius is 1.6mm and the pad's copper reaches 0.3mm from its
    // own centre, so the centre goes that much further out than the gap.
    world.spawn_component(
        RefDes::new("R1"),
        Value::new(""),
        Position::from_mm(10.0 + 1.6 + pad_gap_mm + 0.3, 10.0),
        Rotation::ZERO,
        FootprintRef::new("part"),
        NetConnections::new(),
    );

    world.set_footprints(library.clone());
    world.rebuild_spatial_index_from_library(&library);
    world
}

#[test]
fn copper_beside_a_mounting_hole_is_refused_at_the_fabs_figure() {
    // JLCPCB asks for 0.3mm. Copper 0.2mm from the drilled edge is cut open by
    // it; copper 0.4mm away is not.
    let rules = DesignRules::jlcpcb_2layer();

    let mut too_close = board_with_hole_and_part(0.2, 0.5);
    let violations = MountingHoleClearanceRule.check(&mut too_close, &rules);
    assert_eq!(
        violations.len(),
        1,
        "copper 0.2mm from an unplated hole is not refused"
    );
    assert!(
        violations[0].message.contains("unplated hole H1"),
        "the report does not name the hole: {}",
        violations[0].message
    );

    let mut clear = board_with_hole_and_part(0.4, 0.5);
    assert!(
        MountingHoleClearanceRule
            .check(&mut clear, &rules)
            .is_empty(),
        "copper 0.4mm from an unplated hole is refused against a 0.3mm rule"
    );
}

#[test]
fn a_body_reaching_over_the_hole_is_not_refused_for_copper_it_does_not_have() {
    // The defect. The part's body is 8mm across, so its courtyard swallows the
    // hole whole, and its only pad is 0.4mm from the drilled edge - which
    // clears the fab's 0.3mm. Measured as a courtyard this is a violation of
    // 0.00mm; measured as copper it is not a violation at all.
    let mut world = board_with_hole_and_part(0.4, 4.0);
    let violations = MountingHoleClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());
    assert!(
        violations.is_empty(),
        "a part whose body covers the hole was refused for copper that is \
         0.4mm clear of it: {violations:?}"
    );
}
