//! A KiCad board's plated pad is copper on its inner layers, as read from the file.
//!
//! KiCad writes a plated through-hole pad `(layers "*.Cu" ...)`, "all of the
//! copper layers", and the importer spells that as the two faces. The copper
//! on the inner layers comes from `PadDef::copper_mask`, which knows a plated
//! hole goes through every layer. The tests of that answer build their pads by
//! hand, so nothing held the path a user takes: a `.kicad_pcb` with inner
//! layers, read by the importer, then checked and routed. Taking the inner
//! layers out of `copper_mask` left every test on a KiCad board green except
//! a floor on the count of acute corners, which moved by accident.
//!
//! `multi_ic` is the four-layer benchmark: 30 plated pads, `In1.Cu` and
//! `In2.Cu`. Its pin header J3 has pin 1 on `VCC_3V3` beside pin 2 on `PA13`.

use std::path::Path;

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_core::{Nm, Point};
use cypcb_drc::rules::{ClearanceRule, DrcRule};
use cypcb_drc::{preset_for_world, ruleset_for_world, DesignRules};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_router::apply_routes;
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, NetId, PadShape, Position, RefDes, Rotation,
};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

const INNER: [Layer; 2] = [Layer::Inner(0), Layer::Inner(1)];

fn multi_ic() -> (BoardWorld, FootprintLibrary) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/benchmark/multi_ic.kicad_pcb");
    let parsed = parse_kicad_pcb(&path).expect("multi_ic parses");
    (parsed.world, parsed.library)
}

/// One plated pad where the board puts it.
struct PlatedPad {
    name: String,
    net: Option<NetId>,
    centre: Point,
    /// Half the pad's smaller side: the round part of a circle or an oval,
    /// which is copper whatever else the shape adds.
    radius: i64,
    copper_mask: u32,
    shape: PadShape,
}

fn plated_pads(world: &mut BoardWorld, library: &FootprintLibrary) -> Vec<PlatedPad> {
    let placements: Vec<(String, Point, f64, String, NetConnections)> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(
            &RefDes,
            &Position,
            &Rotation,
            &FootprintRef,
            &NetConnections,
        )>();
        query
            .iter(ecs)
            .map(|(refdes, position, rotation, footprint, nets)| {
                (
                    refdes.0.clone(),
                    position.0,
                    rotation.to_degrees(),
                    footprint.as_str().to_string(),
                    nets.clone(),
                )
            })
            .collect()
    };
    let mut pads = Vec::new();
    for (refdes, position, degrees, footprint, nets) in placements {
        let footprint = library
            .get(&footprint)
            .expect("every footprint is in the library");
        let (sin, cos) = degrees.to_radians().sin_cos();
        for pad in &footprint.pads {
            if !pad.is_through_hole() || pad.is_non_plated() {
                continue;
            }
            let (px, py) = (pad.position.x.0 as f64, pad.position.y.0 as f64);
            pads.push(PlatedPad {
                name: format!("{refdes}.{}", pad.number),
                net: nets.pin_net(&pad.number),
                centre: Point::new(
                    Nm(position.x.0 + (px * cos - py * sin).round() as i64),
                    Nm(position.y.0 + (px * sin + py * cos).round() as i64),
                ),
                radius: pad.size.0 .0.min(pad.size.1 .0) / 2,
                copper_mask: pad.copper_mask(),
                shape: pad.shape,
            });
        }
    }
    pads.sort_by(|a, b| a.name.cmp(&b.name));
    pads
}

/// Distance from `p` to the segment `a`-`b`, in nm.
fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (ax, ay) = (a.x.0 as f64, a.y.0 as f64);
    let (dx, dy) = (b.x.0 as f64 - ax, b.y.0 as f64 - ay);
    let length = dx * dx + dy * dy;
    let t = if length == 0.0 {
        0.0
    } else {
        (((p.x.0 as f64 - ax) * dx + (p.y.0 as f64 - ay) * dy) / length).clamp(0.0, 1.0)
    };
    (p.x.0 as f64 - (ax + dx * t)).hypot(p.y.0 as f64 - (ay + dy * t))
}

#[test]
fn every_plated_pad_of_the_file_is_copper_on_both_inner_layers() {
    let (mut world, library) = multi_ic();
    let pads = plated_pads(&mut world, &library);
    assert_eq!(
        pads.len(),
        30,
        "multi_ic writes 30 pads `(layers \"*.Cu\" ...)`"
    );
    let missing: Vec<&str> = pads
        .iter()
        .filter(|pad| {
            INNER
                .iter()
                .any(|layer| pad.copper_mask & layer.to_copper_mask() == 0)
        })
        .map(|pad| pad.name.as_str())
        .collect();
    assert!(
        missing.is_empty(),
        "plated pads with no copper on an inner layer: {missing:?}"
    );
}

/// Shorts the clearance rule reports against a 0.4mm trace of `PA13` laid on
/// `layer` across the middle of J3.1. The trace stays inside the 0.85mm pad
/// and 1.27mm from the next one, so the only copper it can touch is J3.1.
fn shorts_of_pa13_across_j3_1_on(layer: Layer) -> usize {
    let (mut world, library) = multi_ic();
    let pads = plated_pads(&mut world, &library);
    let pin = pads
        .iter()
        .find(|pad| pad.name == "J3.1")
        .expect("J3.1 is a plated pad");
    assert_eq!(pin.net, world.get_net("VCC_3V3"), "J3.1 is on VCC_3V3");
    let pa13 = world.get_net("PA13").expect("multi_ic has PA13");
    let half = Nm::from_mm(0.2).0;
    let trace = world.spawn_entity((
        Trace {
            segments: vec![TraceSegment::new(
                Point::new(pin.centre.x, Nm(pin.centre.y.0 - half)),
                Point::new(pin.centre.x, Nm(pin.centre.y.0 + half)),
            )],
            width: Nm::from_mm(0.2),
            layer,
            net_id: pa13,
            locked: false,
            source: TraceSource::Manual,
        },
        pa13,
    ));
    world.rebuild_spatial_index_from_library(&library);
    ClearanceRule
        .check(&mut world, &DesignRules::default())
        .iter()
        .filter(|violation| {
            (violation.entity == trace || violation.other_entity == Some(trace))
                && violation.actual.map(|actual| actual.0) == Some(0)
        })
        .count()
}

#[test]
fn the_checker_sees_another_net_across_the_pad_on_an_inner_layer() {
    // The positive control: on the top face the pad's copper was never in
    // doubt, so a rule that reports nothing at all fails here first.
    assert!(
        shorts_of_pa13_across_j3_1_on(Layer::TopCopper) > 0,
        "PA13 across J3.1 on the top face"
    );
    for layer in INNER {
        assert!(
            shorts_of_pa13_across_j3_1_on(layer) > 0,
            "PA13 across J3.1 on {layer:?} is copper touching copper"
        );
    }
}

#[test]
fn the_router_keeps_other_nets_off_the_pads_on_the_inner_layers() {
    let (mut world, library) = multi_ic();
    let preset = preset_for_world(
        cypcb_rules::presets::RulesPreset::JlcpcbStandard2Layer,
        &world,
    );
    let rules = ruleset_for_world(preset, &world);
    let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());
    apply_routes(&mut world, &result);

    let pads = plated_pads(&mut world, &library);
    let traces: Vec<Trace> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query
            .iter(ecs)
            .filter(|trace| INNER.contains(&trace.layer))
            .cloned()
            .collect()
    };

    // Only the round part of each pad is measured, so a touch found here is
    // copper on copper whatever the rest of the pad's outline is.
    let mut own = 0;
    let mut foreign = Vec::new();
    for trace in &traces {
        for segment in &trace.segments {
            for pad in &pads {
                let reach = (pad.radius + trace.width.0 / 2) as f64;
                if distance_to_segment(pad.centre, segment.start, segment.end) >= reach {
                    continue;
                }
                if pad.net == Some(trace.net_id) {
                    own += 1;
                } else {
                    foreign.push(format!(
                        "{} ({:?}) under {:?} on {:?}",
                        pad.name,
                        pad.shape,
                        world.net_name(trace.net_id),
                        trace.layer
                    ));
                }
            }
        }
    }
    // The positive control: traces on the inner layers do end on these pads,
    // so the measure finds copper where it touches.
    assert!(
        own > 0,
        "no inner-layer trace reaches a plated pad of its own net"
    );
    assert!(
        foreign.is_empty(),
        "inner-layer traces over another net's plated pad: {foreign:?}"
    );
}
