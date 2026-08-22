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

use std::path::Path;

use cypcb_autoroute::{noise_band, route_board, AutorouteConfig};
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
fn route_and_count(filename: &str, neck: Option<TraceNeck>) -> (usize, usize, String) {
    let parsed = parse_kicad_pcb(&fixture_path(filename)).expect("the fixture parses");
    let mut world = parsed.world;
    let library = parsed.library;

    let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, &world);
    let rules = ruleset_for_world(preset, &world);
    let drc_rules = DesignRules::from_constraints(&preset.constraints());

    let nets: Vec<NetId> = world.nets().map(|(id, _)| id).collect();
    let net_count = nets.len();
    if let Some(neck) = neck {
        for net in nets {
            let mut carried = world.net_constraints(net).unwrap_or_default();
            carried.neck = Some(neck);
            world.set_net_constraints(net, carried);
        }
    }

    let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());
    apply_routes(&mut world, &result);
    world.rebuild_spatial_index_from_library(&library);

    let drc = run_drc(&mut world, &drc_rules);
    let shorts = drc
        .violations
        .iter()
        .filter(|v| v.kind == ViolationKind::Clearance)
        .filter(|v| v.actual == Some(cypcb_core::Nm::ZERO))
        .count();
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
    let drawn = format!(
        "{} nets, {} runs / {} segs, {:.1}mm thin",
        net_count,
        runs,
        segments,
        thin as f64 / 1_000_000.0
    );
    (drc.violations.len(), shorts, drawn)
}

#[test]
#[ignore = "diagnostic: routes each board twice, with and without a declared neck"]
fn what_a_declared_neck_costs() {
    let neck = TraceNeck {
        width: cypcb_core::Nm::from_mm(0.15),
        length: cypcb_core::Nm::from_mm(1.0),
    };

    eprintln!();
    eprintln!(
        "neck {} for {} on each board's busiest net",
        neck.width.to_mm(),
        neck.length.to_mm()
    );
    eprintln!();
    eprintln!(
        "{:<20} {:>13} {:>13} {:>10} {:>12} what was drawn",
        "board", "without", "with", "band", "delta"
    );

    for benchmark in cypcb_kicad::BENCHMARKS {
        let (before, before_shorts, _) = route_and_count(benchmark.filename, None);
        let (after, after_shorts, drawn) = route_and_count(benchmark.filename, Some(neck));
        let (band, short_band) = noise_band(benchmark.filename);

        let dv = after as i64 - before as i64;
        let ds = after_shorts as i64 - before_shorts as i64;
        let verdict = if dv.abs() > band || ds.abs() > short_band {
            "OUTSIDE the band"
        } else {
            "inside the band"
        };

        eprintln!(
            "{:<20} {:>8}/{:<4} {:>8}/{:<4} {:>5}/{:<4} {:+4} / {:+3} {:<11} {}",
            benchmark.filename.trim_end_matches(".kicad_pcb"),
            before,
            before_shorts,
            after,
            after_shorts,
            band,
            short_band,
            drawn,
            dv,
            ds,
            verdict
        );
    }
}
