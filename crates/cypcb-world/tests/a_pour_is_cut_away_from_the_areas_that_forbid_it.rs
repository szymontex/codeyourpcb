//! A pour is cut away from the rule areas that forbid copper pour.
//!
//! `cargo test -p cypcb-world --test a_pour_is_cut_away_from_the_areas_that_forbid_it`
//!
//! KiCad's zone filler takes a rule area out of a pour when the area forbids
//! copper pour, along the area's outline with no gap (`zone_filler.cpp`
//! `isZoneFillKeepout`, "Keepouts use outline with no clearance"). The fill
//! here read no rule area at all, so a pour went over a keepout in the Gerber
//! and the IPC-2581 file as it did in the viewer. An area that forbids only
//! tracks says nothing about copper pour and leaves the pour whole.

use cypcb_core::pour::PourOptions;
use cypcb_core::{Nm, Point, Rect};
use cypcb_world::components::zone::{KeepoutKind, KeepoutKinds, Zone, ZoneOutline};
use cypcb_world::copper::{fill_outlined_zone, fill_zone};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{BoardWorld, Layer};

fn mm(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

/// A 20 x 20mm pour on GND, on the top layer.
fn pour_zone(world: &mut BoardWorld) -> Zone {
    world.set_board("b".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
    let gnd = world.intern_net("GND");
    let zone = Zone::copper_pour_for_net(Rect::new(mm(0.0, 0.0), mm(20.0, 20.0)), 0b01, gnd);
    world.spawn_entity((zone.clone(),));
    zone
}

fn forbidding(kinds: &[KeepoutKind]) -> Option<KeepoutKinds> {
    Some(KeepoutKinds::of_kinds(kinds))
}

/// The pour's area in whole square millimetres, from the rectangles and the
/// convex pieces both.
fn area_mm2(filled: &cypcb_world::copper::FilledPour) -> f64 {
    let rects: i128 = filled
        .pieces
        .iter()
        .map(|r| 2 * i128::from(r.max.x.0 - r.min.x.0) * i128::from(r.max.y.0 - r.min.y.0))
        .sum();
    let shapes: i128 = filled
        .shapes
        .iter()
        .map(|s| cypcb_core::pour_shape::twice_area(s))
        .sum();
    (rects + shapes) as f64 / 2.0 / 1e12
}

fn fill(world: &mut BoardWorld, zone: &Zone) -> cypcb_world::copper::FilledPour {
    // No clearance: the area is the only thing to cut away.
    let options = PourOptions {
        clearance: Nm::ZERO,
        ..PourOptions::default()
    };
    fill_zone(
        world,
        &FootprintLibrary::new(),
        Layer::TopCopper,
        zone,
        None,
        &options,
    )
}

/// A rule area 5..10mm square on the top layer.
fn square_area(world: &mut BoardWorld, kinds: Option<KeepoutKinds>) {
    let zone = Zone::keepout(Rect::new(mm(5.0, 5.0), mm(10.0, 10.0)), 0b01);
    match kinds {
        Some(kinds) => world.spawn_entity((zone, kinds)),
        None => world.spawn_entity((zone,)),
    };
}

#[test]
fn an_area_that_forbids_pour_takes_its_square_out_of_the_pour() {
    let mut world = BoardWorld::new();
    let zone = pour_zone(&mut world);
    square_area(&mut world, forbidding(&[KeepoutKind::Pour]));
    let filled = fill(&mut world, &zone);
    assert_eq!(area_mm2(&filled), 400.0 - 25.0);
    let hole = Rect::new(mm(5.0, 5.0), mm(10.0, 10.0));
    for piece in &filled.pieces {
        let inside = piece.min.x.0 < hole.max.x.0
            && hole.min.x.0 < piece.max.x.0
            && piece.min.y.0 < hole.max.y.0
            && hole.min.y.0 < piece.max.y.0;
        assert!(!inside, "copper in the area: {piece:?}");
    }
}

#[test]
fn the_five_kind_keepout_takes_it_out_too() {
    // No component: forbids all five, pour among them. It did not cut the pour
    // before either.
    let mut world = BoardWorld::new();
    let zone = pour_zone(&mut world);
    square_area(&mut world, None);
    assert_eq!(area_mm2(&fill(&mut world, &zone)), 400.0 - 25.0);
}

#[test]
fn an_area_that_forbids_something_else_leaves_the_pour_whole() {
    for kind in [
        KeepoutKind::Tracks,
        KeepoutKind::Vias,
        KeepoutKind::Pads,
        KeepoutKind::Footprints,
    ] {
        let mut world = BoardWorld::new();
        let zone = pour_zone(&mut world);
        square_area(&mut world, forbidding(&[kind]));
        assert_eq!(area_mm2(&fill(&mut world, &zone)), 400.0, "{kind:?}");
    }
}

#[test]
fn an_area_on_another_layer_leaves_the_pour_whole() {
    let mut world = BoardWorld::new();
    let zone = pour_zone(&mut world);
    let area = Zone::keepout(Rect::new(mm(5.0, 5.0), mm(10.0, 10.0)), 0b10);
    world.spawn_entity((area, KeepoutKinds::of_kinds(&[KeepoutKind::Pour])));
    assert_eq!(area_mm2(&fill(&mut world, &zone)), 400.0);
}

/// A triangle (5,5) (15,5) (5,15), the area of half a 10mm square: 50 mm2.
fn triangle_area(world: &mut BoardWorld) {
    let ring =
        ZoneOutline::new(vec![mm(5.0, 5.0), mm(15.0, 5.0), mm(5.0, 15.0)]).expect("a triangle");
    world.spawn_outlined_zone(Zone::keepout(Rect::default(), 0b01), ring);
    // Give it the component after the fact, as the importer does.
    let entity = world
        .outlined_zones()
        .first()
        .map(|(entity, _, _)| *entity)
        .expect("the area");
    world
        .ecs_mut()
        .entity_mut(entity)
        .insert(KeepoutKinds::of_kinds(&[KeepoutKind::Pour]));
}

#[test]
fn an_area_at_a_slant_takes_its_triangle_out_and_the_pour_comes_back_as_convex_pieces() {
    let mut world = BoardWorld::new();
    let zone = pour_zone(&mut world);
    triangle_area(&mut world);
    let filled = fill(&mut world, &zone);
    assert!(
        filled.pieces.is_empty(),
        "the rectangles are cut into convex pieces"
    );
    assert_eq!(area_mm2(&filled), 400.0 - 50.0);
    // The hypotenuse runs from (15, 5) to (5, 15), and a corner of the copper
    // beside it stands on that line at (15, 5).
    let at = |x: f64, y: f64| filled.shapes.iter().flatten().any(|c| *c == mm(x, y));
    assert!(at(15.0, 5.0), "{:?}", filled.shapes);
    assert!(at(5.0, 15.0), "{:?}", filled.shapes);
}

#[test]
fn a_pour_drawn_as_a_polygon_is_cut_the_same_way() {
    let mut world = BoardWorld::new();
    world.set_board("b".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
    let gnd = world.intern_net("GND");
    let ring = ZoneOutline::new(vec![
        mm(0.0, 0.0),
        mm(20.0, 0.0),
        mm(20.0, 20.0),
        mm(0.0, 20.0),
    ])
    .expect("a square");
    let zone = Zone::copper_pour_for_net(ring.bounds(), 0b01, gnd);
    world.spawn_outlined_zone(zone.clone(), ring.clone());
    triangle_area(&mut world);
    let options = PourOptions {
        clearance: Nm::ZERO,
        ..PourOptions::default()
    };
    let filled = fill_outlined_zone(
        &mut world,
        &FootprintLibrary::new(),
        Layer::TopCopper,
        &zone,
        &ring,
        &options,
    );
    assert_eq!(area_mm2(&filled), 400.0 - 50.0);
}
