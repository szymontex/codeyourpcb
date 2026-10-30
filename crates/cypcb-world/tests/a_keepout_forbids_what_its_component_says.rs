//! A keepout forbids what its component says, and all five without one.
//!
//! `cargo test -p cypcb-world --test a_keepout_forbids_what_its_component_says`
//!
//! The language's `keepout` has no way to say which kinds, and thirty-five
//! places build a `Zone` literally. A keepout with no `KeepoutKinds` is
//! therefore every kind; reading the missing component as "nothing forbidden"
//! would switch every keepout off.

use cypcb_core::{Nm, Point, Rect};
use cypcb_world::components::zone::{KeepoutKind, KeepoutKinds, Zone};
use cypcb_world::BoardWorld;

fn mm(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

fn area() -> Zone {
    Zone::keepout(Rect::new(mm(0.0, 0.0), mm(5.0, 5.0)), 0b01)
}

#[test]
fn a_keepout_with_no_component_forbids_all_five() {
    let mut world = BoardWorld::new();
    let entity = world.spawn_entity((area(),));
    for kind in KeepoutKind::ALL {
        assert!(world.forbids(entity, &area(), kind), "{kind:?}");
    }
    assert!(world.keepout_kinds(entity).is_all());
}

#[test]
fn a_keepout_with_a_component_forbids_those_and_no_others() {
    let mut world = BoardWorld::new();
    let entity = world.spawn_entity((
        area(),
        KeepoutKinds::of_kinds(&[KeepoutKind::Pour, KeepoutKind::Vias]),
    ));
    for kind in KeepoutKind::ALL {
        let wanted = matches!(kind, KeepoutKind::Pour | KeepoutKind::Vias);
        assert_eq!(world.forbids(entity, &area(), kind), wanted, "{kind:?}");
    }
}

#[test]
fn a_zone_that_is_not_a_keepout_forbids_nothing() {
    let mut world = BoardWorld::new();
    let gnd = world.intern_net("GND");
    let pour = Zone::copper_pour_for_net(Rect::new(mm(0.0, 0.0), mm(5.0, 5.0)), 0b01, gnd);
    let entity = world.spawn_entity((pour.clone(),));
    for kind in KeepoutKind::ALL {
        assert!(!world.forbids(entity, &pour, kind), "{kind:?}");
    }
}

#[test]
fn the_kinds_are_five_independent_switches() {
    assert!(KeepoutKinds::NONE.is_empty());
    assert!(KeepoutKinds::ALL.is_all());
    for (i, kind) in KeepoutKind::ALL.into_iter().enumerate() {
        let one = KeepoutKinds::of_kinds(&[kind]);
        for (j, other) in KeepoutKind::ALL.into_iter().enumerate() {
            assert_eq!(one.forbids(other), i == j);
        }
    }
}

#[test]
fn the_language_writes_a_partial_keepout_as_a_whole_one_and_says_so() {
    let write = |kinds: Option<&[KeepoutKind]>| {
        let mut world = BoardWorld::new();
        world.set_board("b".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);
        match kinds {
            Some(kinds) => world.spawn_entity((area(), KeepoutKinds::of_kinds(kinds))),
            None => world.spawn_entity((area(),)),
        };
        cypcb_world::dsl::board_as_dsl_reporting(&mut world)
    };
    let whole = write(None);
    let partial = write(Some(&[KeepoutKind::Pour]));
    assert!(
        !whole
            .not_written
            .iter()
            .any(|n| n.contains("forbidding everything")),
        "{:?}",
        whole.not_written
    );
    assert!(
        partial
            .not_written
            .iter()
            .any(|n| n.contains("forbidding everything")),
        "{:?}",
        partial.not_written
    );
    // The language says the same thing either way.
    assert_eq!(whole.source, partial.source);
}
