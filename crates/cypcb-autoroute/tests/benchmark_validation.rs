//! Benchmark validation integration tests.
//!
//! Two test functions:
//! - `benchmark_regression` — fast CI gate (non-ignored): routes led_blink with PathFinder,
//!   asserts the solution is complete (0 unrouted, >= 20 routes) and then that quality has
//!   not regressed (composite ≤ 100, DRC 0, smoothness ≥ 0.95).
//! - `benchmark_full_matrix` — comprehensive comparison (`#[ignore]`): routes all 3 fixtures
//!   × 2 strategies, prints comparison table, emits JSON report, confirms PathFinder default.

use std::path::Path;

use serde::Serialize;

use cypcb_autoroute::astar_improved::ImprovedAStarStrategy;
use cypcb_autoroute::pathfinder_v2::PathFinderStrategy;
use cypcb_autoroute::scoring::{score_board, RoutingScore, ScoreWeights};
use cypcb_autoroute::strategy::RoutingStrategy;
use cypcb_autoroute::{route_board, AutorouteConfig};
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

    // Route through the shipped entry point, not the bare strategy: repair is
    // part of what a user gets when they press Route, so it has to be part of
    // what the ratchets measure.
    let result = if strategy.name() == "pathfinder" {
        route_board(&mut world, &library, &rules, &config)
    } else {
        strategy.route(&mut world, &library, &rules, &config)
    };
    let route_count = result.route_count();
    let unrouted = match result.status {
        RoutingStatus::Complete => 0,
        RoutingStatus::Partial { unrouted_count } => unrouted_count,
        RoutingStatus::Failed { .. } => usize::MAX,
    };

    apply_routes(&mut world, &result);

    // Rebuild spatial index for accurate scoring
    world.rebuild_spatial_index_from_library(&library);

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
///
/// The two realistic numbers went **up** when the spatial index stopped boxing
/// every footprint at 1mm - 137 -> 176 and 64 -> 127 on byte-identical routing.
/// That is not a regression; it is the same board measured without a blind
/// spot. 24 of stm32_breakout's violations and 54 of multi_ic's name a
/// component, and those are precisely the pairs the old index could not see.
///
/// Lowered by the DRC-driven repair pass, which routes, reads the real report
/// and re-routes with the offending cells forbidden: 176 -> 167 and 127 -> 110,
/// every board still complete.
/// The most DRC violations each fixture may produce before the gate calls it a
/// regression.
///
/// Re-measured 2026-08-06 and raised, which is normally forbidden. The board
/// did not get worse; the checker stopped being blind. Three fixes landed
/// between the old numbers and these: the clearance rule measures pad copper
/// instead of the courtyard box, its same-net exemption is decided per pad
/// rather than per component, and an imported board finally carries its
/// footprints, which woke `courtyard-clearance`, `solder-mask-bridge` and
/// `silk-clearance` on every KiCad fixture. The old 167 and 110 also predate
/// the repair pass being switched off after it was measured to accept nothing.
///
/// These are totals, including what the fixture violated before routing.
/// `drc_report` separates the two if you need to know which is which.
///
/// Raised again on the same day and for the same kind of reason: the
/// clearance rule reports per offending segment now instead of once per pair
/// of entities, so a trace running too close to a part in two places is two
/// faults rather than one. The board is unchanged - 186 became 271 because
/// the checker stopped merging, which is also what made a saved board report
/// more than the board it was written from.
///
/// led_blink's violations are real and named: a GND trace across C1's
/// SW_OUT pad. The default router still makes it; `PathFinder High-Density`
/// routes the same board with zero, which is what `--variants` is for. Lower
/// these when the router improves. Never raise them for a regression.
/// Lowered on 2026-08-07, and not because the router improved: the clearance
/// rule was counting one gap twice wherever a trace's corner was the nearest
/// point to the other feature, and both segments meeting there reported it.
/// The boards are unchanged - 271 became 251 and 210 became 191 because the
/// checker stopped double-counting.
/// Two columns since 2026-08-07, because they are two different failures. A
/// board with copper touching copper cannot work; a board with a gap under
/// spec is a yield risk a fab may still build. A single count treats one short
/// as better than two near misses, which is backwards, and it hid that
/// reserving trace copper halves the shorts on both dense fixtures.
/// Raised on 2026-08-07, and not for a regression in the copper - for the
/// opposite. `paths_to_output` deleted every via whose cell carried
/// `CELL_PAD`, which covers a pad plus its clearance, so a route that changed
/// layer near any pad lost the via that joined it and the board came back with
/// two halves that never meet. Every check agreed it was fine: DRC saw no
/// overlap because the copper was on different layers, and the unrouted count
/// was zero because a path came back for every edge.
///
/// The search refuses to change layer on a pad now and every via reaches the
/// output. Pins no copper reaches - the measure that matters here - go
/// led_blink 1 -> 0, stm32_breakout 21 -> 6, multi_ic 60 -> 23, and
/// `UnroutedPinRule` is registered so the gate counts them.
///
/// The price is that vias which were being deleted are copper now, and the
/// grid does not model a via's ring, so they land too close to things.
///
/// Moved again the same day, downward on violations: refusing a layer change
/// on a pad was replaced by pricing it, which is the shape that has worked in
/// this vector six times against a veto's five failures. stm32_breakout 250 ->
/// 239 violations with 130 -> 136 shorts, multi_ic 375 -> 336 and 194 -> 166,
/// open pins unchanged at 6 and one better at 22. Never raise these for a
/// regression; lower them when the via ring reaches the grid.
const DRC_RATCHETS: &[(&str, &str, u32, u32)] = &[
    ("led_blink.kicad_pcb", "led_blink", 2, 0),
    ("stm32_breakout.kicad_pcb", "stm32_breakout", 239, 136),
    // Re-measured 2026-08-08 at 318 / 177, after two of its parts stopped
    // sitting 50mm to the left of the board: the file carried `(at 105, 80)`
    // and `(at 140, 55)` and the importer read a malformed coordinate as zero
    // without a word. Its band is 63 violations wide across prices
    // 0.22..0.28, and the shorts range 33 - both wider than the 30 and 23
    // measured when a ferrite bead and an Ethernet transformer were off the
    // board and out of the way.
    ("multi_ic.kicad_pcb", "multi_ic", 381, 210),
    // Measured 2026-08-08 at 81 / 33, plus the board's own band from
    // `via_price_sweep::how_much_of_the_price_is_noise`: 62 to 74 across
    // prices 0.22..0.28, 12 violations wide, and 27 to 34 shorts.
    ("shift_driver.kicad_pcb", "shift_driver", 93, 40),
    // 336 / 179, band 296 to 336 across prices 0.22..0.28 - 40 violations
    // wide. This fixture has now been corrected twice: its headers ran past
    // the board outline, and then two of the four collapsed onto the geometry
    // of the first because the importer keys its footprint library by library
    // name alone. Each correction moved the board the router sees, so each
    // needed its own measurement: 343 on the invalid board, 676 when the model
    // still ran two headers down one edge, 336 now that the file and the model
    // agree.
    ("qfp_fanout.kicad_pcb", "qfp_fanout", 376, 260),
    // The first fixture with a ground plane. Re-measured 2026-08-08 at 28 / 13
    // with 181 routes and nothing unrouted, after one of its decoupling
    // capacitors moved 1mm: C1 and C2 sat 3mm apart and an 0603 courtyard is
    // about 3mm wide, so they overlapped at exactly 0.00mm and the fixture was
    // not a board anybody could assemble. Moving it also moved the routing -
    // 210 routes became 181 and the count fell from 40.
    //
    // The band carried over is the one measured on the board before that move:
    // 9 violations wide and 5 shorts, from
    // `via_price_sweep::how_much_of_the_price_is_noise`. A 1mm move does not
    // change how sensitive a board is to the via price, but it was not
    // re-measured, and the ratchet only tightens either way.
    //
    // The vias on a 12-part board are the point of it: seven surface-mount
    // ground pads sit on F.Cu and the plane is on B.Cu, so each needs a
    // stitching via. No other fixture makes the router do that.
    ("plane_board.kicad_pcb", "plane_board", 37, 18),
];

/// Routes every fixture and holds the line on completeness and DRC count.
///
/// Ignored by default so `cargo test` stays quick; scripts/quality-gate.sh runs
/// it explicitly in the benchmark stage. About 100 seconds: the grid is a track
/// pitch rather than half a clearance, and repair routes each board three times.
#[test]
#[ignore = "slow: routes every fixture"]
fn benchmark_all_fixtures_drc() {
    let pathfinder = PathFinderStrategy;

    eprintln!();
    print_table_header();
    let mut measured = Vec::new();
    for (filename, label, _, _) in DRC_RATCHETS {
        let (score, route_count, unrouted) = route_and_score(&pathfinder, filename);
        print_table_row(&BenchmarkResult::from_score(
            label,
            "PathFinder",
            &score,
            route_count,
            unrouted,
        ));
        measured.push((
            label,
            score.drc_violations,
            score.shorts,
            unrouted,
            route_count,
        ));
    }
    print_table_footer();
    eprintln!();

    // Every fixture is measured and printed before anything fails. A test that
    // stops at the first bad row hides what the other boards did, and the
    // question these numbers answer - is this setting worth its cost - cannot
    // be read off one row.
    let mut failures: Vec<String> = Vec::new();

    for ((label, violations, shorts, unrouted, route_count), (_, _, ratchet, shorts_ratchet)) in
        measured.iter().zip(DRC_RATCHETS)
    {
        eprintln!(
            "  {}: {} routes, {} violations against {}, {} shorts against {}, {} unrouted",
            label, route_count, violations, ratchet, shorts, shorts_ratchet, unrouted
        );

        if *unrouted != 0 {
            failures.push(format!(
                "{label}: {unrouted} unrouted connections, threshold 0"
            ));
        }
        if *route_count == 0 {
            failures.push(format!("{label}: routed nothing at all"));
        }
        if violations > ratchet {
            failures.push(format!(
                "{label}: {violations} DRC violations, threshold {ratchet} - the router got worse"
            ));
        }
        if shorts > shorts_ratchet {
            failures.push(format!(
                "{label}: {shorts} of the violations are copper touching copper, threshold {shorts_ratchet} - the router started shorting the board"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "FAIL benchmark_all_fixtures_drc:\n  {}",
        failures.join("\n  ")
    );
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

    // Copper, not segment count. The threshold used to be 20 segments, which
    // was a stand-in for "the router did not quietly give up" - a job the
    // `unrouted` assertion above already does properly. It went off when the
    // router started reserving copper and solved the same board in 18 segments
    // instead of 23, with the same 79mm of copper: fewer corners is better,
    // and a gate that calls it a regression is measuring the wrong thing.
    let copper_mm = score.total_length.0 as f64 / 1_000_000.0;
    assert!(
        copper_mm >= 70.0,
        "FAIL benchmark_regression: {:.1}mm of copper, threshold >= 70.0mm - the router is emitting far less than a complete solution needs",
        copper_mm
    );
    eprintln!(
        "  ✓ copper: got {:.1}mm in {} segments, threshold >= 70.0mm",
        copper_mm, route_count
    );

    // Quality thresholds are ratchets measured against a complete solution.
    // They are deliberately tight: lower them whenever the router improves,
    // never raise them to accommodate a regression. R107 targets 0 violations.
    //
    // Raised once, on 2026-08-06, and not for a regression: `composite` charges
    // 1000 per DRC violation, and the checker started seeing one that was
    // always there. Until that day the same-net exemption was per component, so
    // a GND trace crossing a part's non-GND pad was waved through because the
    // part had a GND pin somewhere. The board is unchanged - 42.6 of quality
    // score plus one 1000-point short. Put this back to 100.0 the moment the
    // router stops driving through sibling pads; do not raise it again.
    assert!(
        score.composite <= 2100.0,
        "FAIL benchmark_regression: composite got {:.1}, threshold ≤ 2100.0 (baseline 42.6 plus two known shorts at 1000 each)",
        score.composite
    );
    eprintln!(
        "  ✓ composite: got {:.1}, threshold ≤ 2100.0",
        score.composite
    );

    // One, for the same reason the composite threshold moved: the per-pad
    // same-net exemption exposed a GND trace crossing a part's non-GND pad,
    // which the router has always produced and the checker used to excuse.
    // R107 still targets 0, and this is the gap to it.
    assert!(
        score.drc_violations <= 2,
        "FAIL benchmark_regression: drc_violations got {}, threshold 2 - the known short is reported per segment now, anything more is new",
        score.drc_violations
    );
    eprintln!(
        "  ✓ drc_violations: got {}, threshold 2 (R107 targets 0)",
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
    //
    // Matched case-insensitively because the names in this table come from
    // `StrategyKind`'s `Display` - `pathfinder`, `improved-astar` - and the
    // literals here said `PathFinder` and `ImprovedAStar`. The lookups found
    // nothing and the test died on `expect` before reaching the comparison it
    // exists for, which reads as a failing benchmark rather than a stale
    // string.
    let by_strategy = |name: &str| {
        results
            .iter()
            .find(|r| r.fixture == "led_blink" && r.strategy.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| {
                panic!(
                    "{name} led_blink result missing; the table holds {:?}",
                    results
                        .iter()
                        .map(|r| r.strategy.as_str())
                        .collect::<Vec<_>>()
                )
            })
    };
    let pf_led = by_strategy("pathfinder");
    let astar_led = by_strategy("improved-astar");

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
