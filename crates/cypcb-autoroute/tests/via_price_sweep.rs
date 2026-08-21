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
use cypcb_drc::{preset_for_world, ruleset_for_world, run_drc, DesignRules, ViolationKind};
use cypcb_kicad::{parse_kicad_pcb, BENCHMARKS};
use cypcb_router::apply_routes;
use cypcb_rules::presets::RulesPreset;

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
    // Zero is the router before the price existed; 2.0 is the point that
    // behaved like the veto. The three in between are the question.
    let prices = [0.0, 0.25, 0.5, 1.0, 2.0];

    for benchmark in BENCHMARKS {
        eprintln!();
        eprintln!("=== {} ===", benchmark.filename);
        let mut table_printed = false;

        for price in prices {
            let parsed = parse_kicad_pcb(&fixture_path(benchmark.filename))
                .unwrap_or_else(|e| panic!("Failed to parse {}: {:?}", benchmark.filename, e));
            let mut world = parsed.world;
            let library = parsed.library;

            // The table this board would actually be graded against, not a
            // fixed two-layer one: `multi_ic` has four copper layers and
            // `cypcb check` reads it against `jlcpcb_standard_4layer`. A band
            // measured on the wrong table is a band about a board nobody ships.
            let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, &world);
            let drc_rules = DesignRules::from_constraints(&preset.constraints());
            let rules = ruleset_for_world(preset, &world);
            if !table_printed {
                eprintln!("  graded on {}", preset.name());
                table_printed = true;
            }

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
    let prices = [0.22, 0.24, 0.25, 0.26, 0.28];

    for benchmark in BENCHMARKS
        .iter()
        .filter(|b| b.filename != "led_blink.kicad_pcb")
    {
        eprintln!();
        eprintln!("=== {} ===", benchmark.filename);
        let mut table: Option<&'static str> = None;

        let mut introduced_counts = Vec::new();
        let mut short_counts = Vec::new();
        let mut stacked_counts = Vec::new();
        for price in prices {
            let parsed = parse_kicad_pcb(&fixture_path(benchmark.filename))
                .unwrap_or_else(|e| panic!("Failed to parse {}: {:?}", benchmark.filename, e));
            let mut world = parsed.world;
            let library = parsed.library;

            // The table this board would actually be graded against, not a
            // fixed two-layer one: `multi_ic` has four copper layers and
            // `cypcb check` reads it against `jlcpcb_standard_4layer`. A band
            // measured on the wrong table is a band about a board nobody ships.
            let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, &world);
            let drc_rules = DesignRules::from_constraints(&preset.constraints());
            let rules = ruleset_for_world(preset, &world);
            if table.is_none() {
                eprintln!("  graded on {}", preset.name());
                table = Some(preset.name());
            }

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
            // Holes the router put on top of each other. The stacked-hole
            // price table in `docs/routing.md` is read against a band and
            // there was none for this column: every band this project has is
            // a spread of violations, and a conclusion about stacking cannot
            // be drawn from one.
            let stacked = introduced
                .iter()
                .filter(|v| v.kind == ViolationKind::HoleToHole)
                .count();

            eprintln!(
                "  price {:>4.2}: {:>4} introduced, {:>4} shorts, {:>3} stacked, {:>4} segments, {:>3} vias",
                price,
                introduced.len(),
                shorts,
                stacked,
                result.route_count(),
                result.via_count()
            );
            introduced_counts.push(introduced.len());
            short_counts.push(shorts);
            stacked_counts.push(stacked);
        }

        let lo = introduced_counts.iter().copied().min().unwrap_or(0);
        let hi = introduced_counts.iter().copied().max().unwrap_or(0);
        let slo = short_counts.iter().copied().min().unwrap_or(0);
        let shi = short_counts.iter().copied().max().unwrap_or(0);
        let klo = stacked_counts.iter().copied().min().unwrap_or(0);
        let khi = stacked_counts.iter().copied().max().unwrap_or(0);
        eprintln!(
            "  spread across prices 0.22..0.28: {} to {}, {} violations wide",
            lo,
            hi,
            hi - lo
        );
        // The band the probe in `is_the_best_variant_a_local_optimum` compares
        // a neighbour against is a pair, so this prints the pair. It used to
        // print only the violations half and the shorts half was worked out by
        // hand off the per-price lines above, which is a number with nowhere
        // to point when somebody asks where it came from.
        eprintln!(
            "  band for `is_the_best_variant_a_local_optimum`: ({}, {}) on {}",
            hi - lo,
            shi - slo,
            table.unwrap_or("no board")
        );
        eprintln!(
            "  stacked-hole band: {} ({} to {} across the five prices)",
            khi - klo,
            klo,
            khi
        );
    }
}
