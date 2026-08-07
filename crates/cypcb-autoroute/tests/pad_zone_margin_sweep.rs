//! How wide the opening around a pad should be.
//!
//! `cargo test --release -p cypcb-autoroute --test pad_zone_margin_sweep -- --ignored --nocapture`
//!
//! A pad zone switches off every obstacle within its radius so a route can
//! reach the pad it is heading for. The radius has been the pad's own copper
//! plus a flat three cells since the first version, under a comment reading
//! "generous but safe" - 0.762mm on the 0.254mm grid the dense fixtures use,
//! which is wider than the gap between the two pads of an 0402.
//!
//! Narrowing the zone's *scope* to the connection's own two pads was measured
//! and reverted; this measures its *radius*, which is the other half of the
//! same suspicion.

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
#[ignore = "diagnostic: routes every fixture at several pad zone radii"]
fn what_a_pad_opening_should_cost() {
    let drc_rules = DesignRules::jlcpcb_2layer();

    // Zero is the pad's own copper and nothing else; three is what ships.
    let margins = [0u16, 1, 2, 3, 5];

    for benchmark in BENCHMARKS {
        eprintln!();
        eprintln!("=== {} ===", benchmark.filename);

        for margin in margins {
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
                pad_zone_margin_cells: margin,
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
                "  margin {:>2} cells: {:>4} after, {:>4} introduced, {:>4} shorts, {:>4} segments, {:>3} vias, {} unrouted, {:.1}s",
                margin,
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
