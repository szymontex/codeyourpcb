//! Does the router's grid know about the copper the checker complains about?
//!
//! `cargo test -p cypcb-autoroute --test grid_vs_checker -- --ignored --nocapture`
//!
//! A router and a checker that disagree about where copper is cannot converge:
//! the router will keep producing boards the checker rejects, and no amount of
//! cost tuning fixes it. This walks every clearance violation the router
//! introduces, maps its coordinate onto the routing grid the router used, and
//! reports what that cell was marked as.
//!
//! Two outcomes, two different bugs. If the cell was marked `CELL_PAD` and a
//! route still ran through it, the search ignores its own obstacles. If it was
//! free, the marking geometry is wrong and the search was never told.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cypcb_autoroute::grid::{layer_to_index, RoutingGrid, CELL_PAD, CELL_TRACE, CELL_VIA};
use cypcb_autoroute::pathfinder_v2::PathFinderStrategy;
use cypcb_autoroute::strategy::StrategyKind;
use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::{run_drc, DesignRules, ViolationKind};
use cypcb_kicad::{parse_kicad_pcb, BENCHMARKS};
use cypcb_router::apply_routes;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::components::Layer;

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

/// What kind of feature met what kind, from the violation's own wording.
///
/// The message names each side: `trace 'GND'`, `via 'VCC'`, or a refdes for a
/// component. Which two kinds collide says which part of the router to look
/// at - the path search, the via placer, or the pad marking.
fn shape(message: &str) -> String {
    let side = |text: &str| {
        if text.trim_start().starts_with("trace ") {
            "trace"
        } else if text.trim_start().starts_with("via ") {
            "via"
        } else {
            "part"
        }
    };

    let pair = message.split(':').next().unwrap_or(message);
    match pair.split_once('↔') {
        Some((a, b)) => {
            let (mut first, mut second) = (side(a), side(b));
            if first > second {
                std::mem::swap(&mut first, &mut second);
            }
            format!("{first} <-> {second}")
        }
        None => "unknown".to_string(),
    }
}

/// Name the cell's marking the way the grid means it.
fn describe(cell: u8) -> &'static str {
    if cell & CELL_PAD != 0 {
        "pad"
    } else if cell & CELL_VIA != 0 {
        "via"
    } else if cell & CELL_TRACE != 0 {
        "trace"
    } else if cell == 0 {
        "free"
    } else {
        "other"
    }
}

#[test]
#[ignore = "diagnostic: compares the router's obstacle grid against DRC output"]
fn what_the_grid_thought_was_there() {
    let drc_rules = DesignRules::jlcpcb_2layer();

    for (strategy, label) in [
        (StrategyKind::PathFinder, "pathfinder"),
        (StrategyKind::ImprovedAStar, "improved-astar"),
    ] {
        let config = AutorouteConfig {
            strategy,
            ..AutorouteConfig::default()
        };
        for benchmark in BENCHMARKS {
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

            // The grid the router will build, built the same way, before routing
            // changes the world underneath it.
            let resolution = PathFinderStrategy::resolution_for(&mut world, &test_rules(), &config);
            let grid = RoutingGrid::from_board(&mut world, &library, &test_rules(), resolution)
                .expect("a board to build a grid from");

            let result = route_board(&mut world, &library, &test_rules(), &config);
            apply_routes(&mut world, &result);
            world.rebuild_spatial_index_from_library(&library);

            let drc = run_drc(&mut world, &drc_rules);

            let mut by_marking: BTreeMap<&'static str, usize> = BTreeMap::new();
            let mut by_shape: BTreeMap<String, usize> = BTreeMap::new();
            let mut samples: Vec<String> = Vec::new();

            for violation in &drc.violations {
                if violation.kind != ViolationKind::Clearance
                    || baseline.contains(&fingerprint(violation))
                {
                    continue;
                }

                let (gx, gy) = grid.nm_to_grid(violation.location);

                // The violation does not say which layer it is on, so take the
                // strongest marking across the copper layers: if any layer knew
                // there was a pad here, the router had been told.
                let mut best = 0u8;
                for layer in [Layer::TopCopper, Layer::BottomCopper] {
                    if let Some(index) = layer_to_index(layer) {
                        if index < grid.layer_count() as usize {
                            best |= grid.cell(gx, gy, index);
                        }
                    }
                }

                let marking = describe(best);
                *by_marking.entry(marking).or_insert(0) += 1;
                *by_shape
                    .entry(format!(
                        "{} on a {} cell",
                        shape(&violation.message),
                        marking
                    ))
                    .or_insert(0) += 1;

                if samples.len() < 5 {
                    samples.push(format!(
                        "  {} at ({:.3}mm, {:.3}mm) -> cell ({}, {}) marked {}",
                        violation.message,
                        violation.location.x.to_mm(),
                        violation.location.y.to_mm(),
                        gx,
                        gy,
                        marking
                    ));
                }
            }

            eprintln!();
            eprintln!(
                "=== {label} on {} - grid {}x{} at {:.3}mm per cell ===",
                benchmark.filename,
                grid.width(),
                grid.height(),
                grid.resolution() as f64 / 1_000_000.0
            );
            for (marking, count) in &by_marking {
                eprintln!("  introduced clearance violations on a {marking} cell: {count}");
            }
            for (shape, count) in &by_shape {
                eprintln!("  {shape}: {count}");
            }
            for sample in &samples {
                eprintln!("{sample}");
            }
        }
    }
}
