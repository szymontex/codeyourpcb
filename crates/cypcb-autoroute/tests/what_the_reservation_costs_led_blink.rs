//! Why reserving trace copper costs led_blink two near misses and two vias.
//!
//! `cargo test --release -p cypcb-autoroute --test what_the_reservation_costs_led_blink -- --ignored --nocapture`
//!
//! The setting wins both dense benchmark boards - half the shorts on each -
//! and loses the small one, which is what keeps it out of the default. The
//! board is 23 connections, small enough to read net by net instead of
//! arguing from totals.

use std::collections::BTreeMap;
use std::path::Path;

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::{run_drc, DesignRules, ViolationKind};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_router::apply_routes;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};

fn fixture_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// One net's share of a routed board.
#[derive(Default, Clone, PartialEq)]
struct NetRoute {
    segments: usize,
    vias: usize,
    length_mm: f64,
    layers: Vec<String>,
}

fn route_and_break_down(reserve: bool) -> (BTreeMap<String, NetRoute>, Vec<String>) {
    let parsed = parse_kicad_pcb(&fixture_path("led_blink.kicad_pcb")).expect("the fixture parses");
    let mut world = parsed.world;
    let library = parsed.library;

    let config = AutorouteConfig {
        reserve_trace_footprint: reserve,
        ..AutorouteConfig::default()
    };
    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset exists"));
    let result = route_board(&mut world, &library, &rules, &config);

    let mut per_net: BTreeMap<String, NetRoute> = BTreeMap::new();
    for segment in &result.routes {
        let name = world
            .net_name(segment.net_id)
            .unwrap_or("<unnamed>")
            .to_string();
        let entry = per_net.entry(name).or_default();
        entry.segments += 1;
        entry.length_mm += segment.length().0 as f64 / 1_000_000.0;
        let layer = format!("{:?}", segment.layer);
        if !entry.layers.contains(&layer) {
            entry.layers.push(layer);
        }
    }
    for via in &result.vias {
        let name = world
            .net_name(via.net_id)
            .unwrap_or("<unnamed>")
            .to_string();
        per_net.entry(name).or_default().vias += 1;
    }

    apply_routes(&mut world, &result);
    world.rebuild_spatial_index_from_library(&library);
    let drc = run_drc(&mut world, &DesignRules::jlcpcb_2layer());
    let violations: Vec<String> = drc
        .violations
        .iter()
        .filter(|v| v.kind == ViolationKind::Clearance)
        .map(|v| {
            format!(
                "({:.3}mm, {:.3}mm) {}",
                v.location.x.to_mm(),
                v.location.y.to_mm(),
                v.message
            )
        })
        .collect();

    (per_net, violations)
}

#[test]
#[ignore = "diagnostic: routes led_blink twice and prints the difference per net"]
fn what_the_reservation_costs_led_blink() {
    let (plain, plain_violations) = route_and_break_down(false);
    let (reserved, reserved_violations) = route_and_break_down(true);

    eprintln!();
    eprintln!("{:<10} {:>21} {:>21}", "net", "default", "reserved copper");
    eprintln!(
        "{:<10} {:>21} {:>21}",
        "", "segs vias mm layers", "segs vias mm layers"
    );

    let mut names: Vec<&String> = plain.keys().chain(reserved.keys()).collect();
    names.sort_unstable();
    names.dedup();

    for name in names {
        let show = |route: Option<&NetRoute>| match route {
            Some(r) => format!(
                "{:>4} {:>4} {:>6.1} {}",
                r.segments,
                r.vias,
                r.length_mm,
                r.layers.join("+")
            ),
            None => "  - not routed".to_string(),
        };
        let a = plain.get(name);
        let b = reserved.get(name);
        let marker = if a.map(|r| r.vias) != b.map(|r| r.vias) {
            " <- vias differ"
        } else {
            ""
        };
        eprintln!("{:<10} {:>21} {:>21}{}", name, show(a), show(b), marker);
    }

    eprintln!();
    eprintln!("clearance violations, default:");
    for line in &plain_violations {
        eprintln!("  {line}");
    }
    eprintln!("clearance violations, reserved copper:");
    for line in &reserved_violations {
        eprintln!("  {line}");
    }
}
