//! What `crossings` counts, and whether the composite pays for it twice.
//!
//! `cargo test -p cypcb-autoroute --test a_crossing_is_charged_twice`
//!
//! `compute_composite` charges `crossings * 500` beside `drc_violations *
//! 1000`. A crossing is two pieces of copper on the same layer, on different
//! nets, at zero distance - which is also exactly what the clearance rule
//! calls a short. Nobody had checked whether the two terms are counting the
//! same physical fault.
//!
//! They are. Measured on two traces crossing at one point: `crossings` 1,
//! `drc_violations` 1, `shorts` 1 - so the composite pays 500 for the crossing
//! and 1000 for the same contact called a short, 1500 for one place where two
//! nets touch. The same two traces on opposite layers give 0 and 0.
//!
//! That is not a defect in either number - both are correct about what they
//! measure - but the two terms are not independent, and anybody tuning the
//! weights should know it. This comment first said "two clearance violations,
//! one per trace"; measuring said one.

use cypcb_autoroute::scoring::score_board;
use cypcb_core::{Nm, Point};
use cypcb_drc::presets::DesignRules;
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::Layer;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

/// Two traces on `layers` copper layers. When `same_layer`, they cross.
fn crossing_board(same_layer: bool) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(40.0)), 2);
    let a = world.intern_net("A");
    let b = world.intern_net("B");

    let mut lay = |net, layer, from: (f64, f64), to: (f64, f64)| {
        world.spawn_entity((
            Trace {
                segments: vec![TraceSegment::new(
                    Point::from_mm(from.0, from.1),
                    Point::from_mm(to.0, to.1),
                )],
                width: Nm::from_mm(0.2),
                layer,
                net_id: net,
                locked: false,
                source: TraceSource::Autorouted,
            },
            net,
        ));
    };

    // A horizontal run and a vertical one through the same point.
    lay(a, Layer::TopCopper, (5.0, 20.0), (35.0, 20.0));
    lay(
        b,
        if same_layer {
            Layer::TopCopper
        } else {
            Layer::BottomCopper
        },
        (20.0, 5.0),
        (20.0, 35.0),
    );
    world
}

fn score(mut world: BoardWorld) -> cypcb_autoroute::scoring::RoutingScore {
    let library = FootprintLibrary::new();
    world.rebuild_spatial_index_from_library(&library);
    score_board(
        &mut world,
        &DesignRules::jlcpcb_2layer(),
        &Default::default(),
    )
}

#[test]
fn two_nets_meeting_on_one_layer_is_a_crossing() {
    let score = score(crossing_board(true));
    assert_eq!(
        score.crossings, 1,
        "two nets crossing on the same layer is one crossing"
    );
}

#[test]
fn the_same_two_nets_on_different_layers_do_not_cross() {
    // The half that must not change: copper on the top and copper on the
    // bottom pass over each other, which is what layers are for.
    let score = score(crossing_board(false));
    assert_eq!(
        score.crossings, 0,
        "traces on different layers do not cross"
    );
}

#[test]
fn a_crossing_is_also_a_short_so_the_composite_pays_for_it_twice() {
    // Not a defect in either number - both are right about what they measure -
    // but the composite charges 500 for the crossing and 1000 per clearance
    // violation for the same place where two nets touch. Anybody tuning the
    // weights should know the two terms are not independent.
    let crossed = score(crossing_board(true));
    let clear = score(crossing_board(false));

    assert_eq!(crossed.crossings, 1);
    assert_eq!(clear.crossings, 0);
    assert!(
        crossed.drc_violations > clear.drc_violations,
        "the crossing produced no DRC violation, so the two terms are \
         independent after all: {} against {}",
        crossed.drc_violations,
        clear.drc_violations
    );
    assert!(
        crossed.shorts > 0,
        "two nets at zero distance are a short: {} shorts",
        crossed.shorts
    );

    // What the composite actually pays, with the default weights.
    let crossing_term = 500.0;
    let drc_term = (crossed.drc_violations - clear.drc_violations) as f64 * 1000.0;
    assert!(
        drc_term >= crossing_term,
        "the DRC side of the same fault costs {drc_term} against the \
         crossing's {crossing_term}"
    );
}
