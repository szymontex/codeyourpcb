//! A pour is held to the board: inside the edge, outside every cutout, and the
//! fab's distance from both.
//!
//! `cargo test -p cypcb-world --test a_pour_is_held_to_the_board`
//!
//! `fill_zone` cut a pour from the rectangle it was drawn as and from the
//! copper on its layer, and read neither the board's outline nor its cutouts. A
//! pour drawn past the edge left copper off the board and a pour round a window
//! left copper in it. KiCad clips the fill to the board outline with its holes
//! (`zone_filler.cpp:409-410`, `zone.cpp:1558-1564`) and knocks every Edge.Cuts
//! shape - the outline and each cutout - out of it by the edge clearance
//! (`zone_filler.cpp:2172-2180`).
//!
//! Every board here is built by hand, so the numbers are worked out from the
//! numbers above them.

use cypcb_core::pour::PourOptions;
use cypcb_core::pour_shape::{comes_within, convex_contains};
use cypcb_core::{Nm, Point, Rect};
use cypcb_world::components::zone::{Zone, ZoneKind, ZoneOutline};
use cypcb_world::components::{BoardCutouts, BoardOutline, Layer};
use cypcb_world::copper::{fill_outlined_zone, fill_zone, FilledPour};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

fn at(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

fn mm(v: f64) -> Nm {
    Nm::from_mm(v)
}

/// The gap every test asks for, one the defaults do not use.
const GAP: f64 = 0.25;

fn options() -> PourOptions {
    PourOptions {
        edge_clearance: mm(GAP),
        ..PourOptions::default()
    }
}

fn board(width: f64, height: f64) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("b".to_string(), (mm(width), mm(height)), 2);
    world
}

fn ring(world: &mut BoardWorld, points: &[(f64, f64)]) {
    let entity = world.board_entity().unwrap();
    let ring = BoardOutline::new(points.iter().map(|&(x, y)| at(x, y)).collect()).unwrap();
    world.ecs_mut().entity_mut(entity).insert(ring);
}

fn window(world: &mut BoardWorld, points: &[(f64, f64)]) {
    let entity = world.board_entity().unwrap();
    let cutouts = BoardCutouts::new(vec![points.iter().map(|&(x, y)| at(x, y)).collect()]).unwrap();
    world.ecs_mut().entity_mut(entity).insert(cutouts);
}

/// A pour with no net and nothing else on the board to cut it.
fn fill(world: &mut BoardWorld, from: (f64, f64), to: (f64, f64)) -> FilledPour {
    let zone = Zone {
        bounds: Rect::new(at(from.0, from.1), at(to.0, to.1)),
        kind: ZoneKind::CopperPour,
        layer_mask: Layer::TopCopper.to_copper_mask(),
        name: Some("p".to_string()),
        net: None,
    };
    fill_zone(
        world,
        &FootprintLibrary::new(),
        Layer::TopCopper,
        &zone,
        None,
        &options(),
    )
}

/// Every piece of the fill as a ring, rectangles and convex pieces alike.
fn rings(filled: &FilledPour) -> Vec<Vec<Point>> {
    filled
        .pieces
        .iter()
        .map(|r| {
            vec![
                r.min,
                Point::new(r.max.x, r.min.y),
                r.max,
                Point::new(r.min.x, r.max.y),
            ]
        })
        .chain(filled.shapes.iter().cloned())
        .collect()
}

fn covered(filled: &FilledPour, x: f64, y: f64) -> bool {
    rings(filled).iter().any(|r| convex_contains(r, at(x, y)))
}

/// The nearest copper may come to an edge is the gap, less the nanometre or so
/// that cutting a slanted edge to whole nanometres can cost.
fn nowhere_nearer_than_the_gap(world: &BoardWorld, filled: &FilledPour) {
    let edge = cypcb_world::components::BoardEdge::of(world).expect("an edge");
    let allowed = Nm(mm(GAP).0 - 2);
    for piece in rings(filled) {
        for (a, b) in edge.edges() {
            assert!(
                !comes_within(&piece, a, b, allowed),
                "copper {piece:?} is nearer than the gap to {a:?}-{b:?}"
            );
        }
    }
}

#[test]
fn a_pour_wider_than_the_board_is_cut_to_the_gap_inside_it() {
    let mut world = board(40.0, 30.0);
    let filled = fill(&mut world, (-10.0, -10.0), (50.0, 40.0));
    assert_eq!(
        filled.pieces,
        vec![Rect::new(at(GAP, GAP), at(40.0 - GAP, 30.0 - GAP))]
    );
    assert!(filled.shapes.is_empty());
}

#[test]
fn a_pour_that_stays_clear_of_the_edge_is_not_touched() {
    let mut world = board(40.0, 30.0);
    let filled = fill(&mut world, (5.0, 5.0), (35.0, 25.0));
    assert_eq!(filled.pieces, vec![Rect::new(at(5.0, 5.0), at(35.0, 25.0))]);
    assert!(filled.shapes.is_empty());
}

#[test]
fn copper_exactly_the_gap_from_the_edge_is_clear_of_it() {
    let mut world = board(40.0, 30.0);
    let filled = fill(&mut world, (GAP, GAP), (40.0 - GAP, 30.0 - GAP));
    assert_eq!(
        filled.pieces,
        vec![Rect::new(at(GAP, GAP), at(40.0 - GAP, 30.0 - GAP))]
    );
}

#[test]
fn a_rectangular_outline_is_the_same_board_as_its_size() {
    let mut plain = board(40.0, 30.0);
    let mut outlined = board(40.0, 30.0);
    ring(
        &mut outlined,
        &[(0.0, 0.0), (40.0, 0.0), (40.0, 30.0), (0.0, 30.0)],
    );
    let a = fill(&mut plain, (-10.0, -10.0), (50.0, 40.0));
    let b = fill(&mut outlined, (-10.0, -10.0), (50.0, 40.0));
    assert_eq!(a.pieces, b.pieces);
    assert_eq!(a.shapes, b.shapes);
}

#[test]
fn a_board_with_no_size_is_not_cut() {
    // A file whose outline never closed is a board of 0 x 0: there is no edge
    // to hold a pour to, and cutting to nothing would erase every pour on it.
    let mut world = board(0.0, 0.0);
    let filled = fill(&mut world, (5.0, 5.0), (35.0, 25.0));
    assert_eq!(filled.pieces, vec![Rect::new(at(5.0, 5.0), at(35.0, 25.0))]);
}

#[test]
fn a_pour_is_held_the_gap_from_a_slanted_edge() {
    // 20 x 20mm with the corner at the origin cut off along x + y = 5.
    let mut world = board(20.0, 20.0);
    ring(
        &mut world,
        &[
            (5.0, 0.0),
            (20.0, 0.0),
            (20.0, 20.0),
            (0.0, 20.0),
            (0.0, 5.0),
        ],
    );
    let filled = fill(&mut world, (-5.0, -5.0), (25.0, 25.0));
    nowhere_nearer_than_the_gap(&world, &filled);

    // The corner is gone: (1, 1) is off the board.
    assert!(!covered(&filled, 1.0, 1.0));
    // The chamfer's middle is (2.5, 2.5), its inward normal (1, 1) / sqrt 2.
    // Copper is there a little past the gap and not a little inside it.
    let step = |distance: f64| 2.5 + distance * std::f64::consts::FRAC_1_SQRT_2;
    assert!(covered(&filled, step(GAP + 0.05), step(GAP + 0.05)));
    assert!(!covered(&filled, step(GAP - 0.05), step(GAP - 0.05)));
}

#[test]
fn the_gap_is_round_a_corner_the_board_turns_away_from() {
    // An L, 40 x 30 with the top right quarter (x > 20, y > 15) gone. The
    // notch's corner (20, 15) is a corner the board turns away from: the band
    // along each of its two sides stops short of the diagonal.
    let mut world = board(40.0, 30.0);
    ring(
        &mut world,
        &[
            (0.0, 0.0),
            (40.0, 0.0),
            (40.0, 15.0),
            (20.0, 15.0),
            (20.0, 30.0),
            (0.0, 30.0),
        ],
    );
    let filled = fill(&mut world, (-5.0, -5.0), (45.0, 35.0));
    nowhere_nearer_than_the_gap(&world, &filled);

    let diagonal = std::f64::consts::FRAC_1_SQRT_2;
    // 0.20mm from the corner along the diagonal into the board: inside the
    // gap, outside both bands. 0.30mm: outside the gap.
    assert!(!covered(
        &filled,
        20.0 - 0.20 * diagonal,
        15.0 - 0.20 * diagonal
    ));
    assert!(covered(
        &filled,
        20.0 - 0.30 * diagonal,
        15.0 - 0.30 * diagonal
    ));
}

#[test]
fn a_cutout_is_a_hole_in_the_pour_with_the_gap_round_it() {
    let mut world = board(40.0, 30.0);
    window(
        &mut world,
        &[(15.0, 10.0), (25.0, 10.0), (25.0, 20.0), (15.0, 20.0)],
    );
    let filled = fill(&mut world, (-10.0, -10.0), (50.0, 40.0));
    nowhere_nearer_than_the_gap(&world, &filled);

    assert!(!covered(&filled, 20.0, 15.0), "copper in the window");
    assert!(
        !covered(&filled, 14.9, 15.0),
        "copper inside the gap of the window"
    );
    assert!(
        covered(&filled, 14.65, 15.0),
        "no copper just outside the gap"
    );
    assert!(
        covered(&filled, 2.0, 2.0),
        "the rest of the board has its pour"
    );
    // And the pour is still cut at the rim.
    assert!(!covered(&filled, -1.0, 15.0));
}

#[test]
fn a_pour_wholly_inside_a_cutout_is_gone() {
    let mut world = board(40.0, 30.0);
    window(
        &mut world,
        &[(15.0, 10.0), (25.0, 10.0), (25.0, 20.0), (15.0, 20.0)],
    );
    let filled = fill(&mut world, (17.0, 12.0), (23.0, 18.0));
    assert!(filled.pieces.is_empty() && filled.shapes.is_empty());
}

#[test]
fn a_board_with_a_cutout_and_no_outline_has_the_rectangle_of_its_size_for_a_rim() {
    // A rectangle drawn as one shape has no outline ring; the cutout inside it
    // still cuts, and the size is still the rim.
    let mut world = board(40.0, 30.0);
    window(
        &mut world,
        &[(15.0, 10.0), (25.0, 10.0), (25.0, 20.0), (15.0, 20.0)],
    );
    let filled = fill(&mut world, (-10.0, -10.0), (50.0, 40.0));
    assert!(!covered(&filled, 20.0, 15.0));
    assert!(
        !covered(&filled, 0.1, 15.0),
        "copper inside the gap of the rim"
    );
    assert!(covered(&filled, 0.35, 15.0));
}

#[test]
fn the_gap_is_the_one_the_options_state() {
    let mut world = board(40.0, 30.0);
    let wide = PourOptions {
        edge_clearance: mm(0.5),
        ..PourOptions::default()
    };
    let zone = Zone {
        bounds: Rect::new(at(-1.0, -1.0), at(41.0, 31.0)),
        kind: ZoneKind::CopperPour,
        layer_mask: Layer::TopCopper.to_copper_mask(),
        name: None,
        net: None,
    };
    let filled = fill_zone(
        &mut world,
        &FootprintLibrary::new(),
        Layer::TopCopper,
        &zone,
        None,
        &wide,
    );
    assert_eq!(filled.pieces, vec![Rect::new(at(0.5, 0.5), at(39.5, 29.5))]);
}

/// A pour drawn as a polygon, much larger than the board, filled as the
/// exporter fills it.
fn fill_polygon(world: &mut BoardWorld) -> FilledPour {
    let zone = Zone {
        bounds: Rect::default(),
        kind: ZoneKind::CopperPour,
        layer_mask: Layer::TopCopper.to_copper_mask(),
        name: Some("p".to_string()),
        net: None,
    };
    let outline = ZoneOutline::new(vec![
        at(-10.0, -10.0),
        at(50.0, -10.0),
        at(50.0, 40.0),
        at(-10.0, 40.0),
    ])
    .expect("an outline");
    fill_outlined_zone(
        world,
        &FootprintLibrary::new(),
        Layer::TopCopper,
        &zone,
        &outline,
        &options(),
    )
}

#[test]
fn a_pour_drawn_as_a_polygon_is_held_to_a_rectangular_board() {
    let mut world = board(40.0, 30.0);
    let filled = fill_polygon(&mut world);
    assert!(!covered(&filled, -1.0, 15.0), "copper off the board");
    assert!(!covered(&filled, 0.1, 15.0), "copper inside the gap");
    assert!(covered(&filled, 0.4, 15.0));
    assert!(!covered(&filled, 41.0, 15.0));
}

#[test]
fn a_pour_drawn_as_a_polygon_is_held_to_a_board_with_a_cutout() {
    let mut world = board(40.0, 30.0);
    window(
        &mut world,
        &[(15.0, 10.0), (25.0, 10.0), (25.0, 20.0), (15.0, 20.0)],
    );
    let filled = fill_polygon(&mut world);
    nowhere_nearer_than_the_gap(&world, &filled);
    assert!(!covered(&filled, 20.0, 15.0), "copper in the window");
    assert!(!covered(&filled, -1.0, 15.0), "copper off the board");
    assert!(covered(&filled, 14.65, 15.0));
}

#[test]
fn the_polygon_round_a_corner_may_stand_this_far_off_the_circle() {
    // KiCad's `ARC_HIGH_DEF_MM`, 0.005mm: the error it allows an arc turned
    // into segments, which is what the disc round a corner is held to.
    assert_eq!(cypcb_core::pour_shape::ARC_ERROR, mm(0.005));
}
