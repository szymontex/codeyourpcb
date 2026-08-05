//! Benchmark validation integration tests.
//!
//! Two test functions:
//! - `benchmark_regression` — fast CI gate (non-ignored): routes led_blink with PathFinder,
//!   asserts the solution is complete (0 unrouted, >= 20 routes) and then that quality has
//!   not regressed (composite ≤ 2200, DRC ≤ 1, smoothness ≥ 0.95).
//! - `benchmark_full_matrix` — comprehensive comparison (`#[ignore]`): routes all 3 fixtures
//!   × 2 strategies, prints comparison table, emits JSON report, confirms PathFinder default.

use std::path::Path;

use serde::Serialize;

use cypcb_autoroute::astar_improved::ImprovedAStarStrategy;
use cypcb_autoroute::pathfinder_v2::PathFinderStrategy;
use cypcb_autoroute::scoring::{score_board, RoutingScore, ScoreWeights};
use cypcb_autoroute::strategy::RoutingStrategy;
use cypcb_autoroute::AutorouteConfig;
use cypcb_drc::DesignRules;
use cypcb_kicad::{parse_kicad_pcb, BENCHMARKS};
use cypcb_router::apply_routes;
use cypcb_router::types::RoutingStatus;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};

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

/// Route a board with a given strategy and return (RoutingScore, route_count, unrouted).
///
/// Always calls `rebuild_spatial_index_with_traces()` before scoring
/// and uses `DesignRules::jlcpcb_2layer()` for DRC.
fn route_and_score(strategy: &dyn RoutingStrategy, fixture: &str) -> (RoutingScore, usize, usize) {
    let parsed = parse_kicad_pcb(&fixture_path(fixture))
        .unwrap_or_else(|e| panic!("Failed to parse {}: {:?}", fixture, e));
    let mut world = parsed.world;
    let library = parsed.library;
    let rules = test_rules();
    let config = AutorouteConfig::default();

    let result = strategy.route(&mut world, &library, &rules, &config);
    let route_count = result.route_count();
    let unrouted = match result.status {
        RoutingStatus::Complete => 0,
        RoutingStatus::Partial { unrouted_count } => unrouted_count,
        RoutingStatus::Failed { .. } => usize::MAX,
    };

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

    (score, route_count, unrouted)
}

// ============================================================================
// BenchmarkResult (serializable for JSON output)
// ============================================================================

#[derive(Debug, Clone, Serialize)]
struct BenchmarkResult {
    fixture: String,
    strategy: String,
    composite: f64,
    drc_violations: u32,
    smoothness: f64,
    via_count: u32,
    total_length_mm: f64,
    route_count: usize,
    unrouted: usize,
}

impl BenchmarkResult {
    fn from_score(
        fixture: &str,
        strategy: &str,
        score: &RoutingScore,
        route_count: usize,
        unrouted: usize,
    ) -> Self {
        Self {
            fixture: fixture.to_string(),
            strategy: strategy.to_string(),
            composite: score.composite,
            drc_violations: score.drc_violations,
            smoothness: score.smoothness,
            via_count: score.via_count,
            total_length_mm: score.total_length.0 as f64 / 1_000_000.0,
            route_count,
            unrouted,
        }
    }
}

// ============================================================================
// Table printing
// ============================================================================

fn print_table_header() {
    eprintln!("╔═══════════════════╦════════════════╦══════════╦══════════╦══════════╦══════════╦══════════════╦══════════╗");
    eprintln!("║ Strategy          ║ Fixture        ║Composite ║ DRC Viol ║Smoothness║ Vias     ║ Length (mm)  ║ Unrouted ║");
    eprintln!("╠═══════════════════╬════════════════╬══════════╬══════════╬══════════╬══════════╬══════════════╬══════════╣");
}

fn print_table_row(r: &BenchmarkResult) {
    eprintln!(
        "║ {:<17} ║ {:<14} ║ {:>8.1} ║ {:>8} ║ {:>8.3} ║ {:>8} ║ {:>12.2} ║ {:>8} ║",
        r.strategy,
        r.fixture,
        r.composite,
        r.drc_violations,
        r.smoothness,
        r.via_count,
        r.total_length_mm,
        r.unrouted,
    );
}

fn print_table_separator() {
    eprintln!("╠═══════════════════╬════════════════╬══════════╬══════════╬══════════╬══════════╬══════════════╬══════════╣");
}

fn print_table_footer() {
    eprintln!("╚═══════════════════╩════════════════╩══════════╩══════════╩══════════╩══════════╩══════════════╝");
}

// ============================================================================
// Tests
// ============================================================================

/// Every benchmark fixture, with the DRC violation count each one currently
/// produces. led_blink is the only board the gate used to look at, and at 3
/// violations it made the router look healthy; the two realistic fixtures were
/// sitting at 312 and 383. These are ratchets - lower them as the router
/// improves, never raise them to accommodate a regression.
const DRC_RATCHETS: &[(&str, &str, u32)] = &[
    ("led_blink.kicad_pcb", "led_blink", 1),
    ("stm32_breakout.kicad_pcb", "stm32_breakout", 238),
    ("multi_ic.kicad_pcb", "multi_ic", 240),
];

/// Routes every fixture and holds the line on completeness and DRC count.
///
/// Ignored by default: the two realistic fixtures take about two minutes each
/// to route, which does not belong in `cargo test`. scripts/quality-gate.sh
/// runs it explicitly in the benchmark stage.
#[test]
#[ignore = "slow: routes all three fixtures, ~4 minutes"]
fn benchmark_all_fixtures_drc() {
    let pathfinder = PathFinderStrategy;

    eprintln!();
    print_table_header();
    let mut measured = Vec::new();
    for (filename, label, _) in DRC_RATCHETS {
        let (score, route_count, unrouted) = route_and_score(&pathfinder, filename);
        print_table_row(&BenchmarkResult::from_score(
            label,
            "PathFinder",
            &score,
            route_count,
            unrouted,
        ));
        measured.push((label, score.drc_violations, unrouted, route_count));
    }
    print_table_footer();
    eprintln!();

    for ((label, violations, unrouted, route_count), (_, _, ratchet)) in
        measured.iter().zip(DRC_RATCHETS)
    {
        assert_eq!(
            *unrouted, 0,
            "FAIL {}: {} unrouted connections, threshold 0",
            label, unrouted
        );
        assert!(*route_count > 0, "FAIL {}: routed nothing at all", label);
        assert!(
            violations <= ratchet,
            "FAIL {}: {} DRC violations, threshold {} - the router got worse",
            label,
            violations,
            ratchet
        );
        eprintln!(
            "  ✓ {}: {} routes, {} DRC violations (threshold {})",
            label, route_count, violations, ratchet
        );
    }
}

/// Fast CI regression gate: routes led_blink with PathFinder and asserts
/// score thresholds. Non-ignored so it runs in `cargo test --workspace`.
#[test]
fn benchmark_regression() {
    let pathfinder = PathFinderStrategy;
    let (score, route_count, unrouted) = route_and_score(&pathfinder, "led_blink.kicad_pcb");

    // Print score table
    eprintln!();
    let result =
        BenchmarkResult::from_score("led_blink", "PathFinder", &score, route_count, unrouted);
    print_table_header();
    print_table_row(&result);
    print_table_footer();
    eprintln!();

    // --- Regression assertions with diagnostic messages ---
    //
    // Completeness comes first. Every other metric improves when the router
    // abandons connections - fewer traces means less length, fewer vias and
    // fewer DRC violations - so a gate that only reads quality scores rewards
    // giving up. This gate used to assert `route_count > 0` and passed while
    // PathFinder left a connection unrouted and emitted 7 routes.

    assert_eq!(
        unrouted, 0,
        "FAIL benchmark_regression: {} unrouted connections, threshold 0",
        unrouted
    );
    eprintln!("  ✓ unrouted: got {}, threshold 0", unrouted);

    assert!(
        route_count >= 20,
        "FAIL benchmark_regression: route_count got {}, threshold >= 20 - the router is emitting far less copper than a complete solution needs",
        route_count
    );
    eprintln!("  ✓ route_count: got {}, threshold >= 20", route_count);

    // Quality thresholds are ratchets measured against a complete solution.
    // They are deliberately tight: lower them whenever the router improves,
    // never raise them to accommodate a regression. R107 targets 0 violations.
    assert!(
        score.composite <= 2_200.0,
        "FAIL benchmark_regression: composite got {:.1}, threshold ≤ 2200.0 (baseline 2002 × 1.1)",
        score.composite
    );
    eprintln!(
        "  ✓ composite: got {:.1}, threshold ≤ 2200.0",
        score.composite
    );

    assert!(
        score.drc_violations <= 1,
        "FAIL benchmark_regression: drc_violations got {}, threshold ≤ 1 (R107 targets 0)",
        score.drc_violations
    );
    eprintln!(
        "  ✓ drc_violations: got {}, threshold ≤ 1 (R107 targets 0)",
        score.drc_violations
    );

    assert!(
        score.smoothness >= 0.95,
        "FAIL benchmark_regression: smoothness got {:.3}, threshold ≥ 0.95",
        score.smoothness
    );
    eprintln!(
        "  ✓ smoothness: got {:.3}, threshold ≥ 0.95",
        score.smoothness
    );

    eprintln!();
    eprintln!("═══ benchmark_regression PASSED ═══");
    eprintln!(
        "  composite={:.1}  drc={}  smoothness={:.3}  vias={}  length={:.2}mm  routes={}",
        score.composite,
        score.drc_violations,
        score.smoothness,
        score.via_count,
        result.total_length_mm,
        route_count,
    );
}

/// Comprehensive benchmark: all 3 fixtures × 2 strategies.
/// Produces comparison table + JSON report. Confirms PathFinder as default.
#[test]
#[ignore = "slow: full matrix routes all fixtures with both strategies"]
fn benchmark_full_matrix() {
    let strategies: Vec<Box<dyn RoutingStrategy>> = vec![
        Box::new(PathFinderStrategy),
        Box::new(ImprovedAStarStrategy),
    ];

    let mut results: Vec<BenchmarkResult> = Vec::new();

    for benchmark in BENCHMARKS {
        let fixture_label = benchmark
            .filename
            .strip_suffix(".kicad_pcb")
            .unwrap_or(benchmark.filename);

        for strategy in &strategies {
            eprintln!("  [{}] routing {} ...", strategy.name(), fixture_label);

            let (score, route_count, unrouted) =
                route_and_score(strategy.as_ref(), benchmark.filename);

            let br = BenchmarkResult::from_score(
                fixture_label,
                strategy.name(),
                &score,
                route_count,
                unrouted,
            );
            results.push(br);
        }
    }

    // --- Print aggregate comparison table ---
    eprintln!();
    eprintln!("═══ Full Benchmark Matrix ═══");
    eprintln!();
    print_table_header();

    let mut first_fixture = true;
    let mut prev_fixture = String::new();
    for r in &results {
        if r.fixture != prev_fixture {
            if !first_fixture {
                print_table_separator();
            }
            first_fixture = false;
            prev_fixture = r.fixture.clone();
        }
        print_table_row(r);
    }
    print_table_footer();
    eprintln!();

    // --- Emit JSON report ---
    let json = serde_json::to_string(&results).expect("Failed to serialize benchmark results");
    eprintln!("BENCHMARK_JSON: {}", json);
    eprintln!();

    // --- Assert PathFinder ≤ ImprovedAStar on led_blink ---
    let pf_led = results
        .iter()
        .find(|r| r.fixture == "led_blink" && r.strategy == "PathFinder")
        .expect("PathFinder led_blink result missing");
    let astar_led = results
        .iter()
        .find(|r| r.fixture == "led_blink" && r.strategy == "ImprovedAStar")
        .expect("ImprovedAStar led_blink result missing");

    assert!(
        pf_led.composite <= astar_led.composite,
        "FAIL benchmark_full_matrix: PathFinder composite ({:.1}) > ImprovedAStar ({:.1}) on led_blink. \
         PathFinder should be ≤ ImprovedAStar for empirical strategy selection.",
        pf_led.composite,
        astar_led.composite,
    );
    eprintln!(
        "✓ Strategy selection: PathFinder ({:.1}) ≤ ImprovedAStar ({:.1}) on led_blink",
        pf_led.composite, astar_led.composite,
    );

    // --- Assert route_count > 0 for all results ---
    for r in &results {
        assert!(
            r.route_count > 0,
            "FAIL benchmark_full_matrix: {} × {} produced 0 routes",
            r.fixture,
            r.strategy,
        );
    }

    eprintln!();
    eprintln!("═══ Default strategy: PathFinder (empirically validated) ═══");
    eprintln!();
}
