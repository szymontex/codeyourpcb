//! Copper KiCad puts on net 0 is on no net, and no net owns it.
//!
//! `cargo test -p cypcb-cli --test copper_on_no_net_is_foreign_to_every_net`
//!
//! KiCad numbers the unconnected net 0, and a track nobody connected is on
//! it. The importer read net 0 as "no `(net ...)` resolved" and left the
//! default in place, and the default was `NetId(0)` - the first net this
//! board interned. Every piece of copper on net 0 came in on that net: on 8 of
//! the 132 boards the repository holds, 4305 pieces of copper were on a net
//! the file never gave them, and the clearance rule exempted them from
//! every pad and track of that net.
//!
//! The boards here are written by hand, so they are evidence about this
//! importer, not about what KiCad emits.

use cypcb_drc::{run_drc, PresetRules, ViolationKind};
use cypcb_rules::presets::RulesPreset;
use cypcb_world::components::trace::{Trace, TraceSource, Via};
use cypcb_world::{BoardWorld, NetId};

const HEAD: &str = r#"(kicad_pcb (version 20240108) (generator "pcbnew") (generator_version "8.0.0")
  (general (thickness 1.6))
  (paper "A4")
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
  (net 0 "")
  (net 1 "FIRST")
  (net 2 "SECOND")
  (gr_line (start 100.0 100.0) (end 130.0 100.0) (layer "Edge.Cuts") (width 0.05))
  (gr_line (start 130.0 100.0) (end 130.0 120.0) (layer "Edge.Cuts") (width 0.05))
  (gr_line (start 130.0 120.0) (end 100.0 120.0) (layer "Edge.Cuts") (width 0.05))
  (gr_line (start 100.0 120.0) (end 100.0 100.0) (layer "Edge.Cuts") (width 0.05))
"#;

fn board(copper: &str) -> String {
    format!("{HEAD}{copper})\n")
}

/// The board as every import call site makes it: the parsed world, and the
/// reference routes applied as a person's copper.
fn import(text: &str) -> (BoardWorld, usize, usize) {
    let parsed = cypcb_kicad::parse_kicad_pcb_str(text).expect("the board reads");
    let mut world = parsed.world;
    let library = parsed.library;
    let (mut routed_segments, mut routed_vias) = (0, 0);
    if let Some(routes) = parsed.reference_routes {
        routed_segments = routes.routes.len();
        routed_vias = routes.vias.len();
        cypcb_router::apply_routes_as(&mut world, &routes, TraceSource::Manual);
    }
    world.set_footprints(library.clone());
    world.rebuild_spatial_index_from_library(&library);
    (world, routed_segments, routed_vias)
}

fn traces(world: &mut BoardWorld) -> Vec<Trace> {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<&Trace>();
    let mut all: Vec<Trace> = query.iter(ecs).cloned().collect();
    all.sort_by_key(|t| {
        (
            t.net_id.map(|n| n.0),
            t.segments[0].start.x.0,
            t.segments[0].start.y.0,
        )
    });
    all
}

fn vias(world: &mut BoardWorld) -> Vec<Via> {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<&Via>();
    query.iter(ecs).copied().collect()
}

fn clearance_rows(world: &mut BoardWorld) -> usize {
    let preset = cypcb_drc::preset_for_world(RulesPreset::JlcpcbStandard2Layer, world);
    run_drc(world, &preset.rules())
        .violations
        .iter()
        .filter(|v| v.kind == ViolationKind::Clearance)
        .count()
}

#[test]
fn copper_on_net_zero_comes_in_on_no_net() {
    let (mut world, routed_segments, routed_vias) = import(&board(
        r#"
  (segment (start 104.0 104.0) (end 112.0 104.0) (width 0.25) (layer "F.Cu") (net 1))
  (segment (start 104.0 108.0) (end 112.0 108.0) (width 0.25) (layer "F.Cu") (net 0))
  (segment (start 112.0 108.0) (end 116.0 108.0) (width 0.25) (layer "F.Cu") (net 0))
  (arc (start 104.0 112.0) (mid 106.828427 113.171573) (end 108.0 116.0) (width 0.25) (layer "F.Cu") (net 0))
  (via (at 116.0 108.0) (size 0.7) (drill 0.4) (layers "F.Cu" "B.Cu") (net 0))
  (via (at 120.0 104.0) (size 0.7) (drill 0.4) (layers "F.Cu" "B.Cu") (net 2))
"#,
    ));

    // Only copper that has a net is handed to the router's types.
    assert_eq!((routed_segments, routed_vias), (1, 1));

    let all = traces(&mut world);
    let on_no_net: Vec<&Trace> = all.iter().filter(|t| t.net_id.is_none()).collect();
    // Two straight segments that meet end to end are two pieces of copper,
    // not one run: they are not the same net as each other. The arc is one.
    assert_eq!(on_no_net.len(), 3, "{all:#?}");
    let on_first = all
        .iter()
        .filter(|t| t.net_id == Some(NetId::new(0)))
        .count();
    assert_eq!(on_first, 1, "the one segment that named FIRST");

    let all_vias = vias(&mut world);
    assert_eq!(all_vias.len(), 2);
    assert_eq!(all_vias.iter().filter(|v| v.net_id.is_none()).count(), 1);
    assert_eq!(
        all_vias
            .iter()
            .filter(|v| v.net_id == Some(NetId::new(1)))
            .count(),
        1,
        "the via that named SECOND"
    );

    // No `NetId` component either: the clearance rule reads a missing
    // component as "belongs to no net".
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<(&Trace, Option<&NetId>)>();
    for (trace, component) in query.iter(ecs) {
        assert_eq!(trace.net_id, component.copied());
    }
    let mut query = ecs.query::<(&Via, Option<&NetId>)>();
    for (via, component) in query.iter(ecs) {
        assert_eq!(via.net_id, component.copied());
    }

    // And net 0 is not a net: the board has two.
    let names: Vec<String> = world.nets().map(|(_, n)| n.to_string()).collect();
    assert_eq!(names.len(), 2, "{names:?}");
}

#[test]
fn copper_on_no_net_beside_the_first_net_is_a_short() {
    // 0.05mm between the edges of a track on FIRST and a track on no net. On
    // the first net - where the importer put it - the pair was one conductor
    // and the rule said nothing.
    let near = board(
        r#"
  (segment (start 104.0 104.0) (end 120.0 104.0) (width 0.25) (layer "F.Cu") (net 1))
  (segment (start 104.0 104.3) (end 120.0 104.3) (width 0.25) (layer "F.Cu") (net 0))
"#,
    );
    let (mut world, _, _) = import(&near);
    assert!(clearance_rows(&mut world) >= 1);

    // Control: the same pair with the second track named FIRST is one net and
    // is not reported.
    let same = board(
        r#"
  (segment (start 104.0 104.0) (end 120.0 104.0) (width 0.25) (layer "F.Cu") (net 1))
  (segment (start 104.0 104.3) (end 120.0 104.3) (width 0.25) (layer "F.Cu") (net 1))
"#,
    );
    let (mut world, _, _) = import(&same);
    assert_eq!(clearance_rows(&mut world), 0);
}

#[test]
fn two_pieces_of_copper_on_no_net_are_not_one_conductor() {
    // Two tracks on no net, 0.05mm apart. They are not the same net as each
    // other, so the gap is a place the rule reports.
    let (mut world, _, _) = import(&board(
        r#"
  (segment (start 104.0 104.0) (end 120.0 104.0) (width 0.25) (layer "F.Cu") (net 0))
  (segment (start 104.0 104.3) (end 120.0 104.3) (width 0.25) (layer "F.Cu") (net 0))
"#,
    ));
    assert!(clearance_rows(&mut world) >= 1);
}

#[test]
fn copper_on_no_net_is_written_back_as_net_zero() {
    let (mut world, _, _) = import(&board(
        r#"
  (segment (start 104.0 104.0) (end 112.0 104.0) (width 0.25) (layer "F.Cu") (net 1))
  (segment (start 104.0 108.0) (end 112.0 108.0) (width 0.25) (layer "F.Cu") (net 0))
  (via (at 116.0 108.0) (size 0.7) (drill 0.4) (layers "F.Cu" "B.Cu") (net 0))
"#,
    ));
    let written = cypcb_kicad::write_board(&mut world, "test");
    let segments: Vec<&str> = written.lines().filter(|l| l.contains("(segment")).collect();
    assert_eq!(segments.len(), 2, "{written}");
    assert_eq!(segments.iter().filter(|l| l.contains("(net 0)")).count(), 1);
    assert_eq!(segments.iter().filter(|l| l.contains("(net 1)")).count(), 1);
    let via = written.lines().find(|l| l.contains("(via")).expect("a via");
    assert!(via.contains("(net 0)"), "{via}");
}

#[test]
fn copper_on_no_net_is_written_in_the_language_on_a_net_called_unknown() {
    let (mut world, _, _) = import(&board(
        r#"
  (segment (start 104.0 108.0) (end 112.0 108.0) (width 0.25) (layer "F.Cu") (net 0))
  (via (at 116.0 108.0) (size 0.7) (drill 0.4) (layers "F.Cu" "B.Cu") (net 0))
"#,
    ));
    let written = cypcb_world::dsl::board_as_dsl_reporting(&mut world);
    assert!(written.source.contains("net unknown"), "{}", written.source);
    // The copper is on it, and not on the first net.
    assert!(
        written.source.contains("trace unknown"),
        "{}",
        written.source
    );
    assert!(
        !written.source.contains("trace FIRST"),
        "{}",
        written.source
    );
    assert_eq!(
        written.source.matches("trace unknown").count(),
        2,
        "{}",
        written.source
    );
    assert!(
        written
            .not_written
            .iter()
            .any(|n| n.contains("copper on no net")),
        "{:?}",
        written.not_written
    );
}
