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

/// How much of the difference between two prices is the price.
///
/// `cargo test --release -p cypcb-autoroute --test via_price_sweep -- --ignored how_much --nocapture`
///
/// stm32_breakout moved 121 -> 159 -> 138 introduced violations across 0.25,
/// 0.5 and 1.0, which is not the shape of a knob doing one thing. Prices a
/// fraction apart are the control: 0.24 and 0.26 ask the router for almost
/// exactly the same trade, so whatever they differ by is what negotiated
/// congestion does on its own. A tuning value chosen inside that spread is
/// noise with a decimal point.
#[test]
#[ignore = "diagnostic: routes the dense fixtures at prices a hair apart"]
fn how_much_of_the_price_is_noise() {
    let drc_rules = DesignRules::jlcpcb_2layer();
    let prices = [0.22, 0.24, 0.25, 0.26, 0.28];

    for benchmark in BENCHMARKS
        .iter()
        .filter(|b| b.filename != "led_blink.kicad_pcb")
    {
        eprintln!();
        eprintln!("=== {} ===", benchmark.filename);

        let mut introduced_counts = Vec::new();
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
            let result = route_board(&mut world, &library, &rules, &config);

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

            eprintln!(
                "  price {:>4.2}: {:>4} introduced, {:>4} shorts, {:>4} segments, {:>3} vias",
                price,
                introduced.len(),
                shorts,
                result.route_count(),
                result.via_count()
            );
            introduced_counts.push(introduced.len());
        }

        let lo = introduced_counts.iter().copied().min().unwrap_or(0);
        let hi = introduced_counts.iter().copied().max().unwrap_or(0);
        eprintln!(
            "  spread across prices 0.22..0.28: {} to {}, {} violations wide",
            lo,
            hi,
            hi - lo
        );
    }
}
