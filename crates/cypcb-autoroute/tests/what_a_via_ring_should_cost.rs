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
use cypcb_drc::{run_drc, ViolationKind};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};

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
    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("a known preset"));
    let drc_rules = DesignRules::jlcpcb_2layer();

    for name in [
        "stm32_breakout.kicad_pcb",
        "multi_ic.kicad_pcb",
        "qfp_fanout.kicad_pcb",
    ] {
        eprintln!();
        eprintln!("=== {name} ===");

        for penalty in [0.0, 1.0, 3.0, 8.0] {
            let parsed = parse_kicad_pcb(&fixture(name)).expect("the fixture parses");
            let mut world = parsed.world;

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
