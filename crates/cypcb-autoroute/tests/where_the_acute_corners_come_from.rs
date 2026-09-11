//! Which stage of the router draws copper meeting copper below a right angle.
//!
//! `cargo test -p cypcb-autoroute --test where_the_acute_corners_come_from -- --nocapture`
//!
//! The `acute-angle` rule landed on 2026-09-11 and its first finding was about
//! this router rather than about a user's board: every benchmark fixture comes
//! out of it with corners a fabricator is told not to draw. The counts were
//! known per board - led_blink 1, stm32_breakout 18, multi_ic 56,
//! shift_driver 12, qfp_fanout 100, plane_board 8 - and not per stage, so
//! there was nothing to fix yet.
//!
//! Four stages can produce one: the grid search, `postprocess` turning cells
//! into segments, `smooth_routes`, and `optimize_vias`, which is the only one
//! that joins two points that were never neighbours on the grid.
//!
//! This measures through `route_board`, the entry point a user presses, and
//! not through `route_with_debug`: that one assembles its own pipeline out of
//! the same parts - no parameter clamping and no repair pass - so a number
//! taken from it describes the instrument.

use std::path::Path;

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::{preset_for_world, ruleset_for_world, run_drc, DesignRules, ViolationKind};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_router::apply_routes;

/// Every benchmark fixture, in the order the ratchet table lists them.
const FIXTURES: &[&str] = &[
    "led_blink.kicad_pcb",
    "stm32_breakout.kicad_pcb",
    "multi_ic.kicad_pcb",
    "shift_driver.kicad_pcb",
    "qfp_fanout.kicad_pcb",
    "plane_board.kicad_pcb",
];

fn fixture_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// What one run of the router leaves behind, as the rule reads it.
struct Run {
    acute: usize,
    routes: usize,
    vias: usize,
    /// Every acute junction's angle, to one decimal, in the order reported.
    ///
    /// This is the measurement that separates the grid from the stages after
    /// it. The search steps in eight directions and `simplify_path` merges
    /// only collinear steps, so a junction inherited from the grid can hold
    /// exactly one of five angles - 180, 135, 90, 45 or 0 - and only the last
    /// two are acute. Any other value had to come from a stage that joins two
    /// points which were never neighbours on the grid.
    angles: Vec<String>,
}

/// Route one fixture and count what the rule says about the copper.
fn routed(fixture: &str, smoothing: bool) -> Run {
    let parsed = parse_kicad_pcb(&fixture_path(fixture))
        .unwrap_or_else(|e| panic!("failed to parse {fixture}: {e:?}"));
    let mut world = parsed.world;
    let library = parsed.library;
    let preset = preset_for_world(
        cypcb_rules::presets::RulesPreset::JlcpcbStandard2Layer,
        &world,
    );
    let rules = ruleset_for_world(preset, &world);

    let config = AutorouteConfig {
        smoothing,
        ..AutorouteConfig::default()
    };
    let result = route_board(&mut world, &library, &rules, &config);
    let routes = result.route_count();
    let vias = result.vias.len();

    apply_routes(&mut world, &result);
    world.rebuild_spatial_index_from_library(&library);

    let report = run_drc(
        &mut world,
        &DesignRules::from_constraints(&preset.constraints()),
    );
    let acute: Vec<_> = report
        .violations
        .iter()
        .filter(|violation| violation.kind == ViolationKind::AcidTrap)
        .collect();

    // The rule states the angle in its message and nowhere else: an angle is
    // not a length, so `actual` and `required` are deliberately empty on this
    // kind. Copper drawn over itself says so in words instead of a number.
    let angles = acute
        .iter()
        .map(|violation| {
            violation
                .message
                .split(" degrees")
                .next()
                .and_then(|head| head.rsplit(' ').next())
                .filter(|_| violation.message.contains(" degrees"))
                .unwrap_or("drawn over itself")
                .to_string()
        })
        .collect();

    Run {
        acute: acute.len(),
        routes,
        vias,
        angles,
    }
}

#[test]
fn the_smoother_is_not_where_they_come_from() {
    // Measured 2026-09-11 on all six fixtures. Every board that draws an acute
    // corner draws it with the smoother switched off, so the copper arrives
    // bent from the search and `smooth_routes` inherits it. It is not neutral
    // either, and the direction is the opposite of the accusation: it removes
    // corners on three boards (23 -> 18, 64 -> 56, 14 -> 12), leaves two
    // unchanged, and adds three on qfp_fanout (97 -> 100).
    let mut table = Vec::new();
    for fixture in FIXTURES {
        let on = routed(fixture, true);
        let off = routed(fixture, false);
        println!(
            "{fixture:<26} smoothing on {:>4} acute / {:>4} routes / {:>3} vias   \
             off {:>4} acute / {:>4} routes / {:>3} vias",
            on.acute, on.routes, on.vias, off.acute, off.routes, off.vias
        );
        table.push((*fixture, on, off));
    }

    // The control for the second column: a run that quietly ignored the flag
    // would print `off` numbers that are the `on` numbers, and every
    // assertion below would pass on a measurement that never happened.
    assert!(
        table
            .iter()
            .any(|(_, on, off)| on.acute != off.acute || on.routes != off.routes),
        "switching the smoother off changed nothing on any board, so the \
         second column is not a measurement"
    );

    for (fixture, on, off) in &table {
        assert!(
            on.acute == 0 || off.acute > 0,
            "{fixture}: {} acute corners with the smoother and none without it, \
             which would make the smoother the source",
            on.acute
        );
    }

    // The positive control. Without it a rule that had stopped reporting
    // anything at all would satisfy every assertion above.
    let total: usize = table.iter().map(|(_, on, _)| on.acute).sum();
    assert!(
        total >= 195,
        "the six fixtures drew 195 acute corners between them on 2026-09-11 \
         and this run counted {total}"
    );
}

#[test]
fn every_corner_they_draw_is_one_the_grid_can_hold() {
    // The other half of the answer, and the half that names the stage. Eight
    // directions and a collinear-only merge admit exactly five junction
    // angles, two of them acute: 45 degrees and 0. If every acute corner the
    // router draws is one of those two, no stage after the search invented
    // one - not `optimize_vias`, which is the only one that joins points the
    // grid never made neighbours, and not the smoother's chamfer.
    let mut seen: Vec<String> = Vec::new();
    for fixture in FIXTURES {
        for angle in routed(fixture, true).angles {
            if !seen.contains(&angle) {
                seen.push(angle);
            }
        }
    }
    seen.sort();
    println!("angles drawn: {seen:?}");

    assert!(!seen.is_empty(), "the rule reported nothing to measure");
    for angle in &seen {
        assert!(
            angle == "45.0" || angle == "0.0" || angle == "drawn over itself",
            "a corner at {angle} degrees is not one the grid can step into, \
             so a stage after the search drew it: {seen:?}"
        );
    }
}
