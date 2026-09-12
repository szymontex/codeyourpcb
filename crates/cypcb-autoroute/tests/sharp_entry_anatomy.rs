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
    /// The width of the entering segment, in millimetres.
    width_mm: f64,
    /// The smaller of the land's two dimensions, in millimetres. `None` when
    /// the pad this entry names is not among the netted pads described above,
    /// which would mean the two walks disagree about what is on the board.
    land_min_mm: Option<f64>,
    /// The land's outline, joined from the pad walk by the same name.
    shape: &'static str,
    /// The land's larger dimension, so a square land can be told from an
    /// oblong one without measuring the pad a second time.
    land_max_mm: Option<f64>,
    /// The pad's centre, from `pad_centre` - the same centre the rule's own
    /// measurement used.
    centre_mm: Option<(f64, f64)>,
    /// The segment's end inside the land's copper, in millimetres.
    inside_mm: (f64, f64),
    /// Its end outside, in millimetres.
    outside_mm: (f64, f64),
    /// The angle R-08 reported for this entry, in millidegrees. `None` when
    /// the rule refused to measure it.
    reported_millideg: Option<u32>,
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
    let mut entries: Vec<EntryFact> = records
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
            width_mm: r.width.to_mm(),
            land_min_mm: None,
            shape: "unjoined",
            land_max_mm: None,
            centre_mm: None,
            inside_mm: (r.inside.x.to_mm(), r.inside.y.to_mm()),
            outside_mm: (r.outside.x.to_mm(), r.outside.y.to_mm()),
            reported_millideg: r.entry.millideg(),
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
    // The land each entry went into, joined by the name the two walks share.
    // A width means nothing without the land it is being called narrow
    // against, and measuring the pad a second time here is how two readings of
    // one pad start to disagree.
    type LandFacts = (f64, f64, &'static str, (f64, f64));
    let land: std::collections::BTreeMap<&str, LandFacts> = facts
        .iter()
        .map(|f| {
            (
                f.pin.as_str(),
                (
                    f.size_mm.0.min(f.size_mm.1),
                    f.size_mm.0.max(f.size_mm.1),
                    f.shape,
                    f.centre_mm,
                ),
            )
        })
        .collect();
    for entry in &mut entries {
        if let Some((min, max, shape, centre)) = land.get(entry.pin.as_str()).copied() {
            entry.land_min_mm = Some(min);
            entry.land_max_mm = Some(max);
            entry.shape = shape;
            entry.centre_mm = Some(centre);
        }
    }
    drop(land);

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
    // The population a declaration-side teardrop rule would speak about. R-08's
    // second condition implies it: a track end landing in a land wider than the
    // track is the junction a fillet exists to thicken. The count is read
    // against two denominators - every track end, and every entry - because a
    // property most entries have is not a property of the ends.
    let narrower = |e: &&EntryFact| e.land_min_mm.is_some_and(|land| e.width_mm < land);
    let ends: Vec<&EntryFact> = all_entries.iter().filter(|e| e.trace_end).collect();
    let ends_joined = ends.iter().filter(|e| e.land_min_mm.is_some()).count();
    let ends_narrow = ends.iter().copied().filter(narrower).count();
    let entries_narrow = all_entries.iter().filter(narrower).count();
    let sharp_ends_narrow = sharp_entries
        .iter()
        .copied()
        .filter(|e| e.trace_end)
        .filter(narrower)
        .count();
    eprintln!(
        "track narrower than its land: {ends_narrow} of the {ends_total} track ends, \
         {entries_narrow} of {entries_total} entries, \
         {sharp_ends_narrow} of the {sharp_ends} sharp track ends; \
         {ends_joined} of the ends name a land this walk described"
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

    // The join itself, before any conclusion is drawn from it. Every track end
    // names a land the pad walk also described; a count of "narrower than its
    // land" computed over entries whose land was never found would be a count
    // of how many lookups succeeded.
    assert_eq!(
        ends_joined, ends_total,
        "every track end names a land this test described: {ends_joined} of {ends_total}"
    );

    // And the measurement that decides where a teardrop rule can stand. R-08's
    // second condition, read as a property of the design, says a track end
    // landing in a land wider than the track wants a fillet. Every entry on
    // all six boards satisfies it - 897 of 897 - which is what a land is for:
    // a pad narrower than the track it receives would be a pad the track
    // covers. A condition the whole population meets picks out no subset of
    // it, so it cannot stand in front of the `teardrops` declaration without
    // flagging every junction on every board nobody flags. It stands behind
    // the declaration or it is not a rule.
    assert_eq!(
        entries_narrow, entries_total,
        "a track narrower than its land is what every junction on these boards looks like: \
         {entries_narrow} of {entries_total}"
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

/// One entry into a land that is a circle, measured from the land's own
/// geometry instead of from the rule's answer.
///
/// The radius is the land's own dimension halved, and it is only defined when
/// both dimensions are equal: an oblong whose sides differ has two straight
/// flanks and a reading taken against a circle would be a reading of the wrong
/// shape. `PadShape::Oblong` with equal sides degenerates to a circle, which is
/// why the outline alone does not decide it.
struct CircularReading {
    pin: String,
    sharp: bool,
    width_mm: f64,
    radius_mm: f64,
    /// The centre's distance from the line through both ends of the segment.
    p_axis_mm: f64,
    /// The far edge of the copper: the axis distance plus half the width.
    p_far_mm: f64,
    reported_millideg: Option<u32>,
    /// `None` when `p_far` exceeds the radius, which is the edge missing the
    /// land entirely rather than a disagreement.
    recomputed_millideg: Option<i64>,
    /// How far past the rim the inside end sits, along the direction of travel.
    depth_mm: Option<f64>,
    /// How many of the trace's two edges start inside the land. One means the
    /// rule had only the inner edge to read, which is a blunter angle than the
    /// entry actually makes.
    edges_inside: usize,
}

fn circular_reading(e: &EntryFact) -> Option<CircularReading> {
    let (min, max) = (e.land_min_mm?, e.land_max_mm?);
    if (max - min).abs() > 1e-9 {
        return None; // two straight flanks, not a circle
    }
    if e.shape != "circle" && e.shape != "oblong" {
        return None; // a square roundrect still has flat sides
    }
    let centre = e.centre_mm?;
    let radius_mm = min / 2.0;
    let u = (
        e.outside_mm.0 - e.inside_mm.0,
        e.outside_mm.1 - e.inside_mm.1,
    );
    let length = (u.0 * u.0 + u.1 * u.1).sqrt();
    if length == 0.0 {
        return None;
    }
    let to_centre = (centre.0 - e.inside_mm.0, centre.1 - e.inside_mm.1);
    let p_axis_mm = (u.0 * to_centre.1 - u.1 * to_centre.0).abs() / length;
    let p_far_mm = p_axis_mm + e.width_mm / 2.0;
    let unit = (u.0 / length, u.1 / length);

    // The rule takes the smaller of the two trace edges' angles, and an edge
    // whose own end is not inside the land has no angle to give. Reproducing
    // that is not fitting the arithmetic to the answer: `entry_angle` asks
    // `leaving_angle` for each edge in turn and reduces with `f64::min` over
    // the ones that returned something. An edge offset half a width sideways
    // from an end that barely crossed the rim starts outside the copper, and
    // the reading falls back to the other edge.
    let perpendicular = (-unit.1, unit.0);
    let half = e.width_mm / 2.0;
    let mut best_distance: Option<f64> = None;
    let mut edges_inside = 0usize;
    for side in [1.0_f64, -1.0] {
        let start = (
            e.inside_mm.0 + perpendicular.0 * half * side,
            e.inside_mm.1 + perpendicular.1 * half * side,
        );
        let from_centre = (start.0 - centre.0, start.1 - centre.1);
        if (from_centre.0 * from_centre.0 + from_centre.1 * from_centre.1).sqrt() >= radius_mm {
            continue; // this edge never was in the land, so it leaves nothing
        }
        edges_inside += 1;
        let offset = (centre.0 - start.0, centre.1 - start.1);
        let distance = (unit.0 * offset.1 - unit.1 * offset.0).abs();
        // The smaller angle belongs to the edge further from the centre.
        best_distance = Some(best_distance.map_or(distance, |seen: f64| seen.max(distance)));
    }
    let recomputed_millideg =
        best_distance
            .filter(|distance| *distance <= radius_mm)
            .map(|distance| {
                (1_000.0 * (90.0 - (distance / radius_mm).asin().to_degrees())).round() as i64
            });
    let from_centre = (e.inside_mm.0 - centre.0, e.inside_mm.1 - centre.1);
    let along = from_centre.0 * unit.0 + from_centre.1 * unit.1;
    let discriminant = along * along + radius_mm * radius_mm
        - (from_centre.0 * from_centre.0 + from_centre.1 * from_centre.1);
    let depth_mm = (discriminant >= 0.0).then(|| -along + discriminant.sqrt());
    Some(CircularReading {
        pin: e.pin.clone(),
        sharp: e.sharp,
        width_mm: e.width_mm,
        radius_mm,
        p_axis_mm,
        p_far_mm,
        reported_millideg: e.reported_millideg,
        recomputed_millideg,
        depth_mm,
        edges_inside,
    })
}

fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).expect("no NaN in a measured distance"));
    let middle = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    })
}

/// The instrument check the circular-land reading rests on.
///
/// Before any claim is made about why an entry into a round land reads sharp,
/// the arithmetic that would carry the claim has to agree with the rule that
/// is already measuring. `90 - arcsin(p_far / R)` reconstructs R-08's answer
/// from the land's radius, the segment's two ends and its width - nothing it
/// reads comes from the rule. A row differing by more than a tenth of a degree
/// means the two are not measuring the same thing, and nothing downstream of
/// it can be believed.
///
/// It is a check on the instrument and never a finding about boards. The
/// denominator is published for the same reason every other one here is: an
/// agreement over zero rows is what a deleted measurement looks like.
#[test]
#[ignore = "slow: routes all six benchmark fixtures"]
fn a_circular_land_reads_the_same_from_its_own_geometry() {
    let mut readings: Vec<CircularReading> = Vec::new();
    for benchmark in BENCHMARKS {
        let (_, _, entries) = pad_facts(benchmark.filename);
        readings.extend(entries.iter().filter_map(circular_reading));
    }

    let sharp = readings.iter().filter(|r| r.sharp).count();
    let misses = readings
        .iter()
        .filter(|r| r.recomputed_millideg.is_none())
        .count();
    let one_edge = readings.iter().filter(|r| r.edges_inside == 1).count();
    eprintln!(
        "entries into a circular land: {} in all, {sharp} of them sharp, \
         {misses} whose far edge misses the land, \
         {one_edge} read from one trace edge because the other starts outside",
        readings.len()
    );
    for reading in readings.iter().filter(|r| r.edges_inside == 1) {
        eprintln!(
            "  one edge only: {:<8} depth {:.4}mm  reported {:>6}  \
             a far-edge reading would have been {:>6}",
            reading.pin,
            reading.depth_mm.unwrap_or(f64::NAN),
            reading.reported_millideg.unwrap_or_default(),
            (1_000.0
                * (90.0
                    - ((reading.p_far_mm / reading.radius_mm).min(1.0))
                        .asin()
                        .to_degrees()))
            .round() as i64
        );
    }

    let mut compared = 0usize;
    let mut worst: Option<(&str, i64)> = None;
    for reading in &readings {
        let (Some(reported), Some(recomputed)) =
            (reading.reported_millideg, reading.recomputed_millideg)
        else {
            continue;
        };
        compared += 1;
        let gap = (i64::from(reported) - recomputed).abs();
        if worst.is_none_or(|(_, seen)| gap > seen) {
            worst = Some((reading.pin.as_str(), gap));
        }
        eprintln!(
            "  {:<8} {} R {:.3}mm  w {:.3}mm  p_axis {:.4}mm  p_far {:.4}mm  \
             p_far/R {:.3}  depth {:.4}mm  reported {:>6}  recomputed {:>6}  gap {:>5}",
            reading.pin,
            if reading.sharp { "SHARP" } else { "clean" },
            reading.radius_mm,
            reading.width_mm,
            reading.p_axis_mm,
            reading.p_far_mm,
            reading.p_far_mm / reading.radius_mm,
            reading.depth_mm.unwrap_or(f64::NAN),
            reported,
            recomputed,
            gap
        );
    }

    // What the two claims behind this reading would be tested against. They are
    // printed rather than asserted: this test establishes the instrument, and a
    // verdict drawn from four lands on six boards needs its denominator read
    // first.
    let mut clean_ratio: Vec<f64> = readings
        .iter()
        .filter(|r| !r.sharp)
        .map(|r| r.p_far_mm / r.radius_mm)
        .collect();
    let deepest = readings
        .iter()
        .filter(|r| r.sharp)
        .filter_map(|r| r.depth_mm.map(|d| d / r.radius_mm))
        .fold(f64::NEG_INFINITY, f64::max);
    eprintln!(
        "median p_far/R over {} clean circular entries: {:?}; deepest sharp entry: {:.3} R",
        clean_ratio.len(),
        median(&mut clean_ratio),
        deepest
    );

    assert!(
        compared > 0,
        "an agreement over no rows at all is what a deleted measurement looks like"
    );
    let (pin, gap) = worst.expect("compared is above zero, so a worst row exists");
    assert!(
        gap <= 100,
        "the geometry and the rule must answer the same question: {pin} differs by \
         {gap} millidegrees over {compared} circular entries"
    );

    // The branch that had to be reproduced before the two agreed. It is a
    // property of the rule, not a fault found in it: `entry_angle` reduces
    // with `f64::min` over the edges that answered, and an edge whose own end
    // lies outside the copper answers nothing. Whether the surviving reading
    // is the right one is a separate question this test does not settle - an
    // edge that ends outside the land may never have been inside it. What is
    // asserted here is only that the branch is exercised, so reproducing it
    // was not a guess fitted to two stubborn rows.
    assert!(
        one_edge > 0,
        "the one-edge branch is exercised by these fixtures, so reproducing it is \
         not a guess: {one_edge} of {}",
        readings.len()
    );
    assert!(
        one_edge * 10 < readings.len(),
        "and it is the exception rather than the reading: {one_edge} of {}",
        readings.len()
    );
}
