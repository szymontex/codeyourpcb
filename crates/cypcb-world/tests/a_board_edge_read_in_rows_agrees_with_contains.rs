//! A board edge read a row at a time says what `contains` says point by point.
//!
//! `cargo test -p cypcb-world --test a_board_edge_read_in_rows_agrees_with_contains`
//!
//! The router asks about every cell of a grid, a million questions of a ring
//! of hundreds of edges. `spans_at` answers a whole row from one pass over the
//! edges, and has to agree with `contains` on every point or the two readers
//! of one edge disagree about where the board ends.

use cypcb_core::{Nm, Point};
use cypcb_world::components::BoardEdge;
use cypcb_world::BoardWorld;

fn at(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
    vec![at(x0, y0), at(x1, y0), at(x1, y1), at(x0, y1)]
}

fn in_spans(spans: &[(f64, f64)], x: i64) -> bool {
    spans
        .iter()
        .any(|&(from, to)| (x as f64) >= from && (x as f64) < to)
}

/// Every point of a 0.37 mm lattice over the box, through both readers.
fn assert_rows_agree(edge: &BoardEdge, width_mm: f64, height_mm: f64) {
    let step = 370_000;
    let mut inside = 0;
    let mut checked = 0;
    let mut y = -2_000_000;
    while y <= (height_mm * 1e6) as i64 + 2_000_000 {
        let spans = edge.spans_at(y);
        let mut x = -2_000_000;
        while x <= (width_mm * 1e6) as i64 + 2_000_000 {
            let by_point = edge.contains(Point::new(Nm(x), Nm(y)));
            assert_eq!(by_point, in_spans(&spans, x), "at x={x} y={y}");
            inside += by_point as u32;
            checked += 1;
            x += step;
        }
        y += step;
    }
    assert!(
        inside > 0 && inside < checked,
        "{inside} of {checked} inside"
    );
}

#[test]
fn a_rectangle_is_one_span_per_row() {
    let edge = BoardEdge::new(square(0.0, 0.0, 40.0, 30.0), vec![]);
    assert_eq!(edge.spans_at(15_000_000), vec![(0.0, 40_000_000.0)]);
    assert!(edge.spans_at(31_000_000).is_empty(), "above the board");
    assert_rows_agree(&edge, 40.0, 30.0);
}

#[test]
fn a_cutout_splits_the_row_it_crosses() {
    let edge = BoardEdge::new(
        square(0.0, 0.0, 40.0, 30.0),
        vec![square(15.0, 10.0, 25.0, 20.0)],
    );
    assert_eq!(
        edge.spans_at(15_000_000),
        vec![(0.0, 15_000_000.0), (25_000_000.0, 40_000_000.0)]
    );
    assert_eq!(edge.spans_at(5_000_000), vec![(0.0, 40_000_000.0)]);
    assert_rows_agree(&edge, 40.0, 30.0);
}

#[test]
fn a_notched_ring_and_two_cutouts_agree_with_contains() {
    // An L with a triangle and a square cut out of its long arm.
    let edge = BoardEdge::new(
        vec![
            at(0.0, 0.0),
            at(40.0, 0.0),
            at(40.0, 12.0),
            at(14.0, 12.0),
            at(14.0, 30.0),
            at(0.0, 30.0),
        ],
        vec![
            square(3.0, 3.0, 8.0, 8.0),
            vec![at(20.0, 2.0), at(30.0, 2.0), at(25.0, 9.0)],
        ],
    );
    assert_rows_agree(&edge, 40.0, 30.0);
}

#[test]
fn a_board_with_no_ring_has_the_rectangle_of_its_size_for_an_edge() {
    let mut world = BoardWorld::new();
    world.set_board("b".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
    assert_eq!(BoardEdge::of(&world), None);
    let edge = BoardEdge::of_or_size(&world).expect("the size is an edge");
    assert_eq!(edge.outer(), square(0.0, 0.0, 40.0, 30.0).as_slice());
    assert!(edge.cutouts().is_empty());
}

#[test]
fn a_board_of_no_size_has_no_edge() {
    let mut world = BoardWorld::new();
    world.set_board("b".to_string(), (Nm::from_mm(0.0), Nm::from_mm(0.0)), 2);
    assert_eq!(BoardEdge::of_or_size(&world), None);
}
