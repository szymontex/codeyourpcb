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

/// What the small board's short at a narrower opening actually is.
///
/// `cargo test --release -p cypcb-autoroute --test pad_zone_margin_sweep -- --ignored what_led_blink --nocapture`
///
/// The sweep says two cells is better than three on both dense boards and
/// worse on led_blink, which trades two near misses for one short. One board
/// with one fault is a thing to read, not a number to weigh: this prints both
/// boards' violations so the trade can be judged rather than summed.
#[test]
#[ignore = "diagnostic: names led_blink's violations at two pad openings"]
fn what_led_blink_trades_when_the_opening_narrows() {
    let drc_rules = DesignRules::jlcpcb_2layer();

    for margin in [3u16, 2] {
        let parsed = parse_kicad_pcb(&fixture_path("led_blink.kicad_pcb")).expect("the fixture");
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
        let result = route_board(&mut world, &library, &rules, &config);
        apply_routes(&mut world, &result);
        world.rebuild_spatial_index_from_library(&library);

        eprintln!();
        eprintln!(
            "=== margin {margin} cells, {} segments ===",
            result.route_count()
        );
        for violation in run_drc(&mut world, &drc_rules)
            .violations
            .iter()
            .filter(|v| !baseline.contains(&fingerprint(v)))
        {
            eprintln!(
                "  {} at ({:.3}mm, {:.3}mm): {}",
                violation.kind,
                violation.location.x.raw() as f64 / 1e6,
                violation.location.y.raw() as f64 / 1e6,
                violation.message
            );
        }
    }
}

/// Whether a dearer via-on-a-pad closes the one fault that keeps the narrower
/// opening out of the defaults.
///
/// `cargo test --release -p cypcb-autoroute --test pad_zone_margin_sweep -- --ignored does_a_dearer --nocapture`
///
/// At two cells led_blink gains `D1 <-> via 'GND': 0.00mm` - a via placed on a
/// part's pad. That is priced at 50 and evidently not dearly enough once the
/// opening narrows. This sweeps the price at the narrower opening on all three
/// fixtures: the fault has to go without the dense boards giving back what the
/// narrower opening won.
#[test]
#[ignore = "diagnostic: sweeps the via-on-a-pad price at the narrower opening"]
fn does_a_dearer_via_on_a_pad_close_the_last_fault() {
    let drc_rules = DesignRules::jlcpcb_2layer();

    for benchmark in BENCHMARKS {
        eprintln!();
        eprintln!("=== {} at margin 2 ===", benchmark.filename);

        for price in [50.0f64, 150.0, 400.0, 1000.0] {
            let parsed = parse_kicad_pcb(&fixture_path(benchmark.filename)).expect("the fixture");
            let mut world = parsed.world;
            let library = parsed.library;

            world.rebuild_spatial_index_from_library(&library);
            let baseline: BTreeSet<String> = run_drc(&mut world, &drc_rules)
                .violations
                .iter()
                .map(fingerprint)
                .collect();

            let config = AutorouteConfig {
                pad_zone_margin_cells: 2,
                pad_layer_change_penalty: price,
                ..AutorouteConfig::default()
            };
            let rules =
                PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset exists"));
            let result = route_board(&mut world, &library, &rules, &config);
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
                "  price {:>6.0}: {:>4} after, {:>4} introduced, {:>4} shorts, {:>3} vias, {} unrouted",
                price, after, introduced, shorts, result.via_count(), unrouted
            );
        }
    }
}

/// What a pad inside a via's keepout should cost.
///
/// `cargo test --release -p cypcb-autoroute --test pad_zone_margin_sweep -- --ignored what_a_pad_under --nocapture`
///
/// The first instrument in this vector that prices copper the search has never
/// been able to see: `foreign_cells_in_via_keepout` counted routed copper only,
/// so a via paid for landing its ring on another net's trace and nothing for
/// landing it on another net's pad. Charging pads the 0.25 a trace cell costs
/// was measured and lost, because the disc covers many more pad cells than
/// trace cells. This sweeps a price of its own, at both pad openings.
#[test]
#[ignore = "diagnostic: sweeps the price of a pad inside a via keepout"]
fn what_a_pad_under_a_via_should_cost() {
    let drc_rules = DesignRules::jlcpcb_2layer();

    for margin in [3u16, 2] {
        for benchmark in BENCHMARKS {
            eprintln!();
            eprintln!("=== {} at margin {margin} ===", benchmark.filename);

            for price in [0.0f64, 0.02, 0.05, 0.1] {
                let parsed =
                    parse_kicad_pcb(&fixture_path(benchmark.filename)).expect("the fixture");
                let mut world = parsed.world;
                let library = parsed.library;

                // `after` is what this table reads, because the two openings
                // change what the fixture's own faults look like; the
                // introduced count is in the sweep above.
                world.rebuild_spatial_index_from_library(&library);

                let config = AutorouteConfig {
                    pad_zone_margin_cells: margin,
                    via_foreign_pad_penalty: price,
                    ..AutorouteConfig::default()
                };
                let rules = PresetRuleSet::new(
                    RulesPreset::from_name("jlcpcb").expect("the preset exists"),
                );
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
                let unrouted = match result.status {
                    cypcb_router::types::RoutingStatus::Partial { unrouted_count } => {
                        unrouted_count
                    }
                    _ => 0,
                };

                eprintln!(
                    "  price {:>5.2}: {:>4} after, {:>4} shorts, {:>4} segments, {:>3} vias, {} unrouted",
                    price, after, shorts, result.route_count(), result.via_count(), unrouted
                );
            }
        }
    }
}
