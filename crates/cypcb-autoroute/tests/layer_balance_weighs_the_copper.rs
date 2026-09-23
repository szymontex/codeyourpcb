//! `layer_balance` weighs the copper on a layer, not the nets on it.
//!
//! `cargo test -p cypcb-autoroute --test layer_balance_weighs_the_copper`
//!
//! `apply_routes_as` emits one `Trace` per net and layer. `compute_layer_balance`
//! counted those entities until 2026-09-23, so the ratio read how many nets
//! reached a layer and never how much copper did: ten times the copper on one
//! side scored 1.0, and three short nets outweighed one long one. Both cases
//! are built in the shape the router emits, and both now read lengths.

use cypcb_autoroute::scoring::score_board;
use cypcb_core::{Nm, Point};
use cypcb_drc::presets::DesignRules;
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::Layer;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

/// One `Trace` per (net, layer), the only shape `apply_routes_as` emits.
/// Each entry names the layer, the net and the copper length in mm.
fn board(spread: &[(Layer, &str, f64)]) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(60.0), Nm::from_mm(60.0)), 2);
    let mut y = 2.0;
    for &(layer, net_name, length_mm) in spread {
        let net = world.intern_net(net_name);
        world.spawn_entity((
            Trace {
                segments: vec![TraceSegment::new(
                    Point::from_mm(2.0, y),
                    Point::from_mm(2.0 + length_mm, y),
                )],
                width: Nm::from_mm(0.2),
                layer,
                net_id: net,
                locked: false,
                source: TraceSource::Autorouted,
            },
            net,
        ));
        y += 1.5;
    }
    world
}

fn balance_of(mut world: BoardWorld) -> f64 {
    let library = FootprintLibrary::new();
    world.rebuild_spatial_index_from_library(&library);
    score_board(
        &mut world,
        &DesignRules::jlcpcb_2layer(),
        &Default::default(),
    )
    .layer_balance
}

#[test]
fn one_net_a_side_reads_the_copper_not_the_count() {
    let balance = balance_of(board(&[
        (Layer::TopCopper, "A", 50.0),
        (Layer::BottomCopper, "B", 5.0),
    ]));
    assert!(
        (balance - 0.1).abs() < 1e-9,
        "5 mm against 50 mm is a tenth, got {balance}; 1.0 means one net a side was counted"
    );
}

#[test]
fn three_small_nets_do_not_outweigh_one_large_one() {
    let balance = balance_of(board(&[
        (Layer::TopCopper, "A", 2.0),
        (Layer::TopCopper, "B", 2.0),
        (Layer::TopCopper, "C", 2.0),
        (Layer::BottomCopper, "D", 50.0),
    ]));
    assert!(
        (balance - 0.12).abs() < 1e-9,
        "6 mm against 50 mm is 0.12, got {balance}; a third means the nets were counted"
    );
}
