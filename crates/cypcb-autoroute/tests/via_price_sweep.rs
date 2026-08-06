//! What a via should pay for the foreign copper inside its keepout.
//!
//! `cargo test --release -p cypcb-autoroute --test via_price_sweep -- --ignored --nocapture`
//!
//! The price shipped at 0.5 on two measured points - 2.0 behaved like the veto
//! it replaced, 0.5 improved every fixture on both columns. Two samples is a
//! guess with a number attached. This walks the range and reports violations,
//! shorts, copper and seconds per price, so the value is the one the boards
//! agree on rather than the one that was tried first.

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
#[ignore = "diagnostic: routes every fixture at several via prices"]
fn what_a_via_should_pay_for_crowding() {
    let drc_rules = DesignRules::jlcpcb_2layer();

    // Zero is the router before the price existed; 2.0 is the point that
    // behaved like the veto. The three in between are the question.
    let prices = [0.0, 0.25, 0.5, 1.0, 2.0];

    for benchmark in BENCHMARKS {
        eprintln!();
        eprintln!("=== {} ===", benchmark.filename);

        for price in prices {
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
                via_foreign_copper_penalty: price,
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

            let introduced: Vec<_> = drc
                .violations
                .iter()
                .filter(|v| !baseline.contains(&fingerprint(v)))
                .collect();
            let shorts = introduced
                .iter()
                .filter(|v| v.kind == ViolationKind::Clearance)
                .filter(|v| v.actual == Some(cypcb_core::Nm::ZERO))
                .count();

            let unrouted = match result.status {
                cypcb_router::types::RoutingStatus::Partial { unrouted_count } => unrouted_count,
                _ => 0,
            };

            eprintln!(
                "  price {:>4.2}: {:>4} introduced, {:>4} shorts, {:>4} segments, {:>3} vias, {:>6.0}mm, {} unrouted, {:.1}s",
                price,
                introduced.len(),
                shorts,
                result.route_count(),
                result.via_count(),
                result.total_length().to_mm(),
                unrouted,
                elapsed.as_secs_f64()
            );
        }
    }
}
