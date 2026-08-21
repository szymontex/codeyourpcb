//! DRC diagnostic for routed benchmark boards.
//!
//! `cargo test --release -p cypcb-autoroute --test drc_report -- --ignored --nocapture`
//!
//! Routes each benchmark fixture and prints every DRC violation the routed
//! board produces, grouped by kind and listed with board coordinates. The
//! score-based tests report a single number; this one says what the number is
//! made of, which is what R107 (zero violations from the router) needs.
//!
//! It runs DRC twice: once on the fixture as imported, and once after routing.
//! The benchmark fixtures are KiCad boards with parts already placed, and a
//! placement the router never touches can violate the rules on its own - two
//! components overlapping, a part sitting off the board edge. Charging those to
//! the autorouter makes its score unreadable, so only the violations that
//! appear after routing are listed.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::{preset_for_world, ruleset_for_world, run_drc, DesignRules, ViolationKind};
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

/// The fab table this board would actually be graded against, and the rule set
/// the router gets for it.
///
/// `multi_ic` has four copper layers, so `cypcb check` reads it against
/// `jlcpcb_standard_4layer`. A fixed two-layer table reports on a board nobody
/// ships, and this file's output is what `docs/routing.md` quotes.
fn rules_for(world: &BoardWorld) -> (RulesPreset, PresetRuleSet, DesignRules) {
    let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, world);
    (
        preset,
        ruleset_for_world(preset, world),
        DesignRules::from_constraints(&preset.constraints()),
    )
}

/// A clearance violation with nothing between the two pieces of copper.
///
/// The table in `docs/routing.md` carries a shorts column that this file never
/// printed, so the figure had to come from somewhere else or from counting the
/// listing by hand. It is printed now, beside the numbers it belongs with.
fn shorts(violations: &[&cypcb_drc::DrcViolation]) -> usize {
    violations
        .iter()
        .filter(|v| v.kind == ViolationKind::Clearance)
        .filter(|v| v.actual == Some(cypcb_core::Nm::ZERO))
        .count()
}

/// Identify a violation well enough to recognise it across two DRC runs.
///
/// Kind, location and message: entity ids change when routing spawns traces,
/// so they cannot be part of it.
fn fingerprint(violation: &cypcb_drc::DrcViolation) -> String {
    format!(
        "{}|{}|{}|{}",
        violation.kind,
        violation.location.x.raw(),
        violation.location.y.raw(),
        violation.message
    )
}

/// Print every DRC violation of every routed benchmark fixture.
#[test]
#[ignore = "diagnostic: prints the DRC violations behind the benchmark scores"]
fn report_drc_violations_per_fixture() {
    // Both settings, because the difference between them is the question this
    // file was written for. On today's router reserving a trace's copper wins
    // introduced violations on all six boards; `shift_driver` is the only one
    // that trades, 30 fewer violations for 7 more shorts.
    //
    // The second row used to set `reserve_trace_footprint: true`, which has
    // been the default since it shipped, so the file ran the same config twice
    // and printed two identical halves under two different labels. It is the
    // *off* case now, which is the comparison the paragraph above describes.
    let configs = [
        ("default", AutorouteConfig::default()),
        (
            "no reserved copper",
            AutorouteConfig {
                reserve_trace_footprint: false,
                ..AutorouteConfig::default()
            },
        ),
    ];

    for (label, config) in &configs {
        for benchmark in BENCHMARKS {
            let parsed = parse_kicad_pcb(&fixture_path(benchmark.filename))
                .unwrap_or_else(|e| panic!("Failed to parse {}: {:?}", benchmark.filename, e));
            let mut world = parsed.world;
            let library = parsed.library;
            let (preset, rules, drc_rules) = rules_for(&world);

            // What the fixture violates before the router has done anything.
            world.rebuild_spatial_index_from_library(&library);
            let before = run_drc(&mut world, &drc_rules);
            let mut before_by_kind: BTreeMap<String, usize> = BTreeMap::new();
            for violation in &before.violations {
                *before_by_kind
                    .entry(violation.kind.to_string())
                    .or_insert(0) += 1;
            }
            let baseline: BTreeSet<String> = before.violations.iter().map(fingerprint).collect();

            let result = route_board(&mut world, &library, &rules, config);
            apply_routes(&mut world, &result);
            world.rebuild_spatial_index_from_library(&library);

            let drc = run_drc(&mut world, &drc_rules);

            // A violation the fixture already had is not the router's doing.
            let introduced: Vec<_> = drc
                .violations
                .iter()
                .filter(|violation| !baseline.contains(&fingerprint(violation)))
                .collect();

            eprintln!();
            eprintln!(
            "=== {} [{}] on {} - {:?}, {} routes, {} violations before routing, {} after, {} introduced, {} shorts ===",
            benchmark.filename,
            label,
            preset.name(),
            result.status,
            result.route_count(),
            baseline.len(),
            drc.violations.len(),
            introduced.len(),
            shorts(&introduced)
        );

            for (kind, count) in &before_by_kind {
                eprintln!("  before  {:<20} {}", kind, count);
            }

            let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
            for violation in introduced {
                *by_kind.entry(violation.kind.to_string()).or_insert(0) += 1;
                eprintln!(
                    "  {:<20} ({:>8.3}mm, {:>8.3}mm)  {}",
                    violation.kind.to_string(),
                    violation.location.x.to_mm(),
                    violation.location.y.to_mm(),
                    violation.message
                );
            }

            eprintln!("  ---");
            for (kind, count) in &by_kind {
                eprintln!("  {:<20} {}", kind, count);
            }
        }
    }
}
