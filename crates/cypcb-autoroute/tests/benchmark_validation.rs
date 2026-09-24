//! Benchmark validation integration tests.
//!
//! Two test functions:
//! - `benchmark_regression` — fast CI gate (non-ignored): routes led_blink with PathFinder,
//!   asserts the solution is complete (0 unrouted, >= 20 routes) and then that quality has
//!   not regressed (composite ≤ 100, DRC 0, smoothness ≥ 0.95).
//! - `benchmark_full_matrix` — comprehensive comparison (`#[ignore]`): routes all 3 fixtures
//!   × 2 strategies, prints comparison table, emits JSON report, confirms PathFinder default.

use std::path::Path;

use serde::Serialize;

use cypcb_autoroute::astar_improved::ImprovedAStarStrategy;
use cypcb_autoroute::pathfinder_v2::PathFinderStrategy;
use cypcb_autoroute::scoring::{score_board, RoutingScore, ScoreWeights};
use cypcb_autoroute::strategy::RoutingStrategy;
use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::rules::pad_entry::{measure_entries, EntryReport};
use cypcb_drc::rules::{DrcRule, NetSplitRule};
use cypcb_drc::{preset_for_world, ruleset_for_world, DesignRules};
use cypcb_kicad::{parse_kicad_pcb, BENCHMARKS};
use cypcb_router::apply_routes;
use cypcb_router::types::{RouteSegment, RoutingResult, RoutingStatus};
use cypcb_rules::presets::RulesPreset;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

// ============================================================================
// Helpers
// ============================================================================

/// Resolve a benchmark fixture path relative to workspace root.
fn fixture_path(filename: &str) -> std::path::PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// Route a board with a given strategy and return (RoutingScore, route_count, unrouted).
///
/// Always calls `rebuild_spatial_index_with_traces()` before scoring
/// and grades each board against the fab table its own layer count asks for.
///
/// That table was fixed at two layers until 2026-08-21. `multi_ic` has four
/// copper layers, so its ratchets were recorded against the wrong row - and
/// not only the wrong row: the adaptive rule derives the grid cell from the
/// rule set, so the board searched a 0.508mm grid where the shipped tool
/// searches 0.400mm. Its numbers below are a re-baseline, not a regression and
/// not an improvement; neither word applies when the question changed.
/// Routes a fixture and measures it. The fourth value is the census this
/// project owed itself: every entry angle the router's own copper leaves
/// behind, in the same pass that counts the violations, so the two cannot
/// disagree about which board was measured.
/// The fifth value is what `net-split` reports on either side of the
/// smoother, from [`splits_around_smoothing`].
fn route_and_score(
    strategy: &dyn RoutingStrategy,
    fixture: &str,
) -> (
    RoutingScore,
    usize,
    usize,
    (EntryReport, Vec<String>),
    Option<Splits>,
) {
    let parsed = parse_kicad_pcb(&fixture_path(fixture))
        .unwrap_or_else(|e| panic!("Failed to parse {}: {:?}", fixture, e));
    let mut world = parsed.world;
    let library = parsed.library;
    let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, &world);
    let rules = ruleset_for_world(preset, &world);
    let config = AutorouteConfig::default();

    // Route through the shipped entry point, not the bare strategy: repair is
    // part of what a user gets when they press Route, so it has to be part of
    // what the ratchets measure.
    let result = if strategy.name() == "pathfinder" {
        route_board(&mut world, &library, &rules, &config)
    } else {
        strategy.route(&mut world, &library, &rules, &config)
    };
    let route_count = result.route_count();
    let unrouted = match result.status {
        RoutingStatus::Complete => 0,
        RoutingStatus::Partial { unrouted_count } => unrouted_count,
        RoutingStatus::Failed { .. } => usize::MAX,
    };

    let drc_rules = DesignRules::from_constraints(&preset.constraints());
    let splits = splits_around_smoothing(&mut world, &library, &result, &drc_rules);

    apply_routes(&mut world, &result);

    // Rebuild spatial index for accurate scoring
    world.rebuild_spatial_index_from_library(&library);

    let score = score_board(&mut world, &drc_rules, &ScoreWeights::default());

    // The entries by name, and the denominator beside them. `score_board`
    // reports how many violations there are and not which, and a count of
    // sharp entries with nothing under it cannot say whether the board has few
    // of them or few entries at all. `measure_entries` is called rather than
    // the registry because only it returns the report; the violations it hands
    // back are the same rows `PadEntryRule` puts in a DRC run, which is what
    // makes the census comparable to the ratchets above.
    let (entries, report) = measure_entries(&mut world);
    let sharp: Vec<String> = entries.into_iter().map(|v| v.message).collect();

    (score, route_count, unrouted, (report, sharp), splits)
}

/// What `net-split` reports before and after the smoother, one message per
/// piece a net is cut into.
type Splits = (Vec<String>, Vec<String>);

/// `net-split` on the router's own segments and then on the smoother's, each
/// with the router's vias. `None` when the smoother did not run.
///
/// Read from the copper the router keeps from either side of the smoother,
/// not from a second routing with smoothing off. A second routing lays other
/// copper - every later net and every repair pass reacts to what the earlier
/// ones left - so the difference between two runs is not what the smoother
/// did, and it costs a whole routing more.
fn splits_around_smoothing(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    result: &RoutingResult,
    drc_rules: &DesignRules,
) -> Option<Splits> {
    let snapshot = result.smoothing.as_deref()?;
    let mut splits = |routes: &[RouteSegment]| -> Vec<String> {
        apply_routes(
            world,
            &RoutingResult::complete(routes.to_vec(), snapshot.vias.clone()),
        );
        world.rebuild_spatial_index_from_library(library);
        NetSplitRule
            .check(world, drc_rules)
            .into_iter()
            .map(|v| v.message)
            .collect()
    };
    let before = splits(&snapshot.before);
    let after = splits(&snapshot.after);
    Some((before, after))
}

/// A net the router laid in one piece and the smoother cut, on one board.
///
/// Smoothing moves the corners of a run of segments and holds its ends, and
/// until 2026-09-24 it held nothing where a third branch left a run: on
/// `mains-sequencer` with `stop_at_own_copper` it chamfered two T junctions
/// and cut PE and GND in two. Nothing in the gate could see it, because no
/// stage measured the board with the smoother and without it. So `net-split`
/// is read on both sides of the smoother on the board the stage routes anyway,
/// and a piece more after it than before it is a failure. A board with no
/// snapshot is a failure too: the comparison would pass without having been
/// made.
fn smoothing_added_a_piece(label: &str, splits: &Option<Splits>) -> Option<String> {
    let Some((before, after)) = splits else {
        return Some(format!(
            "{label}: the router kept no copper from before the smoother, so nothing measured whether smoothing cut a net"
        ));
    };
    eprintln!(
        "      net-split: {} before the smoother, {} after it",
        before.len(),
        after.len()
    );
    (after.len() > before.len()).then(|| {
        let added: Vec<&String> = after.iter().filter(|m| !before.contains(m)).collect();
        format!(
            "{label}: net-split {} before the smoother and {} after it - smoothing cut a net: {added:?}",
            before.len(),
            after.len()
        )
    })
}

// ============================================================================
// BenchmarkResult (serializable for JSON output)
// ============================================================================

#[derive(Debug, Clone, Serialize)]
struct BenchmarkResult {
    fixture: String,
    strategy: String,
    composite: f64,
    drc_violations: u32,
    smoothness: f64,
    via_count: u32,
    total_length_mm: f64,
    route_count: usize,
    unrouted: usize,
}

impl BenchmarkResult {
    fn from_score(
        fixture: &str,
        strategy: &str,
        score: &RoutingScore,
        route_count: usize,
        unrouted: usize,
    ) -> Self {
        Self {
            fixture: fixture.to_string(),
            strategy: strategy.to_string(),
            composite: score.composite,
            drc_violations: score.drc_violations,
            smoothness: score.smoothness,
            via_count: score.via_count,
            total_length_mm: score.total_length.0 as f64 / 1_000_000.0,
            route_count,
            unrouted,
        }
    }
}

// ============================================================================
// Table printing
// ============================================================================

fn print_table_header() {
    eprintln!("╔═══════════════════╦════════════════╦══════════╦══════════╦══════════╦══════════╦══════════════╦══════════╗");
    eprintln!("║ Strategy          ║ Fixture        ║Composite ║ DRC Viol ║Smoothness║ Vias     ║ Length (mm)  ║ Unrouted ║");
    eprintln!("╠═══════════════════╬════════════════╬══════════╬══════════╬══════════╬══════════╬══════════════╬══════════╣");
}

fn print_table_row(r: &BenchmarkResult) {
    eprintln!(
        "║ {:<17} ║ {:<14} ║ {:>8.1} ║ {:>8} ║ {:>8.3} ║ {:>8} ║ {:>12.2} ║ {:>8} ║",
        r.strategy,
        r.fixture,
        r.composite,
        r.drc_violations,
        r.smoothness,
        r.via_count,
        r.total_length_mm,
        r.unrouted,
    );
}

fn print_table_separator() {
    eprintln!("╠═══════════════════╬════════════════╬══════════╬══════════╬══════════╬══════════╬══════════════╬══════════╣");
}

fn print_table_footer() {
    eprintln!("╚═══════════════════╩════════════════╩══════════╩══════════╩══════════╩══════════╩══════════════╝");
}

// ============================================================================
// Tests
// ============================================================================

/// Every benchmark fixture, with the DRC violation count each one currently
/// produces. led_blink is the only board the gate used to look at, and at 3
/// violations it made the router look healthy; the two realistic fixtures were
/// sitting at 312 and 383. These are ratchets - lower them as the router
/// improves, never raise them to accommodate a regression.
///
/// The two realistic numbers went **up** when the spatial index stopped boxing
/// every footprint at 1mm - 137 -> 176 and 64 -> 127 on byte-identical routing.
/// That is not a regression; it is the same board measured without a blind
/// spot. 24 of stm32_breakout's violations and 54 of multi_ic's name a
/// component, and those are precisely the pairs the old index could not see.
///
/// Lowered by the DRC-driven repair pass, which routes, reads the real report
/// and re-routes with the offending cells forbidden: 176 -> 167 and 127 -> 110,
/// every board still complete.
/// The most DRC violations each fixture may produce before the gate calls it a
/// regression.
///
/// Re-measured 2026-08-06 and raised, which is normally forbidden. The board
/// did not get worse; the checker stopped being blind. Three fixes landed
/// between the old numbers and these: the clearance rule measures pad copper
/// instead of the courtyard box, its same-net exemption is decided per pad
/// rather than per component, and an imported board finally carries its
/// footprints, which woke `courtyard-clearance`, `solder-mask-bridge` and
/// `silk-clearance` on every KiCad fixture. The old 167 and 110 also predate
/// the repair pass being switched off after it was measured to accept nothing.
///
/// These are totals, including what the fixture violated before routing.
/// `drc_report` separates the two if you need to know which is which.
///
/// Raised again on the same day and for the same kind of reason: the
/// clearance rule reports per offending segment now instead of once per pair
/// of entities, so a trace running too close to a part in two places is two
/// faults rather than one. The board is unchanged - 186 became 271 because
/// the checker stopped merging, which is also what made a saved board report
/// more than the board it was written from.
///
/// led_blink's violations are real and named: a GND trace across C1's
/// SW_OUT pad. The default router still makes it; `PathFinder High-Density`
/// routes the same board with zero, which is what `--variants` is for. Lower
/// these when the router improves. Never raise them for a regression.
/// Lowered on 2026-08-07, and not because the router improved: the clearance
/// rule was counting one gap twice wherever a trace's corner was the nearest
/// point to the other feature, and both segments meeting there reported it.
/// The boards are unchanged - 271 became 251 and 210 became 191 because the
/// checker stopped double-counting.
/// Two columns since 2026-08-07, because they are two different failures. A
/// board with copper touching copper cannot work; a board with a gap under
/// spec is a yield risk a fab may still build. A single count treats one short
/// as better than two near misses, which is backwards, and it hid that
/// reserving trace copper halves the shorts on both dense fixtures.
/// Raised on 2026-08-07, and not for a regression in the copper - for the
/// opposite. `paths_to_output` deleted every via whose cell carried
/// `CELL_PAD`, which covers a pad plus its clearance, so a route that changed
/// layer near any pad lost the via that joined it and the board came back with
/// two halves that never meet. Every check agreed it was fine: DRC saw no
/// overlap because the copper was on different layers, and the unrouted count
/// was zero because a path came back for every edge.
///
/// The search refuses to change layer on a pad now and every via reaches the
/// output. Pins no copper reaches - the measure that matters here - go
/// led_blink 1 -> 0, stm32_breakout 21 -> 6, multi_ic 60 -> 23, and
/// `UnroutedPinRule` is registered so the gate counts them.
///
/// The price is that vias which were being deleted are copper now, and the
/// grid does not model a via's ring, so they land too close to things.
///
/// Moved again the same day, downward on violations: refusing a layer change
/// on a pad was replaced by pricing it, which is the shape that has worked in
/// this vector six times against a veto's five failures. stm32_breakout 250 ->
/// 239 violations with 130 -> 136 shorts, multi_ic 375 -> 336 and 194 -> 166,
/// open pins unchanged at 6 and one better at 22. Never raise these for a
/// regression; lower them when the via ring reaches the grid.
/// One row: file, label, violation ratchet, shorts ratchet, and the routed
/// values those two were derived from.
///
/// The routed pair is recorded because the arithmetic is the whole convention -
/// a ratchet is the routed value plus that board's own noise band - and until
/// 2026-08-28 nothing checked it. A band could be edited, or a ratchet moved,
/// and no test would notice: the band feeds diagnostics and the ratchet is a
/// constant, so the two were only ever tied together by whoever wrote the
/// comment. `the_ratchets_are_the_routed_values_plus_their_bands` ties them.
type Ratchet = (&'static str, &'static str, u32, u32, u32, u32);

/// The entry census, in the order of `DRC_RATCHETS`: entries examined, entries
/// refused, entries sharper than R-08 allows. Measured on 2026-09-12, the run
/// after `PadEntryRule` was registered.
///
/// Held three different ways on purpose, because the three numbers fail in
/// three different directions:
///
/// - **sharp is a ratchet**: it may fall and may not rise. More sharp entries
///   than this is the router getting worse at the thing R-08 measures.
/// - **examined is a floor**: it may rise and may not fall. A rule that stops
///   looking reports nothing and looks exactly like a rule that found nothing,
///   which is the distinction `EntryReport` exists to make. Without this line
///   a change that made `measure_entries` see no copper at all would turn the
///   ratchet above green.
/// - **refused is a ceiling**: five today, all on `multi_ic`. A refusal is an
///   entry with no angle, so a rise here is measurement quietly going missing.
///
/// `multi_ic`'s floor fell 287 to 285 on 2026-09-23 and the rule did not stop
/// looking: the two entries were copper `optimize_vias` laid across other
/// nets' pads. With the optimizer switched off the board examines 285 as well,
/// and with the old optimizer 287, on the same tree in one sitting.
///
/// 285 / 5 / 2 -> 283 / 4 / 1 on `multi_ic` on 2026-09-24, and the rule did
/// not stop looking: the change that moved it touches the router and nothing
/// in `cypcb-drc`. The router stopped running tracks through the barrel of a
/// via on an inner layer, the board routes to different copper - 205 vias to
/// 200 - and that copper enters two fewer pads. The floor follows the copper
/// down; sharp and refused fall by one each and are held there.
const ENTRY_CENSUS: [(usize, usize, usize); 6] = [
    (14, 0, 0),  // led_blink
    (180, 0, 1), // stm32_breakout
    (283, 4, 1), // multi_ic
    (178, 0, 3), // shift_driver
    (178, 0, 5), // qfp_fanout
    (60, 0, 3),  // plane_board
];

// The fifth and sixth fields are a **baseline**, not a reading of today's
// router: they record what the board routed to on the day the band beside them
// was measured, and the threshold is the two added together. Nothing compared
// them against a routed board until 2026-09-13, and four of the six had drifted
// by then - stm32_breakout by 1, multi_ic by 2, shift_driver by 3, qfp_fanout
// by 5. Every one was still under its threshold, which is what the band is for;
// what was missing was anybody being told the headroom was going. The walk in
// `benchmark_all_fixtures_drc` prints the drift now and fails at half a band,
// so a baseline is re-measured while there is still room rather than after the
// gate turns red.
const DRC_RATCHETS: &[Ratchet] = &[
    // Every entry re-measured 2026-08-08, and every band with it, on boards
    // that are all fabricable for the first time: no copper outside an
    // outline, no two parts in the same place, no copper the files invent.
    //
    // Each ratchet is the routed value plus that board's own band from
    // `via_price_sweep::how_much_of_the_price_is_noise`, run at the same time
    // as the values. Three of them widen rather than tighten, and that is the
    // point: a ratchet set inside a board's measured noise fails for reasons
    // that have nothing to do with the change being tested.
    //
    // Re-measured 2026-08-10, after the clearance rule started measuring a
    // trace pair from both sides. Every ratchet still holds and none moved:
    // what moved is the routed value, by at most 13 and always inside that
    // board's own band. The two boards whose band is zero did not move at all,
    // which is the check that matters - on a board the router is deterministic
    // on, a checker that had started inventing contacts would show it here.
    //
    // board            routed      band   shorts band   2026-08-10
    // stm32_breakout   180 / 93    59     61            187 / 99
    // multi_ic         291 / 187   65     56            304 / 200
    // shift_driver     65 / 34     17     8             65 / 34
    // qfp_fanout       309 / 147   57     44            318 / 149
    // plane_board      28 / 13     0      0             28 / 13
    // led_blink        2 / 0       0      0             2 / 0
    //
    // Re-baselined 2026-08-21, and only `multi_ic` moved. This harness graded
    // every board on a fixed two-layer table until that date; `multi_ic` has
    // four copper layers, so it was both marked against the wrong row and
    // searched on the wrong grid - the adaptive rule derives the cell from the
    // rule set, giving 0.508mm there against the 0.400mm the shipped tool
    // uses. Its routed value goes 945 routes / 316 / 200 to 970 / 381 / 175.
    // That is neither a regression nor an improvement: the question changed.
    //
    // The other five are unchanged to the digit across the conversion -
    // led_blink 21 / 2 / 0, stm32_breakout 899 / 199 / 99, shift_driver
    // 671 / 65 / 34, qfp_fanout 1478 / 318 / 149, plane_board 181 / 28 / 13 -
    // which is the check that the conversion reached nothing it should not.
    //
    // `multi_ic`'s new ratchet is its routed value plus its own re-measured
    // band from `cypcb_autoroute::noise_band`, 34 / 49: 381 + 34 = 415 and
    // 175 + 49 = 224. The violation side loosens by 59 and the shorts side
    // **tightens by 19**, because both the routed shorts and the band came
    // down.
    // Re-baselined 2026-08-28, after a pad stopped blocking a disc of its
    // longer half-side and started blocking its own rectangle with two cells
    // of reach. Every board moved and every band was re-measured with it, so
    // each ratchet below is again the routed value plus that board's own band
    // - the same arithmetic, on numbers that are all new.
    //
    // board            routed        band      ratchet was   ratchet is
    // led_blink          0 /   0     0 /  0      2 /   0       0 /   0
    // stm32_breakout   187 / 104    64 / 48    239 / 154     251 / 152
    // multi_ic         449 / 134    35 / 15    471 / 224     484 / 149
    // shift_driver       7 /   5    26 / 15     82 /  42      33 /  20
    // qfp_fanout       271 / 150    61 / 46    366 / 191     332 / 196
    // plane_board       26 /  13     0 /  0     28 /  13      26 /  13
    //
    // Four rows tighten and two loosen, and the two that loosen are not the
    // router getting worse: `stm32_breakout` routes 12 violations better and
    // its band widened by 5, `multi_ic` gives up 12 violations while losing 42
    // shorts. A ratchet set inside a board's own noise fails on weather, which
    // is why the band is added rather than the routed value used bare.
    // Re-baselined 2026-09-11, and the router did not move at all: `acute-angle`
    // started counting corners nobody was counting. Measured with the rule
    // unregistered, every board routes to exactly the value it did before -
    // 0, 187, 449, 7, 271 and 26 - with the same route counts and the same
    // shorts; registered, the same run gives 1, 205, 505, 19, 371 and 34. The
    // whole rise is 1, 18, 56, 12, 100 and 8 acute corners this router has
    // always drawn. **The shorts are the control**: identical on all six
    // boards, on and off, which is what says the copper did not change.
    //
    // Bands are unchanged, because they came from a via-price sweep rather
    // than from the rule set, so each ratchet is again the routed value plus
    // that board's own band.
    //
    // board            routed        band      ratchet was   ratchet is
    // led_blink          1 /   0     0 /  0      0 /   0       1 /   0
    // stm32_breakout   205 / 104    64 / 48    251 / 152     269 / 152
    // multi_ic         505 / 134    35 / 15    484 / 149     540 / 149
    // shift_driver      19 /   5    26 / 15     33 /  20      45 /  20
    // qfp_fanout       371 / 150    61 / 46    332 / 196     432 / 196
    // plane_board       34 /  13     0 /  0     26 /  13      34 /  13
    //
    // Lowered 2026-09-23, and every figure that moved went down: `optimize_vias`
    // checked each replacement segment against a list of other nets' copper
    // that every caller passed empty, so it joined via pairs straight across
    // other nets. With the check real it keeps those vias, and the copper it
    // no longer lays was most of the shorts on four boards. Each field is
    // re-baselined on its own, and only where the routed value plus the band
    // comes out under the old ratchet:
    //
    // board            routed        band      ratchet was   ratchet is
    // led_blink          1 /   0     0 /  0      1 /   0       1 /   0
    // stm32_breakout   160 /  58    64 / 48    269 / 152     224 / 106
    // multi_ic         509 /  78    35 / 15    540 / 149     540 /  93
    // shift_driver      20 /   0    26 / 15     45 /  20      45 /  15
    // qfp_fanout       363 / 131    61 / 46    432 / 196     424 / 177
    // plane_board       29 /   3     0 /  0     38 /  13      29 /   3
    //
    // `multi_ic` and `shift_driver` keep their violation baselines of 505 and
    // 19: the kept vias add two reports on the first and one on the second,
    // routed plus band would raise the ratchet, and a ratchet is not raised
    // for this. Both sit inside half a band of the old baseline.
    //
    // Re-baselined 2026-09-24, and the router did not move: every route set
    // hashes as it did. `ClearanceRule` measured a via and a circular pad as
    // the square around the disc, so copper passing the square's corner read
    // closer than it is - a 0.033mm gap as a short, two vias 0.166mm apart as
    // touching. Measured as discs, every report that went was classified
    // against the circle: 177 were never under the rule, 8 shorts are now the
    // gaps they are, and where a pair lost a report its count now equals the
    // places its trace comes under the rule. Same arithmetic as above, routed
    // plus band where that comes out under the old ratchet:
    //
    // board            routed        band      ratchet was   ratchet is
    // led_blink          1 /   0     0 /  0      1 /   0       1 /   0
    // stm32_breakout   139 /  56    64 / 48    224 / 106     203 / 104
    // multi_ic         506 /  66    35 / 15    548 /  88     541 /  81
    // shift_driver      15 /   0    26 / 15     45 /  15      41 /  15
    // qfp_fanout       297 / 112    61 / 46    424 / 177     358 / 158
    // plane_board       24 /   3     0 /  0     29 /   3      24 /   3
    //
    // Re-baselined 2026-09-24 again, and the router did not move: every route set
    // hashes as it did. A `roundrect` pad was measured as its box and an oblong
    // as its box, so a trace passing a rounded corner read closer than the copper
    // is. Each is now its core grown by its radius, and every report that went
    // was classified against the pad's own outline: 9 trace-to-pad pairs over
    // both flag settings were never under the rule, and every pair still reported
    // has a nearest gap that is no larger than its copper's. The one short
    // `qfp_fanout` gained with the flag on is a sixth segment whose copper
    // overlaps the pad; five were reported before because two shared a contact
    // point on the box. `multi_ic` gains an unrouted pin, R8.2, 0.0292mm from the
    // nearest copper of its net, which `UnroutedPinRule` had counted as reached
    // by a box of its own. Routed plus band where that comes out under the old
    // ratchet; `multi_ic` is 507, one over its baseline and inside its band, and
    // a ratchet is not raised for this:
    //
    // board            routed        band      ratchet was   ratchet is
    // led_blink          1 /   0     0 /  0      1 /   0       1 /   0
    // stm32_breakout   138 /  56    64 / 48    203 / 104     202 / 104
    // multi_ic         507 /  66    35 / 15    541 /  81     541 /  81
    // shift_driver      15 /   0    26 / 15     41 /  15      41 /  15
    // qfp_fanout       296 / 112    61 / 46    358 / 158     357 / 158
    // plane_board       20 /   3     0 /  0     24 /   3      20 /   3
    //
    // Re-baselined 2026-09-24 a third time, and the router did not move: every
    // route set hashes as it did. `ClearanceRule` left out the pads on a
    // trace's own net, but measured a via against every pad of the part, so a
    // via dropped onto its own pin read as a short with that pin. On
    // `multi_ic` two such pairs were reported at 0.00mm whose nearest pad of
    // another net clears the rule, and a third at 0.00mm is a gap to the next
    // pin that is still under it. A via now leaves out the pads on its own
    // net, as a trace does. Only `multi_ic` moves; routed plus band:
    //
    // board            routed        band      ratchet was   ratchet is
    // multi_ic         505 /  63    35 / 15    541 /  81     540 /  78
    ("led_blink.kicad_pcb", "led_blink", 1, 0, 1, 0),
    (
        "stm32_breakout.kicad_pcb",
        "stm32_breakout",
        202,
        104,
        138,
        56,
    ),
    // Re-baselined 2026-08-23 for `ViaSpanRule`, and the router did not move:
    // measured with the rule unregistered, `multi_ic` routes to **381**
    // violations, which is 34 *under* the old 415. Registered, the same run is
    // 437 - the whole rise is 56 blind and buried vias this project laid and
    // never asked about, because `blind_vias_allowed` and
    // `buried_vias_allowed` were dropped before they reached a rule. New
    // ratchet is the routed value plus this board's own band of 34, the same
    // arithmetic as every other row: 437 + 34 = 471. Shorts unmoved at 175.
    //
    // 540 / 93 -> 578 / 125 on 2026-09-24, and the router did not move: the
    // spatial index gave a via only the two layers it joins, so on this, the
    // one four-layer board, a through via was invisible on both inner layers
    // and a blind one on the layer it passes. The hole is plated wherever it
    // is drilled and the Gerber export flashes its land there. Measured on
    // the same tree both ways, the route set hashes identically and the
    // checker finds 511 / 78 before and 543 / 110 after. All 32 new reports
    // were classified against the via's circle: 21 tracks through the middle
    // of a hole on an inner layer, 1 via stacked on another net's buried
    // one, and 10 tracks 0.033mm from a land that the square envelope counts
    // as touching - real under 0.10mm, reported as a short. Baselines
    // re-measured at 543 / 110; ratchet is that plus the band, 35 / 15.
    //
    // 578 / 125 -> 548 / 88 on 2026-09-24, and this time the router moved.
    // It reserved a via's ring on the two layers the via joins and on nothing
    // between, so a later net ran its track straight through the hole on an
    // inner layer - 21 segments on this board, 32 with `stop_at_own_copper`.
    // The grid now asks `Via::copper_mask` which layers the hole passes and
    // marks the same footprint on each, and the search refuses a layer change
    // whose barrel lands on another net's copper. Every one of those segments
    // is gone, and the other five boards route to the same hash: a two-layer
    // via has no layer between its ends. Routed 513 / 73, vias 205 -> 200,
    // 0 unrouted; ratchet is that plus the band, 35 / 15.
    ("multi_ic.kicad_pcb", "multi_ic", 540, 78, 505, 63),
    ("shift_driver.kicad_pcb", "shift_driver", 41, 15, 15, 0),
    ("qfp_fanout.kicad_pcb", "qfp_fanout", 357, 158, 296, 112),
    // A band of zero is not a rounding: this board routes identically at every
    // via price from 0.22 to 0.28, 28 violations and 13 shorts each time. Its
    // ratchet is the measured value exactly, so any movement at all is a real
    // change rather than weather.
    //
    // 34 to 37 on 2026-09-12, and the router did not get worse: `PadEntryRule`
    // was registered and started counting something nothing counted before.
    // Measured both ways in one sitting, the same fixtures with the registry
    // entry deleted and restored:
    //
    //   fixture          without  with   sharp entries
    //   led_blink              1     1     0
    //   stm32_breakout       205   206     1
    //   multi_ic             505   507     2
    //   shift_driver          19    22     3
    //   qfp_fanout           371   376     5
    //   plane_board           34    37     3
    //
    // Fourteen entries across 4 349 routes, and shorts did not move on any
    // board, which is the check that says these are wedges and not copper
    // touching copper. The five other rows sit inside their bands and are left
    // where they are; this one has no band, so it moves by exactly the three
    // it gained. **The router emits sharp entries and nobody knew** - that is a
    // routing-quality item, not a threshold to file away.
    //
    // 37 -> 38 on 2026-09-13, and this one is not the router. `PourIslandRule`
    // filled its planes at `min_clearance` and now fills them at
    // `min_copper_pour_clearance`, which every preset publishes as the wider
    // figure; on this fixture, the only one carrying a pour, the plane is cut
    // further back from the copper crossing it and one more piece comes away
    // unreached. Measured both ways on the same tree: 37 violations with the
    // old field, 38 with the new one, and 217 routes, 13 shorts and 0 unrouted
    // in both. **Only `pour_island.rs` reads that field**, so the row that
    // arrived is a pour island and cannot be anything else. Raising a ratchet
    // needs that much: the count moved because the checker measures the plane
    // the fab will make, and the router's own three numbers did not move at
    // all.
    //
    // 38 -> 29 on 2026-09-23, with the via optimizer's check made real; see
    // the table above `led_blink`. Shorts 13 -> 3.
    //
    // 24 -> 20 on 2026-09-24: four trace-to-pad pairs under `U1` measured by
    // the pad's box rather than its copper; see the table above `led_blink`.
    ("plane_board.kicad_pcb", "plane_board", 20, 3, 20, 3),
];

/// Routes every fixture and holds the line on completeness and DRC count.
///
/// Ignored by default so `cargo test` stays quick; scripts/quality-gate.sh runs
/// it explicitly in the benchmark stage. About 100 seconds: the grid is a track
/// pitch rather than half a clearance, and repair routes each board three times.
#[test]
#[ignore = "slow: routes every fixture"]
fn benchmark_all_fixtures_drc() {
    let pathfinder = PathFinderStrategy;

    eprintln!();
    print_table_header();
    let mut measured = Vec::new();
    let mut smoothing = Vec::new();
    for (filename, label, _, _, _, _) in DRC_RATCHETS {
        let (score, route_count, unrouted, entries, splits) =
            route_and_score(&pathfinder, filename);
        smoothing.push((label, splits));
        print_table_row(&BenchmarkResult::from_score(
            label,
            "PathFinder",
            &score,
            route_count,
            unrouted,
        ));
        measured.push((
            label,
            score.drc_violations,
            score.shorts,
            unrouted,
            route_count,
            entries,
        ));
    }
    print_table_footer();
    eprintln!();

    // Every fixture is measured and printed before anything fails. A test that
    // stops at the first bad row hides what the other boards did, and the
    // question these numbers answer - is this setting worth its cost - cannot
    // be read off one row.
    let mut failures: Vec<String> = Vec::new();

    for (label, splits) in &smoothing {
        eprintln!("  {label}:");
        failures.extend(smoothing_added_a_piece(label, splits));
    }

    let mut sharp_total = 0usize;
    let mut examined_total = 0usize;
    let mut refused_total = 0usize;

    for (
        (
            (label, violations, shorts, unrouted, route_count, (report, sharp)),
            (filename, _, ratchet, shorts_ratchet, baseline, _),
        ),
        (examined_floor, refused_ceiling, sharp_ratchet),
    ) in measured.iter().zip(DRC_RATCHETS).zip(ENTRY_CENSUS)
    {
        eprintln!(
            "  {}: {} routes, {} violations against {}, {} shorts against {}, {} unrouted",
            label, route_count, violations, ratchet, shorts, shorts_ratchet, unrouted
        );

        // How far the router has walked from the baseline the threshold was
        // built on. The threshold is that baseline plus a band, and until
        // 2026-09-13 nothing compared the baseline against a routed board:
        // `the_ratchets_are_the_routed_values_plus_their_bands` checks
        // `ratchet == baseline + band`, which is arithmetic between two stored
        // numbers and green whatever the router does. Four of the six had
        // drifted - stm32_breakout by 1, multi_ic by 2, shift_driver by 3,
        // qfp_fanout by 5 - every one still under its threshold, and nothing
        // said the headroom was shrinking.
        //
        // So the drift is printed, and half the band is where it fails. A
        // threshold reached is a cliff; half a band is a warning with room
        // left to act on it.
        let (band, _) = cypcb_autoroute::noise_band::noise_band(filename);
        let drift = violations.saturating_sub(*baseline);
        eprintln!(
            "      baseline {baseline} set when the band was measured, drift +{drift} of a band of {band}"
        );
        if i64::from(drift) * 2 > band {
            failures.push(format!(
                "{label}: routed {violations} against a baseline of {baseline}, which is +{drift} \
                 of a band of {band}. The threshold of {ratchet} is not reached yet and that is \
                 the point - more than half the band is spent, so the baseline is re-measured \
                 now rather than after the gate goes red."
            ));
        }

        // The census, printed rather than counted away. Every entry the
        // router's own copper leaves too sharp, named by its pin and its
        // angle, over the number of entries the rule was able to look at - so
        // the question "is this the router or the fixtures" has something to
        // be answered from, and a board with no sharp entries can be told from
        // a board with no entries.
        sharp_total += sharp.len();
        examined_total += report.examined;
        refused_total += report.refused;
        eprintln!(
            "      entries: {} examined, {} refused, {} sharp",
            report.examined,
            report.refused,
            sharp.len()
        );
        for entry in sharp {
            eprintln!("      sharp entry: {entry}");
        }

        if *unrouted != 0 {
            failures.push(format!(
                "{label}: {unrouted} unrouted connections, threshold 0"
            ));
        }
        if *route_count == 0 {
            failures.push(format!("{label}: routed nothing at all"));
        }
        if violations > ratchet {
            failures.push(format!(
                "{label}: {violations} DRC violations, threshold {ratchet} - the router got worse"
            ));
        }
        if shorts > shorts_ratchet {
            failures.push(format!(
                "{label}: {shorts} of the violations are copper touching copper, threshold {shorts_ratchet} - the router started shorting the board"
            ));
        }
        if sharp.len() > sharp_ratchet {
            failures.push(format!(
                "{label}: {} entries sharper than R-08 allows, ratchet {sharp_ratchet} - the router is meeting more lands at a wedge",
                sharp.len()
            ));
        }
        if report.examined < examined_floor {
            failures.push(format!(
                "{label}: only {} entries examined, floor {examined_floor} - the rule stopped looking, so the ratchet above proves nothing",
                report.examined
            ));
        }
        if report.refused > refused_ceiling {
            failures.push(format!(
                "{label}: {} entries refused, ceiling {refused_ceiling} - entries are losing their angle",
                report.refused
            ));
        }
    }

    eprintln!();
    eprintln!(
        "  across all fixtures: {examined_total} entries examined, {refused_total} refused, {sharp_total} sharp"
    );

    assert!(
        failures.is_empty(),
        "FAIL benchmark_all_fixtures_drc:\n  {}",
        failures.join("\n  ")
    );
}

/// Routes a board with `stop_at_own_copper` and reads `net-split` on either
/// side of the smoother.
fn splits_on_own_copper(mut world: BoardWorld, library: FootprintLibrary) -> Option<Splits> {
    let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, &world);
    let rules = ruleset_for_world(preset, &world);
    let config = AutorouteConfig {
        stop_at_own_copper: true,
        ..AutorouteConfig::default()
    };
    let result = route_board(&mut world, &library, &rules, &config);
    let drc_rules = DesignRules::from_constraints(&preset.constraints());
    splits_around_smoothing(&mut world, &library, &result, &drc_rules)
}

/// The smoother held to the same line with `stop_at_own_copper`, the setting
/// that ends a path on its net's own copper and so lays the T junctions the
/// smoother used to cut. No other stage routes a board with it on, so this
/// costs one routing of each board and measures nothing else.
///
/// None of the six fixtures has a junction the smoother before 2026-09-24
/// cut: with that smoother put back, all six read the same `net-split` on
/// both sides of it. `mains-sequencer` is the board it cut, so it is routed
/// here too - without it this test would pass on the fault it exists for.
#[test]
#[ignore = "slow: routes every fixture and mains-sequencer with stop_at_own_copper"]
fn smoothing_never_adds_a_net_piece_on_its_own_copper() {
    let mut failures = Vec::new();
    for (filename, label, _, _, _, _) in DRC_RATCHETS {
        let parsed = parse_kicad_pcb(&fixture_path(filename))
            .unwrap_or_else(|e| panic!("Failed to parse {}: {:?}", filename, e));
        let splits = splits_on_own_copper(parsed.world, parsed.library);
        eprintln!("  {label}:");
        failures.extend(smoothing_added_a_piece(label, &splits));
    }

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/mains-sequencer.cypcb");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()));
    let parsed = cypcb_parser::parse(&source);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let _ = sync_ast_to_world(&parsed.value, &source, &mut world, &mut library);
    let splits = splits_on_own_copper(world, library);
    eprintln!("  mains-sequencer:");
    failures.extend(smoothing_added_a_piece("mains-sequencer", &splits));
    assert!(
        failures.is_empty(),
        "FAIL smoothing_never_adds_a_net_piece_on_its_own_copper:\n  {}",
        failures.join("\n  ")
    );
}

/// Fast CI regression gate: routes led_blink with PathFinder and asserts
/// score thresholds. Non-ignored so it runs in `cargo test --workspace`.
#[test]
fn benchmark_regression() {
    let pathfinder = PathFinderStrategy;
    let (score, route_count, unrouted, _, _) = route_and_score(&pathfinder, "led_blink.kicad_pcb");

    // Print score table
    eprintln!();
    let result =
        BenchmarkResult::from_score("led_blink", "PathFinder", &score, route_count, unrouted);
    print_table_header();
    print_table_row(&result);
    print_table_footer();
    eprintln!();

    // --- Regression assertions with diagnostic messages ---
    //
    // Completeness comes first. Every other metric improves when the router
    // abandons connections - fewer traces means less length, fewer vias and
    // fewer DRC violations - so a gate that only reads quality scores rewards
    // giving up. This gate used to assert `route_count > 0` and passed while
    // PathFinder left a connection unrouted and emitted 7 routes.

    assert_eq!(
        unrouted, 0,
        "FAIL benchmark_regression: {} unrouted connections, threshold 0",
        unrouted
    );
    eprintln!("  ✓ unrouted: got {}, threshold 0", unrouted);

    // Copper, not segment count. The threshold used to be 20 segments, which
    // was a stand-in for "the router did not quietly give up" - a job the
    // `unrouted` assertion above already does properly. It went off when the
    // router started reserving copper and solved the same board in 18 segments
    // instead of 23, with the same 79mm of copper: fewer corners is better,
    // and a gate that calls it a regression is measuring the wrong thing.
    let copper_mm = score.total_length.0 as f64 / 1_000_000.0;
    assert!(
        copper_mm >= 70.0,
        "FAIL benchmark_regression: {:.1}mm of copper, threshold >= 70.0mm - the router is emitting far less than a complete solution needs",
        copper_mm
    );
    eprintln!(
        "  ✓ copper: got {:.1}mm in {} segments, threshold >= 70.0mm",
        copper_mm, route_count
    );

    // Quality thresholds are ratchets measured against a complete solution.
    // They are deliberately tight: lower them whenever the router improves,
    // never raise them to accommodate a regression. R107 targets 0 violations.
    //
    // Raised once, on 2026-08-06, and not for a regression: `composite` charges
    // 1000 per DRC violation, and the checker started seeing one that was
    // always there. Until that day the same-net exemption was per component, so
    // a GND trace crossing a part's non-GND pad was waved through because the
    // part had a GND pin somewhere. The board is unchanged - 42.6 of quality
    // score plus one 1000-point short. Put this back to 100.0 the moment the
    // router stops driving through sibling pads; do not raise it again.
    assert!(
        score.composite <= 2100.0,
        "FAIL benchmark_regression: composite got {:.1}, threshold ≤ 2100.0 (baseline 42.6 plus two known shorts at 1000 each)",
        score.composite
    );
    eprintln!(
        "  ✓ composite: got {:.1}, threshold ≤ 2100.0",
        score.composite
    );

    // One, for the same reason the composite threshold moved: the per-pad
    // same-net exemption exposed a GND trace crossing a part's non-GND pad,
    // which the router has always produced and the checker used to excuse.
    // R107 still targets 0, and this is the gap to it.
    assert!(
        score.drc_violations <= 2,
        "FAIL benchmark_regression: drc_violations got {}, threshold 2 - the known short is reported per segment now, anything more is new",
        score.drc_violations
    );
    eprintln!(
        "  ✓ drc_violations: got {}, threshold 2 (R107 targets 0)",
        score.drc_violations
    );

    assert!(
        score.smoothness >= 0.95,
        "FAIL benchmark_regression: smoothness got {:.3}, threshold ≥ 0.95",
        score.smoothness
    );
    eprintln!(
        "  ✓ smoothness: got {:.3}, threshold ≥ 0.95",
        score.smoothness
    );

    eprintln!();
    eprintln!("═══ benchmark_regression PASSED ═══");
    eprintln!(
        "  composite={:.1}  drc={}  smoothness={:.3}  vias={}  length={:.2}mm  routes={}",
        score.composite,
        score.drc_violations,
        score.smoothness,
        score.via_count,
        result.total_length_mm,
        route_count,
    );
}

/// Comprehensive benchmark: all 3 fixtures × 2 strategies.
/// Produces comparison table + JSON report. Confirms PathFinder as default.
#[test]
#[ignore = "5s: routes all fixtures with both strategies; named by the gate"]
fn benchmark_full_matrix() {
    let strategies: Vec<Box<dyn RoutingStrategy>> = vec![
        Box::new(PathFinderStrategy),
        Box::new(ImprovedAStarStrategy),
    ];

    let mut results: Vec<BenchmarkResult> = Vec::new();

    for benchmark in BENCHMARKS {
        let fixture_label = benchmark
            .filename
            .strip_suffix(".kicad_pcb")
            .unwrap_or(benchmark.filename);

        for strategy in &strategies {
            eprintln!("  [{}] routing {} ...", strategy.name(), fixture_label);

            let (score, route_count, unrouted, _, _) =
                route_and_score(strategy.as_ref(), benchmark.filename);

            let br = BenchmarkResult::from_score(
                fixture_label,
                strategy.name(),
                &score,
                route_count,
                unrouted,
            );
            results.push(br);
        }
    }

    // --- Print aggregate comparison table ---
    eprintln!();
    eprintln!("═══ Full Benchmark Matrix ═══");
    eprintln!();
    print_table_header();

    let mut first_fixture = true;
    let mut prev_fixture = String::new();
    for r in &results {
        if r.fixture != prev_fixture {
            if !first_fixture {
                print_table_separator();
            }
            first_fixture = false;
            prev_fixture = r.fixture.clone();
        }
        print_table_row(r);
    }
    print_table_footer();
    eprintln!();

    // --- Emit JSON report ---
    let json = serde_json::to_string(&results).expect("Failed to serialize benchmark results");
    eprintln!("BENCHMARK_JSON: {}", json);
    eprintln!();

    // --- Assert PathFinder ≤ ImprovedAStar on led_blink ---
    //
    // Matched case-insensitively because the names in this table come from
    // `StrategyKind`'s `Display` - `pathfinder`, `improved-astar` - and the
    // literals here said `PathFinder` and `ImprovedAStar`. The lookups found
    // nothing and the test died on `expect` before reaching the comparison it
    // exists for, which reads as a failing benchmark rather than a stale
    // string.
    let by_strategy = |name: &str| {
        results
            .iter()
            .find(|r| r.fixture == "led_blink" && r.strategy.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| {
                panic!(
                    "{name} led_blink result missing; the table holds {:?}",
                    results
                        .iter()
                        .map(|r| r.strategy.as_str())
                        .collect::<Vec<_>>()
                )
            })
    };
    let pf_led = by_strategy("pathfinder");
    let astar_led = by_strategy("improved-astar");

    assert!(
        pf_led.composite <= astar_led.composite,
        "FAIL benchmark_full_matrix: PathFinder composite ({:.1}) > ImprovedAStar ({:.1}) on led_blink. \
         PathFinder should be ≤ ImprovedAStar for empirical strategy selection.",
        pf_led.composite,
        astar_led.composite,
    );
    eprintln!(
        "✓ Strategy selection: PathFinder ({:.1}) ≤ ImprovedAStar ({:.1}) on led_blink",
        pf_led.composite, astar_led.composite,
    );

    // --- Assert route_count > 0 for all results ---
    for r in &results {
        assert!(
            r.route_count > 0,
            "FAIL benchmark_full_matrix: {} × {} produced 0 routes",
            r.fixture,
            r.strategy,
        );
    }

    eprintln!();
    eprintln!("═══ Default strategy: PathFinder (empirically validated) ═══");
    eprintln!();
}

/// A ratchet is the routed value plus that board's own noise band.
///
/// That sentence has been the convention since 2026-08-08 and lived only in a
/// comment. A band edited without its ratchet, or a ratchet moved without a
/// measurement, changed what the gate enforces and failed nothing: `noise_band`
/// feeds diagnostics, `DRC_RATCHETS` is a constant, and the two were tied
/// together by prose. This is the arithmetic, checked.
///
/// It does not route anything - it reads three numbers per board - so it runs
/// in the ordinary `cargo test` rather than behind `--ignored`.
#[test]
fn the_ratchets_are_the_routed_values_plus_their_bands() {
    for (filename, label, ratchet, shorts_ratchet, routed, routed_shorts) in DRC_RATCHETS {
        let (band, shorts_band) = cypcb_autoroute::noise_band::noise_band(filename);
        assert_eq!(
            u64::from(*ratchet),
            u64::from(*routed) + band as u64,
            "{label}: the violation ratchet is the routed {routed} plus its band {band}"
        );
        assert_eq!(
            u64::from(*shorts_ratchet),
            u64::from(*routed_shorts) + shorts_band as u64,
            "{label}: the shorts ratchet is the routed {routed_shorts} plus its band {shorts_band}"
        );
    }
}
