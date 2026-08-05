//! DRC diagnostic for routed benchmark boards.
//!
//! `cargo test --release -p cypcb-autoroute --test drc_report -- --ignored --nocapture`
//!
//! Routes each benchmark fixture and prints every DRC violation the routed
//! board produces, grouped by kind and listed with board coordinates. The
//! score-based tests report a single number; this one says what the number is
//! made of, which is what R107 (zero violations from the router) needs.

use std::collections::BTreeMap;
use std::path::Path;

use cypcb_autoroute::pathfinder_v2::PathFinderStrategy;
use cypcb_autoroute::strategy::RoutingStrategy;
use cypcb_autoroute::AutorouteConfig;
use cypcb_drc::{run_drc, DesignRules};
use cypcb_kicad::{parse_kicad_pcb, BENCHMARKS};
use cypcb_router::apply_routes;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};

fn fixture_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

fn test_rules() -> PresetRuleSet {
    PresetRuleSet::new(RulesPreset::from_name("jlcpcb").unwrap())
}

/// Print every DRC violation of every routed benchmark fixture.
#[test]
#[ignore = "diagnostic: prints the DRC violations behind the benchmark scores"]
fn report_drc_violations_per_fixture() {
    let strategy = PathFinderStrategy;
    let drc_rules = DesignRules::jlcpcb_2layer();

    for benchmark in BENCHMARKS {
        let parsed = parse_kicad_pcb(&fixture_path(benchmark.filename))
            .unwrap_or_else(|e| panic!("Failed to parse {}: {:?}", benchmark.filename, e));
        let mut world = parsed.world;
        let library = parsed.library;

        let result = strategy.route(
            &mut world,
            &library,
            &test_rules(),
            &AutorouteConfig::default(),
        );
        apply_routes(&mut world, &result);
        world.rebuild_spatial_index_from_library(&library);

        let drc = run_drc(&mut world, &drc_rules);

        eprintln!();
        eprintln!(
            "=== {} — {:?}, {} routes, {} violations ===",
            benchmark.filename,
            result.status,
            result.route_count(),
            drc.violations.len()
        );

        let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
        for violation in &drc.violations {
            *by_kind.entry(violation.kind.to_string()).or_insert(0) += 1;
            eprintln!(
                "  {:<20} ({:>8.3}mm, {:>8.3}mm)  {}",
                violation.kind.to_string(),
                violation.location.x.to_mm(),
                violation.location.y.to_mm(),
                violation.message
            );
        }

        eprintln!("  ---");
        for (kind, count) in &by_kind {
            eprintln!("  {:<20} {}", kind, count);
        }
    }
}
