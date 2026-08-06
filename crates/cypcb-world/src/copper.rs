//! What a copper pour becomes, once the board it sits on is taken into account.
//!
//! A zone as written is a rectangle and a net. The copper it turns into is that
//! rectangle minus every other piece of copper on the layer, each grown by the
//! clearance the fabricator demands - and minus a smaller gap around the pads
//! it is meant to reach, bridged by thermal spokes so those pads can still be
//! soldered.
//!
//! This lives beside the world rather than in the exporter because two places
//! need the same answer. The exporter writes those rectangles into Gerber, and
//! the viewer draws them on screen. When the two computed it separately, the
//! screen showed a plain rectangle and the fabricator got copper cut around
//! every pad - the same board, disagreeing with itself.

use cypcb_core::pour::{self, PourOptions};
use cypcb_core::{Nm, Point, Rect};

use crate::components::zone::Zone;
use crate::components::trace::{Trace, Via};
use crate::components::{Layer, NetId};
use crate::footprint::FootprintLibrary;
use crate::world::BoardWorld;

/// The copper one pour becomes, as the rectangles that get made.
///
/// Split so a caller can tell the plane from the bridges into it: the exporter
/// emits both as regions, and a viewer may want to draw them differently.
#[derive(Debug, Clone, Default)]
pub struct FilledPour {
    /// The plane itself, cut around foreign copper and around its own pads.
    pub pieces: Vec<Rect>,
    /// Thermal spokes, clipped to the zone and to the copper they may occupy.
    pub spokes: Vec<Rect>,
}

impl FilledPour {
    /// Every rectangle in this pour, plane and spokes alike.
    pub fn all(&self) -> impl Iterator<Item = &Rect> {
        self.pieces.iter().chain(self.spokes.iter())
    }
}

/// Fill one zone against the board it sits on.
///
/// `layer` is the copper layer being filled; a zone spanning several layers is
/// filled once per layer, because what obstructs it differs on each.
pub fn fill_zone(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    layer: Layer,
    zone: &Zone,
    options: &PourOptions,
) -> FilledPour {
    let (obstacles, own_pads) = copper_on_layer(world, library, layer, zone.net);

    // The clearance applies to the foreign copper only. A pad on the pour's own
    // net is cut with the thermal gap instead and then bridged, so the joint
    // can be soldered: solid copper carries heat away from a pin faster than an
    // iron can put it in.
    let mut pieces = pour::fill(zone.bounds, &obstacles, options.clearance);
    for pad in &own_pads {
        let keepout = pour::grown(*pad, options.thermal_gap);
        pieces = pieces
            .into_iter()
            .flat_map(|piece| pour::fill(piece, &[keepout], Nm::ZERO))
            .collect();
    }

    // Put the spokes back, cut to the copper they are allowed to occupy.
    //
    // A spoke reaches a quarter of a millimetre past its pad, which on a dense
    // board is far enough to cross into a neighbour's clearance - measured on
    // two 0402s a millimetre apart, where the horizontal bar ran from 6.946mm
    // to 8.054mm and the foreign keepout started at 7.9mm. Subtracting the same
    // obstacles the pour was cut against keeps the bridge inside the plane it
    // bridges to.
    let mut spokes = Vec::new();
    for pad in &own_pads {
        for spoke in pour::thermal_spokes(*pad, options) {
            let Some(spoke) = pour::intersect(spoke, zone.bounds) else {
                continue;
            };
            spokes.extend(pour::fill(spoke, &obstacles, options.clearance));
        }
    }

    FilledPour { pieces, spokes }
}

/// Every piece of copper on this layer that a pour on `pour_net` must keep away
/// from, and separately the pads it is meant to reach.
///
/// Copper on the pour's own net is left out of the first list: the pour is that
/// net, and keeping clear of it would leave the plane unconnected to the thing
/// it grounds.
pub fn copper_on_layer(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    layer: Layer,
    pour_net: Option<NetId>,
) -> (Vec<Rect>, Vec<Rect>) {
    use crate::components::{FootprintRef, NetConnections, Position, Rotation};

    let mut boxes = Vec::new();
    let mut own = Vec::new();

    /// A placed part as this function needs it: where it is, how it is turned,
    /// which footprint it wears and what its pins are wired to.
    type Placement = (Point, f64, String, Vec<(String, NetId)>);

    let placements: Vec<Placement> = {
        let ecs = world.ecs_mut();
        let mut query =
            ecs.query::<(&Position, &Rotation, &FootprintRef, Option<&NetConnections>)>();
        query
            .iter(ecs)
            .map(|(position, rotation, footprint, nets)| {
                let pins = nets
                    .map(|n| n.iter().map(|p| (p.pin.clone(), p.net)).collect())
                    .unwrap_or_default();
                (
                    position.0,
                    rotation.to_degrees(),
                    footprint.as_str().to_string(),
                    pins,
                )
            })
            .collect()
    };

    for (position, degrees, name, pins) in placements {
        let Some(footprint) = library.get(&name) else {
            continue;
        };
        let radians = degrees.to_radians();
        let (sin, cos) = radians.sin_cos();

        for pad in &footprint.pads {
            if !pad.layers.contains(&layer) {
                continue;
            }
            let pad_net = pins
                .iter()
                .find(|(pin, _)| *pin == pad.number)
                .map(|(_, net)| *net);
            let is_own = pad_net.is_some() && pad_net == pour_net;

            let px = pad.position.x.0 as f64;
            let py = pad.position.y.0 as f64;
            let cx = position.x.0 + (px * cos - py * sin).round() as i64;
            let cy = position.y.0 + (px * sin + py * cos).round() as i64;
            let half_w = pad.size.0 .0 as f64 / 2.0;
            let half_h = pad.size.1 .0 as f64 / 2.0;
            let ex = (half_w * cos.abs() + half_h * sin.abs()).round() as i64;
            let ey = (half_w * sin.abs() + half_h * cos.abs()).round() as i64;

            let box_ = Rect {
                min: Point::new(Nm(cx - ex), Nm(cy - ey)),
                max: Point::new(Nm(cx + ex), Nm(cy + ey)),
            };
            if is_own {
                own.push(box_);
            } else {
                boxes.push(box_);
            }
        }
    }

    // Traces and vias.
    let traces: Vec<Trace> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query.iter(ecs).cloned().collect()
    };
    for trace in traces {
        if trace.layer != layer || Some(trace.net_id) == pour_net {
            continue;
        }
        let half = trace.width.0 / 2;
        for segment in &trace.segments {
            boxes.push(Rect {
                min: Point::new(
                    Nm(segment.start.x.0.min(segment.end.x.0) - half),
                    Nm(segment.start.y.0.min(segment.end.y.0) - half),
                ),
                max: Point::new(
                    Nm(segment.start.x.0.max(segment.end.x.0) + half),
                    Nm(segment.start.y.0.max(segment.end.y.0) + half),
                ),
            });
        }
    }

    let vias: Vec<Via> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Via>();
        query.iter(ecs).copied().collect()
    };
    for via in vias {
        if Some(via.net_id) == pour_net {
            continue;
        }
        let radius = via.outer_diameter.0 / 2;
        boxes.push(Rect {
            min: Point::new(Nm(via.position.x.0 - radius), Nm(via.position.y.0 - radius)),
            max: Point::new(Nm(via.position.x.0 + radius), Nm(via.position.y.0 + radius)),
        });
    }

    (boxes, own)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::zone::{Zone, ZoneKind};
    use crate::components::{FootprintRef, Layer, NetId, PadShape, Position, Rotation};
    use crate::footprint::{Footprint, PadDef};
    use crate::components::PinConnection;
    use crate::{NetConnections, RefDes, Value};

    fn board_with_one_pad(pad_net: u32) -> (BoardWorld, FootprintLibrary) {
        let mut world = BoardWorld::new();
        world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);

        let mut library = FootprintLibrary::new();
        library.register(Footprint {
            name: "PAD1".into(),
            description: String::new(),
            bounds: Rect::new(Point::ORIGIN, Point::ORIGIN),
            courtyard: Rect::new(Point::ORIGIN, Point::ORIGIN),
            silk: Vec::new(),
            pads: vec![PadDef {
                number: "1".into(),
                shape: PadShape::Rect,
                position: Point::ORIGIN,
                size: (Nm::from_mm(1.0), Nm::from_mm(1.0)),
                drill: None,
                layers: vec![Layer::TopCopper],
            }],
        });

        let mut connections = NetConnections::new();
        connections.add(PinConnection::new("1".to_string(), NetId::new(pad_net)));
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("1k"),
            Position::from_mm(10.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("PAD1"),
            connections,
        );

        (world, library)
    }

    fn ground_zone() -> Zone {
        Zone {
            bounds: Rect {
                min: Point::from_mm(5.0, 5.0),
                max: Point::from_mm(15.0, 15.0),
            },
            kind: ZoneKind::CopperPour,
            layer_mask: Layer::TopCopper.to_copper_mask(),
            name: None,
            net: Some(NetId::new(1)),
        }
    }

    #[test]
    fn a_pad_on_a_foreign_net_is_cut_out_of_the_plane() {
        let (mut world, library) = board_with_one_pad(2);
        let filled = fill_zone(
            &mut world,
            &library,
            Layer::TopCopper,
            &ground_zone(),
            &PourOptions::default(),
        );

        // The pad sits in the middle of the zone, so cutting it leaves several
        // pieces and none of them may contain the pad's centre.
        assert!(filled.pieces.len() > 1, "a hole in the middle splits a plane");
        let centre = Point::from_mm(10.0, 10.0);
        for piece in filled.all() {
            let inside = piece.min.x.0 <= centre.x.0
                && centre.x.0 <= piece.max.x.0
                && piece.min.y.0 <= centre.y.0
                && centre.y.0 <= piece.max.y.0;
            assert!(!inside, "copper covers a pad on another net");
        }
        assert!(
            filled.spokes.is_empty(),
            "a foreign pad gets no bridge to the plane"
        );
    }

    #[test]
    fn a_pad_on_the_pours_own_net_gets_bridged_to_it() {
        let (mut world, library) = board_with_one_pad(1);
        let filled = fill_zone(
            &mut world,
            &library,
            Layer::TopCopper,
            &ground_zone(),
            &PourOptions::default(),
        );

        assert!(
            !filled.spokes.is_empty(),
            "a pad on the pour's own net is connected by thermal spokes"
        );
    }
}
