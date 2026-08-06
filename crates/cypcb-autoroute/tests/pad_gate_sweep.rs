//! Can the cost model pay for closing the pad-zone gate?
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
#[ignore = "diagnostic: sweeps the pad-zone gate against the price of a via"]
fn what_a_cheaper_via_buys_the_closed_gate() {
    let drc_rules = DesignRules::jlcpcb_2layer();

    for benchmark in BENCHMARKS {
        eprintln!();
        eprintln!("=== {} ===", benchmark.filename);

        for gate_closed in [false, true] {
            for via_cost in [0.25f64, 0.5, 1.0] {
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
                    pad_zone_blocks_foreign_copper: gate_closed,
                    params: AutorouteParams {
                        via_cost,
                        ..AutorouteParams::default()
                    },
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
                let count =
                    |kind: ViolationKind| introduced.iter().filter(|v| v.kind == kind).count();

                eprintln!(
                    "  gate {:>6}, via {:>4}: {:?}, {} routes, {} introduced (clearance {}, edge {}, hole {}), {:.1}s",
                    if gate_closed { "closed" } else { "open" },
                    via_cost,
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
