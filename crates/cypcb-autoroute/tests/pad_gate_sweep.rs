//! Can the cost model pay for closing the pad-zone gate, and what does a via
//! ring cost when the search can finally see it?
//!
//! `cargo test -p cypcb-autoroute --test pad_gate_sweep -- --ignored --nocapture`
//!
//! Refusing a pad-zone cell that another net's copper already holds removes 20
//! of stm32_breakout's 26 copper-on-copper overlaps and costs detours: 858
//! segments become 1084, and the violations those extra segments cause more
//! than repay the ones removed. A detour is what the router chooses when
//! walking around an obstacle is cheaper than diving under it, so the price of
//! a via is the knob that decides it. This sweeps the gate against that price.

use std::collections::BTreeSet;
use std::path::Path;

use cypcb_autoroute::{route_board, AutorouteConfig, AutorouteParams};
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
#[ignore = "diagnostic: sweeps the pad-zone gate against the price of a via"]
fn what_a_priced_via_ring_buys() {
    for benchmark in BENCHMARKS {
        eprintln!();
        eprintln!("=== {} ===", benchmark.filename);
        let mut table: Option<&'static str> = None;

        for gate_closed in [false, true] {
            for ring_penalty in [0.0f64, 1.0, 3.0] {
                let via_cost = 1.0f64;
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
                    via_ring_penalty: ring_penalty,
                    pad_zone_blocks_foreign_copper: gate_closed,
                    params: AutorouteParams {
                        via_cost,
                        ..AutorouteParams::default()
                    },
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
                let count =
                    |kind: ViolationKind| introduced.iter().filter(|v| v.kind == kind).count();

                eprintln!(
                    "  gate {:>6}, ring {:>3}: {:?}, {} routes, {} introduced (clearance {}, edge {}, hole {}), {:.1}s",
                    if gate_closed { "closed" } else { "open" },
                    ring_penalty,
                    result.status,
                    result.route_count(),
                    introduced.len(),
                    count(ViolationKind::Clearance),
                    count(ViolationKind::EdgeClearance),
                    count(ViolationKind::HoleToHole),
                    elapsed.as_secs_f64()
                );
            }
        }
    }
}
