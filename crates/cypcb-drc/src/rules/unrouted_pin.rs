//! A pin the design connects and no copper reaches.
//!
//! `UnconnectedPinRule` asks whether the schematic names a net for a pin. That
//! is a question about intent: a pin listed in `net GND { U1.4 }` passes it
//! whether or not one millimetre of copper was ever laid. The board is then
//! made with an open circuit and nothing in the file set says so.
//!
//! This asks the other question - is there copper of that net touching the pad
//! - and it is the one a fabricated board answers.

use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::{Trace, Via};
use cypcb_world::components::zone::{Zone, ZoneKind, ZoneOutline};
use cypcb_world::components::{FootprintRef, NetConnections, NetId, Position, RefDes, Rotation};
use cypcb_world::footprint::{FootprintLibrary, PadDef};
use cypcb_world::in_build_order;
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::clearance::{
    copper_distance, pad_copper, trace_to_copper_distance, Copper, Polygon, TraceData,
};
use super::{layer_bit, rotate_point, DrcRule};

/// Rule for pins a net names and no copper reaches.
pub struct UnroutedPinRule;

impl DrcRule for UnroutedPinRule {
    fn name(&self) -> &'static str {
        "unrouted-pin"
    }

    fn check(&self, world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        let traces: Vec<Trace> = {
            let ecs = world.ecs_mut();
            in_build_order::<&Trace>(ecs).into_iter().cloned().collect()
        };
        let vias: Vec<Via> = {
            let ecs = world.ecs_mut();
            in_build_order::<&Via>(ecs).into_iter().copied().collect()
        };
        let pours = pours_of(world);

        // How many pads each net has. A net with one pad has nothing to be
        // routed to, and reporting it would be reporting the design rather
        // than the board.
        let components: Vec<_> = {
            let ecs = world.ecs_mut();
            in_build_order::<(
                bevy_ecs::entity::Entity,
                &RefDes,
                &FootprintRef,
                &NetConnections,
                &Position,
                &cypcb_world::components::Rotation,
            )>(ecs)
            .into_iter()
            .map(|(e, r, f, n, p, rot)| (e, r.clone(), f.clone(), n.clone(), *p, *rot))
            .collect()
        };

        let library = world.footprints().clone();
        let mut pads_per_net: std::collections::HashMap<u32, usize> =
            std::collections::HashMap::new();
        for (_, _, footprint_ref, nets, _, _) in &components {
            let Some(footprint) = library.get(footprint_ref.as_str()) else {
                continue;
            };
            for pad in &footprint.pads {
                if let Some(net) = nets.pin_net(&pad.number) {
                    *pads_per_net.entry(net.id()).or_default() += 1;
                }
            }
        }

        // Which pads copper reaches, as `copper_pieces` joins them: a pad
        // touching another pad of its net is reached by it. Asking the pieces
        // keeps this rule, the ratsnest and the router on one answer.
        let reached: std::collections::HashSet<(bevy_ecs::entity::Entity, String, i64, i64)> =
            super::copper_pieces(world)
                .into_iter()
                .flat_map(|net| net.pieces)
                .flat_map(|piece| piece.pins)
                .filter(|pin| pin.reached)
                .map(|pin| (pin.entity, pin.pin, pin.at.x.0, pin.at.y.0))
                .collect();
        let mut violations = Vec::new();

        for (entity, refdes, footprint_ref, nets, position, rotation) in &components {
            let Some(footprint) = library.get(footprint_ref.as_str()) else {
                continue;
            };

            for pad in &footprint.pads {
                let Some(net) = nets.pin_net(&pad.number) else {
                    continue; // No net at all - that is UnconnectedPinRule's question
                };
                if pads_per_net.get(&net.id()).copied().unwrap_or(0) < 2 {
                    continue;
                }

                let centre = pad_centre(pad, position, rotation);
                let copper = pad_copper(pad, position.0, rotation.to_degrees());
                if reached.contains(&(*entity, pad.number.clone(), centre.x.0, centre.y.0))
                    || pad_is_reached(&traces, &vias, &pours, net, pad, &copper)
                {
                    continue;
                }

                violations.push(DrcViolation::unrouted_pin(
                    *entity,
                    &pad.number,
                    refdes.as_str(),
                    centre,
                ));
            }
        }

        violations
    }
}

/// A pad on a net, with the copper and layers this rule reaches it by.
///
/// For code outside the checker that has to keep a pad reached: the smoother
/// asks it whether a segment it is about to move touched a pad, and whether
/// the segment it moved there still does.
#[derive(Clone, Debug)]
pub struct NetPad {
    /// The net the pad is on.
    pub net: NetId,
    /// The copper layers the pad is on, as `PadDef::copper_mask()` gives them.
    pub mask: u32,
    copper: Copper,
}

impl NetPad {
    /// Whether a trace of `half_width` from `from` to `to` touches the pad,
    /// as `UnroutedPinRule` measures it. The layer is the caller's question.
    pub fn touched_by(&self, from: Point, to: Point, half_width: i64) -> bool {
        let trace = TraceData {
            half_width,
            segments: vec![([from.x.0, from.y.0], [to.x.0, to.y.0])],
        };
        trace_to_copper_distance(&trace, &self.copper).1 <= half_width
    }
}

/// Every pad of `world` that is on a net, with its copper.
pub fn net_pads(world: &mut BoardWorld, library: &FootprintLibrary) -> Vec<NetPad> {
    let ecs = world.ecs_mut();
    let mut pads = Vec::new();
    for (footprint_ref, nets, position, rotation) in
        in_build_order::<(&FootprintRef, &NetConnections, &Position, &Rotation)>(ecs)
    {
        let Some(footprint) = library.get(footprint_ref.as_str()) else {
            continue;
        };
        for pad in &footprint.pads {
            let Some(net) = nets.pin_net(&pad.number) else {
                continue;
            };
            let mask = pad.copper_mask();
            pads.push(NetPad {
                net,
                mask: if mask == 0 { u32::MAX } else { mask },
                copper: pad_copper(pad, position.0, rotation.to_degrees()),
            });
        }
    }
    pads
}

/// Where a pad's centre sits on the board.
pub(crate) fn pad_centre(pad: &PadDef, position: &Position, rotation: &Rotation) -> Point {
    let offset = rotate_point(pad.position, rotation.to_degrees());
    Point::new(
        Nm(position.0.x.0 + offset.x.0),
        Nm(position.0.y.0 + offset.y.0),
    )
}

/// A copper pour as the rules that ask what it reaches see it: the zone, and
/// the ring of corners when it is not a rectangle.
pub(crate) struct Pour {
    pub zone: Zone,
    pub outline: Option<ZoneOutline>,
}

/// Every copper pour on the board, the rectangles first and then those drawn as
/// a polygon.
pub(crate) fn pours_of(world: &mut BoardWorld) -> Vec<Pour> {
    let rectangles = world
        .zones()
        .into_iter()
        .filter(|(_, zone)| zone.kind == ZoneKind::CopperPour)
        .map(|(_, zone)| Pour {
            zone,
            outline: None,
        });
    let outlined = world
        .outlined_zones()
        .into_iter()
        .filter(|(_, zone, _)| zone.kind == ZoneKind::CopperPour)
        .map(|(_, zone, outline)| Pour {
            zone,
            outline: Some(outline),
        });
    rectangles.chain(outlined).collect()
}

/// Whether this rule counts the pad as reached by copper of `net`.
///
/// `NetSplitRule` asks it too, so that a pin nothing reaches is reported by
/// one rule and not by both.
pub(crate) fn pad_is_reached(
    traces: &[Trace],
    vias: &[Via],
    pours: &[Pour],
    net: NetId,
    pad: &PadDef,
    copper: &Copper,
) -> bool {
    // A pad whose layer list names no copper this rule understands is treated
    // as being on every layer rather than on none: reporting a pin because its
    // footprint spells its layers in a way this code has not met would be
    // reporting the reader, not the board.
    let mask: u32 = pad.copper_mask();
    let mask = if mask == 0 { u32::MAX } else { mask };
    copper_reaches(traces, vias, pours, net, copper, mask)
}

/// Whether any copper of `net` touches this pad, on a layer the pad is on.
///
/// "Touches" is what `ClearanceRule` and `NetSplitRule` mean by it: no gap
/// between the copper, a via and a round pad measured as discs and a trace as
/// its centreline grown by half its width. A pour is its bounds, as
/// `NetSplitRule` takes it.
fn copper_reaches(
    traces: &[Trace],
    vias: &[Via],
    pours: &[Pour],
    net: NetId,
    copper: &Copper,
    mask: u32,
) -> bool {
    for trace in traces {
        if trace.net_id != Some(net) {
            continue;
        }
        match layer_bit(trace.layer) {
            Some(bit) if mask & bit != 0 => {}
            _ => continue,
        }
        let data = TraceData {
            half_width: trace.width.0 / 2,
            segments: trace
                .segments
                .iter()
                .map(|segment| {
                    (
                        [segment.start.x.0, segment.start.y.0],
                        [segment.end.x.0, segment.end.y.0],
                    )
                })
                .collect(),
        };
        if trace_to_copper_distance(&data, copper).1 <= data.half_width {
            return true;
        }
    }

    for via in vias {
        if via.net_id != Some(net) {
            continue;
        }
        let disc = Copper::circle(
            [via.position.x.0, via.position.y.0],
            via.outer_diameter.0 / 2,
        );
        if copper_distance(&disc, copper) == 0 {
            return true;
        }
    }

    for pour in pours {
        let zone = &pour.zone;
        if zone.net != Some(net) || zone.layer_mask & mask == 0 {
            continue;
        }
        // A pour drawn as a polygon is its ring and not the box round it.
        if let Some(outline) = &pour.outline {
            if Polygon::new(outline.corners()).to_copper(copper).1 == 0 {
                return true;
            }
            continue;
        }
        let bounds = Copper::boxed(rstar::AABB::from_corners(
            [zone.bounds.min.x.0, zone.bounds.min.y.0],
            [zone.bounds.max.x.0, zone.bounds.max.y.0],
        ));
        if copper_distance(&bounds, copper) == 0 {
            return true;
        }
    }

    false
}
