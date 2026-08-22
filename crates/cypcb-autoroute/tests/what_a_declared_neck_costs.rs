//! What declaring a neck costs the board it is declared on.
//!
//! `cargo test --release -p cypcb-autoroute --test what_a_declared_neck_costs -- --ignored --nocapture`
//!
//! A neck is drawn onto the finished route, so the search does not see it and
//! the copper it produces is narrower than the copper it planned. Narrower
//! copper near a pad is copper further from its neighbours, which should read
//! as *fewer* clearance faults - but it also shortens the reach of every
//! same-net exemption, and nothing here had measured which way it goes.
//!
//! Read against each board's own noise band from `cypcb_autoroute::noise_band`
//! and not against zero. `plane_board` is the board to watch: its band is zero
//! on both columns, so any movement there is signal in both directions.
//!
//! **The nets are widened before they are necked, and that is the point.** A
//! net the router lays at the fab's minimum has nothing to neck *to*: anything
//! narrower is copper the fabricator will not etch, which `NeckDownRule` and
//! `MinTraceWidthRule` both report, so the board gets worse for a reason that
//! has nothing to do with necking. The case a neck exists for is the opposite
//! one - `netclass Mains [current 10A]` gives copper millimetres wide and the
//! pad pitch has nowhere to put it - so both columns here are routed at a
//! width the design chose, and only the neck differs between them.

use std::path::Path;

use cypcb_autoroute::{noise_band, route_board, AutorouteConfig};
use cypcb_core::Nm;
use cypcb_drc::{preset_for_world, ruleset_for_world, run_drc, DesignRules, ViolationKind};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_router::apply_routes;
use cypcb_rules::presets::RulesPreset;
use cypcb_world::components::trace::TraceNeck;
use cypcb_world::NetId;

fn fixture_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// Route `filename`, optionally declaring `neck` on **every** net first.
///
/// Every net rather than the busiest one, deliberately. A neck on one net is
/// the case a design would write, and a neck on all of them is the upper bound
/// of what the feature can cost a board - if that is inside the noise, so is
/// any subset of it, and the question is answered without picking a net and
/// then arguing about the pick.
/// What one routing of one board is worth reading.
struct Reading {
    /// Every violation the checker reported.
    raw: usize,
    /// Violations of the same kind at the same point, counted once.
    ///
    /// The clearance rule reports per segment pair, and necking cuts two
    /// segments per run - so a raw count can rise because there are more pairs
    /// to report the same contact in. This is the count a split cannot
    /// inflate.
    distinct: usize,
    /// Raw clearance violations with nothing between the copper.
    shorts: usize,
    /// The same, deduplicated.
    distinct_shorts: usize,
    /// Total copper laid, in millimetres. Necking moves where a width changes
    /// and must not move this: if it does, the two columns are not the same
    /// board and nothing else in the row can be compared.
    copper_mm: f64,
    /// What the neck actually drew, for the reader.
    drawn: String,
    /// How many violations of each kind, so a delta can be attributed.
    ///
    /// A row that says "worse by 161" and nothing else is a number nobody can
    /// act on. Clearance faults and min-width faults have different causes and
    /// the fix for one is not the fix for the other.
    by_kind: std::collections::BTreeMap<String, usize>,
}

fn route_and_count(filename: &str, width: Nm, neck: Option<TraceNeck>) -> Reading {
    let parsed = parse_kicad_pcb(&fixture_path(filename)).expect("the fixture parses");
    let mut world = parsed.world;
    let library = parsed.library;

    let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, &world);
    let drc_rules = DesignRules::from_constraints(&preset.constraints());

    let nets: Vec<NetId> = world.nets().map(|(id, _)| id).collect();
    let net_count = nets.len();
    for net in nets {
        let mut carried = world.net_constraints(net).unwrap_or_default();
        carried.width = Some(width);
        carried.neck = neck;
        world.set_net_constraints(net, carried);
    }
    // The router has to plan at the width the design asked for, not the fab's
    // floor, or the "wide" column is the same board as the narrow one.
    let rules = ruleset_for_world(preset, &world);

    let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());
    apply_routes(&mut world, &result);
    world.rebuild_spatial_index_from_library(&library);

    let drc = run_drc(&mut world, &drc_rules);
    let is_short = |v: &&cypcb_drc::DrcViolation| {
        v.kind == ViolationKind::Clearance && v.actual == Some(Nm::ZERO)
    };
    let shorts = drc.violations.iter().filter(is_short).count();

    let place =
        |v: &cypcb_drc::DrcViolation| (v.kind.to_string(), v.location.x.raw(), v.location.y.raw());
    let distinct: std::collections::BTreeSet<_> = drc.violations.iter().map(place).collect();
    let distinct_shorts: std::collections::BTreeSet<_> =
        drc.violations.iter().filter(is_short).map(place).collect();
    // How much copper actually ended up thin, and across how many chains. A
    // table of zero differences is either a feature that costs nothing or one
    // that did nothing, and only these two numbers tell the two apart.
    let (mut runs, mut segments, mut thin) = (0usize, 0usize, 0i64);
    {
        use cypcb_world::components::trace::Trace;
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        for trace in query.iter(ecs) {
            runs += trace.runs().len();
            segments += trace.segments.len();
            thin += trace.necked_length().raw();
        }
    }
    let widest = {
        use cypcb_world::components::trace::Trace;
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query
            .iter(ecs)
            .map(|trace| trace.width.raw())
            .max()
            .unwrap_or(0)
    };
    let copper_mm = {
        use cypcb_world::components::trace::Trace;
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query
            .iter(ecs)
            .flat_map(|trace| trace.segments.iter())
            .map(|segment| segment.length().0)
            .sum::<i64>() as f64
            / 1_000_000.0
    };
    let drawn = format!(
        "{} nets at {:.3}mm, {} runs / {} segs, {:.1}mm thin",
        net_count,
        widest as f64 / 1_000_000.0,
        runs,
        segments,
        thin as f64 / 1_000_000.0
    );
    let mut by_kind: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for violation in &drc.violations {
        *by_kind.entry(violation.kind.to_string()).or_insert(0) += 1;
    }

    Reading {
        raw: drc.violations.len(),
        distinct: distinct.len(),
        shorts,
        distinct_shorts: distinct_shorts.len(),
        copper_mm,
        drawn,
        by_kind,
    }
}

#[test]
#[ignore = "diagnostic: routes each board twice, with and without a declared neck"]
fn what_a_declared_neck_costs() {
    // A width a design would choose for current, and a neck back down to the
    // fab's own minimum - legal copper at both widths, so nothing here is
    // reported for being unetchable.
    //
    // Two earlier versions of this diagnostic measured their own bad input:
    // 0.15mm, which is *wider* than the 0.127mm the router lays, so
    // `apply_neck` refused every net; then 0.09mm, which is under what the fab
    // will etch, so every necked segment became a min-width fault.
    let width = Nm::from_mm(1.0);
    let neck = TraceNeck {
        width: Nm::from_mm(0.127),
        length: Nm::from_mm(1.0),
    };

    eprintln!();
    eprintln!(
        "every net at width {}mm, with and without neck {}mm for {}mm",
        width.to_mm(),
        neck.width.to_mm(),
        neck.length.to_mm()
    );
    eprintln!();
    eprintln!("violations / shorts, without the neck -> with it");

    for benchmark in cypcb_kicad::BENCHMARKS {
        let before = route_and_count(benchmark.filename, width, None);
        let after = route_and_count(benchmark.filename, width, Some(neck));
        let (band, short_band) = noise_band(benchmark.filename);

        let dv = after.raw as i64 - before.raw as i64;
        let ds = after.shorts as i64 - before.shorts as i64;
        let dd = after.distinct as i64 - before.distinct as i64;
        let dds = after.distinct_shorts as i64 - before.distinct_shorts as i64;
        let verdict = if dd.abs() > band || dds.abs() > short_band {
            "OUTSIDE the band"
        } else {
            "inside the band"
        };

        // The copper comes first because it is the check on the row: if the
        // two routings do not lay the same copper they are not the same board,
        // and every other column compares two different things.
        eprintln!();
        eprintln!(
            "{}  copper {:.1}mm -> {:.1}mm{}",
            benchmark.filename.trim_end_matches(".kicad_pcb"),
            before.copper_mm,
            after.copper_mm,
            if (after.copper_mm - before.copper_mm).abs() < 0.05 {
                ", unchanged"
            } else {
                "  <- CHANGED, the rest of this row compares two boards"
            }
        );
        eprintln!(
            "  raw      {:>5}/{:<4} -> {:>5}/{:<4}   {:+5} / {:+4}",
            before.raw, before.shorts, after.raw, after.shorts, dv, ds
        );
        eprintln!(
            "  distinct {:>5}/{:<4} -> {:>5}/{:<4}   {:+5} / {:+4}   band {}/{}  {}",
            before.distinct,
            before.distinct_shorts,
            after.distinct,
            after.distinct_shorts,
            dd,
            dds,
            band,
            short_band,
            verdict
        );
        eprintln!("  {}", after.drawn);

        let mut kinds: Vec<String> = Vec::new();
        for (kind, count) in &after.by_kind {
            let was = before.by_kind.get(kind).copied().unwrap_or(0);
            if *count != was {
                kinds.push(format!("{kind} {was}->{count}"));
            }
        }
        for (kind, was) in &before.by_kind {
            if !after.by_kind.contains_key(kind) {
                kinds.push(format!("{kind} {was}->0"));
            }
        }
        eprintln!(
            "  moved by kind: {}",
            if kinds.is_empty() {
                "nothing".to_string()
            } else {
                kinds.join(", ")
            }
        );
    }
}
