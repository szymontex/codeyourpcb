//! Smoothness looked for bends where there were never any.
//!
//! `cargo test -p cypcb-autoroute --test smoothness_measures_the_corners`
//!
//! `compute_smoothness` walked `trace.segments.windows(2)` and skipped any
//! trace with fewer than two segments. `apply_routes` builds one entity per
//! route segment - `segments: vec![TraceSegment::new(..)]` in all four places
//! it does so - so every trace had exactly one segment, every trace was
//! skipped, and the function returned its `total_bends == 0` default of 1.0.
//! Every board ever scored read as perfectly smooth without a single corner
//! being examined.
//!
//! It still reads 1.0 on boards the in-house router produces, because that
//! router works on a 45-degree grid and a 45-degree turn costs nothing. The
//! difference is that it now says so after looking. These tests are the proof:
//! a board with an off-grid corner scores below 1.0, and one built the same
//! way with a 45-degree corner scores exactly 1.0.

use cypcb_autoroute::scoring::score_board;
use cypcb_core::{Nm, Point};
use cypcb_drc::presets::DesignRules;
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::Layer;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

/// Two segments meeting at (10, 10), the second leaving at `angle` degrees
/// from straight-on.
fn board_with_a_corner(degrees: f64) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(40.0)), 2);
    let net = world.intern_net("SIG");

    let joint = Point::from_mm(10.0, 10.0);
    let radians = degrees.to_radians();
    let far = Point::from_mm(10.0 + 8.0 * radians.cos(), 10.0 + 8.0 * radians.sin());

    for (start, end) in [(Point::from_mm(2.0, 10.0), joint), (joint, far)] {
        world.spawn_entity((
            Trace {
                segments: vec![TraceSegment::new(start, end)],
                width: Nm::from_mm(0.2),
                layer: Layer::TopCopper,
                net_id: net,
                locked: false,
                source: TraceSource::Autorouted,
            },
            net,
        ));
    }
    world
}

fn smoothness_of(mut world: BoardWorld) -> f64 {
    let library = FootprintLibrary::new();
    world.rebuild_spatial_index_from_library(&library);
    score_board(
        &mut world,
        &DesignRules::jlcpcb_2layer(),
        &Default::default(),
    )
    .smoothness
}

#[test]
fn a_corner_off_the_grid_is_not_smooth() {
    // 20 degrees from straight is 25 degrees from the nearest 45, which is
    // past the 22.5 the penalty saturates at.
    let smoothness = smoothness_of(board_with_a_corner(20.0));
    assert!(
        smoothness < 1.0,
        "a 20-degree corner scored {smoothness}, which is what a board with no \
         corners at all scores"
    );
}

#[test]
fn a_corner_on_the_grid_costs_nothing() {
    // 45 degrees is what this router lays, and it is smooth by definition.
    let smoothness = smoothness_of(board_with_a_corner(45.0));
    assert!(
        (smoothness - 1.0).abs() < 1e-9,
        "a 45-degree corner scored {smoothness}"
    );
}

#[test]
fn straight_copper_through_a_joint_is_smooth() {
    let smoothness = smoothness_of(board_with_a_corner(0.0));
    assert!(
        (smoothness - 1.0).abs() < 1e-9,
        "copper running straight through a joint scored {smoothness}"
    );
}
