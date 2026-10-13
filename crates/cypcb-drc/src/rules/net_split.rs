//! A net whose copper is in more than one piece.
//!
//! `UnroutedPinRule` asks whether any copper of a pin's net touches the pad.
//! Every pin of a net cut in two passes that: each half has copper and each
//! pin is reached by it, and the board is still an open circuit between the
//! halves. The via optimizer did this eight times across the benchmark boards
//! before its clearance check was made real - it removed a via that another
//! branch of the net climbed out of - and no rule said anything.
//!
//! This joins every feature of a net that touches another - pad, trace
//! segment, via, pour - and reports each piece beyond the largest that carries
//! a pin. "Touches" is `ClearanceRule`'s measurement at zero: the same pad
//! copper, the same segment-to-segment and segment-to-copper distances less
//! half the trace width, the same via disc on every layer it passes. Two features that
//! `ClearanceRule` would find 0 apart if they were on different nets are joined
//! here, and nothing else is.
//!
//! A pour counts as its whole outline, the way `UnroutedPinRule` counts it.
//! The fill can cut a plane into pieces that the outline joins; a piece no pad
//! reaches is `PourIslandRule`'s report. Reading the outline can hide a split,
//! and cannot invent one.
//!
//! A piece with no trace, via or pour in it, whose every pin `UnroutedPinRule`
//! already reports, is left to that rule: one fault, one line.

use cypcb_core::Point;
use cypcb_world::components::trace::{Trace, Via};
use cypcb_world::components::Layer;
use cypcb_world::components::{FootprintRef, NetConnections, NetId, Position, RefDes, Rotation};
use cypcb_world::in_build_order;
use cypcb_world::{BoardWorld, Entity};
use hashbrown::HashMap;
use rstar::AABB;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::clearance::{
    aabb_distance, component_pads, copper_distance, segment_distance, trace_to_copper_distance,
    Copper, Polygon, TraceData,
};
use super::unrouted_pin::{pad_centre, pad_is_reached, pours_of};
use super::DrcRule;

/// Rule for a net whose copper does not join all of its pins.
pub struct NetSplitRule;

/// A pad of a net, as a piece of that net's copper holds it.
#[derive(Clone, Debug)]
pub struct PiecePin {
    /// `R1.2`: the part and the pad number.
    pub label: String,
    /// The part the pad belongs to.
    pub entity: Entity,
    /// The pad number, as the footprint names it.
    pub pin: String,
    /// The centre of the pad on the board.
    pub at: Point,
    /// Whether copper reaches it: a trace, a via, a pour or another pad of
    /// its net that it touches. `UnroutedPinRule` reports the pads for which
    /// this is false.
    pub reached: bool,
}

/// A trace segment, as a piece of copper holds it.
#[derive(Clone, Copy, Debug)]
pub struct PieceSegment {
    /// The layer the segment is on.
    pub layer: Layer,
    /// One end.
    pub start: Point,
    /// The other end.
    pub end: Point,
    /// Half the trace width.
    pub half_width: i64,
}

/// Copper of one net that touches itself: the pads, segments, vias and pours
/// joined into one conductor.
#[derive(Clone, Debug, Default)]
pub struct CopperPiece {
    /// The pads in it.
    pub pins: Vec<PiecePin>,
    /// The trace segments in it.
    pub segments: Vec<PieceSegment>,
    /// The vias in it.
    pub vias: Vec<Via>,
    /// Whether a pour's outline is part of it.
    pub pour: bool,
    /// The outlines of the teardrops in it.
    pub teardrops: Vec<Vec<Point>>,
}

impl CopperPiece {
    /// Whether it holds any copper beyond pads: a pad on its own is a pin
    /// nothing reaches, not a conductor.
    pub fn is_conductor(&self) -> bool {
        !self.segments.is_empty()
            || !self.vias.is_empty()
            || self.pour
            || !self.teardrops.is_empty()
    }
}

/// A net's copper, in the pieces it is in.
#[derive(Clone, Debug)]
pub struct NetCopper {
    /// The net.
    pub net: NetId,
    /// Its name, or `#<id>` for a net the board does not name.
    pub name: String,
    /// How many pads the net has.
    pub pad_count: usize,
    /// Every piece, in the order its first feature was gathered: pads in
    /// build order, then segments, vias, pours.
    pub pieces: Vec<CopperPiece>,
}

impl NetCopper {
    /// The pieces that hold at least one pad. Two pads in different pieces
    /// are an open circuit; a piece with no pad is copper going nowhere,
    /// which is `PourIslandRule`'s report and not a missing connection.
    pub fn pieces_with_pads(&self) -> impl Iterator<Item = &CopperPiece> {
        self.pieces.iter().filter(|piece| !piece.pins.is_empty())
    }
}

enum Shape {
    Pad(PiecePin),
    Segment(PieceSegment),
    Via(Via),
    Pour,
    /// A pour drawn as a polygon: its ring.
    PourRing(Polygon),
    /// A teardrop: the corners as the model holds them, and as a polygon.
    Teardrop(Vec<Point>, Polygon),
}

struct Feature {
    shape: Shape,
    layer_mask: u32,
    /// The copper's extent; for a segment, grown by half its width.
    bounds: AABB<[i64; 2]>,
    /// The copper itself, for a pad, a via or a pour.
    copper: Copper,
}

/// Every net with a pad, and the pieces its copper is in.
///
/// The one answer to "which pads does the copper on this board already
/// join". `NetSplitRule` reports from it, the router routes only between its
/// pieces, and the viewer draws a ratsnest line for each connection it says
/// is missing. When those were three pieces of code, the viewer dropped the
/// ratsnest of every net that had a single trace, while the checker still
/// reported the pad that trace did not reach.
///
/// "Touches" is `ClearanceRule`'s measurement at zero: the same pad copper,
/// the same segment-to-segment and segment-to-copper distances less half the
/// trace width, the same via disc on every layer it passes. A pour counts as
/// its whole outline.
pub fn copper_pieces(world: &mut BoardWorld) -> Vec<NetCopper> {
    let traces: Vec<Trace> = {
        let ecs = world.ecs_mut();
        in_build_order::<&Trace>(ecs).into_iter().cloned().collect()
    };
    let vias: Vec<Via> = {
        let ecs = world.ecs_mut();
        in_build_order::<&Via>(ecs).into_iter().copied().collect()
    };
    let pours = pours_of(world);
    let components: Vec<_> = {
        let ecs = world.ecs_mut();
        in_build_order::<(
            Entity,
            &RefDes,
            &FootprintRef,
            &NetConnections,
            &Position,
            &Rotation,
        )>(ecs)
        .into_iter()
        .map(|(e, r, f, n, p, rot)| (e, r.clone(), f.clone(), n.clone(), *p, *rot))
        .collect()
    };
    let pad_boxes = component_pads(world);
    let names: HashMap<u32, String> = world
        .nets()
        .map(|(net, name)| (net.id(), name.to_string()))
        .collect();
    let library = world.footprints().clone();

    let mut by_net: HashMap<u32, Vec<Feature>> = HashMap::new();

    for (entity, refdes, footprint_ref, nets, position, rotation) in &components {
        let Some(footprint) = library.get(footprint_ref.as_str()) else {
            continue;
        };
        let Some(boxes) = pad_boxes.get(&entity.index()) else {
            continue;
        };
        for (pad, pad_box) in footprint.pads.iter().zip(boxes) {
            let Some(net) = nets.pin_net(&pad.number) else {
                continue;
            };
            let at = pad_centre(pad, position, rotation);
            // A pad on no copper layer this crate names is taken to be on
            // every layer, as `UnroutedPinRule` takes it. On none, it
            // would be reported as cut off for how its footprint spells a
            // layer.
            let layer_mask = if pad_box.layer_mask == 0 {
                u32::MAX
            } else {
                pad_box.layer_mask
            };
            by_net.entry(net.id()).or_default().push(Feature {
                shape: Shape::Pad(PiecePin {
                    label: format!("{}.{}", refdes.as_str(), pad.number),
                    entity: *entity,
                    pin: pad.number.clone(),
                    at,
                    reached: pad_is_reached(&traces, &vias, &pours, net, pad, &pad_box.copper),
                }),
                layer_mask,
                bounds: pad_box.copper.bounds(),
                copper: pad_box.copper,
            });
        }
    }

    let mut pads_per_net: HashMap<u32, usize> = HashMap::new();
    for (net, features) in &by_net {
        pads_per_net.insert(*net, features.len());
    }

    for trace in &traces {
        let layer_mask = trace.layer.to_copper_mask();
        if layer_mask == 0 || !by_net.contains_key(&trace.net_id.id()) {
            continue;
        }
        let half_width = trace.width.0 / 2;
        let features = by_net.entry(trace.net_id.id()).or_default();
        for segment in &trace.segments {
            let a = [segment.start.x.0, segment.start.y.0];
            let b = [segment.end.x.0, segment.end.y.0];
            let bounds = AABB::from_corners(
                [a[0].min(b[0]) - half_width, a[1].min(b[1]) - half_width],
                [a[0].max(b[0]) + half_width, a[1].max(b[1]) + half_width],
            );
            features.push(Feature {
                shape: Shape::Segment(PieceSegment {
                    layer: trace.layer,
                    start: segment.start,
                    end: segment.end,
                    half_width,
                }),
                layer_mask,
                bounds,
                copper: Copper::boxed(bounds),
            });
        }
    }

    for via in &vias {
        let Some(features) = by_net.get_mut(&via.net_id.id()) else {
            continue;
        };
        let copper = Copper::circle(
            [via.position.x.0, via.position.y.0],
            via.outer_diameter.0 / 2,
        );
        features.push(Feature {
            shape: Shape::Via(*via),
            layer_mask: via.copper_mask(),
            bounds: copper.bounds(),
            copper,
        });
    }

    // A teardrop is copper of its track's net, and joins what it touches as
    // the track does.
    for (_, teardrop, net) in cypcb_world::teardrop::teardrop_copper(world) {
        let Some(features) = by_net.get_mut(&net.id()) else {
            continue;
        };
        let bounds = teardrop.bounds();
        let bounds = AABB::from_corners(
            [bounds.min.x.0, bounds.min.y.0],
            [bounds.max.x.0, bounds.max.y.0],
        );
        features.push(Feature {
            shape: Shape::Teardrop(teardrop.outline.clone(), Polygon::new(&teardrop.outline)),
            layer_mask: teardrop.layer.to_copper_mask(),
            bounds,
            copper: Copper::boxed(bounds),
        });
    }

    for pour in &pours {
        let zone = &pour.zone;
        let Some(features) = zone.net.and_then(|net| by_net.get_mut(&net.id())) else {
            continue;
        };
        let bounds = AABB::from_corners(
            [zone.bounds.min.x.0, zone.bounds.min.y.0],
            [zone.bounds.max.x.0, zone.bounds.max.y.0],
        );
        // A pour drawn as a polygon joins what its ring touches, not what its box does.
        let shape = match &pour.outline {
            Some(outline) => Shape::PourRing(Polygon::new(outline.corners())),
            None => Shape::Pour,
        };
        features.push(Feature {
            shape,
            layer_mask: zone.layer_mask,
            bounds,
            copper: Copper::boxed(bounds),
        });
    }

    let mut nets: Vec<u32> = by_net.keys().copied().collect();
    nets.sort_unstable();

    let mut result = Vec::with_capacity(nets.len());
    for net in nets {
        let mut features: Vec<Option<Feature>> = by_net
            .remove(&net)
            .unwrap_or_default()
            .into_iter()
            .map(Some)
            .collect();
        let groups = {
            let present: Vec<&Feature> = features.iter().flatten().collect();
            pieces_of(&present)
        };
        let pieces = groups
            .into_iter()
            .map(|members| {
                let mut piece = CopperPiece::default();
                for index in members {
                    match features[index].take().map(|feature| feature.shape) {
                        Some(Shape::Pad(pin)) => piece.pins.push(pin),
                        Some(Shape::Segment(segment)) => piece.segments.push(segment),
                        Some(Shape::Via(via)) => piece.vias.push(via),
                        Some(Shape::Pour | Shape::PourRing(_)) => piece.pour = true,
                        Some(Shape::Teardrop(outline, _)) => piece.teardrops.push(outline),
                        None => {}
                    }
                }
                // A pad that touches another pad of its net is reached by
                // that pad's copper, the way it is reached by a trace.
                if piece.pins.len() > 1 {
                    for pin in &mut piece.pins {
                        pin.reached = true;
                    }
                }
                piece
            })
            .collect();
        result.push(NetCopper {
            net: NetId::new(net),
            name: names
                .get(&net)
                .cloned()
                .unwrap_or_else(|| format!("#{net}")),
            pad_count: pads_per_net.get(&net).copied().unwrap_or(0),
            pieces,
        });
    }
    result
}

impl DrcRule for NetSplitRule {
    fn name(&self) -> &'static str {
        "net-split"
    }

    fn check(&self, world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        let mut violations = Vec::new();
        for net in copper_pieces(world) {
            // A net with one pin has nothing to be joined to.
            if net.pad_count < 2 {
                continue;
            }

            let mut reported: Vec<(Vec<&PiecePin>, bool)> = net
                .pieces_with_pads()
                .map(|piece| {
                    let mut pins: Vec<&PiecePin> = piece.pins.iter().collect();
                    pins.sort_by(|a, b| a.label.cmp(&b.label));
                    (pins, piece.is_conductor())
                })
                .filter(|(pins, conductor)| *conductor || pins.iter().any(|pin| pin.reached))
                .collect();
            if reported.len() < 2 {
                continue;
            }

            // The largest piece is the net; every other one is cut off from it.
            reported.sort_by(|(a, _), (b, _)| {
                b.len()
                    .cmp(&a.len())
                    .then_with(|| a[0].label.cmp(&b[0].label))
            });
            let rest: Vec<String> = reported[0].0.iter().map(|pin| pin.label.clone()).collect();
            let main = reported[0].0[0].entity;
            for (pins, _) in &reported[1..] {
                let cut_off: Vec<String> = pins.iter().map(|pin| pin.label.clone()).collect();
                violations.push(DrcViolation::net_split(
                    pins[0].entity,
                    main,
                    &net.name,
                    &cut_off,
                    &rest,
                    pins[0].at,
                ));
            }
        }

        violations
    }
}

/// The features that touch one another, gathered into pieces.
fn pieces_of(features: &[&Feature]) -> Vec<Vec<usize>> {
    let mut parent: Vec<usize> = (0..features.len()).collect();
    fn root(parent: &mut [usize], mut at: usize) -> usize {
        while parent[at] != at {
            parent[at] = parent[parent[at]];
            at = parent[at];
        }
        at
    }

    for i in 0..features.len() {
        for j in (i + 1)..features.len() {
            if touches(features[i], features[j]) {
                let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                if a != b {
                    parent[a.max(b)] = a.min(b);
                }
            }
        }
    }

    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..features.len() {
        let r = root(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    let mut pieces: Vec<Vec<usize>> = groups.into_values().collect();
    pieces.sort_by_key(|members| members[0]);
    pieces
}

/// Whether two features of one net are joined: `ClearanceRule` would measure
/// no gap between them.
fn touches(one: &Feature, other: &Feature) -> bool {
    if one.layer_mask & other.layer_mask == 0 || aabb_distance(&one.bounds, &other.bounds) > 0 {
        return false;
    }
    let ends = |segment: &PieceSegment| {
        (
            [segment.start.x.0, segment.start.y.0],
            [segment.end.x.0, segment.end.y.0],
        )
    };
    match (&one.shape, &other.shape) {
        // Two pads of one net whose copper touches are one land, as KiCad's
        // connectivity joins them (`CN_VISITOR`, `connectivity_algo.cpp`,
        // read 2026-09-27). Touching is a gap of zero, the gap at which
        // `ClearanceRule` reports two nets shorted and the paste rule takes
        // two openings of one net for one hole.
        (Shape::Pad(_), Shape::Pad(_)) => copper_distance(&one.copper, &other.copper) == 0,
        (Shape::Segment(first), Shape::Segment(second)) => {
            let ((a1, a2), (b1, b2)) = (ends(first), ends(second));
            segment_distance(a1, a2, b1, b2) <= first.half_width + second.half_width
        }
        (
            Shape::Teardrop(_, first) | Shape::PourRing(first),
            Shape::Teardrop(_, second) | Shape::PourRing(second),
        ) => first.to_polygon(second).1 == 0,
        (Shape::Teardrop(_, polygon) | Shape::PourRing(polygon), Shape::Segment(segment))
        | (Shape::Segment(segment), Shape::Teardrop(_, polygon) | Shape::PourRing(polygon)) => {
            let (a, b) = ends(segment);
            polygon.to_segment(a, b).1 <= segment.half_width
        }
        (Shape::Teardrop(_, polygon) | Shape::PourRing(polygon), _) => {
            polygon.to_copper(&other.copper).1 == 0
        }
        (_, Shape::Teardrop(_, polygon) | Shape::PourRing(polygon)) => {
            polygon.to_copper(&one.copper).1 == 0
        }
        (Shape::Segment(segment), _) => {
            let (a, b) = ends(segment);
            segment_touches(a, b, segment.half_width, &other.copper)
        }
        (_, Shape::Segment(segment)) => {
            let (a, b) = ends(segment);
            segment_touches(a, b, segment.half_width, &one.copper)
        }
        _ => copper_distance(&one.copper, &other.copper) == 0,
    }
}

fn segment_touches(a: [i64; 2], b: [i64; 2], half_width: i64, copper: &Copper) -> bool {
    let trace = TraceData {
        half_width,
        segments: vec![(a, b)],
    };
    trace_to_copper_distance(&trace, copper).1 <= half_width
}

#[cfg(test)]
mod tests {
    use super::*;

    use cypcb_core::{Nm, Rect};
    use cypcb_world::components::trace::{TraceSegment, TraceSource};
    use cypcb_world::components::zone::Zone;
    use cypcb_world::components::{NetId, PinConnection, Value};
    use cypcb_world::Layer;

    use crate::rules::UnroutedPinRule;
    use crate::ViolationKind;

    /// A 0402 whose pin 1 is on `net` and pin 2 on a net of its own.
    fn part(world: &mut BoardWorld, refdes: &str, at: (f64, f64), net: NetId) -> (f64, f64) {
        let own = world.intern_net(&format!("{refdes}_2"));
        let mut nets = NetConnections::new();
        nets.add(PinConnection::new("1", net));
        nets.add(PinConnection::new("2", own));
        world.spawn_component(
            RefDes::new(refdes),
            Value::new("10k"),
            Position::from_mm(at.0, at.1),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            nets,
        );
        let pad = world
            .footprints()
            .get("0402")
            .and_then(|footprint| footprint.pads.iter().find(|pad| pad.number == "1"))
            .map(|pad| pad.position)
            .expect("0402 has a pin 1");
        (at.0 + pad.x.to_mm(), at.1 + pad.y.to_mm())
    }

    fn trace(world: &mut BoardWorld, net: NetId, layer: Layer, points: &[(f64, f64)]) {
        let segments = points
            .windows(2)
            .map(|pair| {
                TraceSegment::new(
                    Point::from_mm(pair[0].0, pair[0].1),
                    Point::from_mm(pair[1].0, pair[1].1),
                )
            })
            .collect();
        world.spawn_entity((
            Trace {
                segments,
                width: Nm::from_mm(0.25),
                layer,
                net_id: net,
                locked: false,
                source: TraceSource::Manual,
            },
            net,
        ));
    }

    fn via(world: &mut BoardWorld, net: NetId, at: (f64, f64)) {
        world.spawn_entity((
            Via {
                position: Point::from_mm(at.0, at.1),
                drill: Nm::from_mm(0.3),
                outer_diameter: Nm::from_mm(0.6),
                start_layer: Layer::TopCopper,
                end_layer: Layer::BottomCopper,
                net_id: net,
                locked: false,
            },
            net,
        ));
    }

    fn blind_via(world: &mut BoardWorld, net: NetId, at: (f64, f64)) {
        world.spawn_entity((
            Via {
                position: Point::from_mm(at.0, at.1),
                drill: Nm::from_mm(0.3),
                outer_diameter: Nm::from_mm(0.6),
                start_layer: Layer::TopCopper,
                end_layer: Layer::Inner(1),
                net_id: net,
                locked: false,
            },
            net,
        ));
    }

    fn splits(world: &mut BoardWorld) -> Vec<String> {
        NetSplitRule
            .check(world, &DesignRules::default())
            .into_iter()
            .map(|violation| {
                assert_eq!(violation.kind, ViolationKind::NetSplit);
                violation.message
            })
            .collect()
    }

    /// Three GND pins; R1 and R2 are joined, R3 has copper that stops in
    /// open board.
    fn cut_board() -> (BoardWorld, NetId, [(f64, f64); 3]) {
        let mut world = BoardWorld::new();
        let gnd = world.intern_net("GND");
        let r1 = part(&mut world, "R1", (10.0, 10.0), gnd);
        let r2 = part(&mut world, "R2", (30.0, 10.0), gnd);
        let r3 = part(&mut world, "R3", (20.0, 30.0), gnd);
        trace(&mut world, gnd, Layer::TopCopper, &[r1, r2]);
        trace(&mut world, gnd, Layer::TopCopper, &[r3, (r3.0, 20.0)]);
        (world, gnd, [r1, r2, r3])
    }

    #[test]
    fn a_net_cut_in_two_is_one_violation_naming_both_sides() {
        let (mut world, _, _) = cut_board();
        let found = splits(&mut world);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(
            found[0],
            "net GND is in pieces: no copper joins R3.1 to R1.1, R2.1"
        );
        // Each pin has copper on it, which is all `UnroutedPinRule` asks.
        assert!(UnroutedPinRule
            .check(&mut world, &DesignRules::default())
            .is_empty());
    }

    #[test]
    fn the_same_net_joined_is_quiet() {
        let (mut world, gnd, [_, _, r3]) = cut_board();
        trace(
            &mut world,
            gnd,
            Layer::TopCopper,
            &[(r3.0, 20.0), (r3.0, 10.0)],
        );
        assert!(splits(&mut world).is_empty());
    }

    #[test]
    fn a_via_joins_two_layers_and_without_it_the_branch_is_cut_off() {
        // The shape the old via optimizer left on `led_blink`: a branch leaves
        // on the bottom layer from a via, and the via goes.
        let build = |with_via: bool| {
            let mut world = BoardWorld::new();
            let gnd = world.intern_net("GND");
            let r1 = part(&mut world, "R1", (10.0, 10.0), gnd);
            let r2 = part(&mut world, "R2", (30.0, 10.0), gnd);
            trace(&mut world, gnd, Layer::TopCopper, &[r1, r2]);
            trace(
                &mut world,
                gnd,
                Layer::BottomCopper,
                &[(20.0, r1.1), (20.0, 25.0)],
            );
            via(&mut world, gnd, (20.0, 25.0));
            let r3 = part(&mut world, "R3", (25.0, 25.0), gnd);
            trace(&mut world, gnd, Layer::TopCopper, &[(20.0, 25.0), r3]);
            if with_via {
                via(&mut world, gnd, (20.0, r1.1));
            }
            world
        };
        assert!(splits(&mut build(true)).is_empty());
        let found = splits(&mut build(false));
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("no copper joins R3.1 to R1.1, R2.1"));
    }

    #[test]
    fn a_via_joins_the_layers_it_is_drilled_through_as_well_as_its_ends() {
        // Two Top-to-Inner2 vias joined by a track on Inner1, the layer both
        // pass through and neither names.
        let build = |between: Layer| {
            let mut world = BoardWorld::new();
            let gnd = world.intern_net("GND");
            let r1 = part(&mut world, "R1", (10.0, 10.0), gnd);
            let r2 = part(&mut world, "R2", (30.0, 10.0), gnd);
            trace(&mut world, gnd, Layer::TopCopper, &[r1, (15.0, r1.1)]);
            trace(&mut world, gnd, Layer::TopCopper, &[(25.0, r2.1), r2]);
            blind_via(&mut world, gnd, (15.0, r1.1));
            blind_via(&mut world, gnd, (25.0, r2.1));
            trace(&mut world, gnd, between, &[(15.0, r1.1), (25.0, r2.1)]);
            world
        };
        assert!(splits(&mut build(Layer::Inner(0))).is_empty());
        // The control: on the face the vias stop short of, nothing joins.
        let found = splits(&mut build(Layer::BottomCopper));
        assert_eq!(found.len(), 1, "{found:?}");
    }

    #[test]
    fn touching_is_what_clearance_measures_as_no_gap() {
        // The 0402 pad is 0.6mm wide; its edge is 0.3mm from its centre. A
        // 0.25mm trace whose end sits 0.425mm from the centre has its copper
        // edge on the pad edge; 0.01mm further and there is a gap.
        for (end_from_centre, cut) in [(0.425, false), (0.435, true)] {
            let mut world = BoardWorld::new();
            let gnd = world.intern_net("GND");
            let r1 = part(&mut world, "R1", (10.0, 10.0), gnd);
            let r2 = part(&mut world, "R2", (30.0, 10.0), gnd);
            trace(
                &mut world,
                gnd,
                Layer::TopCopper,
                &[r2, (r1.0 + end_from_centre, r1.1)],
            );
            trace(&mut world, gnd, Layer::TopCopper, &[r1, (r1.0, 5.0)]);
            assert_eq!(
                splits(&mut world).len(),
                usize::from(cut),
                "trace end {end_from_centre}mm from the pad centre"
            );
        }
    }

    #[test]
    fn copper_on_the_other_layer_does_not_touch_a_surface_pad() {
        let mut world = BoardWorld::new();
        let gnd = world.intern_net("GND");
        let r1 = part(&mut world, "R1", (10.0, 10.0), gnd);
        let r2 = part(&mut world, "R2", (30.0, 10.0), gnd);
        trace(&mut world, gnd, Layer::TopCopper, &[r1, (r1.0, 5.0)]);
        trace(&mut world, gnd, Layer::BottomCopper, &[r1, r2]);
        trace(&mut world, gnd, Layer::TopCopper, &[r2, (r2.0, 5.0)]);
        assert_eq!(splits(&mut world).len(), 1);
    }

    #[test]
    fn a_pin_no_copper_reaches_is_left_to_unrouted_pin() {
        let mut world = BoardWorld::new();
        let gnd = world.intern_net("GND");
        let r1 = part(&mut world, "R1", (10.0, 10.0), gnd);
        let r2 = part(&mut world, "R2", (30.0, 10.0), gnd);
        part(&mut world, "R3", (20.0, 30.0), gnd);
        trace(&mut world, gnd, Layer::TopCopper, &[r1, r2]);
        assert!(splits(&mut world).is_empty());
        assert_eq!(
            UnroutedPinRule
                .check(&mut world, &DesignRules::default())
                .len(),
            1
        );
    }

    #[test]
    fn a_pour_joins_the_copper_it_covers() {
        let (mut world, gnd, _) = cut_board();
        world.ecs_mut().spawn(Zone::copper_pour_for_net(
            Rect::from_points(Point::from_mm(5.0, 5.0), Point::from_mm(35.0, 35.0)),
            Layer::TopCopper.to_copper_mask(),
            gnd,
        ));
        assert!(splits(&mut world).is_empty());
    }
}
