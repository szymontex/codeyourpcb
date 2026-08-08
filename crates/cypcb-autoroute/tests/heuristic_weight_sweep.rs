//! What a search that stops looking for the best path buys, and what it costs.
//!
//! `cargo test --release -p cypcb-autoroute --test heuristic_weight_sweep -- --ignored --nocapture`
//!
//! A* explores widely because its estimate of the remaining distance never
//! overestimates: that is what makes the path it returns the cheapest one.
//! Multiplying the estimate makes the search believe the goal is further than
//! it is, so it follows the most promising direction harder and settles for a
//! path that may cost up to that factor more.
//!
//! Every routing knob in this project has been decided by routing all six
//! fixtures and reading the result against each board's own measured noise
//! band. This is that measurement for the heuristic weight.

use std::collections::BTreeSet;
use std::path::Path;

use cypcb_autoroute::{route_board, AutorouteConfig};
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

/// A violation's identity, so the ones the fixture arrived with can be told
/// from the ones the router made.
fn fingerprint(violation: &cypcb_drc::DrcViolation) -> String {
    format!(
        "{}|{}|{}|{}",
        violation.kind,
        violation.location.x.raw(),
        violation.location.y.raw(),
        violation.message
    )
}

#[test]
#[ignore = "diagnostic: routes every fixture at several heuristic weights"]
fn what_a_weighted_heuristic_buys() {
    let drc_rules = DesignRules::jlcpcb_2layer();

    // 1.0 is what ships and the only weight that keeps A* optimal. The rest
    // bracket the range anybody would consider: a tenth over, a quarter over,
    // and half again.
    let weights = [1.0, 1.1, 1.25, 1.5];

    for benchmark in BENCHMARKS {
        eprintln!();
        eprintln!("=== {} ===", benchmark.filename);

        for weight in weights {
            let parsed = parse_kicad_pcb(&fixture_path(benchmark.filename))
                .unwrap_or_else(|e| panic!("Failed to parse {}: {:?}", benchmark.filename, e));
            let mut world = parsed.world;
            let library = parsed.library;

            world.rebuild_spatial_index_from_library(&library);
            let baseline: BTreeSet<String> = run_drc(&mut world, &drc_rules)
                .violations
                .iter()
                .map(fingerprint)
                .collect();

            let config = AutorouteConfig {
                heuristic_weight: weight,
                ..AutorouteConfig::default()
            };
            let rules =
                PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset exists"));

            let started = std::time::Instant::now();
            let result = route_board(&mut world, &library, &rules, &config);
            let elapsed = started.elapsed();

            apply_routes(&mut world, &result);
            world.rebuild_spatial_index_from_library(&library);
            let drc = run_drc(&mut world, &drc_rules);

            let after = drc.violations.len();
            let introduced = drc
                .violations
                .iter()
                .filter(|v| !baseline.contains(&fingerprint(v)))
                .count();
            let shorts = drc
                .violations
                .iter()
                .filter(|v| v.actual == Some(cypcb_core::Nm::ZERO))
                .count();
            let unrouted = match result.status {
                cypcb_router::types::RoutingStatus::Partial { unrouted_count } => unrouted_count,
                _ => 0,
            };

            eprintln!(
                "  weight {:>4.2}: {:>4} after, {:>4} introduced, {:>4} shorts, {:>5} segments, {:>3} vias, {} unrouted, {:.2}s",
                weight,
                after,
                introduced,
                shorts,
                result.route_count(),
                result.via_count(),
                unrouted,
                elapsed.as_secs_f64()
            );
        }
    }
}
