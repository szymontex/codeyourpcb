//! Which rule the `stop_at_own_copper` flag moves, board by board.
//!
//! `cargo test --release -p cypcb-autoroute --test which_rule_the_flag_moves -- --nocapture`
//!
//! The flag was measured on totals first: 1135 violations across the six
//! benchmark boards become 844, and the copper drawn over copper falls from
//! 142 junctions to 21. One board went the other way - `shift_driver` from 19
//! to 32 - and a total that improves while one board gets worse is a total
//! that has to be broken down before a default is moved, because a board is
//! fabricated on its own and not as a sixth of an average.
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
fn the_board_that_gets_worse_gets_worse_at_one_other_rule() {
    // `shift_driver` is the board the totals lose on, and a default cannot be
    // argued either way until the 13 extra reports have a name. They have one:
    // the flag succeeds there at what it is for - 12 acute junctions become 5 -
    // and pays for it in clearance, where 7 reports become 27. Clearance
    // exempts pairs on the same net (`rules/clearance.rs:180`), so those are
    // this board's copper coming closer to somebody else's, not a junction the
    // new end test made with the net's own trace.
    let off = by_kind("shift_driver.kicad_pcb", false);
    let on = by_kind("shift_driver.kicad_pcb", true);

    for kind in kinds_of(&off, &on) {
        let before = off.get(&kind).copied().unwrap_or(0);
        let after = on.get(&kind).copied().unwrap_or(0);
        println!("shift_driver {kind:<16} {before:>4} -> {after:>4}");
        if kind == "Clearance" {
            assert!(
                after > before,
                "the regression this test is about is gone: {kind} {before} -> {after}"
            );
        } else {
            assert!(
                after <= before,
                "a second rule regressed and this test would have called it clearance: \
                 {kind} {before} -> {after}"
            );
        }
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
    assert!(
        after_total * 2 < before_total,
        "HoleToHole {before_total} -> {after_total}"
    );
}

#[test]
fn the_flag_puts_copper_on_copper_and_that_is_why_it_is_not_the_default() {
    // The reason the default did not move on 2026-09-11, and the reason is
    // R-11's own tier order rather than a preference. Counting violation rows
    // says the flag is a clear win: 1135 reports become 844. Counting copper
    // touching copper says something the row count hides - `led_blink`, the
    // simplest board here and the only one that routes clean, comes out with a
    // short. Under R-11 a short is tier 2 and no quantity of tier 3 or tier 4
    // offsets one: a board with a short does not work, while a board with a
    // gap under minimum is a yield risk a fabricator may still build.
    //
    // This test is a pin on a defect rather than a claim that the defect is
    // right. When the short is found and removed it fails, which is what
    // forces the default to be reconsidered instead of forgotten.
    let mut clean_boards_that_short = Vec::new();
    let mut off_total = 0;
    let mut on_total = 0;

    for fixture in FIXTURES {
        let off = shorts_of(fixture, false);
        let on = shorts_of(fixture, true);
        println!("{fixture:<26} shorts {off:>4} -> {on:>4}");
        if off == 0 && on > 0 {
            clean_boards_that_short.push(*fixture);
        }
        off_total += off;
        on_total += on;
    }
    println!("all six boards: shorts {off_total} -> {on_total}");

    assert!(
        !clean_boards_that_short.is_empty(),
        "no board goes from no shorts to shorts, so the reason recorded for \
         keeping the default off no longer holds"
    );
    assert!(
        !AutorouteConfig::default().stop_at_own_copper,
        "the default was switched on while a board it routes clean still shorts"
    );
}
