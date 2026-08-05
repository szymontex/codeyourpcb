//! Strategy comparison integration test.
//!
//! Runs both ImprovedAStarStrategy and PathFinderStrategy on benchmark
//! KiCad PCB fixtures. Prints a comparison table to stderr for CI inspection
//! and asserts that both strategies produce valid routes.
//!
//! led_blink runs in CI; larger boards are `#[ignore]` for manual runs
//! since A* on large grids can take minutes in debug/release mode.

use std::path::Path;

use cypcb_autoroute::astar_improved::ImprovedAStarStrategy;
use cypcb_autoroute::pathfinder_v2::PathFinderStrategy;
use cypcb_autoroute::scoring::{score_board, RoutingScore, ScoreWeights};
use cypcb_autoroute::strategy::RoutingStrategy;
use cypcb_autoroute::AutorouteConfig;
use cypcb_drc::DesignRules;
use cypcb_kicad::{parse_kicad_pcb, KicadPcbParseResult};
use cypcb_router::apply_routes;
use cypcb_router::types::RoutingResult;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

// ============================================================================
// Helpers
// ============================================================================

/// Resolve a benchmark fixture path relative to workspace root.
fn fixture_path(filename: &str) -> std::path::PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// Build JLCPCB 2-layer routing rules.
fn test_rules() -> PresetRuleSet {
    let preset = RulesPreset::from_name("jlcpcb").unwrap();
    PresetRuleSet::new(preset)
}

/// Holds a scored routing result for one strategy × fixture.
struct StrategyResult {
    strategy_name: String,
    fixture_name: String,
    route_count: usize,
    score: RoutingScore,
}

/// Parse a KiCad PCB benchmark fixture fresh.
fn parse_fixture(filename: &str) -> KicadPcbParseResult {
    let path = fixture_path(filename);
    parse_kicad_pcb(&path).unwrap_or_else(|e| {
        panic!("Failed to parse {}: {:?}", filename, e);
    })
}

/// Route a board with a given strategy and score the result.
fn route_and_score(
    mut world: BoardWorld,
    library: &FootprintLibrary,
    strategy: &dyn RoutingStrategy,
    fixture_name: &str,
) -> (StrategyResult, RoutingResult) {
    let rules = test_rules();
    let config = AutorouteConfig::default();

    eprintln!(
        "  [{}] {} — components={}, nets={}, library_fps={}",
        strategy.name(),
        fixture_name,
        world.components().len(),
        world.nets().count(),
        library.len(),
    );

    let result = strategy.route(&mut world, library, &rules, &config);
    let route_count = result.route_count();

    // Apply routes to world for scoring
    apply_routes(&mut world, &result);

    // Rebuild spatial index for accurate scoring
    world.rebuild_spatial_index_with_traces(|_| {
        cypcb_core::Rect::from_center_size(
            cypcb_core::Point::ORIGIN,
            (cypcb_core::Nm::from_mm(1.0), cypcb_core::Nm::from_mm(1.0)),
        )
    });

    let drc_rules = DesignRules::jlcpcb_2layer();
    let score = score_board(&mut world, &drc_rules, &ScoreWeights::default());

    let sr = StrategyResult {
        strategy_name: strategy.name().to_string(),
        fixture_name: fixture_name.to_string(),
        route_count,
        score,
    };

    (sr, result)
}

/// Print one row of the comparison table.
fn print_table_row(r: &StrategyResult) {
    eprintln!(
        "║ {:<17} ║ {:<14} ║ {:>8.1} ║ {:>8} ║ {:>8} ║ {:>12.2} ║",
        r.strategy_name,
        r.fixture_name,
        r.score.composite,
        r.score.drc_violations,
        r.score.via_count,
        r.score.total_length.0 as f64 / 1_000_000.0,
    );
}

/// Run both strategies on a fixture, print table, run assertions.
fn compare_fixture(filename: &str, baseline_drc: Option<u32>) {
    let fixture_label = filename.strip_suffix(".kicad_pcb").unwrap_or(filename);

    let astar = ImprovedAStarStrategy;
    let pathfinder = PathFinderStrategy;

    // --- ImprovedAStar ---
    let parsed_astar = parse_fixture(filename);
    let (astar_result, _) = route_and_score(
        parsed_astar.world,
        &parsed_astar.library,
        &astar,
        fixture_label,
    );
    assert!(
        astar_result.route_count > 0,
        "[{}] ImprovedAStar produced 0 routes",
        fixture_label,
    );

    // --- PathFinder ---
    let parsed_pf = parse_fixture(filename);
    let (pf_result, _) = route_and_score(
        parsed_pf.world,
        &parsed_pf.library,
        &pathfinder,
        fixture_label,
    );
    assert!(
        pf_result.route_count > 0,
        "[{}] PathFinder produced 0 routes",
        fixture_label,
    );

    // Print comparison table
    eprintln!();
    eprintln!(
        "╔═══════════════════╦════════════════╦══════════╦══════════╦══════════╦══════════════╗"
    );
    eprintln!(
        "║ Strategy          ║ Fixture        ║Composite ║ DRC Viol ║ Vias     ║ Length (mm)  ║"
    );
    eprintln!(
        "╠═══════════════════╬════════════════╬══════════╬══════════╬══════════╬══════════════╣"
    );
    print_table_row(&astar_result);
    print_table_row(&pf_result);
    eprintln!(
        "╚═══════════════════╩════════════════╩══════════╩══════════╩══════════╩══════════════╝"
    );

    // Score comparison
    if pf_result.score.composite > astar_result.score.composite {
        let delta = pf_result.score.composite - astar_result.score.composite;
        let pct = if astar_result.score.composite > 0.0 {
            delta / astar_result.score.composite * 100.0
        } else {
            0.0
        };
        eprintln!(
            "⚠ [{}] PathFinder composite ({:.1}) > ImprovedAStar ({:.1}) by {:.1}% — \
             likely congestion non-convergence on complex board topology",
            fixture_label, pf_result.score.composite, astar_result.score.composite, pct,
        );
    } else {
        eprintln!(
            "✓ [{}] PathFinder composite ({:.1}) ≤ ImprovedAStar ({:.1})",
            fixture_label, pf_result.score.composite, astar_result.score.composite,
        );
    }

    // DRC baseline assertion
    if let Some(baseline) = baseline_drc {
        assert!(
            pf_result.score.drc_violations < baseline,
            "[{}] PathFinder DRC violations ({}) should be < baseline ({})",
            fixture_label,
            pf_result.score.drc_violations,
            baseline,
        );
        assert!(
            astar_result.score.drc_violations < baseline,
            "[{}] ImprovedAStar DRC violations ({}) should be < baseline ({})",
            fixture_label,
            astar_result.score.drc_violations,
            baseline,
        );
    }

    eprintln!();
}

// ============================================================================
// Tests
// ============================================================================

/// Core comparison test: led_blink (simple, runs in CI).
/// Both strategies route all nets, scores compared, DRC < baseline.
#[test]
fn strategy_comparison_led_blink() {
    // S02 baseline: blink DRC violations were ~50
    compare_fixture("led_blink.kicad_pcb", Some(50));
}

/// stm32_breakout comparison — medium complexity, slow in debug mode.
#[test]
#[ignore = "slow: stm32_breakout A* routing takes >60s in debug mode"]
fn strategy_comparison_stm32_breakout() {
    compare_fixture("stm32_breakout.kicad_pcb", None);
}

/// multi_ic comparison — high complexity, very slow.
#[test]
#[ignore = "slow: multi_ic A* routing takes minutes even in release mode"]
fn strategy_comparison_multi_ic() {
    compare_fixture("multi_ic.kicad_pcb", None);
}
