//! Does the number the router reports mean what its field says?
//!
//! `cargo test --release -p cypcb-autoroute --test what_partial_reports -- --nocapture`
//!
//! `RoutingStatus::Partial` carries one number and its own documentation calls
//! it connections. Every producer put `unrouted.len()` there, which is a count
//! of nets. The two are equal exactly when no net loses more than one of its
//! connections, which is why the difference went unseen: the benchmark boards
//! route completely, and a board that fails usually fails one connection at a
//! time.
//!
//! Forcing the failure needs no special board. A coarse grid makes the search
//! fail on real fixtures, and at 1.6 mm `shift_driver` loses 20 connections
//! spread over 18 nets - so two of its nets lost two apiece, and the old number
//! understated the work by two.

use std::path::Path;

use cypcb_autoroute::grid::RoutingGrid;
use cypcb_autoroute::orchestrator::{extract_ratsnest, order_nets};
use cypcb_autoroute::pathfinder_v2::pathfinder_loop;
use cypcb_autoroute::strategy::StrategyKind;
use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::{preset_for_world, ruleset_for_world};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_router::types::RoutingStatus;

/// What one run left behind: nets short of something, and connections missing.
struct Shortfall {
    nets: usize,
    connections: usize,
}

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(name)
}

/// The strategy is pinned rather than defaulted. `route_board` dispatches on
/// `config.strategy`, and the first version of the end-to-end test below
/// compared PathFinder's loop against whatever the default happened to be -
/// which reported a third number, 30, that belonged to the other router
/// entirely. The two numbers being compared have to come out of the same
/// search or the comparison means nothing.
fn config_at(resolution_nm: i64) -> AutorouteConfig {
    AutorouteConfig {
        grid_resolution_nm: Some(resolution_nm),
        strategy: StrategyKind::PathFinder,
        ..AutorouteConfig::default()
    }
}

/// Run the loop directly, which is the only place both numbers exist at once.
fn shortfall(name: &str, resolution_nm: i64) -> Shortfall {
    let parsed = parse_kicad_pcb(&fixture(name)).expect("fixture parses");
    let mut world = parsed.world;
    let library = parsed.library;
    let preset = preset_for_world(
        cypcb_rules::presets::RulesPreset::JlcpcbStandard2Layer,
        &world,
    );
    let rules = ruleset_for_world(preset, &world);
    let config = config_at(resolution_nm);
    let mut grid =
        RoutingGrid::from_board(&mut world, &library, &rules, resolution_nm).expect("grid builds");
    let ratsnest = extract_ratsnest(&mut world, &library);
    let order = order_nets(&ratsnest);
    let result = pathfinder_loop(&mut grid, &ratsnest, &order, &rules, &config, None);
    Shortfall {
        nets: result.unrouted.len(),
        connections: result.unrouted_connections,
    }
}

#[test]
fn a_net_short_of_two_connections_counts_two() {
    // The case that shows the difference. Stated before the run rather than
    // read off it: at this resolution `shift_driver` has to report strictly
    // more connections than nets, because at least one net loses more than one
    // connection. A run where the two are equal proves nothing about which
    // quantity is being reported, so equality here is a failure and not a pass.
    let measured = shortfall("shift_driver.kicad_pcb", 1_600_000);
    println!(
        "shift_driver at 1.6mm: {} nets, {} connections",
        measured.nets, measured.connections
    );

    assert!(
        measured.nets > 0,
        "the coarse grid has to make the search fail, or there is nothing to count"
    );
    assert!(
        measured.connections > measured.nets,
        "this is the case that separates the two numbers: {} nets against {} \
         connections means every net lost exactly one, and a report of either \
         number would look the same",
        measured.nets,
        measured.connections
    );
}

#[test]
fn a_shortfall_is_never_smaller_than_the_nets_it_is_spread_over() {
    // The invariant, on every board and both coarse grids. A net in the
    // unrouted list is short of at least one connection, so the connection
    // count can never come out below the net count - and if it ever does, the
    // two are being counted from different places.
    let mut any_failed = false;
    for name in [
        "multi_ic.kicad_pcb",
        "qfp_fanout.kicad_pcb",
        "shift_driver.kicad_pcb",
    ] {
        for resolution in [800_000i64, 1_600_000] {
            let measured = shortfall(name, resolution);
            println!(
                "{name:<24} {resolution:>9}: {:>3} nets, {:>3} connections",
                measured.nets, measured.connections
            );
            assert!(
                measured.connections >= measured.nets,
                "{name} at {resolution} reports {} connections over {} nets, \
                 which is fewer than one apiece",
                measured.connections,
                measured.nets
            );
            any_failed |= measured.nets > 0;
        }
    }

    // The control. Six runs that all routed completely would satisfy the
    // assertion above without ever comparing anything.
    assert!(
        any_failed,
        "no run left anything unrouted, so the invariant was never tested"
    );
}

#[test]
fn the_public_entry_point_reports_a_number_that_is_not_the_loops() {
    // What this test proves and what it does not.
    //
    // The two tests above prove the loop now counts connections: at 1.6 mm
    // `shift_driver` leaves 18 nets short of 20 connections, and before this
    // change `RoutingStatus::Partial` would have carried the 18.
    //
    // Going through `route_board` gives a third number - 30 - with the same
    // board, the same resolution and the strategy pinned to the same router.
    // So the loop is not the last word on what a caller sees, and this test
    // does NOT claim the public number is the connection count, because that
    // has not been established. What it holds is the part that is known: the
    // entry point reports a partial result on this board, and that number is
    // neither of the loop's two.
    //
    // Where the difference comes from is open. It is not the strategy (pinned
    // here) and it is not `repair::repair_routes`, which builds no status of
    // its own. The remaining candidates are the config `route_board` clamps
    // before dispatching, and the order in which the grid and the ratsnest are
    // built - this test builds the grid first, and the strategy may not.
    let parsed = parse_kicad_pcb(&fixture("shift_driver.kicad_pcb")).expect("fixture parses");
    let mut world = parsed.world;
    let library = parsed.library;
    let preset = preset_for_world(
        cypcb_rules::presets::RulesPreset::JlcpcbStandard2Layer,
        &world,
    );
    let rules = ruleset_for_world(preset, &world);

    let result = route_board(&mut world, &library, &rules, &config_at(1_600_000));
    let reported = match result.status {
        RoutingStatus::Partial { unrouted_count } => unrouted_count,
        other => panic!("a 1.6mm grid has to leave work behind, got {other:?}"),
    };
    let measured = shortfall("shift_driver.kicad_pcb", 1_600_000);
    println!(
        "route_board reported {reported}; the loop left {} nets and {} connections",
        measured.nets, measured.connections
    );

    assert!(
        reported > 0,
        "the entry point has to report the work it left behind"
    );

    // The open question, pinned as a number so it cannot drift unnoticed. If
    // this stops holding, the two paths have converged and the comment above
    // needs rewriting rather than the assertion relaxing.
    assert_ne!(
        reported, measured.connections,
        "the two paths agree now; the discrepancy this test documents is gone \
         and the comment above is stale"
    );
}
