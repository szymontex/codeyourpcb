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
//! boxes, the same segment-to-segment and segment-to-box distances less half
//! the trace width, the same via box on the same two layers. Two features that
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
use cypcb_world::components::zone::{Zone, ZoneKind};
use cypcb_world::components::{FootprintRef, NetConnections, Position, RefDes, Rotation};
use cypcb_world::{BoardWorld, Entity};
use hashbrown::HashMap;
use rstar::AABB;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::clearance::{
    aabb_distance, component_pads, segment_distance, trace_to_aabb_distance, TraceData,
};
use super::unrouted_pin::{pad_centre, pad_is_reached};
use super::DrcRule;

/// Rule for a net whose copper does not join all of its pins.
pub struct NetSplitRule;

/// A pin, as a piece of a net's copper names it.
struct Pin {
    label: String,
    entity: Entity,
    at: Point,
    /// Whether `UnroutedPinRule` counts copper as reaching it.
    reached: bool,
}

enum Shape {
    Pad(Pin),
    Segment {
        a: [i64; 2],
        b: [i64; 2],
        half_width: i64,
    },
    /// A via or a pour: copper that is its box.
    Solid,
}

struct Feature {
    shape: Shape,
    layer_mask: u32,
    /// The copper's extent; for a segment, grown by half its width.
    bounds: AABB<[i64; 2]>,
}

impl DrcRule for NetSplitRule {
    fn name(&self) -> &'static str {
        "net-split"
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
        let components: Vec<_> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(
                Entity,
                &RefDes,
                &FootprintRef,
                &NetConnections,
                &Position,
                &Rotation,
            )>();
            query
                .iter(ecs)
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
                    shape: Shape::Pad(Pin {
                        label: format!("{}.{}", refdes.as_str(), pad.number),
                        entity: *entity,
                        at,
                        reached: pad_is_reached(&traces, &vias, &pours, net, pad, at),
                    }),
                    layer_mask,
                    bounds: pad_box.box_,
                });
            }
        }

        let mut pins_per_net: HashMap<u32, usize> = HashMap::new();
        for (net, features) in &by_net {
            pins_per_net.insert(*net, features.len());
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
                features.push(Feature {
                    shape: Shape::Segment { a, b, half_width },
                    layer_mask,
                    bounds: AABB::from_corners(
                        [a[0].min(b[0]) - half_width, a[1].min(b[1]) - half_width],
                        [a[0].max(b[0]) + half_width, a[1].max(b[1]) + half_width],
                    ),
                });
            }
        }

        for via in &vias {
            let Some(features) = by_net.get_mut(&via.net_id.id()) else {
                continue;
            };
            let radius = via.outer_diameter.0 / 2;
            let (x, y) = (via.position.x.0, via.position.y.0);
            features.push(Feature {
                shape: Shape::Solid,
                layer_mask: via.start_layer.to_copper_mask() | via.end_layer.to_copper_mask(),
                bounds: AABB::from_corners([x - radius, y - radius], [x + radius, y + radius]),
            });
        }

        for pour in &pours {
            let Some(features) = pour.net.and_then(|net| by_net.get_mut(&net.id())) else {
                continue;
            };
            features.push(Feature {
                shape: Shape::Solid,
                layer_mask: pour.layer_mask,
                bounds: AABB::from_corners(
                    [pour.bounds.min.x.0, pour.bounds.min.y.0],
                    [pour.bounds.max.x.0, pour.bounds.max.y.0],
                ),
            });
        }

        let mut nets: Vec<u32> = by_net.keys().copied().collect();
        nets.sort_unstable();

        let mut violations = Vec::new();
        for net in nets {
            // A net with one pin has nothing to be joined to.
            if pins_per_net.get(&net).copied().unwrap_or(0) < 2 {
                continue;
            }
            let features = &by_net[&net];
            let pieces = pieces_of(features);
            let name = names
                .get(&net)
                .cloned()
                .unwrap_or_else(|| format!("#{net}"));

            let mut reported: Vec<(Vec<&Pin>, bool)> = pieces
                .into_iter()
                .filter_map(|members| {
                    let mut pins: Vec<&Pin> = Vec::new();
                    let mut conductor = false;
                    for index in members {
                        match &features[index].shape {
                            Shape::Pad(pin) => pins.push(pin),
                            _ => conductor = true,
                        }
                    }
                    if pins.is_empty() {
                        return None;
                    }
                    pins.sort_by(|a, b| a.label.cmp(&b.label));
                    Some((pins, conductor))
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
                    &name,
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
fn pieces_of(features: &[Feature]) -> Vec<Vec<usize>> {
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
            if touches(&features[i], &features[j]) {
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
    match (&one.shape, &other.shape) {
        (
            Shape::Segment {
                a: a1,
                b: a2,
                half_width: wa,
            },
            Shape::Segment {
                a: b1,
                b: b2,
                half_width: wb,
            },
        ) => segment_distance(*a1, *a2, *b1, *b2) <= wa + wb,
        (Shape::Segment { a, b, half_width }, _) => {
            segment_touches_box(*a, *b, *half_width, &other.bounds)
        }
        (_, Shape::Segment { a, b, half_width }) => {
            segment_touches_box(*a, *b, *half_width, &one.bounds)
        }
        // Pads, vias and pours are their boxes, and the boxes already meet.
        _ => true,
    }
}

fn segment_touches_box(a: [i64; 2], b: [i64; 2], half_width: i64, bounds: &AABB<[i64; 2]>) -> bool {
    let trace = TraceData {
        half_width,
        segments: vec![(a, b)],
    };
    trace_to_aabb_distance(&trace, bounds).1 <= half_width
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
