//! A keepout flags a part only if it forbids footprints.
//!
//! `cargo test -p cypcb-drc --test a_keepout_flags_a_part_only_if_it_forbids_footprints`
//!
//! `KeepoutRule` reports a part whose centre stands in a keepout. A KiCad rule
//! area that forbids only copper pour, or only tracks, says nothing about where
//! a part may stand: of the 62 rows it gave on the 132 boards of the test set,
//! 60 were parts standing in such an area. A keepout with no component forbids
//! all five kinds, footprints among them, and still flags.

use cypcb_core::{Point, Rect};
use cypcb_drc::rules::KeepoutRule;
use cypcb_drc::{DesignRules, DrcRule};
use cypcb_world::components::zone::{KeepoutKind, KeepoutKinds, Zone};
use cypcb_world::components::{FootprintRef, NetConnections, Position, RefDes, Rotation, Value};
use cypcb_world::BoardWorld;

/// A part at (15, 15) and a keepout round it, as a rectangle or as a ring.
fn rows(kinds: Option<&[KeepoutKind]>, outlined: bool) -> usize {
    let mut world = BoardWorld::new();
    let bounds = Rect::new(Point::from_mm(10.0, 10.0), Point::from_mm(20.0, 20.0));
    let zone = Zone::keepout(bounds, 0xFFFF_FFFF);
    let entity = if outlined {
        let ring = cypcb_world::components::zone::ZoneOutline::new(vec![
            bounds.min,
            Point::new(bounds.max.x, bounds.min.y),
            bounds.max,
            Point::new(bounds.min.x, bounds.max.y),
        ])
        .expect("a square");
        world.spawn_outlined_zone(zone, ring)
    } else {
        world.spawn_entity((zone,))
    };
    if let Some(kinds) = kinds {
        world
            .ecs_mut()
            .entity_mut(entity)
            .insert(KeepoutKinds::of_kinds(kinds));
    }
    world.spawn_component(
        RefDes::new("R1"),
        Value::new("10k"),
        Position::from_mm(15.0, 15.0),
        Rotation::ZERO,
        FootprintRef::new("0402"),
        NetConnections::new(),
    );
    KeepoutRule.check(&mut world, &DesignRules::default()).len()
}

#[test]
fn an_area_that_forbids_footprints_flags_the_part() {
    for outlined in [false, true] {
        assert_eq!(rows(Some(&[KeepoutKind::Footprints]), outlined), 1);
        assert_eq!(
            rows(
                Some(&[KeepoutKind::Tracks, KeepoutKind::Footprints]),
                outlined
            ),
            1
        );
    }
}

#[test]
fn a_keepout_with_no_component_forbids_footprints_too() {
    for outlined in [false, true] {
        assert_eq!(rows(None, outlined), 1);
    }
}

#[test]
fn an_area_that_forbids_anything_else_flags_nothing() {
    for outlined in [false, true] {
        for kind in [
            KeepoutKind::Tracks,
            KeepoutKind::Vias,
            KeepoutKind::Pads,
            KeepoutKind::Pour,
        ] {
            assert_eq!(rows(Some(&[kind]), outlined), 0, "{kind:?}");
        }
        assert_eq!(
            rows(
                Some(&[
                    KeepoutKind::Tracks,
                    KeepoutKind::Vias,
                    KeepoutKind::Pads,
                    KeepoutKind::Pour
                ]),
                outlined
            ),
            0
        );
    }
}
