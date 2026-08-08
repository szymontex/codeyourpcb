//! Layer balance scored a one-layer route as perfectly balanced.
//!
//! `cargo test -p cypcb-autoroute --test layer_balance_means_what_it_says`
//!
//! `compute_layer_balance` counted only the layers that carried copper, so a
//! route that put everything on the top of a two-layer board had one entry in
//! its map, `min == max`, and returned 1.0 under the comment "single layer,
//! balanced by definition".
//!
//! On `led_blink` that was not hypothetical. The `Low-Via` variant lays zero
//! vias and scored `balance 1.000`, while every variant that actually spreads
//! across both layers scored 0.200 - so the term subtracted 40 points from
//! each balanced route and nothing from the one that used a single layer. It
//! rewarded the opposite of its name in the case where it varies most.
//!
//! Whether spreading is desirable at all is a different question, and one the
//! composite answers elsewhere: it charges per via. This metric's job is to
//! say what it is named for.

use cypcb_autoroute::scoring::score_board;
use cypcb_core::{Nm, Point};
use cypcb_drc::presets::DesignRules;
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::Layer;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

/// A board of `layers` copper layers carrying `per_layer` traces on each named
/// layer.
fn board(layers: u8, spread: &[(Layer, usize)]) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        "t".to_string(),
        (Nm::from_mm(40.0), Nm::from_mm(40.0)),
        layers,
    );
    let net = world.intern_net("SIG");

    let mut y = 2.0;
    for (layer, count) in spread {
        for _ in 0..*count {
            world.spawn_entity((
                Trace {
                    segments: vec![TraceSegment::new(
                        Point::from_mm(2.0, y),
                        Point::from_mm(30.0, y),
                    )],
                    width: Nm::from_mm(0.2),
                    layer: *layer,
                    net_id: net,
                    locked: false,
                    source: TraceSource::Autorouted,
                },
                net,
            ));
            y += 1.5;
        }
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
fn everything_on_one_layer_of_two_is_not_balanced() {
    let balance = balance_of(board(2, &[(Layer::TopCopper, 6)]));
    assert_eq!(
        balance, 0.0,
        "a route that never touched the bottom layer scored {balance}"
    );
}

#[test]
fn an_even_split_is_balanced() {
    let balance = balance_of(board(2, &[(Layer::TopCopper, 5), (Layer::BottomCopper, 5)]));
    assert_eq!(balance, 1.0, "an even split scored {balance}");
}

#[test]
fn an_uneven_split_is_the_ratio_of_the_two() {
    let balance = balance_of(board(2, &[(Layer::TopCopper, 8), (Layer::BottomCopper, 2)]));
    assert!(
        (balance - 0.25).abs() < 1e-9,
        "two traces against eight is a quarter, got {balance}"
    );
}

#[test]
fn two_layers_of_a_four_layer_board_are_not_balanced() {
    // The case the old code got most wrong: a board with four layers using
    // two of them read as perfectly balanced, because the two it used carried
    // the same count.
    let balance = balance_of(board(4, &[(Layer::TopCopper, 4), (Layer::BottomCopper, 4)]));
    assert_eq!(
        balance, 0.0,
        "half the stack unused scored {balance}, which is what using all of it \
         evenly scores"
    );
}
