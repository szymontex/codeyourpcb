//! Two vias the router placed on one spot for one net are one hole.
//!
//! Two paths of a net that changed layer in the same cell each brought their
//! own via, and the file asked for two holes drilled into one another. They
//! are one via now when that joins nothing the two did not join: never across
//! a gap between their spans, and never for two nets.

use cypcb_autoroute::via_optimizer::{optimize_vias, BoardObstacles};
use cypcb_core::{Nm, Point};
use cypcb_router::types::ViaPlacement;
use cypcb_world::{Layer, NetId};

const TOP: Layer = Layer::TopCopper;
const IN1: Layer = Layer::Inner(0);
const IN2: Layer = Layer::Inner(1);
const BOTTOM: Layer = Layer::BottomCopper;

fn via(net: u32, at: (f64, f64), from: Layer, to: Layer) -> ViaPlacement {
    ViaPlacement::new(
        NetId::new(net),
        Point::from_mm(at.0, at.1),
        Nm::from_mm(0.3),
        from,
        to,
    )
}

fn optimize(vias: Vec<ViaPlacement>) -> Vec<ViaPlacement> {
    optimize_vias(
        Vec::new(),
        vias,
        &BoardObstacles::default(),
        Nm::from_mm(0.15),
        Nm::from_mm(0.5),
    )
    .1
}

/// Each via as (net, x, y, top of its span, bottom of its span), sorted.
fn holes(vias: &[ViaPlacement]) -> Vec<(u32, i64, i64, Layer, Layer)> {
    let depth = |l: Layer| match l {
        Layer::TopCopper => 0,
        Layer::Inner(n) => n as u16 + 1,
        _ => u16::MAX,
    };
    let mut out: Vec<_> = vias
        .iter()
        .map(|v| {
            let (a, b) = if depth(v.start_layer) <= depth(v.end_layer) {
                (v.start_layer, v.end_layer)
            } else {
                (v.end_layer, v.start_layer)
            };
            (v.net_id.id(), v.position.x.0, v.position.y.0, a, b)
        })
        .collect();
    out.sort_by_key(|h| (h.0, h.1, h.2, depth(h.3), depth(h.4)));
    out
}

fn at(x: f64, y: f64) -> (i64, i64) {
    let p = Point::from_mm(x, y);
    (p.x.0, p.y.0)
}

#[test]
fn two_through_vias_of_one_net_on_one_spot_are_one_via() {
    let merged = optimize(vec![
        via(1, (10.0, 10.0), TOP, BOTTOM),
        via(1, (10.0, 10.0), BOTTOM, TOP),
    ]);
    let (x, y) = at(10.0, 10.0);
    assert_eq!(holes(&merged), vec![(1, x, y, TOP, BOTTOM)]);
}

#[test]
fn spans_that_end_on_one_layer_become_one_via_across_both() {
    let merged = optimize(vec![
        via(1, (10.0, 10.0), TOP, IN1),
        via(1, (10.0, 10.0), IN1, BOTTOM),
    ]);
    let (x, y) = at(10.0, 10.0);
    assert_eq!(holes(&merged), vec![(1, x, y, TOP, BOTTOM)]);
}

#[test]
fn spans_that_overlap_become_one_via_across_both() {
    let merged = optimize(vec![
        via(1, (10.0, 10.0), IN1, BOTTOM),
        via(1, (10.0, 10.0), TOP, IN2),
    ]);
    let (x, y) = at(10.0, 10.0);
    assert_eq!(holes(&merged), vec![(1, x, y, TOP, BOTTOM)]);
}

#[test]
fn a_via_that_bridges_two_others_joins_all_three() {
    // Top to In1 and In2 to bottom have a gap between them; the third via
    // closes it, and the first merge must not stop the second.
    let merged = optimize(vec![
        via(1, (10.0, 10.0), TOP, IN1),
        via(1, (10.0, 10.0), IN2, BOTTOM),
        via(1, (10.0, 10.0), IN1, IN2),
    ]);
    let (x, y) = at(10.0, 10.0);
    assert_eq!(holes(&merged), vec![(1, x, y, TOP, BOTTOM)]);
}

#[test]
fn spans_with_a_gap_between_them_stay_two_vias() {
    // One hole from top to bottom would join In1 to In2 here, which neither
    // via did.
    let merged = optimize(vec![
        via(1, (10.0, 10.0), TOP, IN1),
        via(1, (10.0, 10.0), IN2, BOTTOM),
    ]);
    let (x, y) = at(10.0, 10.0);
    assert_eq!(
        holes(&merged),
        vec![(1, x, y, TOP, IN1), (1, x, y, IN2, BOTTOM)]
    );
}

#[test]
fn vias_of_two_nets_on_one_spot_are_never_one_via() {
    let merged = optimize(vec![
        via(1, (10.0, 10.0), TOP, BOTTOM),
        via(2, (10.0, 10.0), TOP, BOTTOM),
    ]);
    let (x, y) = at(10.0, 10.0);
    assert_eq!(
        holes(&merged),
        vec![(1, x, y, TOP, BOTTOM), (2, x, y, TOP, BOTTOM)]
    );
}

#[test]
fn vias_of_one_net_a_cell_apart_stay_two_vias() {
    let merged = optimize(vec![
        via(1, (10.0, 10.0), TOP, BOTTOM),
        via(1, (10.254, 10.0), TOP, BOTTOM),
    ]);
    let (x1, y1) = at(10.0, 10.0);
    let (x2, y2) = at(10.254, 10.0);
    assert_eq!(
        holes(&merged),
        vec![(1, x1, y1, TOP, BOTTOM), (1, x2, y2, TOP, BOTTOM)]
    );
}

#[test]
fn vias_with_different_drills_stay_two_vias() {
    let mut wide = via(1, (10.0, 10.0), TOP, BOTTOM);
    wide.drill = Nm::from_mm(0.4);
    wide.outer_diameter = Nm::from_mm(0.8);
    let merged = optimize(vec![via(1, (10.0, 10.0), TOP, BOTTOM), wide]);
    assert_eq!(merged.len(), 2);
}
