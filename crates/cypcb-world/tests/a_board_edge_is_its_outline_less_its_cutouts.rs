//! The edge of a board is its outline less its cutouts, in one place.
//!
//! `cargo test -p cypcb-world --test a_board_edge_is_its_outline_less_its_cutouts`
//!
//! `BoardOutline` is the rim and `BoardCutouts` the windows. Every reader that
//! asks "is this on the board" or "how far is the edge" goes through
//! `BoardEdge`, so none of them learns the cutouts on its own.

use cypcb_core::{Nm, Point};
use cypcb_world::components::{BoardCutouts, BoardEdge, BoardOutline};
use cypcb_world::BoardWorld;

fn at(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
    vec![at(x0, y0), at(x1, y0), at(x1, y1), at(x0, y1)]
}

fn board() -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("b".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
    world
}

#[test]
fn a_board_with_neither_outline_nor_cutout_has_no_edge_of_its_own() {
    // The rectangle of the size stands, and every reader keeps its old path.
    assert_eq!(BoardEdge::of(&board()), None);
}

#[test]
fn an_outline_alone_is_the_edge_with_no_cutouts() {
    let mut world = board();
    let entity = world.board_entity().unwrap();
    let ring = square(0.0, 0.0, 40.0, 30.0);
    world
        .ecs_mut()
        .entity_mut(entity)
        .insert(BoardOutline::new(ring.clone()).unwrap());
    let edge = BoardEdge::of(&world).expect("an edge");
    assert_eq!(edge.outer(), ring.as_slice());
    assert!(edge.cutouts().is_empty());
    assert_eq!(edge.edges().count(), 4);
}

#[test]
fn cutouts_without_an_outline_sit_in_the_rectangle_of_the_size() {
    let mut world = board();
    let entity = world.board_entity().unwrap();
    world
        .ecs_mut()
        .entity_mut(entity)
        .insert(BoardCutouts::new(vec![square(15.0, 10.0, 25.0, 20.0)]).unwrap());
    let edge = BoardEdge::of(&world).expect("an edge");
    assert_eq!(edge.outer(), square(0.0, 0.0, 40.0, 30.0).as_slice());
    assert_eq!(edge.cutouts().len(), 1);
}

#[test]
fn on_the_board_is_inside_the_ring_and_outside_every_cutout() {
    let mut world = board();
    let entity = world.board_entity().unwrap();
    world
        .ecs_mut()
        .entity_mut(entity)
        .insert(BoardOutline::new(square(0.0, 0.0, 40.0, 30.0)).unwrap());
    world.ecs_mut().entity_mut(entity).insert(
        BoardCutouts::new(vec![
            square(5.0, 5.0, 10.0, 10.0),
            square(15.0, 10.0, 25.0, 20.0),
        ])
        .unwrap(),
    );
    let edge = BoardEdge::of(&world).unwrap();

    assert!(edge.contains(at(2.0, 2.0)), "on the board");
    assert!(edge.contains(at(12.0, 15.0)), "between the windows");
    assert!(!edge.contains(at(7.0, 7.0)), "in the first window");
    assert!(!edge.contains(at(20.0, 15.0)), "in the second window");
    assert!(!edge.contains(at(50.0, 15.0)), "off the board");
}

#[test]
fn the_edge_is_the_rim_and_the_rim_of_every_cutout() {
    let edge = BoardEdge::new(
        square(0.0, 0.0, 40.0, 30.0),
        vec![
            square(5.0, 5.0, 10.0, 10.0),
            vec![at(20.0, 5.0), at(25.0, 5.0), at(22.0, 9.0)],
        ],
    );
    assert_eq!(edge.rings().count(), 3);
    assert_eq!(edge.edges().count(), 4 + 4 + 3);
    // Each ring is closed: the last edge of a ring returns to its first point.
    let last = edge.edges().nth(10).unwrap();
    assert_eq!(last, (at(22.0, 9.0), at(20.0, 5.0)));
}
