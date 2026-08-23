//! Nothing is drilled where the board bends.
//!
//! `cargo test -p cypcb-drc --test nothing_is_drilled_where_the_board_bends`
//!
//! The barrel of a plated hole is a tube of copper on the wall of a drilled
//! hole. The laminate around it moves every time the board is folded and the
//! barrel does not: it work-hardens and splits, usually at the knee where the
//! plating meets the pad, and usually after the product has shipped. Every
//! flex design guide says the same thing - no holes in the bend - and until
//! `flex` was a word here nothing could say it about a specific hole.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::{run_drc, Preset, PresetRules, ViolationKind};
use cypcb_world::components::trace::Via;
use cypcb_world::components::zone::Zone;
use cypcb_world::components::Layer;
use cypcb_world::BoardWorld;

/// A 60x20 board with a via at `via_x`, and a bend from 20mm to 40mm when
/// `bends` says so.
fn board(via_x: f64, bends: bool, name: Option<&str>) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        "wearable".to_string(),
        (Nm::from_mm(60.0), Nm::from_mm(20.0)),
        2,
    );
    if bends {
        let region = Zone::flex(
            Rect::new(Point::from_mm(20.0, 0.0), Point::from_mm(40.0, 20.0)),
            0xFFFF_FFFF,
        );
        let region = match name {
            Some(name) => region.with_name(name),
            None => region,
        };
        world.ecs_mut().spawn(region);
    }
    let net_id = world.intern_net("SIG");
    world.ecs_mut().spawn((
        Via {
            position: Point::from_mm(via_x, 10.0),
            drill: Nm::from_mm(0.3),
            outer_diameter: Nm::from_mm(0.6),
            net_id,
            start_layer: Layer::TopCopper,
            end_layer: Layer::BottomCopper,
            locked: false,
        },
        net_id,
    ));
    world
}

fn faults(world: &mut BoardWorld) -> Vec<String> {
    run_drc(world, &Preset::JlcpcbStandard2Layer.rules())
        .violations
        .into_iter()
        .filter(|violation| violation.kind == ViolationKind::FlexHole)
        .map(|violation| violation.message)
        .collect()
}

#[test]
fn a_hole_in_the_bend_is_reported() {
    let mut world = board(30.0, true, Some("bend"));
    let said = faults(&mut world);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("flexible region 'bend'"), "{}", said[0]);
    assert!(
        said[0].contains("0.300mm across"),
        "the message names the hole: {}",
        said[0]
    );
}

#[test]
fn a_hole_outside_the_bend_is_left_alone() {
    // The rigid end of the same board. This is where the connectors go, and a
    // rule that reported them would report every rigid-flex board ever drawn.
    let mut world = board(10.0, true, Some("bend"));
    assert_eq!(faults(&mut world), Vec::<String>::new());
}

#[test]
fn a_board_that_does_not_bend_says_nothing() {
    // The control, and it is every board this project shipped before `flex`
    // was a word: no region, no report, whatever is drilled where.
    let mut world = board(30.0, false, None);
    assert_eq!(faults(&mut world), Vec::<String>::new());
}

#[test]
fn an_unnamed_region_still_says_which_kind_of_place_it_is() {
    let mut world = board(30.0, true, None);
    let said = faults(&mut world);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("a flexible region"), "{}", said[0]);
}

#[test]
fn the_region_is_the_area_and_not_the_whole_board() {
    // The boundary itself: 20mm is inside the region, 19.9mm is not.
    let mut inside = board(20.0, true, Some("bend"));
    assert_eq!(faults(&mut inside).len(), 1);

    let mut outside = board(19.9, true, Some("bend"));
    assert_eq!(faults(&mut outside), Vec::<String>::new());
}
