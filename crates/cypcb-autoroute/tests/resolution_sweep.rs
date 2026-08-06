//! What a finer routing grid buys, and what it costs.
//!
//! `cargo test -p cypcb-autoroute --test resolution_sweep -- --ignored --nocapture`
//!
//! The grid is 0.254mm per cell on the benchmark boards, against a 0.127mm
//! trace and a via ring 0.277mm across. A cell cannot hold the distinction the
//! checker measures, which is why every cell-level instrument tried against
//! the remaining violations has made the board worse. This measures the one
//! lever that addresses the cause: routes, violations and seconds per
//! resolution, on the same fixtures.

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
#[ignore = "diagnostic: routes every fixture at several grid resolutions"]
fn what_a_finer_grid_buys() {
    let drc_rules = DesignRules::jlcpcb_2layer();

    // The adaptive default, then the track pitch, then half of it. The
    // adaptive rule doubles the pitch on boards over 80mm, so on a large board
    // the middle entry answers whether that doubling costs quality; on a small
    // one it repeats the first, which is a free consistency check.
    let resolutions: [Option<i64>; 3] = [None, Some(254_000), Some(127_000)];

    // A quarter of the track pitch was tried and abandoned: stm32_breakout ran
    // for over nine minutes without finishing, against 20.5s at the default,
    // and was killed. Anything past this cap gets skipped out loud rather than
    // silently, because a table with a row quietly missing reads as a table
    // that was measured.
    const MAX_CELLS: i64 = 400_000;

    for benchmark in BENCHMARKS {
        eprintln!();
        eprintln!("=== {} ===", benchmark.filename);

        for resolution in resolutions {
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
                grid_resolution_nm: resolution,
                ..AutorouteConfig::default()
            };

            // Report the resolution the router will actually use, not the one
            // asked for. They were not the same until the adaptive rule stopped
            // scaling explicit values, and a table cannot be read if its labels
            // are aspirational.
            let (label, cells) = match world.board_info() {
                Some((size, _)) => {
                    let effective = config.resolve_adaptive_grid_resolution(
                        &test_rules(),
                        size.width.raw(),
                        size.height.raw(),
                    );
                    (
                        format!("{:.3}mm", effective as f64 / 1_000_000.0),
                        (size.width.raw() / effective) * (size.height.raw() / effective),
                    )
                }
                None => ("no board".to_string(), 0),
            };

            if cells > MAX_CELLS {
                eprintln!(
                    "  {:>8}: skipped, {} cells is past the {} cap",
                    label, cells, MAX_CELLS
                );
                continue;
            }

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
            let clearance = introduced
                .iter()
                .filter(|v| v.kind == ViolationKind::Clearance)
                .count();

            eprintln!(
                "  {:>8}: {:?}, {} routes, {} introduced ({} clearance), {:.1}s",
                label,
                result.status,
                result.route_count(),
                introduced.len(),
                clearance,
                elapsed.as_secs_f64()
            );
        }
    }
}
