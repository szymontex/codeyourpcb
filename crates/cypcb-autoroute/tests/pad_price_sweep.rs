//! What another net's pad should cost to walk across.
//!
//! `cargo test --release -p cypcb-autoroute --test pad_price_sweep -- --ignored --nocapture`
//!
//! The price shipped at 20 as a variant on one measured point, and one point
//! is a guess with a number attached - the same mistake the via price made
//! before it was swept. This walks the range on all three fixtures.
//!
//! A depth-weighted version of the price was built and measured here too -
//! full on the pad's copper, tapering to nothing at the outer edge of its
//! clearance, on the theory that a flat price charges a short and a near miss
//! the same. It was reverted: at the same prices it is better on
//! stm32_breakout at 5 and 50, worse at 20, and worse on multi_ic everywhere
//! (267 -> 413 after at price 20). The numbers are in the tracker.

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
#[ignore = "diagnostic: routes every fixture at several pad prices"]
fn what_a_foreign_pad_should_cost() {
    let drc_rules = DesignRules::jlcpcb_2layer();

    // Zero is the shipped default - the router without the price at all. 20 is
    // the value the flat version shipped at, and the rest bracket it.
    let prices = [0.0, 5.0, 20.0, 50.0, 100.0];

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
                foreign_pad_penalty: price,
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

            // Both readings, because the gate holds one and this vector's
            // notes are written in the other.
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
                "  price {:>5.1}: {:>4} after, {:>4} introduced, {:>4} shorts, {:>4} segments, {:>3} vias, {} unrouted, {:.1}s",
                price,
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

/// How much of the difference between two pad prices is the price.
///
/// `cargo test --release -p cypcb-autoroute --test pad_price_sweep -- --ignored how_much --nocapture`
///
/// multi_ic reads 257 after at price 5 and 267 at 20, which is the size of
/// difference this vector has twice mistaken for a result. Prices a unit apart
/// ask the router for almost the same trade, so whatever they differ by is
/// what negotiated congestion does on its own.
#[test]
#[ignore = "diagnostic: routes multi_ic at pad prices a unit apart"]
fn how_much_of_the_pad_price_is_noise() {
    let drc_rules = DesignRules::jlcpcb_2layer();
    let prices = [4.0, 5.0, 6.0, 7.0];
    let filename = "multi_ic.kicad_pcb";

    let mut afters: Vec<usize> = Vec::new();
    let mut shorts_seen: Vec<usize> = Vec::new();

    for price in prices {
        let parsed = parse_kicad_pcb(&fixture_path(filename))
            .unwrap_or_else(|e| panic!("Failed to parse {}: {:?}", filename, e));
        let mut world = parsed.world;
        let library = parsed.library;

        world.rebuild_spatial_index_from_library(&library);

        let config = AutorouteConfig {
            foreign_pad_penalty: price,
            ..AutorouteConfig::default()
        };
        let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset exists"));
        let result = route_board(&mut world, &library, &rules, &config);

        apply_routes(&mut world, &result);
        world.rebuild_spatial_index_from_library(&library);
        let drc = run_drc(&mut world, &drc_rules);

        let after = drc.violations.len();
        let shorts = drc
            .violations
            .iter()
            .filter(|v| v.actual == Some(cypcb_core::Nm::ZERO))
            .count();
        afters.push(after);
        shorts_seen.push(shorts);

        eprintln!("  price {price:>5.1}: {after:>4} after, {shorts:>4} shorts");
    }

    let spread = afters.iter().max().unwrap() - afters.iter().min().unwrap();
    let short_spread = shorts_seen.iter().max().unwrap() - shorts_seen.iter().min().unwrap();
    eprintln!(
        "  spread across prices 4..7: {} violations, {} shorts",
        spread, short_spread
    );
}
