//! What charging for a via's own ring buys, and what it costs.
//!
//! `cargo test --release -p cypcb-autoroute --test what_a_via_ring_should_cost -- --ignored --nocapture`
//!
//! Routing leaves holes stacked on each other - 4 hole-to-hole violations on
//! `stm32_breakout`, 7 on `multi_ic`, 15 on `qfp_fanout`, most of them a net
//! against itself at 0.00mm. Filtering them out of the output was measured and
//! dropped: the vias have different layer spans, so they are not duplicates of
//! anything, and removing them is a cost-model question.
//!
//! The cost model already has the knob. `CongestionMap` tracks which cells a
//! via's ring covers and charges `ring_penalty` per ring, and the default is
//! **0.0** - the search knows exactly where every hole is and pays nothing to
//! put another one on top.
//!
//! This measures what a price does, on both counts that matter: the holes it
//! removes and the violations it costs.

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::{preset_for_world, ruleset_for_world, run_drc, ViolationKind};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_rules::presets::RulesPreset;

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("tests/fixtures/benchmark")
        .join(name)
}

#[test]
#[ignore = "diagnostic: routes three fixtures at four prices"]
fn what_a_via_ring_should_cost() {
    // Three boards until 2026-08-13, and the three it left out were the ones
    // that mattered. `is_the_best_variant_a_local_optimum` found `via_ring 1`
    // beating the shipped pick on `plane_board` - a board this sweep had never
    // run - and the tracker recorded, wrongly, that the price had been
    // measured everywhere already. It had not: the sweep with "price" in its
    // name measures `via_foreign_copper_penalty`, a different knob, and this
    // one covered half the fixtures.
    for benchmark in cypcb_kicad::BENCHMARKS {
        let name = benchmark.filename;
        eprintln!();
        eprintln!("=== {name} ===");
        let mut table: Option<&'static str> = None;

        for penalty in [0.0, 1.0, 3.0, 8.0] {
            let parsed = parse_kicad_pcb(&fixture(name)).expect("the fixture parses");
            let mut world = parsed.world;

            // The table this board would actually be graded against. On
            // `multi_ic` the four-layer row is tighter, which moves the grid
            // cell as well as the grading - 0.400mm against 0.508mm - so a
            // fixed two-layer answer here is a different search.
            let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, &world);
            let rules = ruleset_for_world(preset, &world);
            let drc_rules = DesignRules::from_constraints(&preset.constraints());
            if table.is_none() {
                eprintln!("  graded on {}", preset.name());
                table = Some(preset.name());
            }

            let config = AutorouteConfig {
                via_ring_penalty: penalty,
                ..AutorouteConfig::default()
            };
            let started = std::time::Instant::now();
            let result = route_board(&mut world, &parsed.library, &rules, &config);
            let elapsed = started.elapsed();

            cypcb_router::apply_routes(&mut world, &result);
            world.rebuild_spatial_index_from_library(&parsed.library);
            let report = run_drc(&mut world, &drc_rules);

            let holes = report
                .violations
                .iter()
                .filter(|v| v.kind == ViolationKind::HoleToHole)
                .count();
            let shorts = report
                .violations
                .iter()
                .filter(|v| v.actual == Some(cypcb_core::Nm::ZERO))
                .count();

            eprintln!(
                "  ring {penalty:>4}: {:>4} violations, {:>3} shorts, {:>3} hole-to-hole, \
                 {:>4} vias, {:>4} routes, {:.1}s",
                report.violations.len(),
                shorts,
                holes,
                result.vias.len(),
                result.routes.len(),
                elapsed.as_secs_f64(),
            );
        }
    }
}
