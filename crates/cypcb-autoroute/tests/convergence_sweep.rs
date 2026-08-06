//! What the router buys by iterating longer.
//!
//! `cargo test -p cypcb-autoroute --test convergence_sweep -- --ignored --nocapture`
//!
//! PathFinder ends its runs with hundreds of overused cells - two nets sharing
//! one cell of copper - because a stagnation break stops the loop after three
//! iterations that fail to shrink the overused set. That break was added for
//! speed, and speed was measured; correctness was not. This routes each
//! fixture at several stagnation limits and reports overuse, the violations
//! the router introduces, and the seconds it takes.

use std::collections::BTreeSet;
use std::path::Path;

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::{run_drc, DesignRules, ViolationKind};
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
#[ignore = "diagnostic: routes every fixture at several stagnation limits"]
fn what_iterating_longer_buys() {
    let drc_rules = DesignRules::jlcpcb_2layer();

    // 3 is the shipped default. 0 disables the break entirely, leaving only
    // the hard cap of 50 iterations.
    let limits = [3u32, 6, 12, 0];

    for benchmark in BENCHMARKS {
        eprintln!();
        eprintln!("=== {} ===", benchmark.filename);

        for limit in limits {
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
                stagnation_limit: limit,
                ..AutorouteConfig::default()
            };

            let started = std::time::Instant::now();
            let result = route_board(&mut world, &library, &test_rules(), &config);
            let elapsed = started.elapsed();

            apply_routes(&mut world, &result);
            world.rebuild_spatial_index_from_library(&library);
            let drc = run_drc(&mut world, &drc_rules);

            let introduced: Vec<_> = drc
                .violations
                .iter()
                .filter(|v| !baseline.contains(&fingerprint(v)))
                .collect();
            let clearance = introduced
                .iter()
                .filter(|v| v.kind == ViolationKind::Clearance)
                .count();

            let label = if limit == 0 {
                "no break".to_string()
            } else {
                format!("limit {limit}")
            };

            eprintln!(
                "  {:>8}: {:?}, {} routes, {} introduced ({} clearance), {:.1}s",
                label,
                result.status,
                result.route_count(),
                introduced.len(),
                clearance,
                elapsed.as_secs_f64()
            );
        }
    }
}
