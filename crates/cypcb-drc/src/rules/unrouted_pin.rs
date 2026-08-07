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
use cypcb_world::components::zone::{Zone, ZoneKind};
use cypcb_world::components::{FootprintRef, Layer, NetConnections, NetId, Position, RefDes};
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::{rotate_point, DrcRule};

/// Rule for pins a net names and no copper reaches.
pub struct UnroutedPinRule;

impl DrcRule for UnroutedPinRule {
    fn name(&self) -> &'static str {
        "unrouted-pin"
    }

    fn check(&self, world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        let traces: Vec<Trace> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<&Trace>();
            query.iter(ecs).cloned().collect()
        };
        let vias: Vec<Via> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<&Via>();
            query.iter(ecs).copied().collect()
        };
        let pours: Vec<Zone> = world
            .zones()
            .into_iter()
            .map(|(_, zone)| zone)
            .filter(|zone| zone.kind == ZoneKind::CopperPour)
            .collect();

        // How many pads each net has. A net with one pad has nothing to be
        // routed to, and reporting it would be reporting the design rather
        // than the board.
        let components: Vec<_> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(
                bevy_ecs::entity::Entity,
                &RefDes,
                &FootprintRef,
                &NetConnections,
                &Position,
                &cypcb_world::components::Rotation,
            )>();
            query
                .iter(ecs)
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

                let offset = rotate_point(pad.position, rotation.to_degrees());
                let centre = Point::new(
                    Nm(position.0.x.0 + offset.x.0),
                    Nm(position.0.y.0 + offset.y.0),
                );
                let half = (pad.size.0 .0 / 2, pad.size.1 .0 / 2);
                // A pad whose layer list names no copper this rule
                // understands is treated as being on every layer rather than
                // on none: reporting a pin because its footprint spells its
                // layers in a way this code has not met would be reporting the
                // reader, not the board.
                let mask: u32 = pad
                    .layers
                    .iter()
                    .filter_map(|layer| layer_bit(*layer))
                    .fold(0, |mask, bit| mask | bit);
                let mask = if mask == 0 { u32::MAX } else { mask };

                if copper_reaches(&traces, &vias, &pours, net, centre, half, mask) {
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

fn layer_bit(layer: Layer) -> Option<u32> {
    match layer {
        Layer::TopCopper => Some(0b01),
        Layer::BottomCopper => Some(0b10),
        Layer::Inner(n) if n < 30 => Some(1 << (n + 2)),
        _ => None,
    }
}

/// Whether any copper of `net` touches this pad, on a layer the pad is on.
fn copper_reaches(
    traces: &[Trace],
    vias: &[Via],
    pours: &[Zone],
    net: NetId,
    centre: Point,
    half: (i64, i64),
    mask: u32,
) -> bool {
    let overlaps = |min_x: i64, min_y: i64, max_x: i64, max_y: i64| -> bool {
        centre.x.0 + half.0 >= min_x
            && centre.x.0 - half.0 <= max_x
            && centre.y.0 + half.1 >= min_y
            && centre.y.0 - half.1 <= max_y
    };

    for trace in traces {
        if trace.net_id != net {
            continue;
        }
        match layer_bit(trace.layer) {
            Some(bit) if mask & bit != 0 => {}
            _ => continue,
        }
        let grow = trace.width.0 / 2;
        if trace.segments.iter().any(|segment| {
            overlaps(
                segment.start.x.0.min(segment.end.x.0) - grow,
                segment.start.y.0.min(segment.end.y.0) - grow,
                segment.start.x.0.max(segment.end.x.0) + grow,
                segment.start.y.0.max(segment.end.y.0) + grow,
            )
        }) {
            return true;
        }
    }

    for via in vias {
        if via.net_id != net {
            continue;
        }
        let radius = via.outer_diameter.0 / 2;
        if overlaps(
            via.position.x.0 - radius,
            via.position.y.0 - radius,
            via.position.x.0 + radius,
            via.position.y.0 + radius,
        ) {
            return true;
        }
    }

    for pour in pours {
        if pour.net != Some(net) || pour.layer_mask & mask == 0 {
            continue;
        }
        if overlaps(
            pour.bounds.min.x.0,
            pour.bounds.min.y.0,
            pour.bounds.max.x.0,
            pour.bounds.max.y.0,
        ) {
            return true;
        }
    }

    false
}
