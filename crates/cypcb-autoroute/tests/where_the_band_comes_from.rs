//! Why a cost change of a hundredth moves the result by 28 violations.
//!
//! `cargo test --release -p cypcb-autoroute --test where_the_band_comes_from -- --ignored --nocapture`
//!
//! Prices 0.24 and 0.26 ask the router for nearly the same trade and produce
//! boards 23 violations apart. Best-of-N over net orderings was measured and
//! bought nothing, so the ordering is not what moves. What is left is the
//! negotiation itself: PathFinder re-routes whatever passes through overused
//! cells, and one cell changing hands early puts a different set of nets in
//! the next iteration.
//!
//! This prints the overused count at the end of every iteration for both
//! prices. Two shapes to tell apart: runs that diverge in the first few
//! iterations and never come back point at the ramp being too steep to
//! recover from an early accident, and runs that track each other and settle
//! on different fixed points point at the cost model having several answers it
//! considers equally good.

use std::path::Path;

use cypcb_autoroute::grid::RoutingGrid;
use cypcb_autoroute::orchestrator::{extract_ratsnest, order_nets};
use cypcb_autoroute::pathfinder_v2::pathfinder_loop;
use cypcb_autoroute::AutorouteConfig;
use cypcb_kicad::parse_kicad_pcb;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};

fn fixture_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

fn trajectory(filename: &str, price: f64) -> (Vec<usize>, u32, bool) {
    let parsed = parse_kicad_pcb(&fixture_path(filename)).expect("the fixture parses");
    let mut world = parsed.world;
    let library = parsed.library;

    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset exists"));
    let config = AutorouteConfig {
        via_foreign_copper_penalty: price,
        ..AutorouteConfig::default()
    };

    let resolution = config.resolve_adaptive_grid_resolution(
        &rules,
        world.board_info().expect("the fixture has a board").0.width.raw(),
        world.board_info().expect("the fixture has a board").0.height.raw(),
    );
    let mut grid = RoutingGrid::from_board(&mut world, &library, &rules, resolution)
        .expect("the grid builds");

    let ratsnest = extract_ratsnest(&mut world, &library);
    let order = order_nets(&ratsnest);
    let result = pathfinder_loop(&mut grid, &ratsnest, &order, &rules, &config);

    (
        result.overuse_per_iteration,
        result.iterations,
        result.converged,
    )
}

#[test]
#[ignore = "diagnostic: prints the negotiation trajectory at two neighbouring prices"]
fn two_prices_a_hundredth_apart_take_different_paths() {
    for filename in ["stm32_breakout.kicad_pcb", "multi_ic.kicad_pcb"] {
        eprintln!();
        eprintln!("=== {filename} ===");

        let (low, low_iters, low_converged) = trajectory(filename, 0.24);
        let (high, high_iters, high_converged) = trajectory(filename, 0.26);

        eprintln!(
            "  price 0.24: {} iterations, converged {}, overuse {:?}",
            low_iters, low_converged, low
        );
        eprintln!(
            "  price 0.26: {} iterations, converged {}, overuse {:?}",
            high_iters, high_converged, high
        );

        let first_difference = low
            .iter()
            .zip(high.iter())
            .position(|(a, b)| a != b)
            .map(|i| (i + 1).to_string())
            .unwrap_or_else(|| "never within the shorter run".to_string());
        eprintln!("  first iteration where they differ: {first_difference}");
    }
}
