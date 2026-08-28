//! What shape a pad should block on the routing grid.
//!
//! `cargo test --release -p cypcb-autoroute --test pad_obstacle_shape_sweep -- --ignored --nocapture`
//!
//! `RoutingGrid` marks a **disc** of the pad's longer half-side. K011 and
//! D-DRC-002 both say a rotated pad's bounds are a rotated rectangle, and the
//! checker has obeyed that since it was written - so on a 2.0mm by 0.6mm pad
//! the grid blocks 0.7mm of empty board on each long side that no rule asks
//! it to.
//!
//! Closing that on its own was measured on 2026-08-28 and refused: two
//! fixtures improved and two started shorting. What the disc buys is margin
//! the cost model does not ask for, so this sweeps the rectangle **with** a
//! margin - `extra` cells beyond the clearance - to find whether any figure
//! keeps the gains without the shorts.

use std::collections::BTreeSet;
use std::path::Path;

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::{preset_for_world, ruleset_for_world, run_drc, DesignRules};
use cypcb_kicad::{parse_kicad_pcb, BENCHMARKS};
use cypcb_router::apply_routes;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::BoardWorld;

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

/// The table this board would be graded against, as every sweep here does it.
fn rules_for(world: &BoardWorld) -> (RulesPreset, PresetRuleSet, DesignRules) {
    let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, world);
    (
        preset,
        ruleset_for_world(preset, world),
        DesignRules::from_constraints(&preset.constraints()),
    )
}

#[test]
#[ignore = "diagnostic: routes every fixture at several pad obstacle shapes"]
fn what_shape_a_pad_should_block() {
    // `None` is the disc that ships; the rest are the pad's own rectangle with
    // that many cells of reach beyond the clearance.
    let shapes: [Option<u16>; 5] = [None, Some(0), Some(1), Some(2), Some(3)];

    for benchmark in BENCHMARKS {
        eprintln!();
        eprintln!("=== [pad shape] {} ===", benchmark.filename);

        for shape in shapes {
            let parsed = parse_kicad_pcb(&fixture_path(benchmark.filename))
                .unwrap_or_else(|e| panic!("Failed to parse {}: {:?}", benchmark.filename, e));
            let mut world = parsed.world;
            let library = parsed.library;
            let (_preset, rules, drc_rules) = rules_for(&world);

            world.rebuild_spatial_index_from_library(&library);
            let baseline: BTreeSet<String> = run_drc(&mut world, &drc_rules)
                .violations
                .iter()
                .map(fingerprint)
                .collect();

            let config = AutorouteConfig {
                pad_rect_extra_cells: shape,
                ..AutorouteConfig::default()
            };

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

            let name = match shape {
                None => "disc     ".to_string(),
                Some(extra) => format!("rect +{extra}   "),
            };
            eprintln!(
                "  {}: {:>4} after, {:>4} introduced, {:>4} shorts, {:>4} segments, {:>3} vias, {} unrouted, {:.1}s",
                name,
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
