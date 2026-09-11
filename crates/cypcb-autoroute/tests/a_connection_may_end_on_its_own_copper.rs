//! What happens when a connection is allowed to stop on its own net's copper.
//!
//! `cargo test --release -p cypcb-autoroute --test a_connection_may_end_on_its_own_copper -- --nocapture`
//!
//! A net with three pins is decomposed into two-pin connections and each is
//! searched pad to pad. Copper the net has already laid is cheap to walk along
//! and is never a destination, so the second connection follows the first down
//! the same corridor and writes a second copy of it: **635 grid cells shared
//! between two paths of one net on the six benchmark boards, against 0 cells
//! repeated inside a single path** - a shared trunk, measured on 2026-09-11.
//!
//! `stop_at_own_copper` is the published fix, from a multi-sink Lee-Moore
//! router: the search for a connection ends at the first cell of its own net's
//! copper it reaches, so the second copy cannot be drawn rather than being
//! removed afterwards. Two things had to move with it. The search runs from
//! the pad that is not in the spanning tree yet towards the tree, because the
//! other pad is inside the target set and a search that starts on its own goal
//! draws nothing. And the heuristic has to be a lower bound for the nearest
//! goal rather than for the far pad, which is what `TargetBounds` is for.
//!
//! The figures live behind the flag until they are measured, which is what
//! this file does. It measures through `route_board`, the entry point a user
//! presses - not `route_with_debug`, which assembles its own pipeline out of
//! the same parts and therefore describes the instrument.

use std::path::Path;
use std::time::Instant;

use cypcb_autoroute::grid::RoutingGrid;
use cypcb_autoroute::orchestrator::{extract_ratsnest, order_nets};
use cypcb_autoroute::pathfinder::GridNode;
use cypcb_autoroute::pathfinder_v2::pathfinder_loop;
use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::{preset_for_world, ruleset_for_world, run_drc, DesignRules, ViolationKind};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_router::apply_routes;

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

/// The four figures a fabricated board is judged on, plus the two halves of
/// the acute report.
struct Run {
    /// Junctions where copper is laid back along copper - the defect this
    /// flag is aimed at.
    doubled: usize,
    /// Everything the checker reports, which is the figure the benchmark
    /// ratchets hold and the one that decides whether this becomes default.
    violations: usize,
    /// Junctions where two arms leave a point at 45 degrees - the other half
    /// of the acute report, which this flag is not aimed at.
    wedges: usize,
    segments: usize,
    vias: usize,
    /// Total copper drawn, in nanometres of centre line.
    length_nm: i64,
    millis: u128,
}

/// Route one fixture with the flag in one position and read the copper.
fn routed(fixture: &str, stop_at_own_copper: bool) -> Run {
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
        stop_at_own_copper,
        ..AutorouteConfig::default()
    };

    let started = Instant::now();
    let result = route_board(&mut world, &library, &rules, &config);
    let millis = started.elapsed().as_millis();

    let segments = result.route_count();
    let vias = result.vias.len();
    let length_nm = result
        .routes
        .iter()
        .map(|segment| {
            let dx = (segment.end.x.raw() - segment.start.x.raw()) as f64;
            let dy = (segment.end.y.raw() - segment.start.y.raw()) as f64;
            dx.hypot(dy) as i64
        })
        .sum();

    apply_routes(&mut world, &result);
    world.rebuild_spatial_index_from_library(&library);

    let report = run_drc(
        &mut world,
        &DesignRules::from_constraints(&preset.constraints()),
    );

    // The rule states the angle in its message and nowhere else, and says so
    // in words when the two arms are collinear - which is copper drawn over
    // copper rather than a wedge. Splitting on that word is how the 195
    // reports were split into 53 and 142 on 2026-09-11.
    let mut doubled = 0;
    let mut wedges = 0;
    for violation in report
        .violations
        .iter()
        .filter(|violation| violation.kind == ViolationKind::AcidTrap)
    {
        if violation.message.contains(" degrees") {
            wedges += 1;
        } else {
            doubled += 1;
        }
    }

    Run {
        doubled,
        violations: report.violations.len(),
        wedges,
        segments,
        vias,
        length_nm,
        millis,
    }
}

/// The grid paths one board produces, with only the nets named in `order`.
fn grid_paths(
    fixture: &str,
    stop_at_own_copper: bool,
    two_pin_only: bool,
) -> Vec<Vec<Vec<GridNode>>> {
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
        stop_at_own_copper,
        ..AutorouteConfig::default()
    };

    let resolution = match world.board_info() {
        Some((size, _)) => {
            config.resolve_adaptive_grid_resolution(&rules, size.width.raw(), size.height.raw())
        }
        None => config.resolve_grid_resolution(&rules),
    };
    let mut grid = RoutingGrid::from_board(&mut world, &library, &rules, resolution)
        .unwrap_or_else(|| panic!("{fixture} has no grid"));

    let ratsnest = extract_ratsnest(&mut world, &library);
    let full_order = order_nets(&ratsnest);
    let order: Vec<usize> = full_order
        .into_iter()
        .filter(|index| !two_pin_only || ratsnest[*index].pads.len() == 2)
        .collect();
    let result = pathfinder_loop(&mut grid, &ratsnest, &order, &rules, &config, None);

    // Keyed by net id so two runs are comparable whatever order the map
    // iterates in.
    let mut nets: Vec<u32> = result.routed_paths.keys().copied().collect();
    nets.sort_unstable();
    nets.iter()
        .map(|net| result.routed_paths[net].clone())
        .collect()
}

#[test]
fn a_net_with_one_connection_is_routed_exactly_as_before() {
    // The control, and it is the pitfall this design was warned about: a flag
    // that swapped the search round whenever it was on, rather than only when
    // the net already holds copper, would move every board here. A net of two
    // pads has one connection and no copper of its own when that connection is
    // searched, so with only such nets in the order the two runs have to be
    // identical cell for cell.
    let mut boards_with_paths = 0;
    for fixture in FIXTURES {
        let off = grid_paths(fixture, false, true);
        let on = grid_paths(fixture, true, true);
        let cells: usize = off.iter().flatten().map(|path| path.len()).sum();
        println!("{fixture:<26} two-pin nets only: {cells} cells");
        assert_eq!(off, on, "{fixture} moved with no net able to use the flag");
        if cells > 0 {
            boards_with_paths += 1;
        }
    }
    assert!(
        boards_with_paths >= 4,
        "the control compared {boards_with_paths} boards that drew anything"
    );
}

#[test]
fn the_flag_changes_the_copper_where_a_net_has_more_than_two_pins() {
    // The positive control for the one above: with every net in the order,
    // at least one board has to come out different, or the flag does nothing
    // anywhere and both tests here are empty.
    let moved = FIXTURES
        .iter()
        .filter(|fixture| grid_paths(fixture, false, false) != grid_paths(fixture, true, false))
        .count();
    println!(
        "boards whose grid paths move with the flag on: {moved} of {}",
        FIXTURES.len()
    );
    assert!(moved > 0, "the flag changed no board's copper at all");
}

#[test]
fn what_ending_on_its_own_copper_costs_and_saves() {
    // The verdict rule. Its first version asked only that the copper drawn
    // over copper fall across the six boards, and a mutation showed that to be
    // too loose to mean anything: with the end test deleted, so that the flag
    // only reverses the direction the search runs in, the count still falls
    // 142 -> 126 while every other figure gets worse - 1135 violations become
    // 1182 and the copper gets 91mm longer. Turning a search round is not the
    // mechanism; ending it on the net's own copper is.
    //
    // So the rule is what the mechanism claims. A connection that stops at the
    // first cell of its own net's copper cannot draw a second copy of the
    // trunk at all, which is most of the duplication rather than a tenth of
    // it, and a board must not get worse overall for it: no board loses
    // connectivity - measured as segments drawn, which cannot go to zero on a
    // board that routed before - and the checker's total falls. The wedge
    // count, which this flag is not aimed at, is printed beside it rather than
    // claimed as a result.
    let mut off_doubled = 0;
    let mut on_doubled = 0;
    let mut off_length = 0i64;
    let mut on_length = 0i64;
    let mut off_violations = 0;
    let mut on_violations = 0;

    for fixture in FIXTURES {
        let off = routed(fixture, false);
        let on = routed(fixture, true);
        println!(
            "{fixture:<26} off {:>4} drc / {:>4} doubled / {:>3} wedges / {:>5} seg / {:>3} vias / \
             {:>9.3}mm / {:>6}ms      on {:>4} drc / {:>4} doubled / {:>3} wedges / {:>5} seg / {:>3} vias / \
             {:>9.3}mm / {:>6}ms",
            off.violations,
            off.doubled,
            off.wedges,
            off.segments,
            off.vias,
            off.length_nm as f64 / 1e6,
            off.millis,
            on.violations,
            on.doubled,
            on.wedges,
            on.segments,
            on.vias,
            on.length_nm as f64 / 1e6,
            on.millis,
        );

        if off.segments > 0 {
            assert!(
                on.segments > 0,
                "{fixture} drew {} segments and now draws none",
                off.segments
            );
        }

        off_doubled += off.doubled;
        on_doubled += on.doubled;
        off_violations += off.violations;
        on_violations += on.violations;
        off_length += off.length_nm;
        on_length += on.length_nm;
    }

    println!(
        "all six boards: copper drawn over copper {off_doubled} -> {on_doubled}, \
         every violation {off_violations} -> {on_violations}, \
         total length {:.3}mm -> {:.3}mm",
        off_length as f64 / 1e6,
        on_length as f64 / 1e6
    );
    assert!(
        on_doubled * 4 <= off_doubled,
        "most of the {off_doubled} doubled junctions should be impossible to \
         draw, and {on_doubled} are left"
    );
    assert!(
        on_violations < off_violations,
        "the board got worse overall: {off_violations} violations became {on_violations}"
    );
}
