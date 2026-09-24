//! Which rule the `stop_at_own_copper` flag moves, board by board.
//!
//! `cargo test --release -p cypcb-autoroute --test which_rule_the_flag_moves -- --nocapture`
//!
//! The flag was measured on totals first: 1150 violations across the six
//! benchmark boards become 859, and the copper drawn over copper falls from
//! 142 junctions to 21. One board went the other way - `shift_driver` from 22
//! to 33 - and a total that improves while one board gets worse is a total
//! that has to be broken down before a default is moved, because a board is
//! fabricated on its own and not as a sixth of an average. Broken down, the
//! loss turned out to belong to the via optimizer and not to the flag; since
//! 2026-09-23 that board goes 20 to 9.
//!
//! The breakdown is per violation kind, which is what says whether the flag
//! failed at its own job on that board or succeeded at it and cost something
//! elsewhere. The two have opposite answers: the first would mean the design
//! is wrong, the second means the price is named and can be paid or refused.

use std::collections::BTreeMap;
use std::path::Path;

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::{preset_for_world, ruleset_for_world, run_drc, shorts, DesignRules};
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

/// The kind the acute-angle rule reports under, and the one the flag is aimed
/// at: copper meeting copper below a right angle, which on this router's
/// output is mostly copper laid back along copper.
const AIMED_AT: &str = "AcidTrap";

fn fixture_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// Route one board with the flag in one position and count the report by kind.
fn by_kind(fixture: &str, stop_at_own_copper: bool) -> BTreeMap<String, usize> {
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

    let result = route_board(&mut world, &library, &rules, &config);
    apply_routes(&mut world, &result);
    world.rebuild_spatial_index_from_library(&library);

    let report = run_drc(
        &mut world,
        &DesignRules::from_constraints(&preset.constraints()),
    );

    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for violation in &report.violations {
        *counts.entry(format!("{:?}", violation.kind)).or_default() += 1;
    }
    counts
}

/// Route one board with the flag in one position and count the copper that
/// touches other copper.
///
/// `shorts` is the checker's own definition - a clearance report measured at
/// 0.00 mm - rather than a kind of its own, which is why it cannot be read off
/// the per-kind table above.
fn shorts_of(fixture: &str, stop_at_own_copper: bool) -> usize {
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

    let result = route_board(&mut world, &library, &rules, &config);
    apply_routes(&mut world, &result);
    world.rebuild_spatial_index_from_library(&library);

    let report = run_drc(
        &mut world,
        &DesignRules::from_constraints(&preset.constraints()),
    );
    shorts(&report.violations)
}

/// Every kind either side names, so a kind that appears only with the flag on
/// is counted rather than skipped.
fn kinds_of(off: &BTreeMap<String, usize>, on: &BTreeMap<String, usize>) -> Vec<String> {
    let mut all: Vec<String> = off.keys().chain(on.keys()).cloned().collect();
    all.sort();
    all.dedup();
    all
}

#[test]
fn the_flag_does_its_own_job_on_every_board() {
    // The claim the design stands on. A connection that stops at the first
    // cell of its own net's copper cannot draw a second copy of a trunk, and
    // that is true of a board whose total gets worse exactly as much as of one
    // whose total improves - so the kind the flag is aimed at must fall
    // everywhere, `shift_driver` included, or the mechanism does not hold.
    let mut fell = 0;
    for fixture in FIXTURES {
        let off = by_kind(fixture, false);
        let on = by_kind(fixture, true);
        let before = off.get(AIMED_AT).copied().unwrap_or(0);
        let after = on.get(AIMED_AT).copied().unwrap_or(0);
        println!("{fixture:<26} {AIMED_AT} {before:>4} -> {after:>4}");
        assert!(
            after <= before,
            "{fixture} draws more {AIMED_AT} with the flag on: {before} -> {after}"
        );
        if after < before {
            fell += 1;
        }
    }
    assert!(
        fell >= 5,
        "the flag moved its own kind on only {fell} of the six boards"
    );
}

#[test]
fn the_board_that_got_worse_was_paying_for_the_via_optimizer() {
    // `shift_driver` was the board the totals lost on, 22 reports to 33, and
    // the extra reports had a name: the flag cut acute junctions 12 to 2 and
    // clearance went 7 to 18, most of it shorts. The clearance half was not
    // the flag's. `optimize_vias` checked each replacement segment against a
    // list of other nets' copper that every caller passed empty, and the flag
    // hands it more pairs to join. With the check real (2026-09-23) clearance
    // was 5 either way and the board went 20 to 9. With a via measured as a
    // disc (2026-09-24) clearance is 0 either way and the board goes 15 to 4,
    // so the flag costs this board nothing any rule counts.
    let off = by_kind("shift_driver.kicad_pcb", false);
    let on = by_kind("shift_driver.kicad_pcb", true);

    // The figures R-11 and R-19 quote off this board. They are exact on
    // purpose: a change to the via price, the grid or the weights will move
    // them, and when it does the two sections in the canon have to be read
    // again rather than quietly left behind.
    const CANON_FIGURES: &[(&str, usize, usize)] =
        &[("AcidTrap", 12, 2), ("Clearance", 0, 0), ("PadEntry", 3, 2)];
    for (kind, before, after) in CANON_FIGURES {
        assert_eq!(
            (
                off.get(*kind).copied().unwrap_or(0),
                on.get(*kind).copied().unwrap_or(0)
            ),
            (*before, *after),
            "R-11 states {kind} {before} to {after} on this board; if the router moved, the \
             sentence in the canon moves with it"
        );
    }

    // R-19 quoted this board as mostly its rule: 18 of 22 while the optimizer
    // was drawing shorts here, 5 of 9 without them. All five measured a via
    // as the square around it. As a disc the nearest copper is 0.145mm and
    // 0.175mm away against 0.127mm (2026-09-24), and the board reports no
    // clearance row with the flag off or on.
    let total: usize = on.values().sum();
    let clearance = on.get("Clearance").copied().unwrap_or(0);
    println!("shift_driver clearance share {clearance} of {total}");

    // No kind rises. This is the assertion that would have caught the
    // optimizer: it was the one rule going the other way.
    for kind in kinds_of(&off, &on) {
        let before = off.get(&kind).copied().unwrap_or(0);
        let after = on.get(&kind).copied().unwrap_or(0);
        println!("shift_driver {kind:<16} {before:>4} -> {after:>4}");
        assert!(
            after <= before,
            "the flag costs shift_driver a rule again: {kind} {before} -> {after}"
        );
    }
}

#[test]
fn the_holes_come_off_the_boards_that_had_the_most() {
    // Reported because it is the largest fall after the acute count and it was
    // not the aim: hole to hole is a spacing between drilled holes, so fewer
    // reports means fewer vias placed on top of each other rather than a rule
    // reading differently.
    let mut before_total = 0;
    let mut after_total = 0;
    for fixture in FIXTURES {
        let off = by_kind(fixture, false);
        let on = by_kind(fixture, true);
        before_total += off.get("HoleToHole").copied().unwrap_or(0);
        after_total += on.get("HoleToHole").copied().unwrap_or(0);
    }
    println!("all six boards: HoleToHole {before_total} -> {after_total}");
    // 65 -> 33 on 2026-09-23, the first run in which `optimize_vias` kept
    // every pair whose replacement would cross another net. That is half
    // rounded up, one hole short of the strict half this asserted while the
    // optimizer was deleting vias it had no business deleting.
    assert!(
        after_total <= before_total.div_ceil(2),
        "HoleToHole {before_total} -> {after_total}"
    );
}

#[test]
fn the_flag_shorts_no_board_the_default_routes_clean() {
    // This was the pin on the reason the default did not move on 2026-09-11:
    // `led_blink`, the one board that routes clean, came out of the flag with
    // a short, and under R-11 a short is tier 2 and no quantity of tier 3 or
    // tier 4 offsets one. The pin was written to fail when the short was
    // removed, so the default would be reconsidered rather than forgotten.
    //
    // It failed on 2026-09-23. The short was never the flag's: `optimize_vias`
    // joined a GND via pair straight across SW_OUT because the list of other
    // nets' copper it checked against was empty. With the check real the
    // flag leaves no board with more shorts than the default does - 270
    // become 206 across the six - and this test now holds that.
    //
    // The default stays off, and that is a decision this test does not make.
    // One tier-1 figure still goes the other way: `multi_ic` leaves 6 pins
    // unrouted with the flag off and 7 with it on.
    let mut boards_the_flag_shorts_more = Vec::new();
    let mut off_total = 0;
    let mut on_total = 0;

    for fixture in FIXTURES {
        let off = shorts_of(fixture, false);
        let on = shorts_of(fixture, true);
        println!("{fixture:<26} shorts {off:>4} -> {on:>4}");
        if on > off {
            boards_the_flag_shorts_more.push(*fixture);
        }
        off_total += off;
        on_total += on;
    }
    println!("all six boards: shorts {off_total} -> {on_total}");

    assert!(
        boards_the_flag_shorts_more.is_empty(),
        "the flag draws more shorts than the default on {boards_the_flag_shorts_more:?}"
    );
}
