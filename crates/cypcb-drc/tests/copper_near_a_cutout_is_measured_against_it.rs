//! Copper and holes beside a cutout are measured against the cutout.
//!
//! `cargo test -p cypcb-drc --test copper_near_a_cutout_is_measured_against_it`
//!
//! A board states its edge as an outline and the cutouts inside it. The edge
//! rules measured the outline alone, so a via beside a window in the board was
//! "well clear of the edge" however close it sat to the window, and a via in
//! the window was not on the board at all and was not noticed. KiCad puts every
//! Edge.Cuts shape in one tree and measures copper against all of them
//! (drc_test_provider_edge_clearance.cpp:272-279, :358-361), so a window is an
//! edge like the rim.
//!
//! The board here is 40 x 30mm with a 10 x 10mm window from (15, 10) to
//! (25, 20). Every distance is worked out in the test from the numbers above.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::rules::{DrcRule, EdgeClearanceRule, HoleToEdgeRule};
use cypcb_world::components::trace::Via;
use cypcb_world::components::zone::{Zone, ZoneOutline};
use cypcb_world::components::{BoardCutouts, BoardOutline, Layer};
use cypcb_world::BoardWorld;

const VIA_OUTER: f64 = 0.6;
const VIA_DRILL: f64 = 0.3;

fn at(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

/// The window, as a ring.
fn window() -> Vec<Point> {
    vec![
        at(15.0, 10.0),
        at(25.0, 10.0),
        at(25.0, 20.0),
        at(15.0, 20.0),
    ]
}

/// The 40 x 30mm board. `outline` says whether it states its rim as a ring or
/// leaves it to the size, as a board drawn as one rectangle does.
fn board(outline: bool, cutouts: Vec<Vec<Point>>) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        "windowed".to_string(),
        (Nm::from_mm(40.0), Nm::from_mm(30.0)),
        2,
    );
    let entity = world.board_entity().expect("a board");
    if outline {
        let ring = BoardOutline::new(vec![
            at(0.0, 0.0),
            at(40.0, 0.0),
            at(40.0, 30.0),
            at(0.0, 30.0),
        ])
        .expect("a ring");
        world.ecs_mut().entity_mut(entity).insert(ring);
    }
    if let Some(cutouts) = BoardCutouts::new(cutouts) {
        world.ecs_mut().entity_mut(entity).insert(cutouts);
    }
    world
}

fn spawn_via(world: &mut BoardWorld, (x, y): (f64, f64)) -> bevy_ecs::entity::Entity {
    world.spawn_entity((Via {
        position: at(x, y),
        drill: Nm::from_mm(VIA_DRILL),
        outer_diameter: Nm::from_mm(VIA_OUTER),
        net_id: None,
        start_layer: Layer::TopCopper,
        end_layer: Layer::BottomCopper,
        locked: false,
    },))
}

/// The one report for `entity` from `rule`, as the distance it measured.
fn reported(
    rule: &dyn DrcRule,
    world: &mut BoardWorld,
    entity: bevy_ecs::entity::Entity,
) -> Option<Nm> {
    world.rebuild_spatial_index_from_library(&cypcb_world::footprint::FootprintLibrary::new());
    let rules = DesignRules::jlcpcb_2layer();
    let rows: Vec<_> = rule
        .check(world, &rules)
        .into_iter()
        .filter(|v| v.entity == entity)
        .collect();
    assert!(rows.len() <= 1, "one report per piece of copper: {rows:?}");
    rows.first().map(|v| v.actual.expect("a distance"))
}

#[test]
fn a_via_too_near_a_cutout_is_reported_with_the_distance_to_the_cutout() {
    let required = DesignRules::jlcpcb_2layer().min_edge_clearance;
    // The disc's near side is `required - 0.05mm` from the window's left side.
    let gap = required.0 - Nm::from_mm(0.05).0;
    let centre_x = 15.0 - (gap as f64 / 1e6) - VIA_OUTER / 2.0;

    let mut world = board(true, vec![window()]);
    let via = spawn_via(&mut world, (centre_x, 15.0));
    let measured = reported(&EdgeClearanceRule, &mut world, via).expect("reported");

    // Worked from the numbers: the window's left side is x = 15mm, the via
    // centre is at `centre_x`, the disc's radius is half of VIA_OUTER.
    let expected = at(15.0, 0.0).x.0 - at(centre_x, 0.0).x.0 - Nm::from_mm(VIA_OUTER / 2.0).0;
    assert_eq!(measured.0, expected);
    assert!(measured.0 < required.0);
    assert!(
        (measured.0 - gap).abs() <= 1,
        "{} against {gap}",
        measured.0
    );
}

#[test]
fn a_via_far_enough_from_a_cutout_is_not_reported() {
    let required = DesignRules::jlcpcb_2layer().min_edge_clearance;
    let gap = required.0 + Nm::from_mm(0.05).0;
    let centre_x = 15.0 - (gap as f64 / 1e6) - VIA_OUTER / 2.0;

    let mut world = board(true, vec![window()]);
    let via = spawn_via(&mut world, (centre_x, 15.0));
    assert_eq!(reported(&EdgeClearanceRule, &mut world, via), None);
}

#[test]
fn the_same_via_on_a_board_without_the_cutout_is_not_reported() {
    // Control: the cutout is what makes the report.
    let required = DesignRules::jlcpcb_2layer().min_edge_clearance;
    let gap = required.0 - Nm::from_mm(0.05).0;
    let centre_x = 15.0 - (gap as f64 / 1e6) - VIA_OUTER / 2.0;

    let mut world = board(true, Vec::new());
    let via = spawn_via(&mut world, (centre_x, 15.0));
    assert_eq!(reported(&EdgeClearanceRule, &mut world, via), None);
}

#[test]
fn a_via_in_a_cutout_is_off_the_board() {
    let mut world = board(true, vec![window()]);
    let via = spawn_via(&mut world, (20.0, 15.0));
    assert_eq!(
        reported(&EdgeClearanceRule, &mut world, via),
        Some(Nm(0)),
        "copper in the window is copper with no board under it"
    );
}

#[test]
fn a_hole_too_near_a_cutout_is_reported() {
    let required = DesignRules::jlcpcb_2layer().min_hole_to_edge;
    let gap = required.0 - Nm::from_mm(0.05).0;
    // The drill's wall, not the copper around it, is `gap` from the window's
    // right side at x = 25mm.
    let centre_x = 25.0 + (gap as f64 / 1e6) + VIA_DRILL / 2.0;

    let mut world = board(true, vec![window()]);
    let via = spawn_via(&mut world, (centre_x, 15.0));
    let measured = reported(&HoleToEdgeRule, &mut world, via).expect("reported");
    let expected = at(centre_x, 0.0).x.0 - at(25.0, 0.0).x.0 - Nm::from_mm(VIA_DRILL / 2.0).0;
    assert_eq!(measured.0, expected);
    assert!(
        (measured.0 - gap).abs() <= 1,
        "{} against {gap}",
        measured.0
    );

    // Control: without the window the same hole is nowhere near an edge.
    let mut world = board(true, Vec::new());
    let via = spawn_via(&mut world, (centre_x, 15.0));
    assert_eq!(reported(&HoleToEdgeRule, &mut world, via), None);
}

#[test]
fn a_board_drawn_as_a_rectangle_with_a_cutout_has_both_edges() {
    // A rectangle drawn as one shape has no ring, so the board holds no outline
    // and one cutout. The rim is the rectangle its size describes.
    let mut world = board(false, vec![window()]);
    let entity = world.board_entity().unwrap();
    assert!(world.ecs().get::<BoardOutline>(entity).is_none());

    let required = DesignRules::jlcpcb_2layer().min_edge_clearance;
    let near = required.0 - Nm::from_mm(0.05).0;

    // Beside the window.
    let centre_x = 15.0 - (near as f64 / 1e6) - VIA_OUTER / 2.0;
    let beside_window = spawn_via(&mut world, (centre_x, 15.0));
    // Beside the rim, 5mm up the left side.
    let rim_x = (near as f64 / 1e6) + VIA_OUTER / 2.0;
    let beside_rim = spawn_via(&mut world, (rim_x, 5.0));
    // Well inside, clear of both.
    let clear = spawn_via(&mut world, (6.0, 25.0));

    world.rebuild_spatial_index_from_library(&cypcb_world::footprint::FootprintLibrary::new());
    let rows: Vec<_> = EdgeClearanceRule
        .check(&mut world, &DesignRules::jlcpcb_2layer())
        .into_iter()
        .map(|v| (v.entity, v.actual.unwrap().0))
        .collect();
    let of = |e| {
        rows.iter()
            .find(|(entity, _)| *entity == e)
            .map(|(_, d)| *d)
    };
    assert_eq!(
        of(beside_window),
        Some(at(15.0, 0.0).x.0 - at(centre_x, 0.0).x.0 - Nm::from_mm(VIA_OUTER / 2.0).0)
    );
    assert_eq!(
        of(beside_rim),
        Some(at(rim_x, 0.0).x.0 - Nm::from_mm(VIA_OUTER / 2.0).0)
    );
    assert_eq!(of(clear), None);
}

#[test]
fn two_cutouts_are_both_edges() {
    let second = vec![
        at(30.0, 10.0),
        at(35.0, 10.0),
        at(35.0, 20.0),
        at(30.0, 20.0),
    ];
    let required = DesignRules::jlcpcb_2layer().min_edge_clearance;
    let near = required.0 - Nm::from_mm(0.05).0;
    let centre_x = 30.0 - (near as f64 / 1e6) - VIA_OUTER / 2.0;

    let mut world = board(true, vec![window(), second]);
    let via = spawn_via(&mut world, (centre_x, 15.0));
    let measured = reported(&EdgeClearanceRule, &mut world, via).expect("reported");
    assert_eq!(
        measured.0,
        at(30.0, 0.0).x.0 - at(centre_x, 0.0).x.0 - Nm::from_mm(VIA_OUTER / 2.0).0
    );
}

/// A pour drawn as a polygon: the ring is what is measured.
fn pour_ring(world: &mut BoardWorld, points: &[(f64, f64)]) -> bevy_ecs::entity::Entity {
    let net = world.intern_net("GND");
    let ring = ZoneOutline::new(points.iter().map(|&(x, y)| at(x, y)).collect()).expect("a ring");
    world.spawn_outlined_zone(Zone::copper_pour_for_net(Rect::default(), 0b01, net), ring)
}

#[test]
fn a_pour_ring_a_tenth_of_a_millimetre_from_a_cutout_is_reported_at_that_distance() {
    let mut world = board(true, vec![window()]);
    // The ring's right side is x = 14.9mm, 0.1mm from the window's left side.
    let pour = pour_ring(
        &mut world,
        &[(5.0, 5.0), (14.9, 5.0), (14.9, 25.0), (5.0, 25.0)],
    );
    assert_eq!(
        reported(&EdgeClearanceRule, &mut world, pour),
        Some(Nm::from_mm(0.1))
    );

    // Control: the same ring on the board without the window is clear.
    let mut world = board(true, Vec::new());
    let pour = pour_ring(
        &mut world,
        &[(5.0, 5.0), (14.9, 5.0), (14.9, 25.0), (5.0, 25.0)],
    );
    assert_eq!(reported(&EdgeClearanceRule, &mut world, pour), None);
}

#[test]
fn a_pour_ring_with_a_corner_in_a_cutout_reads_as_zero() {
    let mut world = board(true, vec![window()]);
    let pour = pour_ring(
        &mut world,
        &[(10.0, 12.0), (20.0, 12.0), (20.0, 18.0), (10.0, 18.0)],
    );
    assert_eq!(
        reported(&EdgeClearanceRule, &mut world, pour),
        Some(Nm::ZERO)
    );
}
