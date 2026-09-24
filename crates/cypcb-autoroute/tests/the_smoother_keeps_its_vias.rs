//! Does a via still sit on its copper after the smoother has moved the copper?
//!
//! `cargo test -p cypcb-autoroute --test the_smoother_keeps_its_vias`
//!
//! The smoother never moves the first or the last point of a run of connected
//! segments, and a path starts and ends on a pad, so a pad keeps its copper
//! without the smoother knowing pads exist. A via is not always such an end.
//! Two boards showed what happens when it is not:
//!
//! - `multi_ic` with `stop_at_own_copper`, USB_DM: a path came down to a via
//!   on Inner(1) and turned there, the corner was chamfered, and the via was
//!   left 20.7 µm off the copper with the net in two pieces.
//! - `multi_ic` with `stop_at_own_copper`, VCC_3V3: a Top segment ran straight
//!   across a via without ending on it, the smoother moved that segment, and
//!   the via was left 150 µm off the copper, again with the net in two pieces.
//!
//! Both are rebuilt here from the same shapes, and the via has to lie on its
//! copper on both of its layers afterwards.

use cypcb_autoroute::smoother::smooth_routes;
use cypcb_core::{Nm, Point};
use cypcb_router::types::RouteSegment;
use cypcb_world::{Layer, NetId};

/// The router's default `roundness`.
const ROUNDNESS: f64 = 0.5;

fn segment(layer: Layer, from: (f64, f64), to: (f64, f64)) -> RouteSegment {
    RouteSegment::new(
        NetId::new(1),
        layer,
        Nm::from_mm(0.1),
        Point::from_mm(from.0, from.1),
        Point::from_mm(to.0, to.1),
    )
}

/// Distance in nanometres from `point` to the nearest segment on `layer`.
fn distance_on(layer: Layer, point: Point, segments: &[RouteSegment]) -> f64 {
    let p = [point.x.0 as f64, point.y.0 as f64];
    segments
        .iter()
        .filter(|s| s.layer == layer)
        .map(|s| {
            let (a, b) = (
                [s.start.x.0 as f64, s.start.y.0 as f64],
                [s.end.x.0 as f64, s.end.y.0 as f64],
            );
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let length_sq = dx * dx + dy * dy;
            let t = if length_sq == 0.0 {
                0.0
            } else {
                (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / length_sq).clamp(0.0, 1.0)
            };
            (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
        })
        .fold(f64::INFINITY, f64::min)
}

fn assert_on_both_layers(via: Point, layers: [Layer; 2], smoothed: &[RouteSegment]) {
    for layer in layers {
        let distance = distance_on(layer, via, smoothed);
        assert!(
            distance < 1.0,
            "the via at {via:?} is {distance:.0} nm from the copper on {layer:?} after smoothing:\n{smoothed:#?}"
        );
    }
}

#[test]
fn a_corner_on_a_via_is_not_cut_off() {
    let via = Point::from_mm(23.8, 57.0);
    let segments = [
        // Down to the via on Inner(1) and away along it: a 90 degree corner
        // the smoother chamfers by 0.5 mm when nothing holds it.
        segment(Layer::Inner(1), (23.8, 53.0), (23.8, 57.0)),
        segment(Layer::Inner(1), (23.8, 57.0), (20.3, 57.0)),
        // And on up through the via to Top.
        segment(Layer::TopCopper, (23.8, 57.0), (26.0, 57.0)),
    ];

    let smoothed = smooth_routes(&segments, &[], &[via], Nm(0), ROUNDNESS);

    assert_on_both_layers(via, [Layer::Inner(1), Layer::TopCopper], &smoothed);
}

#[test]
fn a_line_across_a_via_is_not_moved_off_it() {
    let via = Point::from_mm(12.4, 0.0);
    let segments = [
        // A staircase on Top whose first step runs straight over the via. The
        // smoother collapses it into a diagonal from (0, 0) when nothing
        // holds the via's point.
        segment(Layer::TopCopper, (0.0, 0.0), (14.0, 0.0)),
        segment(Layer::TopCopper, (14.0, 0.0), (14.0, 1.0)),
        segment(Layer::TopCopper, (14.0, 1.0), (15.0, 1.0)),
        // The via goes down to Inner(0) and leaves there.
        segment(Layer::Inner(0), (12.4, 0.0), (12.4, -3.0)),
    ];

    let smoothed = smooth_routes(&segments, &[], &[via], Nm(0), ROUNDNESS);

    assert_on_both_layers(via, [Layer::TopCopper, Layer::Inner(0)], &smoothed);
}
