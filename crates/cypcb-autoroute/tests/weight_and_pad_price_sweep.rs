//! The two knobs that pay, swept together.
//!
//! `cargo test --release -p cypcb-autoroute --test weight_and_pad_price_sweep -- --ignored --nocapture`
//!
//! `foreign_pad_penalty` and `heuristic_weight` were each measured alone -
//! `pad_price_sweep` and `heuristic_weight_sweep` - and each shipped as a
//! variant on that evidence. Neither sweep says anything about the two
//! together, and they are not independent: the pad price changes what a path
//! costs, and the weight changes how hard the search looks for the cheap one.
//!
//! Twelve points a board, six boards. Every routing knob in this project is
//! decided by reading the result against each board's own noise band, and the
//! ranking that picks a variant puts abandoned connections first, then shorts,
//! then the composite - so the last column here is the one that decides.

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

/// One point of the sweep.
struct Point {
    weight: f64,
    price: f64,
    violations: usize,
    shorts: usize,
    unrouted: usize,
    seconds: f64,
}

#[test]
#[ignore = "diagnostic: routes every fixture at twelve knob combinations"]
fn the_pad_price_and_the_heuristic_weight_together() {
    let drc_rules = DesignRules::jlcpcb_2layer();

    // The weight that shipped as `PathFinder Eager`, the one that keeps A*
    // optimal, and one either side. The prices are the shipped default, the
    // one `PathFinder Pad Aware` uses, and the value it used to ship at.
    let weights = [1.0, 1.1, 1.25, 1.5];
    let prices = [0.0, 5.0, 20.0];

    for benchmark in BENCHMARKS {
        eprintln!();
        eprintln!("=== {} ===", benchmark.filename);
        let mut points: Vec<Point> = Vec::new();

        for weight in weights {
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
                    heuristic_weight: weight,
                    foreign_pad_penalty: price,
                    ..AutorouteConfig::default()
                };
                let rules = PresetRuleSet::new(
                    RulesPreset::from_name("jlcpcb").expect("the preset exists"),
                );

                let started = std::time::Instant::now();
                let result = route_board(&mut world, &library, &rules, &config);
                let seconds = started.elapsed().as_secs_f64();

                apply_routes(&mut world, &result);
                world.rebuild_spatial_index_from_library(&library);
                let drc = run_drc(&mut world, &drc_rules);

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
                    cypcb_router::types::RoutingStatus::Partial { unrouted_count } => {
                        unrouted_count
                    }
                    _ => 0,
                };

                eprintln!(
                    "  weight {:>4.2} price {:>5.1}: {:>4} after, {:>4} introduced, {:>4} shorts, {:>5} segments, {:>3} vias, {} unrouted, {:.2}s",
                    weight,
                    price,
                    drc.violations.len(),
                    introduced,
                    shorts,
                    result.route_count(),
                    result.via_count(),
                    unrouted,
                    seconds
                );

                points.push(Point {
                    weight,
                    price,
                    violations: drc.violations.len(),
                    shorts,
                    unrouted,
                    seconds,
                });
            }
        }

        // The same order the variant ranking uses: a board that gave up loses
        // whatever it scores, then copper touching copper, then the total.
        points.sort_by(|a, b| {
            a.unrouted
                .cmp(&b.unrouted)
                .then_with(|| a.shorts.cmp(&b.shorts))
                .then_with(|| a.violations.cmp(&b.violations))
        });
        let best = &points[0];
        eprintln!(
            "  -> best: weight {:.2} price {:.1} with {} violations, {} shorts, {} unrouted, {:.2}s",
            best.weight, best.price, best.violations, best.shorts, best.unrouted, best.seconds
        );
    }
}
