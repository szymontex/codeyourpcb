//! Can a 45 degree junction be rewritten where it stands, or does the path need routing again?
//!
//! `cargo test -p cypcb-autoroute --test can_a_wedge_be_cut_where_it_stands -- --nocapture`
//!
//! Every acute corner this router draws is 45 degrees and comes out of the
//! search, measured in `where_the_acute_corners_come_from`. A 45 degree
//! interior angle on an eight-way grid is a diagonal step followed by a turn
//! of 135: copper doubling back on itself. Two ways out, and they cost very
//! different amounts. A pass after the search rewrites the junction and leaves
//! every measurement in the project standing. A bend penalty in the cost
//! function cures the cause and re-bases all of it: `neighbor_cost` sees a step
//! and never the step before it, so the node has to carry its entry direction
//! and the state space multiplies by eight.
//!
//! What decides between them is whether the copper has room for the rewrite.
//!
//! **The shape of the rewrite, and why it cannot be symmetric.** Cut the apex
//! off: take a point on one arm at distance `a`, a point on the other at
//! `a * sqrt(2)`. With arms 45 degrees apart the chord between those two
//! points runs at a multiple of 45 and the two new interior angles are 90 and
//! 135, so all three segments obey the direction rule and neither new corner
//! is acute. Cutting both arms at the same distance looks more natural and is
//! wrong: the chord then runs at 112.5 degrees, which the router's own
//! `is_valid_angle` rejects. There are two assignments of `a` and
//! `a * sqrt(2)` to the two arms; one passing is enough.
//!
//! **Where the length floor comes from.** Two bands of copper of width `w`
//! whose axes meet at 45 degrees are already one solid body within
//! `w / (2 * sin(22.5 degrees))` = 1.307 * w of the apex. A cut closer than
//! that lies inside copper that is continuous anyway - it moves the wedge
//! rather than removing it. Hence `a >= 1.5 * w`, 1.307 rounded up to a number
//! that can be written without a footnote about square roots.
//!
//! **What this measures and what it does not.** Length only. A junction whose
//! shorter arm cannot hold the cut at any `a` cannot be rewritten where it
//! stands whatever else is true, so the count of those is exact. The
//! complement is not: a junction with the length for a cut may still not have
//! the clearance for one, and that is a separate measurement which has to
//! compare copper edges rather than axes - the router's own `is_drc_clean`
//! measures axis to axis, so reading it as a gap is off by half a width on
//! each side.

use std::path::Path;

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::{preset_for_world, ruleset_for_world, run_drc, DesignRules, ViolationKind};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_router::apply_routes;
use cypcb_router::types::RouteSegment;

const FIXTURES: &[&str] = &[
    "led_blink.kicad_pcb",
    "stm32_breakout.kicad_pcb",
    "multi_ic.kicad_pcb",
    "shift_driver.kicad_pcb",
    "qfp_fanout.kicad_pcb",
    "plane_board.kicad_pcb",
];

/// The floor on a cut, in widths. See the module note.
const CUT: f64 = 1.5;

fn fixture_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// One junction where two arms of one net leave a point at under 90 degrees.
struct Wedge {
    /// Length of each arm, in units of the wider of the two arms' widths.
    arms: [f64; 2],
}

impl Wedge {
    /// Whether the apex can be cut off without leaving the solid copper the
    /// two bands already make, for at least one of the two assignments.
    fn cuttable(&self) -> bool {
        let [short, long] = if self.arms[0] <= self.arms[1] {
            [self.arms[0], self.arms[1]]
        } else {
            [self.arms[1], self.arms[0]]
        };
        // The cut is `a` on one arm and `a * sqrt(2)` on the other, so the
        // only assignment worth testing puts the longer reach on the longer
        // arm. The other one is strictly harder on the same arms.
        short >= CUT && long >= CUT * std::f64::consts::SQRT_2
    }
}

/// What one board's routed segments hold: the wedges, and the junctions where
/// the router laid copper back along copper it had just laid.
struct Scan {
    wedges: Vec<Wedge>,
    overlaps: usize,
}

/// Every acute junction the router drew on one board, arms measured in widths.
fn wedges(routes: &[RouteSegment]) -> Scan {
    use std::collections::BTreeMap;

    // An arm is one end of one segment: where it starts and where it goes.
    type Arm = (i64, i64, i64); // far x, far y, width
    let mut ends: BTreeMap<(u32, u32, i64, i64), Vec<Arm>> = BTreeMap::new();

    for segment in routes {
        if segment.start == segment.end {
            continue;
        }
        let key = (segment.net_id.id(), segment.layer.to_copper_mask());
        ends.entry((key.0, key.1, segment.start.x.0, segment.start.y.0))
            .or_default()
            .push((segment.end.x.0, segment.end.y.0, segment.width.0));
        ends.entry((key.0, key.1, segment.end.x.0, segment.end.y.0))
            .or_default()
            .push((segment.start.x.0, segment.start.y.0, segment.width.0));
    }

    let mut found = Vec::new();
    let mut overlaps = 0;
    for ((_, _, x, y), arms) in &ends {
        if arms.len() < 2 {
            continue;
        }
        // Copper laid back along copper, inside this run's own output.
        if arms.iter().enumerate().any(|(i, first)| {
            arms[i + 1..].iter().any(|second| {
                let (ax, ay) = ((first.0 - x) as i128, (first.1 - y) as i128);
                let (bx, by) = ((second.0 - x) as i128, (second.1 - y) as i128);
                ax * bx + ay * by > 0 && ax * by - ay * bx == 0
            })
        }) {
            overlaps += 1;
        }
        // The sharpest pair at this point, which is the one the rule reports.
        let mut sharpest: Option<(f64, [f64; 2])> = None;
        for (i, first) in arms.iter().enumerate() {
            for second in &arms[i + 1..] {
                let (ax, ay) = ((first.0 - x) as i128, (first.1 - y) as i128);
                let (bx, by) = ((second.0 - x) as i128, (second.1 - y) as i128);
                let dot = ax * bx + ay * by;
                if dot <= 0 {
                    continue;
                }
                // Copper drawn over itself is not a wedge: collinear arms
                // leave no gap to cut. The rule says the same thing in words.
                if ax * by - ay * bx == 0 {
                    continue;
                }
                let width = first.2.max(second.2) as f64;
                let lengths = [
                    ((ax * ax + ay * ay) as f64).sqrt() / width,
                    ((bx * bx + by * by) as f64).sqrt() / width,
                ];
                let sharpness = dot as f64
                    / (((ax * ax + ay * ay) as f64).sqrt() * ((bx * bx + by * by) as f64).sqrt());
                if sharpest.is_none_or(|(best, _)| sharpness > best) {
                    sharpest = Some((sharpness, lengths));
                }
            }
        }
        if let Some((_, arms)) = sharpest {
            found.push(Wedge { arms });
        }
    }
    Scan {
        wedges: found,
        overlaps,
    }
}

/// Route one fixture and hand back its wedges and what the checker counted.
fn measured(fixture: &str) -> (Scan, usize, usize) {
    let parsed = parse_kicad_pcb(&fixture_path(fixture))
        .unwrap_or_else(|e| panic!("failed to parse {fixture}: {e:?}"));
    let mut world = parsed.world;
    let library = parsed.library;
    let preset = preset_for_world(
        cypcb_rules::presets::RulesPreset::JlcpcbStandard2Layer,
        &world,
    );
    let rules = ruleset_for_world(preset, &world);

    let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());
    let found = wedges(&result.routes);

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
    let wedged = acute
        .iter()
        .filter(|violation| violation.message.contains(" degrees"))
        .count();
    (found, wedged, acute.len() - wedged)
}

#[test]
fn the_wedges_this_scan_finds_are_the_ones_the_checker_reports() {
    // The control for every number below. This walks the router's own
    // segments; the rule walks the traces they became in a world that also
    // holds whatever copper the fixture was drawn with. Two implementations of
    // one predicate drift apart silently, so they are held against each other
    // on every board.
    //
    // They are not equal, and the difference is the rule's own reporting: a
    // junction where copper doubles back exactly along itself is reported as
    // drawn over itself rather than as a wedge, even when a third arm leaves
    // the same point at 45 degrees. That arm is a wedge and this scan counts
    // it, so the scan may come in over the rule's wedge count - never over its
    // total.
    for fixture in FIXTURES {
        let (scan, wedged, overlapping) = measured(fixture);
        println!(
            "{fixture:<26} checker {:>4} acute = {wedged:>3} wedges + {overlapping:>3} drawn \
             over itself   scan {:>3} wedges, {:>3} overlaps in the router's own copper",
            wedged + overlapping,
            scan.wedges.len(),
            scan.overlaps
        );
        assert!(
            scan.wedges.len() <= wedged + overlapping,
            "{fixture}: the scan found more junctions than the rule reported"
        );
        assert!(
            wedged == 0 || !scan.wedges.is_empty(),
            "{fixture}: the rule reported {wedged} wedges and the scan found none"
        );
    }
}

#[test]
fn the_router_lays_copper_back_along_its_own_copper() {
    // 142 of the 195 acute reports are copper laid back along copper rather
    // than a wedge, and the first guess about them was wrong: the guess was
    // that the fixture arrived with copper and the router added a second copy,
    // which is the defect the `.cypcb` writer carried until 2026-09-11. It is
    // not. Measured on the router's output alone, before any of it reaches a
    // world that holds anything else, **129 of the 142 are already there**.
    //
    // Per board, reported and then how many of them the router's own segments
    // hold: led_blink 0/0, stm32_breakout 14/10, multi_ic 35/30,
    // shift_driver 4/2, qfp_fanout 89/87, plane_board 0/0.
    let mut own = 0;
    let mut reported = 0;
    for fixture in FIXTURES {
        let (scan, _, overlapping) = measured(fixture);
        own += scan.overlaps;
        reported += overlapping;
    }
    println!("copper drawn over itself: {reported} reported, {own} inside the router's own output");

    // The control: a scan that found nothing inside the router would make the
    // claim below vacuous, and so would a rule that had stopped reporting.
    assert!(reported >= 100, "the rule reported {reported} of them");
    // The other side of the same control: what the router holds is a subset of
    // what the rule sees, because the rule reads a world that holds this
    // copper and more. A count that runs past it is counting something else -
    // a straight join read as a doubling back, for one.
    assert!(
        own <= reported,
        "{own} overlaps inside the router against {reported} the rule reports"
    );
    assert!(
        own * 100 / reported >= 80,
        "{own} of {reported} are inside the router's own output, and the \
         finding is that most of them are"
    );
}

#[test]
fn how_many_wedges_have_the_room_to_be_cut() {
    let mut total = 0;
    let mut cuttable = 0;
    let mut shortest_arms: Vec<f64> = Vec::new();

    for fixture in FIXTURES {
        let (scan, _, _) = measured(fixture);
        let here = scan.wedges.len();
        let ok = scan.wedges.iter().filter(|wedge| wedge.cuttable()).count();
        let mut shortest: Vec<f64> = scan
            .wedges
            .iter()
            .map(|wedge| wedge.arms[0].min(wedge.arms[1]))
            .collect();
        shortest.sort_by(f64::total_cmp);
        let median = shortest.get(shortest.len() / 2).copied().unwrap_or(0.0);
        println!(
            "{fixture:<26} {here:>4} wedges, {ok:>4} with room to cut, \
             shortest arm median {median:.2}w, min {:.2}w",
            shortest.first().copied().unwrap_or(0.0)
        );
        total += here;
        cuttable += ok;
        shortest_arms.extend(shortest);
    }

    shortest_arms.sort_by(f64::total_cmp);
    let median = shortest_arms[shortest_arms.len() / 2];
    println!(
        "all six boards: {total} wedges, {cuttable} with room to cut, \
         shortest arm median {median:.2}w"
    );

    // The positive control: a scan that found nothing would satisfy a ratio.
    assert!(
        total >= 50,
        "the six boards drew 53 wedges on 2026-09-11 and this scan sees {total}"
    );
    // A ratchet on the answer rather than on a threshold nobody measured: the
    // share with room to cut may rise, and a fall means a board changed shape.
    assert!(
        cuttable * 100 / total >= 90,
        "{cuttable} of {total} junctions have room for the cut"
    );
}
