//! What copper closer than the fab allows should cost the search.
//!
//! `cargo test --release -p cypcb-autoroute --test clearance_barrier_sweep -- --ignored --nocapture`
//!
//! Step 4 of `docs/router-plan.md`, and the first instrument in this vector to
//! charge the search for the thing the checker measures rather than for a
//! proxy. A node closer to copper than `min_clearance + min_trace_width / 2`
//! pays `k * (short / required)^2`, zero in the legal region and unbounded at
//! contact.
//!
//! The plan's acceptance bar is stricter than a total and is what this output
//! is read against: a value counts only when it moves a board **outside that
//! board's own noise band**, and a total across boards is not a criterion -
//! the weighted-heuristic sweep looked like a 96-violation win on the total
//! and was noise on five of six boards plus one real regression.
//!
//! Each board's band, from `via_price_sweep::how_much_of_the_price_is_noise`,
//! printed beside its rows so the reading needs no second file:
//!
//! | board | band, violations / shorts |
//! |---|---|
//! | `led_blink` | 0 / 0 |
//! | `stm32_breakout` | 59 / 61 |
//! | `multi_ic` | 65 / 56 |
//! | `shift_driver` | 17 / 8 |
//! | `qfp_fanout` | 57 / 44 |
//! | `plane_board` | 0 / 0 |
//!
//! `led_blink` and `plane_board` have a band of zero, so any movement on
//! either is signal in both directions. They are where step 2 showed its
//! result and where every earlier experiment in this vector hid.

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

/// Each board's measured noise band, violations and shorts.
fn band(filename: &str) -> (i64, i64) {
    match filename {
        "led_blink.kicad_pcb" => (0, 0),
        "stm32_breakout.kicad_pcb" => (59, 61),
        "multi_ic.kicad_pcb" => (65, 56),
        "shift_driver.kicad_pcb" => (17, 8),
        "qfp_fanout.kicad_pcb" => (57, 44),
        "plane_board.kicad_pcb" => (0, 0),
        _ => (0, 0),
    }
}

#[test]
#[ignore = "diagnostic: routes every fixture at several barrier prices"]
fn what_the_clearance_barrier_should_cost() {
    let drc_rules = DesignRules::jlcpcb_2layer();

    // Zero is the shipped router, and is the row every other row is read
    // against. The rest span two orders of magnitude, because nothing in this
    // project has measured what a price on the checker's own metric is worth
    // and a narrow range would only prove the range was narrow.
    let prices = [0.0, 1.0, 10.0, 100.0];

    for benchmark in BENCHMARKS {
        let (band_violations, band_shorts) = band(benchmark.filename);
        eprintln!();
        eprintln!(
            "=== {} (band {} / {}) ===",
            benchmark.filename, band_violations, band_shorts
        );

        let mut baseline_row: Option<(i64, i64)> = None;

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
                clearance_barrier: price,
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

            let (v, s) = (introduced.len() as i64, shorts as i64);
            if baseline_row.is_none() {
                baseline_row = Some((v, s));
            }
            // Read against the board's own band rather than against the other
            // rows: a difference smaller than the band is the negotiation
            // going differently, not the price doing anything.
            let verdict = match baseline_row {
                Some((bv, bs)) if price > 0.0 => {
                    let dv = v - bv;
                    let ds = s - bs;
                    let outside = dv.abs() > band_violations || ds.abs() > band_shorts;
                    if !outside {
                        "inside its band".to_string()
                    } else if dv <= 0 && ds <= 0 {
                        format!("OUTSIDE, better ({dv:+} / {ds:+})")
                    } else if dv >= 0 && ds >= 0 {
                        format!("OUTSIDE, worse ({dv:+} / {ds:+})")
                    } else {
                        format!("OUTSIDE, mixed ({dv:+} / {ds:+})")
                    }
                }
                _ => "baseline".to_string(),
            };

            eprintln!(
                "  k {:>6.1}: {:>4} introduced, {:>4} shorts, {:>4} segments, {:>3} vias, \
                 {} unrouted, {:.1}s  {}",
                price,
                v,
                s,
                result.route_count(),
                result.via_count(),
                unrouted,
                elapsed.as_secs_f64(),
                verdict
            );
        }
    }

    eprintln!();
    eprintln!("Read the verdict column, not the totals. A price earns the default");
    eprintln!("only by moving a board outside its own band the right way while");
    eprintln!("moving none of them outside it the wrong way.");
}
