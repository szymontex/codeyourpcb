//! `layer_balance` counts nets on a layer, not copper on it.
//!
//! `cargo test -p cypcb-autoroute --test layer_balance_counts_nets_not_copper`
//!
//! `apply_routes_as` emits one `Trace` per net and layer, and
//! `compute_layer_balance` counts entities. So the ratio reads how many nets
//! reached a layer, never how much copper did.
//!
//! `layer_balance_means_what_it_says` pins the same function against several
//! entities on one net per layer, which is a shape the router never emits.
//! These two cases use the production shape instead.

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
fn one_net_a_side_reads_balanced_however_the_copper_falls() {
    let balance = balance_of(board(&[
        (Layer::TopCopper, "A", 50.0),
        (Layer::BottomCopper, "B", 5.0),
    ]));
    assert_eq!(
        balance, 1.0,
        "ten times the copper on top scored {balance}, so the ratio never read a length"
    );
}

#[test]
fn three_small_nets_outweigh_one_large_one() {
    let balance = balance_of(board(&[
        (Layer::TopCopper, "A", 2.0),
        (Layer::TopCopper, "B", 2.0),
        (Layer::TopCopper, "C", 2.0),
        (Layer::BottomCopper, "D", 50.0),
    ]));
    assert!(
        (balance - 1.0 / 3.0).abs() < 1e-9,
        "the layer holding 6 mm of 56 mm scored {balance} against the layer holding the rest"
    );
}
