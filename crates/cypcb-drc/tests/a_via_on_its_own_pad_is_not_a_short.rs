//! A via standing on a pad of its own net is a join, not a short.
//!
//! `cargo test -p cypcb-drc --test a_via_on_its_own_pad_is_not_a_short`
//!
//! `ClearanceRule` leaves out the pads of a part that carry the net of the
//! trace it is measured against, but measured a via against every pad of the
//! part. A via dropped onto its own pin, as the router does to change layer at
//! a fine-pitch part, touches that pin, and the pair came out as a short at
//! 0.00mm while the nearest pad of another net was 0.10mm or more away. The
//! via's own net now leaves those pads out, as it does for a trace.
//!
//! The part has two 1.2 x 0.3mm pads, one above the other. The via is the
//! default 0.6mm disc centred on pad 1, so its copper reaches 0.30mm up; the
//! gap to pad 2 is the height of pad 2's lower edge minus 0.30mm.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{ClearanceRule, DrcRule};
use cypcb_drc::{shorts, ViolationKind};
use cypcb_world::components::trace::Via;
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, NetId, PadShape, PinConnection, Position, RefDes,
    Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::{BoardWorld, SpatialEntry, SpatialIndex};

const CENTRE: i64 = 10_000_000;
const OWN: NetId = NetId::new(1);
const OTHER: NetId = NetId::new(2);

fn pad(number: &str, y_nm: i64) -> PadDef {
    PadDef {
        number: number.to_string(),
        shape: PadShape::Rect,
        position: Point::new(Nm(0), Nm(y_nm)),
        size: (Nm::from_mm(1.2), Nm::from_mm(0.3)),
        drill: None,
        slot: None,
        layers: vec![Layer::TopCopper],
        mask_margin: None,
        rotation: Rotation::ZERO,
    }
}

/// A part with pad 1 on `OWN` at its origin and pad 2 on `OTHER`, centred
/// `second_pad_nm` above it, and a via on `via_net` at the centre of pad 1.
/// `turned` hands the pair to the rule the other way round, the part first:
/// each side of the pair has its own exemption.
fn board(second_pad_nm: i64, via_net: NetId, turned: bool) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(40.0)), 2);
    let mut library = FootprintLibrary::new();
    library.register(Footprint {
        name: "two".to_string(),
        description: "two pads".to_string(),
        pads: vec![pad("1", 0), pad("2", second_pad_nm)],
        bounds: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(2.0), Nm::from_mm(2.0))),
        courtyard: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(2.0), Nm::from_mm(2.0))),
        silk: Vec::new(),
    });
    let mut nets = NetConnections::new();
    nets.add(PinConnection::new("1".to_string(), OWN));
    nets.add(PinConnection::new("2".to_string(), OTHER));
    world.spawn_component(
        RefDes::new("U1"),
        Value::new("two"),
        Position(Point::new(Nm(CENTRE), Nm(CENTRE))),
        Rotation::from_degrees(0.0),
        FootprintRef::new("two"),
        nets,
    );
    world.set_footprints(library);
    world.spawn_entity((
        Via::new(Point::new(Nm(CENTRE), Nm(CENTRE)), via_net),
        via_net,
    ));
    world.rebuild_spatial_index_with_traces(|_| {
        Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(2.0), Nm::from_mm(2.0)))
    });
    if turned {
        // Loading the entries in the order the index hands them out hands
        // them out the other way round. Which way it went is checked below.
        let entries: Vec<SpatialEntry> = world.spatial().iter().cloned().collect();
        world
            .ecs_mut()
            .resource_mut::<SpatialIndex>()
            .rebuild(entries);
    }
    let first = world.spatial().iter().next().unwrap().entity;
    assert_eq!(
        world.ecs().get::<Via>(first).is_none(),
        turned,
        "turned {turned}: the part is the side the rule starts from"
    );
    world
}

fn clearance(world: &mut BoardWorld) -> Vec<cypcb_drc::DrcViolation> {
    let rules = DesignRules {
        min_clearance: Nm::from_mm(0.127),
        ..DesignRules::jlcpcb_2layer()
    };
    ClearanceRule.check(world, &rules)
}

fn actual_nm(violations: &[cypcb_drc::DrcViolation]) -> Vec<i64> {
    violations
        .iter()
        .map(|v| v.actual.map(|n| n.0).unwrap_or(-1))
        .collect()
}

#[test]
fn a_via_on_its_own_pad_with_the_next_pad_clear_is_not_reported() {
    // Pad 2 centred 0.65mm up: lower edge at 0.50mm, 0.200mm above the disc.
    for turned in [false, true] {
        let found = clearance(&mut board(650_000, OWN, turned));
        assert!(
            found.is_empty(),
            "turned {turned}: nothing under 0.127mm: {:?}",
            actual_nm(&found)
        );
    }
}

#[test]
fn a_via_on_its_own_pad_is_measured_to_the_next_pad() {
    // Pad 2 centred 0.55mm up: lower edge at 0.40mm, 0.100mm above the disc.
    for turned in [false, true] {
        let found = clearance(&mut board(550_000, OWN, turned));
        assert_eq!(
            actual_nm(&found),
            vec![100_000],
            "turned {turned}: one gap of 0.100mm"
        );
        assert_eq!(found[0].kind, ViolationKind::Clearance);
        assert_eq!(
            shorts(&found),
            0,
            "board between via and pad 2 is not a short"
        );
    }
}

#[test]
fn a_via_of_another_net_on_a_pad_is_a_short() {
    // Control: the same via on a third net sits on pad 1 and is a short.
    for turned in [false, true] {
        let found = clearance(&mut board(650_000, NetId::new(3), turned));
        assert_eq!(actual_nm(&found), vec![0], "turned {turned}: one short");
        assert_eq!(shorts(&found), 1);
    }
}
