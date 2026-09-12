//! Where the sharp entries sit - the anatomy of every entry R-08 reports on
//! the six benchmark boards.
//!
//! The census in `benchmark_validation` counts them. It cannot say what they
//! have in common, and two readings were open when it landed: the router
//! searches a grid whose cells the pads do not sit on, or the fixtures'
//! footprints are turned in a way that puts a land's side at an angle to
//! every direction the router can travel. Neither needs a change to the
//! router to tell apart.
//!
//! So this measures, for every pad the entry walk can reach, how far its
//! centre lies from the centre of the grid cell it falls in - on the grid
//! this board was actually routed on, at the resolution the router resolves
//! for it. The sharp pads are then compared against that whole population
//! rather than against nothing, because a property shared by every pad on the
//! board explains no subset of them.
//!
//! It also records each land's outline. An entry into a rectangle and an
//! entry into the rounded end of an oblong are not the same measurement: on a
//! rectangle both trace edges cross a straight side at the angle the arm
//! makes with it, and on a round end the reading sits below the arm's angle
//! by an amount that grows with trace width. Figures from the two are not
//! comparable, and a cluster read across both would be an artefact.

use std::path::Path;

use cypcb_autoroute::grid::RoutingGrid;
use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::rules::pad_entry::{entry_records, pad_centre};
use cypcb_drc::{preset_for_world, ruleset_for_world};
use cypcb_kicad::{parse_kicad_pcb, BENCHMARKS};
use cypcb_router::apply_routes;
use cypcb_rules::presets::RulesPreset;
use cypcb_world::components::{FootprintRef, NetConnections, Position, RefDes, Rotation};
use cypcb_world::PadShape;

/// How far a pad centre may sit from a cell centre and still be called
/// on-grid. One micrometre: the fixtures place parts on a 10nm lattice at
/// worst, so this is a rounding allowance, not a tolerance that decides
/// anything.
const ON_GRID_NM: i64 = 1_000;

fn fixture_path(filename: &str) -> std::path::PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// One pad the entry walk could reach, with the facts that bear on its angle.
struct PadFact {
    pin: String,
    /// The angle R-08 reported, or `None` when this pad was not reported.
    sharp_degrees: Option<f64>,
    shape: &'static str,
    size_mm: (f64, f64),
    part_rotation_deg: f64,
    centre_mm: (f64, f64),
    off_grid_nm: (i64, i64),
}

impl PadFact {
    fn on_grid(&self) -> bool {
        self.off_grid_nm.0.abs() <= ON_GRID_NM && self.off_grid_nm.1.abs() <= ON_GRID_NM
    }
}

/// One segment that crossed a land's boundary, reduced to the two facts the
/// segment-order question needs.
struct EntryFact {
    pin: String,
    sharp: bool,
    /// Whether it is the last segment of its own trace.
    last: bool,
    /// Whether it is the first.
    first: bool,
    /// Its position in the trace, and the trace's length in segments.
    index: usize,
    count: usize,
    /// Which trace on its board carried it.
    trace: usize,
    /// Whether the end in the land is an end of the whole track, which is the
    /// only junction the Gerber writer grows a fillet from.
    trace_end: bool,
    /// The length of the entering segment itself, in millimetres.
    length_mm: f64,
}

fn shape_name(shape: &PadShape) -> &'static str {
    match shape {
        PadShape::Circle => "circle",
        PadShape::Rect => "rect",
        PadShape::RoundRect { .. } => "roundrect",
        PadShape::Oblong => "oblong",
    }
}

/// Route a fixture, then describe every netted pad on it.
fn pad_facts(fixture: &str) -> (i64, Vec<PadFact>, Vec<EntryFact>) {
    let parsed = parse_kicad_pcb(&fixture_path(fixture))
        .unwrap_or_else(|e| panic!("Failed to parse {}: {:?}", fixture, e));
    let mut world = parsed.world;
    let library = parsed.library;
    let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, &world);
    let rules = ruleset_for_world(preset, &world);
    let config = AutorouteConfig::default();

    let result = route_board(&mut world, &library, &rules, &config);
    apply_routes(&mut world, &result);
    world.rebuild_spatial_index_from_library(&library);

    // The same resolution rule the router resolves, so "off-grid" means off
    // the grid this board was routed on and not off some grid this test chose.
    let resolution = match world.board_info() {
        Some((size, _)) => {
            config.resolve_adaptive_grid_resolution(&rules, size.width.raw(), size.height.raw())
        }
        None => config.resolve_grid_resolution(&rules),
    };
    let grid = RoutingGrid::from_board(&mut world, &library, &rules, resolution)
        .expect("every benchmark fixture has a board entity");

    // The entries come from the rule's own walk rather than a second one, so
    // the set this test describes and the set the census counts cannot differ.
    let records = entry_records(&mut world);
    let sharp: Vec<(String, f64)> = records
        .iter()
        .filter(|r| r.entry.is_violation())
        .filter_map(|r| {
            r.entry
                .millideg()
                .map(|millideg| (r.pin.clone(), f64::from(millideg) / 1_000.0))
        })
        .collect();

    // Where in its own trace each entering segment sits, and how long it is.
    // A route that ends by leaving the lattice ends on a segment whose
    // direction the grid never constrained, so "is the entry the last
    // segment" is the question the grid measurement left behind.
    let entries: Vec<EntryFact> = records
        .iter()
        .map(|r| EntryFact {
            pin: r.pin.clone(),
            sharp: r.entry.is_violation(),
            last: r.segment_index + 1 == r.segment_count,
            first: r.segment_index == 0,
            index: r.segment_index,
            count: r.segment_count,
            trace: r.trace_index,
            trace_end: r.inside_is_trace_end,
            length_mm: {
                let dx = (r.inside.x.raw() - r.outside.x.raw()) as f64;
                let dy = (r.inside.y.raw() - r.outside.y.raw()) as f64;
                (dx * dx + dy * dy).sqrt() / 1_000_000.0
            },
        })
        .collect();

    let components: Vec<_> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(
            &RefDes,
            &FootprintRef,
            &NetConnections,
            &Position,
            &Rotation,
        )>();
        query
            .iter(ecs)
            .map(|(r, f, n, p, rot)| (r.clone(), f.clone(), n.clone(), *p, *rot))
            .collect()
    };

    let footprints = world.footprints();
    let mut facts = Vec::new();
    for (refdes, footprint_ref, nets, position, rotation) in &components {
        let Some(footprint) = footprints.get(footprint_ref.as_str()) else {
            continue;
        };
        let rotation_deg = rotation.to_degrees();
        for pad in &footprint.pads {
            if nets.pin_net(&pad.number).is_none() {
                continue; // no net, no entry to measure
            }
            let pin = format!("{}.{}", refdes.as_str(), pad.number);
            let centre = pad_centre(pad, position.0, rotation_deg);
            let cell_x = grid.grid_to_nm_x(grid.nm_to_grid_x(centre.x.raw()));
            let cell_y = grid.grid_to_nm_y(grid.nm_to_grid_y(centre.y.raw()));
            facts.push(PadFact {
                sharp_degrees: sharp
                    .iter()
                    .find(|(name, _)| *name == pin)
                    .map(|(_, degrees)| *degrees),
                pin,
                shape: shape_name(&pad.shape),
                size_mm: (pad.size.0.to_mm(), pad.size.1.to_mm()),
                part_rotation_deg: rotation_deg,
                centre_mm: (centre.x.to_mm(), centre.y.to_mm()),
                off_grid_nm: (centre.x.raw() - cell_x, centre.y.raw() - cell_y),
            });
        }
    }
    (resolution, facts, entries)
}

#[test]
#[ignore = "slow: routes all six benchmark fixtures"]
fn the_sharp_entries_against_every_pad_that_could_have_been_one() {
    let mut sharp: Vec<PadFact> = Vec::new();
    let mut netted_total = 0usize;
    let mut netted_on_grid = 0usize;
    let mut netted_turned = 0usize;
    let mut all_entries: Vec<EntryFact> = Vec::new();
    let mut most_lands_on_one_trace = 0usize;

    for benchmark in BENCHMARKS {
        let label = benchmark
            .filename
            .strip_suffix(".kicad_pcb")
            .unwrap_or(benchmark.filename);
        let (resolution, facts, entries) = pad_facts(benchmark.filename);

        let on_grid = facts.iter().filter(|f| f.on_grid()).count();
        netted_total += facts.len();
        netted_on_grid += on_grid;
        netted_turned += facts.iter().filter(|f| f.part_rotation_deg != 0.0).count();
        // How many lands one trace enters. One `Trace` carries a whole net's
        // copper, so this is what says that a segment's position in a trace is
        // not its position in an approach to a pad - and it is counted rather
        // than assumed.
        let mut per_trace: std::collections::BTreeMap<usize, usize> =
            std::collections::BTreeMap::new();
        for entry in &entries {
            *per_trace.entry(entry.trace).or_default() += 1;
        }
        most_lands_on_one_trace =
            most_lands_on_one_trace.max(per_trace.values().copied().max().unwrap_or(0));
        all_entries.extend(entries);

        eprintln!();
        eprintln!(
            "=== {label}: grid {:.3}mm, {} netted pads, {} of them on-grid",
            resolution as f64 / 1_000_000.0,
            facts.len(),
            on_grid
        );

        for fact in facts.into_iter().filter(|f| f.sharp_degrees.is_some()) {
            eprintln!(
                "  {:<8} {:>5.1} deg  {:<9} {:.3}x{:.3}mm  part {:>5.1} deg  centre {:.4},{:.4}mm  off-grid {:>7},{:>7} nm  {}",
                fact.pin,
                fact.sharp_degrees.unwrap_or_default(),
                fact.shape,
                fact.size_mm.0,
                fact.size_mm.1,
                fact.part_rotation_deg,
                fact.centre_mm.0,
                fact.centre_mm.1,
                fact.off_grid_nm.0,
                fact.off_grid_nm.1,
                if fact.on_grid() { "ON-GRID" } else { "off" }
            );
            sharp.push(fact);
        }
    }

    let entries_total = all_entries.len();
    let entries_last = all_entries.iter().filter(|e| e.last).count();
    let sharp_entries: Vec<&EntryFact> = all_entries.iter().filter(|e| e.sharp).collect();
    let sharp_last = sharp_entries.iter().filter(|e| e.last).count();
    let mean = |set: &[&EntryFact]| -> f64 {
        if set.is_empty() {
            0.0
        } else {
            set.iter().map(|e| e.length_mm).sum::<f64>() / set.len() as f64
        }
    };
    let clean_entries: Vec<&EntryFact> = all_entries.iter().filter(|e| !e.sharp).collect();
    eprintln!();
    let ends_total = all_entries.iter().filter(|e| e.trace_end).count();
    let sharp_ends = sharp_entries.iter().filter(|e| e.trace_end).count();
    eprintln!(
        "track ends landing in a land: {ends_total} of {entries_total} entries, \
         {sharp_ends} of the {} sharp",
        sharp_entries.len()
    );
    let entries_first = all_entries.iter().filter(|e| e.first).count();
    let sharp_first = sharp_entries.iter().filter(|e| e.first).count();
    eprintln!(
        "entering segments: last {entries_last} of {entries_total}, first {entries_first}; \
         of the {} sharp ones, last {sharp_last}, first {sharp_first}",
        sharp_entries.len()
    );
    for e in &sharp_entries {
        eprintln!(
            "  {:<8} segment {} of {}  length {:.3}mm",
            e.pin,
            e.index + 1,
            e.count,
            e.length_mm
        );
    }
    eprintln!(
        "entering segment length: sharp mean {:.3}mm, clean mean {:.3}mm",
        mean(&sharp_entries),
        mean(&clean_entries)
    );

    let sharp_on_grid = sharp.iter().filter(|f| f.on_grid()).count();
    let sharp_turned = sharp.iter().filter(|f| f.part_rotation_deg != 0.0).count();
    eprintln!();
    eprintln!(
        "sharp entries: {sharp_on_grid} of {} on a grid cell centre, {sharp_turned} on a turned part",
        sharp.len()
    );
    eprintln!(
        "netted pads:   {netted_on_grid} of {netted_total} on a grid cell centre, {netted_turned} on a turned part"
    );
    for outline in ["rect", "roundrect", "oblong", "circle"] {
        eprintln!(
            "  sharp lands shaped {outline}: {}",
            sharp.iter().filter(|f| f.shape == outline).count()
        );
    }

    eprintln!("most entries carried by one trace: {most_lands_on_one_trace}");

    // The census this is read beside. If the two disagree, one of them
    // measured a different board.
    assert_eq!(
        sharp.len(),
        14,
        "the census reports fourteen sharp entries across the six fixtures"
    );

    // The first reading, and the denominator that kills it. Every sharp pad is
    // off-grid - and so is essentially every pad on every board, including the
    // seven hundred and sixty-odd that are entered cleanly. A property shared
    // by the whole population explains no subset of it, so being off-grid is
    // not what the fourteen have in common and no repair aimed at the grid can
    // be justified by them.
    assert_eq!(
        sharp_on_grid, 0,
        "no sharp entry sits on a grid cell centre"
    );
    assert!(
        netted_on_grid * 100 < netted_total,
        "off-grid pads are the rule, not the exception: {netted_on_grid} of {netted_total} \
         netted pads sit on a cell centre, so being off-grid cannot distinguish the fourteen"
    );

    // The second reading does not get refuted here - it gets ruled out of
    // court. Not one part on any of the six boards is turned, so their
    // footprints' rotations cannot explain a subset of anything on them. This
    // assertion cannot tell a correct reader from one that always answers
    // zero, and the evidence that makes it true is outside this crate: of the
    // 174 footprints in the six fixtures, `grep -cE "^\\s*\\(at [-0-9.]+
    // [-0-9.]+ [-0-9.]+\\)" *.kicad_pcb` in `tests/fixtures/benchmark` finds
    // two placements carrying a rotation at all, `(at 100 70 0)` and
    // `(at 115 72 0)`, and both of those are zero. Answering the rotation
    // question needs a board with turned parts on it, which these are not.
    assert_eq!(
        netted_turned, 0,
        "no part on any benchmark fixture is turned, so rotation explains nothing here"
    );

    // What the fourteen do have in common, and the reason the published
    // rectangle arithmetic does not reach them: not one of these lands is a
    // plain rectangle. Every one is a rounded rectangle or an oblong, both of
    // which curve where the trace crosses, so the entry angle depends on the
    // trace width as well as on the direction it arrives from.
    assert_eq!(
        sharp.iter().filter(|f| f.shape == "rect").count(),
        0,
        "every sharp land is a roundrect or an oblong, so a rectangle's edge arithmetic does not apply to any of them"
    );

    // R-08's two halves are not about the same junctions, and this is the
    // number that says so. The angle half measures every crossing of a land's
    // boundary; the teardrop half can only ever fillet a track's own end
    // landing inside a pad, which the Gerber writer says in as many words. A
    // condition written as though the two sets were one would be a condition
    // about a set that does not exist.
    assert!(
        ends_total < entries_total,
        "a track end in a land is a narrower thing than a crossing of its boundary: \
         {ends_total} of {entries_total}"
    );

    // The reading the grid measurement left: that the sharp entries are the
    // final off-lattice segments, the ones whose direction the grid never
    // constrained. They are not. Sharp entries are if anything rarer at a
    // trace's end than entries in general, and the comparison is a ratio
    // rather than a count because 1 of 14 means nothing without the 184 of
    // 897 beside it.
    assert!(
        sharp_last * entries_total < entries_last * sharp_entries.len(),
        "sharp entries are rarer at a trace's end than entries in general: {sharp_last} of {} against \
         {entries_last} of {entries_total}",
        sharp_entries.len()
    );

    // And the reason that comparison is the end of this line of questioning
    // rather than the start of another: a trace here is a net's copper, not a
    // pad-to-pad route, so a segment's position in it was never the position
    // of an approach. One trace enters many lands.
    assert!(
        most_lands_on_one_trace > 1,
        "one trace enters more than one land, so segment position is not approach position"
    );
}
