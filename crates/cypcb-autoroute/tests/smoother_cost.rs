//! What the smoother does to clearance.
//!
//! `cargo test -p cypcb-autoroute --test smoother_cost -- --ignored --nocapture`
//!
//! The grid reasons about clearance in whole cells and hands its paths to
//! `smooth_routes`, which replaces staircases with diagonals - moving copper
//! after the reasoning is over. Every aggregate measurement in this project so
//! far has been taken with smoothing on, so a violation the smoother creates
//! and one the search creates look the same. This routes each fixture both
//! ways.

use std::collections::BTreeSet;
use std::path::Path;

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::{preset_for_world, ruleset_for_world, run_drc, DesignRules, ViolationKind};
use cypcb_kicad::{parse_kicad_pcb, BENCHMARKS};
use cypcb_router::apply_routes;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::BoardWorld;

fn fixture_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// The fab table this board would actually be graded against, and the rule set
/// the router gets for it.
///
/// `multi_ic` has four copper layers, so a fixed two-layer table grades it as
/// a board nobody ships - `cypcb check` reads it against
/// `jlcpcb_standard_4layer`.
fn rules_for(world: &BoardWorld) -> (RulesPreset, PresetRuleSet, DesignRules) {
    let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, world);
    (
        preset,
        ruleset_for_world(preset, world),
        DesignRules::from_constraints(&preset.constraints()),
    )
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
#[ignore = "diagnostic: routes every fixture with and without the smoother"]
fn what_the_smoother_costs() {
    for benchmark in BENCHMARKS {
        eprintln!();
        eprintln!("=== {} ===", benchmark.filename);
        let mut table: Option<&'static str> = None;

        for smoothing in [true, false] {
            let parsed = parse_kicad_pcb(&fixture_path(benchmark.filename))
                .unwrap_or_else(|e| panic!("Failed to parse {}: {:?}", benchmark.filename, e));
            let mut world = parsed.world;
            let (preset, rules, drc_rules) = rules_for(&world);
            if table.is_none() {
                eprintln!("  graded on {}", preset.name());
                table = Some(preset.name());
            }
            let library = parsed.library;

            world.rebuild_spatial_index_from_library(&library);
            let baseline: BTreeSet<String> = run_drc(&mut world, &drc_rules)
                .violations
                .iter()
                .map(fingerprint)
                .collect();

            let config = AutorouteConfig {
                smoothing,
                ..AutorouteConfig::default()
            };

            let started = std::time::Instant::now();
            let result = route_board(&mut world, &library, &rules, &config);
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

            eprintln!(
                "  smoothing {:>3}: {:?}, {} routes, {} introduced ({} clearance), {:.1}s",
                if smoothing { "on" } else { "off" },
                result.status,
                result.route_count(),
                introduced.len(),
                clearance,
                elapsed.as_secs_f64()
            );
        }
    }
}
