//! Clearance checking rule.
//!
//! Detects copper features that are too close together for manufacturing.
//! Uses the spatial index for efficient O(log n) candidate selection.

use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::{Trace, Via};
use cypcb_world::components::{NetConnections, NetId};
use cypcb_world::BoardWorld;
use hashbrown::{HashMap, HashSet};
use rstar::{Envelope, AABB};

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for checking minimum clearance between copper features.
///
/// This rule verifies that all copper features on the same layer maintain
/// at least the minimum clearance distance specified by the design rules.
///
/// # Algorithm
///
/// 1. Iterate over all spatial entries
/// 2. For each entry, expand its bounding box by min_clearance
/// 3. Query the spatial index for overlapping candidates
/// 4. Filter candidates:
///    - Skip self
///    - Skip different layers (no copper overlap possible)
///    - Skip already-checked pairs (canonical ordering)
/// 5. Calculate actual AABB distance
/// 6. Report violations if distance < min_clearance
///
/// # Examples
///
/// ```rust,ignore
/// use cypcb_drc::rules::{ClearanceRule, DrcRule};
/// use cypcb_drc::presets::DesignRules;
/// use cypcb_world::BoardWorld;
///
/// let mut world = BoardWorld::new();
/// // ... populate world ...
///
/// let rules = DesignRules::jlcpcb_2layer();
/// let violations = ClearanceRule.check(&mut world, &rules);
///
/// for v in violations {
///     println!("Clearance violation at {:?}: {}", v.location, v.message);
/// }
/// ```
pub struct ClearanceRule;

impl DrcRule for ClearanceRule {
    fn name(&self) -> &'static str {
        "clearance"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let mut violations = Vec::new();
        // Every contact on the board, before they are counted into places.
        let mut found: Vec<Contact> = Vec::new();
        let min_clearance = rules.min_clearance;

        // Build entity -> NetId lookup for same-net exemption.
        // Entities on the same net (e.g. two pads both on VCC) should not
        // generate clearance violations — they're intentionally connected.
        let net_map: HashMap<u32, NetId> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &NetId)>();
            query.iter(ecs).map(|(e, n)| (e.index(), *n)).collect()
        };

        // What each net's own block asks for. A design that writes
        // `net HV [clearance 0.5mm]` has stated a rule; the fab preset is a
        // floor, not the answer, and a checker that quietly applies the floor
        // instead passes a board the designer said was wrong.
        let net_clearance: HashMap<u32, Nm> = {
            let ids: Vec<u32> = world.nets().map(|(net, _name)| net.id()).collect();
            ids.into_iter()
                .filter_map(|id| {
                    let stated = world.net_constraints(NetId::new(id))?.clearance?;
                    Some((id, stated))
                })
                .collect()
        };

        // Build entity -> NetConnections lookup for components.
        // Components (footprints) don't have a single NetId — they have
        // NetConnections mapping each pin to a net. A trace touching a
        // component's pad should be exempt if the trace's net matches
        // any of the component's pin nets.
        let net_connections_map: HashMap<u32, Vec<NetId>> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &NetConnections)>();
            query
                .iter(ecs)
                .map(|(e, nc)| {
                    let nets: Vec<NetId> = nc.iter().map(|pc| pc.net).collect();
                    (e.index(), nets)
                })
                .collect()
        };

        // Pre-collect each component's pad copper.
        //
        // A component sits in the spatial index as its courtyard - the
        // assembly keepout that covers the whole part body. Clearance is a
        // copper rule, and the body is not copper: measured against the
        // courtyard, a trace running through the gap between two pads reads as
        // a dead short, which is ordinary manufacturing. Bodies that collide
        // are `CourtyardClearanceRule`'s subject, not this one's.
        let pad_map: HashMap<u32, Vec<PadBox>> = component_pads(world);

        // Pre-collect trace data for refined segment distance checking.
        // Each trace entity maps to (half_width, segments) for exact distance.
        let trace_map: HashMap<u32, TraceData> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Trace)>();
            query
                .iter(ecs)
                .map(|(e, t)| {
                    let segs: Vec<([i64; 2], [i64; 2])> = t
                        .segments
                        .iter()
                        .map(|s| ([s.start.x.0, s.start.y.0], [s.end.x.0, s.end.y.0]))
                        .collect();
                    (
                        e.index(),
                        TraceData {
                            half_width: t.width.0 / 2,
                            segments: segs,
                        },
                    )
                })
                .collect()
        };

        // A via is a disc of copper. Its entry in the spatial index is the
        // square around that disc, and measured as the square a trace passing
        // the corner reads 0.00mm from copper it clears: on `multi_ic`, a gap
        // of 0.033mm reported as a short, and two vias whose discs are
        // 0.166mm apart reported as touching.
        let via_map: HashMap<u32, Copper> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Via)>();
            query
                .iter(ecs)
                .map(|(e, via)| {
                    (
                        e.index(),
                        Copper::circle(
                            [via.position.x.0, via.position.y.0],
                            via.outer_diameter.0 / 2,
                        ),
                    )
                })
                .collect()
        };
        // The copper of anything that is not a component: a via's disc, or
        // the entry's own box.
        let shape_of = |idx: u32, envelope: &AABB<[i64; 2]>| -> Copper {
            via_map
                .get(&idx)
                .copied()
                .unwrap_or_else(|| Copper::boxed(*envelope))
        };

        // Track checked pairs to avoid A-B and B-A duplicates
        let mut checked_pairs: HashSet<(u32, u32)> = HashSet::new();

        // Collect all entries first to avoid borrowing issues
        let entries: Vec<_> = world.spatial().iter().cloned().collect();

        // The broad phase has to reach as far as the strictest rule in play.
        // Expanding by the preset alone means a net that asks for more can
        // never be caught: the pair is filtered out before anyone looks at what
        // it required.
        let widest = net_clearance
            .values()
            .copied()
            .fold(min_clearance, |acc, stated| acc.max(stated));

        for entry in &entries {
            // A component is looked for from as far as its pads reach, not
            // only its box in the index. That box is the courtyard moved to
            // the part's position and never turned with it, so on a part
            // rotated 90 degrees the pads stand outside it: C3 on
            // `esp32_starter` has copper a trace overlaps, and the pair was
            // only ever measured because another segment of the same trace
            // entity reached the courtyard. Cut into one entity per segment,
            // that trace lost the short. The pair is found from this side;
            // the other side's query may still miss the box.
            let mut reach = entry.envelope;
            for pad in pad_map.get(&entry.entity.index()).into_iter().flatten() {
                reach.merge(&pad.copper.bounds());
            }
            let query_min = Point::new(
                Nm(reach.lower()[0] - widest.0),
                Nm(reach.lower()[1] - widest.0),
            );
            let query_max = Point::new(
                Nm(reach.upper()[0] + widest.0),
                Nm(reach.upper()[1] + widest.0),
            );

            // Phase 1: R*-tree query for candidates
            for candidate in world.spatial().query_region_entries(query_min, query_max) {
                // Skip self (same entity — traces may have multiple AABB entries)
                if candidate.entity == entry.entity {
                    continue;
                }

                // Skip if different layers (no copper overlap possible)
                if !entry.layers_overlap(candidate.layer_mask) {
                    continue;
                }

                // Canonical pair ordering to avoid duplicate checks
                let pair = canonical_pair(entry.entity.index(), candidate.entity.index());
                if !checked_pairs.insert(pair) {
                    continue; // Already checked
                }

                // Same-net exemption: skip clearance check if both entities
                // belong to the same net (they're electrically connected).
                //
                // Three cases:
                //  1. Both have NetId → direct comparison (trace-trace, trace-via)
                //  2. One has NetId, the other has NetConnections → the trace/via
                //     net must appear in the component's pin connections
                //     (trace touching its own component's pad)
                //  3. Both have NetConnections → share at least one common net
                //     (two components with connected pads adjacent)
                let a_idx = entry.entity.index();
                let b_idx = candidate.entity.index();
                let net_a = net_map.get(&a_idx);
                let net_b = net_map.get(&b_idx);
                let nc_a = net_connections_map.get(&a_idx);
                let nc_b = net_connections_map.get(&b_idx);

                // Where pad geometry exists the exemption is decided per pad,
                // further down, and not here: a part with one GND pin is not a
                // GND part, and exempting the whole component would hide a
                // trace running across its VCC pad.
                let has_pads_a = pad_map.contains_key(&a_idx);
                let has_pads_b = pad_map.contains_key(&b_idx);

                let same_net = match (net_a, net_b) {
                    // Case 1: both have a single NetId
                    (Some(na), Some(nb)) => na == nb,
                    // Case 2a: A has NetId, B is a component
                    (Some(na), None) => !has_pads_b && nc_b.is_some_and(|nets| nets.contains(na)),
                    // Case 2b: B has NetId, A is a component
                    (None, Some(nb)) => !has_pads_a && nc_a.is_some_and(|nets| nets.contains(nb)),
                    // Case 3: both are components — share a common net
                    (None, None) => match (nc_a, nc_b) {
                        _ if has_pads_a && has_pads_b => false,
                        (Some(nets_a), Some(nets_b)) => nets_a.iter().any(|n| nets_b.contains(n)),
                        _ => false,
                    },
                };

                if same_net {
                    continue;
                }

                // Phase 2: Calculate actual distance.
                // If either entity is a trace, use refined segment-based
                // distance instead of raw AABB distance.
                let trace_a = trace_map.get(&entry.entity.index());
                let trace_b = trace_map.get(&candidate.entity.index());

                //
                // The location travels with the distance. Reporting the gap the
                // checker actually measured - rather than a centroid of the two
                // entities - is what makes the coordinate usable: a long GND
                // trace and a pad have a centroid nowhere near the short.
                // Copper of the other side, when that side is a component: its
                // pads on the layers this pair shares. An empty result means
                // the part has no copper the other one can reach, and the pair
                // is not this rule's business.
                // The other side's copper, when that side is a component: the
                // pads it has on the layers this pair shares, minus any pad
                // that carries `exempt` - the net the other side is on, which
                // that pad is meant to be connected to.
                let copper_of =
                    |idx: u32, mask: u32, exempt: Option<&NetId>| -> Option<Vec<&PadBox>> {
                        let pads = pad_map.get(&idx)?;
                        Some(
                            pads.iter()
                                .filter(|pad| pad.layer_mask & mask != 0)
                                .filter(|pad| match (pad.net, exempt) {
                                    (Some(pad_net), Some(other)) => pad_net != *other,
                                    _ => true,
                                })
                                .collect(),
                        )
                    };

                let side_a = side_of(a_idx, net_a, trace_a.is_some(), entry.layer_mask);
                let side_b = side_of(b_idx, net_b, trace_b.is_some(), candidate.layer_mask);
                // How far apart two contacts of these two sides can be and
                // still be one place: the width of the trace copper that makes
                // them. See `one_row_per_place`.
                let reach =
                    trace_a.map_or(0, |t| t.half_width) + trace_b.map_or(0, |t| t.half_width);

                // The pair's requirement is the strictest thing either side
                // asked for, never below the fab floor.
                //
                // A trace or via names one net. A component names several
                // through its pins, and the spatial index boxes the whole
                // component, so the strictest of its nets applies to all of it.
                // That over-reports for a part with one high-voltage pin among
                // many - and over-reporting a rule the design stated is the
                // right way to be wrong, where staying silent is not.
                let stated = |net: Option<&NetId>, connections: Option<&Vec<NetId>>| -> Nm {
                    let single = net.and_then(|n| net_clearance.get(&n.id())).copied();
                    let many = connections
                        .into_iter()
                        .flatten()
                        .filter_map(|n| net_clearance.get(&n.id()).copied());
                    single
                        .into_iter()
                        .chain(many)
                        .fold(Nm(0), |acc, s| acc.max(s))
                };
                let required = min_clearance
                    .max(stated(net_a, nc_a))
                    .max(stated(net_b, nc_b));

                let mut no_copper_in_reach = false;
                let contacts: Vec<Measured> = match (trace_a, trace_b) {
                    // Both are traces: segment-to-segment distance minus both
                    // half-widths, measured from both sides.
                    //
                    // One side is not enough, and the asymmetry was visible as
                    // two commands disagreeing about the same board. On
                    // examples/blink.cypcb the router lays a GND run straight
                    // up x=22.479 and VCC crosses it twice - once with a
                    // horizontal segment at y=21.971 and once with a diagonal
                    // at y=24.765. Measuring per segment of GND gives that one
                    // segment one closest approach and so reports one short;
                    // measuring per segment of VCC gives two, because they are
                    // two segments. Two real shorts, and which number came out
                    // depended on which entity the spatial index happened to
                    // hand over first - so the same board scored 4 in memory
                    // and 5 after being written to a file and read back.
                    //
                    // Measured segment against segment, every pair of them,
                    // because that is the one answer that does not depend on
                    // how the copper is grouped into entities. Measuring a
                    // segment against the other trace as a whole finds one
                    // closest point per piece the other trace is cut into, so
                    // the same net cut in two pieces gained a contact. Which
                    // of these points are one place is decided once, over the
                    // whole board, in `one_row_per_place`.
                    (Some(ta), Some(tb)) => {
                        let (first, second) = if side_a <= side_b { (ta, tb) } else { (tb, ta) };
                        let limit = required.0 + ta.half_width + tb.half_width;
                        segment_pairs(first, second)
                            .into_iter()
                            .map(|(s, t, at, seg_dist)| {
                                let pieces = if seg_dist < limit {
                                    [
                                        stretch(s, limit, |p| {
                                            point_to_segment_distance(p, t.0, t.1)
                                        }),
                                        stretch(t, limit, |p| {
                                            point_to_segment_distance(p, s.0, s.1)
                                        }),
                                    ]
                                } else {
                                    [(s.0, s.0); 2]
                                };
                                (
                                    at,
                                    (seg_dist - ta.half_width - tb.half_width).max(0),
                                    pieces,
                                    None,
                                )
                            })
                            .collect()
                    }
                    // One is a trace, the other is a via or a component
                    (Some(t), None) => {
                        let measured = match copper_of(b_idx, entry.layer_mask, net_a) {
                            Some(pads) if pads.is_empty() => {
                                no_copper_in_reach = true;
                                Vec::new()
                            }
                            Some(pads) => per_segment_to_copper(
                                t,
                                &copper_of_pads(&pads),
                                required.0 + t.half_width,
                            ),
                            None => per_segment_to_copper(
                                t,
                                &[&shape_of(b_idx, &candidate.envelope)],
                                required.0 + t.half_width,
                            ),
                        };
                        measured
                            .into_iter()
                            .map(|(at, seg_dist, pieces, pad)| {
                                (at, (seg_dist - t.half_width).max(0), pieces, pad)
                            })
                            .collect()
                    }
                    (None, Some(t)) => {
                        let measured = match copper_of(a_idx, candidate.layer_mask, net_b) {
                            Some(pads) if pads.is_empty() => {
                                no_copper_in_reach = true;
                                Vec::new()
                            }
                            Some(pads) => per_segment_to_copper(
                                t,
                                &copper_of_pads(&pads),
                                required.0 + t.half_width,
                            ),
                            None => per_segment_to_copper(
                                t,
                                &[&shape_of(a_idx, &entry.envelope)],
                                required.0 + t.half_width,
                            ),
                        };
                        measured
                            .into_iter()
                            .map(|(at, seg_dist, pieces, pad)| {
                                (at, (seg_dist - t.half_width).max(0), pieces, pad)
                            })
                            .collect()
                    }
                    // Neither is a trace: vias and pads. A component stands for
                    // its pads, a via for its disc, anything else for its own
                    // box. A via leaves out the pads on its own net, as a trace
                    // does: a via dropped onto its own pin is a join, and
                    // measured against that pin it read as a short.
                    (None, None) => vec![at_a_point({
                        let a_boxes = copper_of(a_idx, candidate.layer_mask, net_b);
                        let b_boxes = copper_of(b_idx, entry.layer_mask, net_a);
                        if a_boxes.as_ref().is_some_and(|p| p.is_empty())
                            || b_boxes.as_ref().is_some_and(|p| p.is_empty())
                        {
                            no_copper_in_reach = true;
                            (Point::ORIGIN, i64::MAX)
                        } else {
                            match (a_boxes, b_boxes) {
                                // Two components: pad against pad, skipping the
                                // pairs that are meant to touch - two pads of
                                // the same net, which is a deliberate join.
                                (Some(a_pads), Some(b_pads)) => {
                                    let pairs = nearest_pad_pair(&a_pads, &b_pads);
                                    match pairs {
                                        Some(found) => found,
                                        None => {
                                            no_copper_in_reach = true;
                                            (Point::ORIGIN, i64::MAX)
                                        }
                                    }
                                }
                                (Some(a_pads), None) => nearest_pair(
                                    &copper_of_pads(&a_pads),
                                    &[&shape_of(b_idx, &candidate.envelope)],
                                ),
                                (None, Some(b_pads)) => nearest_pair(
                                    &[&shape_of(a_idx, &entry.envelope)],
                                    &copper_of_pads(&b_pads),
                                ),
                                (None, None) => nearest_pair(
                                    &[&shape_of(a_idx, &entry.envelope)],
                                    &[&shape_of(b_idx, &candidate.envelope)],
                                ),
                            }
                        }
                    })],
                };

                if no_copper_in_reach {
                    continue;
                }

                // Held the same way round however the loop reached the pair:
                // the lower side first. The outer loop walks the spatial index,
                // whose order is not guaranteed run to run, and a pair named in
                // a different order is a different line of the report -
                // measured on stm32_breakout, 308 violations both runs and 152
                // printed lines different before pairs were ordered.
                for (at, distance, stretch, pad) in contacts {
                    if distance >= required.0 {
                        continue;
                    }
                    let (side_a, side_b) = (side_a.at_pad(pad), side_b.at_pad(pad));
                    let (sides, entities) = if side_a <= side_b {
                        ((side_a, side_b), (entry.entity, candidate.entity))
                    } else {
                        ((side_b, side_a), (candidate.entity, entry.entity))
                    };
                    found.push(Contact {
                        sides,
                        entities,
                        at,
                        distance,
                        required,
                        reach,
                        stretch,
                    });
                }
            }
        }

        violations.extend(one_row_per_place(found));
        violations
    }
}

/// One side of a contact: which copper it is, as far as counting goes.
///
/// A trace is its net on its layers. The file decides how a net's copper is cut
/// into trace entities - the router holds one per net and layer, the reader one
/// per `path` line, a KiCad board one per net and layer again - and none of
/// those cuts is copper. A via is one piece of copper however the board was
/// written, so it stands for itself, and so does a pad: a trace measured
/// against a part is measured against each of its pads, and each pad is on a
/// net of its own. Held as the part, a trace running too close along a row of
/// a QFP's pads was one place against five nets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Side {
    Trace {
        net: u32,
        layers: u32,
    },
    Piece(u32),
    /// One pad of a part, or the disc of a via, known by the part and the
    /// lower corner of the copper's core.
    Pad {
        part: u32,
        at: [i64; 2],
    },
}

impl Side {
    /// This side narrowed to the one piece of its copper a contact was
    /// measured against, when there is one and this side is not a trace.
    fn at_pad(self, pad: Option<[i64; 2]>) -> Side {
        match (self, pad) {
            (Side::Piece(part), Some(at)) => Side::Pad { part, at },
            _ => self,
        }
    }
}

fn side_of(index: u32, net: Option<&NetId>, is_trace: bool, layers: u32) -> Side {
    match (is_trace, net) {
        (true, Some(net)) => Side::Trace {
            net: net.id(),
            layers,
        },
        _ => Side::Piece(index),
    }
}

/// A point where two sides come closer than their rule allows.
struct Contact {
    sides: (Side, Side),
    /// The entities that met there, the same way round as `sides`. They name
    /// the row; which of a net's trace entities it is does not change the name.
    entities: (bevy_ecs::entity::Entity, bevy_ecs::entity::Entity),
    at: Point,
    distance: i64,
    required: Nm,
    reach: i64,
    /// The copper that is too close: on each side that is a trace, the part
    /// of the segment nearer the other side than the rule allows. A via or a
    /// pad against another contributes the point it was measured at.
    stretch: [Stretch; 2],
}

/// A piece of a centreline, from one end to the other.
type Stretch = ([i64; 2], [i64; 2]);

/// One contact as measured, before it is known to be a violation: where, how
/// far, the stretch of copper that is too close, and the pad it was measured
/// against when the other side is a part.
type Measured = (Point, i64, [Stretch; 2], Option<[i64; 2]>);

/// A contact measured between two pieces that are not traces, which has a
/// point and nothing along it.
fn at_a_point((at, distance): (Point, i64)) -> Measured {
    let p = [at.x.0, at.y.0];
    (at, distance, [(p, p); 2], None)
}

/// The part of `segment` where `distance` stays under `limit`.
///
/// Distance to a convex piece of copper - a pad, a disc, another segment -
/// taken along a straight line falls and then rises, so the part under any
/// limit is one unbroken stretch. It is found by walking to the lowest point
/// and bisecting out to each end; a segment that never gets under the limit
/// gives the lowest point alone.
fn stretch(segment: Stretch, limit: i64, distance: impl Fn([i64; 2]) -> i64) -> Stretch {
    let (a, b) = segment;
    let at = |u: f64| {
        [
            (a[0] as f64 + u * (b[0] - a[0]) as f64).round() as i64,
            (a[1] as f64 + u * (b[1] - a[1]) as f64).round() as i64,
        ]
    };
    let f = |u: f64| distance(at(u));
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for _ in 0..64 {
        let (m1, m2) = (lo + (hi - lo) / 3.0, hi - (hi - lo) / 3.0);
        if f(m1) <= f(m2) {
            hi = m2;
        } else {
            lo = m1;
        }
    }
    let best = (lo + hi) / 2.0;
    if f(best) >= limit {
        let p = at(best);
        return (p, p);
    }
    let edge = |mut inside: f64, mut outside: f64| {
        if f(outside) < limit {
            return outside;
        }
        for _ in 0..64 {
            let middle = (inside + outside) / 2.0;
            if f(middle) < limit {
                inside = middle;
            } else {
                outside = middle;
            }
        }
        inside
    };
    (at(edge(best, 0.0)), at(edge(best, 1.0)))
}

/// Every contact the board has, one row per place.
///
/// **One place:** a contact is the stretch of copper that is too close - the
/// part of a trace segment nearer the other side than the rule allows, or the
/// point where two pads or vias were measured. Two contacts between the same
/// two sides are one place when those stretches come no further apart than
/// the trace copper that makes them is wide: the half-widths of the traces on
/// either side added together, zero where neither side is a trace. Taken
/// transitively, so a trace that runs too close along a pad's edge across three
/// segments is one place, and a trace that crosses another net twice is two.
///
/// Stretches rather than points, because a point is where one segment
/// happened to be measured from. A run along a pad edge cut by a vertex in the
/// middle of the straight gave two closest points 0.34mm apart on
/// `esp32_starter` - `IO6` against U1 - where the copper is one unbroken run.
///
/// A place is reported at its worst contact, at the lowest point when two are
/// equally bad. Nothing here reads an entity, so the rows are the same for any
/// cut of the same copper into entities.
fn one_row_per_place(mut found: Vec<Contact>) -> Vec<DrcViolation> {
    found.sort_by_key(|c| (c.sides, c.at.x.0, c.at.y.0, c.distance));

    let mut rows = Vec::new();
    let mut start = 0;
    while start < found.len() {
        let mut end = start;
        while end < found.len() && found[end].sides == found[start].sides {
            end += 1;
        }
        let group = &found[start..end];

        // Union-find over the group.
        let mut parent: Vec<usize> = (0..group.len()).collect();
        fn root(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        for i in 0..group.len() {
            for j in (i + 1)..group.len() {
                let reach = group[i].reach.max(group[j].reach);
                let apart = group[i]
                    .stretch
                    .iter()
                    .flat_map(|p| {
                        group[j]
                            .stretch
                            .iter()
                            .map(move |q| segment_distance(p.0, p.1, q.0, q.1))
                    })
                    .min()
                    .unwrap_or(i64::MAX);
                if apart <= reach {
                    let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                    parent[ri.max(rj)] = ri.min(rj);
                }
            }
        }

        let mut worst: HashMap<usize, usize> = HashMap::new();
        for i in 0..group.len() {
            let r = root(&mut parent, i);
            let held = worst.entry(r).or_insert(i);
            let (h, c) = (&group[*held], &group[i]);
            if (c.distance, c.at.x.0, c.at.y.0) < (h.distance, h.at.x.0, h.at.y.0) {
                *held = i;
            }
        }
        let mut places: Vec<&Contact> = worst.values().map(|&i| &group[i]).collect();
        places.sort_by_key(|c| (c.at.x.0, c.at.y.0));
        for c in places {
            rows.push(DrcViolation::clearance(
                c.entities.0,
                c.entities.1,
                Nm(c.distance),
                c.required,
                c.at,
            ));
        }
        start = end;
    }
    rows
}

/// One pad's copper, in board coordinates, with the layers and the net it is
/// on.
///
/// The net is per pad, not per component. A part with one GND pin does not
/// make its other pads GND, and treating it that way exempts a trace from
/// copper it can genuinely short.
pub(crate) struct PadBox {
    /// The copper itself: a circle, an oblong or a rounded rectangle is its
    /// core grown by a radius, a rectangle is its box.
    pub(crate) copper: Copper,
    pub(crate) layer_mask: u32,
    pub(crate) net: Option<NetId>,
}

/// Every component's pad copper, keyed by entity index.
///
/// Pads are placed the way the exporter and the renderer place them: the pad
/// offset is rotated around the component origin and added to its position. A
/// pad rotated off the axes has its core boxed by the extent of the rotated
/// core, which is never smaller than the copper - a checker may over-report,
/// and may not under-report.
pub(crate) fn component_pads(world: &mut BoardWorld) -> HashMap<u32, Vec<PadBox>> {
    use cypcb_world::components::{FootprintRef, Position, Rotation};

    // Which net each pin is on, per component. `PadDef::number` and
    // `PinConnection::pin` are the same identifier seen from the footprint and
    // from the schematic.
    let pin_nets: HashMap<u32, HashMap<String, NetId>> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(bevy_ecs::entity::Entity, &NetConnections)>();
        query
            .iter(ecs)
            .map(|(entity, connections)| {
                let by_pin = connections
                    .iter()
                    .map(|pin| (pin.pin.clone(), pin.net))
                    .collect();
                (entity.index(), by_pin)
            })
            .collect()
    };

    let placements: Vec<(u32, Point, f64, String)> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(
            bevy_ecs::entity::Entity,
            &Position,
            &Rotation,
            &FootprintRef,
        )>();
        query
            .iter(ecs)
            .map(|(entity, position, rotation, footprint)| {
                (
                    entity.index(),
                    position.0,
                    rotation.to_degrees(),
                    footprint.as_str().to_string(),
                )
            })
            .collect()
    };

    let library = world.footprints();
    let mut out = HashMap::new();

    for (index, position, degrees, name) in placements {
        let Some(footprint) = library.get(&name) else {
            // No footprint means no pad geometry to measure. Leaving the entry
            // out keeps the courtyard fallback, which over-reports rather than
            // silently passing a part nobody can see.
            continue;
        };

        let boxes = footprint
            .pads
            .iter()
            .map(|pad| {
                let copper = pad_copper(pad, position, degrees);

                let layer_mask = pad.copper_mask();

                PadBox {
                    copper,
                    layer_mask,
                    net: pin_nets
                        .get(&index)
                        .and_then(|by_pin| by_pin.get(&pad.number))
                        .copied(),
                }
            })
            .collect();

        out.insert(index, boxes);
    }

    out
}

/// The copper of one pad of a part placed at `position`, turned `degrees`.
///
/// Every shape is a rectangle grown by a radius, and the copper says so:
/// a circle is its centre grown by half its width, which is what
/// `aperture_for_pad` sends to the fab; an oblong is the segment between the
/// centres of its two ends, grown by half its short side; a `roundrect` is its
/// rectangle shrunk by the corner radius on every side and grown back by it,
/// the radius being the short side times `corner_ratio`, as the Gerber and SVG
/// writers draw it. A pad turned off the axes keeps the box of its turned
/// core, which is never smaller than the copper.
pub(crate) fn pad_copper(
    pad: &cypcb_world::footprint::PadDef,
    position: Point,
    degrees: f64,
) -> Copper {
    use cypcb_world::components::PadShape;

    let (sin, cos) = degrees.to_radians().sin_cos();
    let px = pad.position.x.0 as f64;
    let py = pad.position.y.0 as f64;
    let cx = position.x.0 + (px * cos - py * sin).round() as i64;
    let cy = position.y.0 + (px * sin + py * cos).round() as i64;

    let extent = |half_w: f64, half_h: f64| {
        (
            (half_w * cos.abs() + half_h * sin.abs()).round() as i64,
            (half_w * sin.abs() + half_h * cos.abs()).round() as i64,
        )
    };
    let (width, height) = (pad.size.0 .0, pad.size.1 .0);
    let radius = match pad.shape {
        PadShape::Circle => return Copper::circle([cx, cy], width / 2),
        PadShape::Rect => 0,
        PadShape::Oblong => width.min(height) / 2,
        PadShape::RoundRect { corner_ratio } => width.min(height) * i64::from(corner_ratio) / 100,
    };
    let (core_x, core_y) = extent(
        (width - 2 * radius) as f64 / 2.0,
        (height - 2 * radius) as f64 / 2.0,
    );
    let core = AABB::from_corners([cx - core_x, cy - core_y], [cx + core_x, cy + core_y]);
    Copper { core, radius }
}

/// The copper out of a list of pads, once the net filtering is done.
fn copper_of_pads<'a>(pads: &[&'a PadBox]) -> Vec<&'a Copper> {
    pads.iter().map(|pad| &pad.copper).collect()
}

/// Copper as the checker measures it: a box grown by a radius.
///
/// A rectangle is its box and no radius. A disc - a via, a circular pad - is
/// its centre, a box of no size, grown by its radius, and then the distance
/// between two of them is the distance between their cores less both radii,
/// which is exact. Measured as the square around it, a disc is closer to a
/// diagonal neighbour than it is by `r * (sqrt(2) - 1)`, and on a 0.6mm via
/// that is 0.124mm - near a whole 0.127mm clearance.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Copper {
    pub(crate) core: AABB<[i64; 2]>,
    pub(crate) radius: i64,
}

impl Copper {
    pub(crate) fn boxed(box_: AABB<[i64; 2]>) -> Self {
        Copper {
            core: box_,
            radius: 0,
        }
    }

    pub(crate) fn circle(centre: [i64; 2], radius: i64) -> Self {
        Copper {
            core: AABB::from_point(centre),
            radius,
        }
    }

    /// The box the copper fits in.
    pub(crate) fn bounds(&self) -> AABB<[i64; 2]> {
        let (lo, hi) = (self.core.lower(), self.core.upper());
        AABB::from_corners(
            [lo[0] - self.radius, lo[1] - self.radius],
            [hi[0] + self.radius, hi[1] + self.radius],
        )
    }
}

/// The gap between two pieces of copper; 0 where they touch or overlap.
pub(crate) fn copper_distance(a: &Copper, b: &Copper) -> i64 {
    (aabb_distance(&a.core, &b.core) - a.radius - b.radius).max(0)
}

/// Closest approach from trace centrelines to a piece of copper.
pub(crate) fn trace_to_copper_distance(trace: &TraceData, copper: &Copper) -> (Point, i64) {
    let (at, distance) = trace_to_aabb_distance(trace, &copper.core);
    (at, distance.saturating_sub(copper.radius).max(0))
}

/// One piece of the copper an index entry stands for, as the rules that
/// measure copper against something other than copper see it: the board edge,
/// a slot, a bare hole.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Piece {
    /// A box grown by a radius: a pad, a via's disc, a pour.
    Area(Copper),
    /// A centreline grown by a radius: one segment of a trace.
    Stroke {
        from: [i64; 2],
        to: [i64; 2],
        radius: i64,
    },
}

impl Piece {
    /// Where a report about this piece points: the middle of its copper.
    pub(crate) fn centre(&self) -> Point {
        match self {
            Piece::Area(copper) => aabb_center(&copper.core),
            Piece::Stroke { from, to, .. } => midpoint_raw(*from, *to),
        }
    }
}

/// The copper behind each entry of the spatial index, for the rules that walk
/// the index without measuring copper against copper.
///
/// The index holds boxes. A trace segment sits there as the box around it,
/// grown by half its width, and a via as the square around its disc - and a
/// diagonal segment's box has corners the copper never reaches. Measured as
/// that box, a diagonal trace running beside the board edge, a slot or a
/// mounting hole reads closer than it is. `ClearanceRule` measures a trace by
/// its centreline and a via by its disc; so does everything that asks this.
///
/// A pour stays the box: `Zone` holds nothing but its bounds, so the box is
/// the whole of what the model knows about its copper.
pub(crate) struct EntryCopper {
    pads: HashMap<u32, Vec<PadBox>>,
    vias: HashMap<u32, Copper>,
    traces: HashMap<u32, TraceData>,
}

impl EntryCopper {
    pub(crate) fn collect(world: &mut BoardWorld) -> Self {
        let pads = component_pads(world);
        let ecs = world.ecs_mut();
        let vias = ecs
            .query::<(bevy_ecs::entity::Entity, &Via)>()
            .iter(ecs)
            .map(|(e, via)| {
                (
                    e.index(),
                    Copper::circle(
                        [via.position.x.0, via.position.y.0],
                        via.outer_diameter.0 / 2,
                    ),
                )
            })
            .collect();
        let traces = ecs
            .query::<(bevy_ecs::entity::Entity, &Trace)>()
            .iter(ecs)
            .map(|(e, t)| {
                let segments = t
                    .segments
                    .iter()
                    .map(|s| ([s.start.x.0, s.start.y.0], [s.end.x.0, s.end.y.0]))
                    .collect();
                (
                    e.index(),
                    TraceData {
                        half_width: t.width.0 / 2,
                        segments,
                    },
                )
            })
            .collect();
        EntryCopper { pads, vias, traces }
    }

    /// A component's pads, when the entity is one.
    pub(crate) fn pads(&self, entity: bevy_ecs::entity::Entity) -> Option<&[PadBox]> {
        self.pads.get(&entity.index()).map(Vec::as_slice)
    }

    /// The copper `entry` stands for: a component's pads where it has them, a
    /// via's disc, the trace segment the entry was indexed for, and the
    /// entry's own box for anything else.
    pub(crate) fn pieces(&self, entry: &cypcb_world::SpatialEntry) -> Vec<Piece> {
        let index = entry.entity.index();
        if let Some(pads) = self.pads.get(&index).filter(|pads| !pads.is_empty()) {
            return pads.iter().map(|pad| Piece::Area(pad.copper)).collect();
        }
        if let Some(via) = self.vias.get(&index) {
            return vec![Piece::Area(*via)];
        }
        if let Some(trace) = self.traces.get(&index) {
            // One entry per segment, each the segment's box grown by half the
            // width, which is how the index is built; the segments whose grown
            // box this is are the copper this entry stands for.
            let radius = trace.half_width;
            let own: Vec<Piece> = trace
                .segments
                .iter()
                .filter(|(from, to)| {
                    let grown = AABB::from_corners(
                        [from[0].min(to[0]) - radius, from[1].min(to[1]) - radius],
                        [from[0].max(to[0]) + radius, from[1].max(to[1]) + radius],
                    );
                    grown == entry.envelope
                })
                .map(|(from, to)| Piece::Stroke {
                    from: *from,
                    to: *to,
                    radius,
                })
                .collect();
            if !own.is_empty() {
                return own;
            }
        }
        vec![Piece::Area(Copper::boxed(entry.envelope))]
    }
}

/// Closest approach between two components' pads, ignoring pad pairs that
/// share a net.
///
/// `None` when every pair shares one, which means the two parts have no
/// copper that could short.
fn nearest_pad_pair(a: &[&PadBox], b: &[&PadBox]) -> Option<(Point, i64)> {
    a.iter()
        .flat_map(|pad_a| b.iter().map(move |pad_b| (pad_a, pad_b)))
        .filter(|(pad_a, pad_b)| match (pad_a.net, pad_b.net) {
            (Some(net_a), Some(net_b)) => net_a != net_b,
            _ => true,
        })
        .map(|(pad_a, pad_b)| {
            (
                midpoint(
                    aabb_center(&pad_a.copper.core),
                    aabb_center(&pad_b.copper.core),
                ),
                copper_distance(&pad_a.copper, &pad_b.copper),
            )
        })
        .min_by_key(|(_, distance)| *distance)
}

/// Closest approach between two sets of copper, and where it happens.
fn nearest_pair(a: &[&Copper], b: &[&Copper]) -> (Point, i64) {
    a.iter()
        .flat_map(|copper_a| {
            b.iter().map(move |copper_b| {
                (
                    midpoint(aabb_center(&copper_a.core), aabb_center(&copper_b.core)),
                    copper_distance(copper_a, copper_b),
                )
            })
        })
        .min_by_key(|(_, distance)| *distance)
        .unwrap_or((Point::ORIGIN, i64::MAX))
}

/// Create a canonical pair ordering to avoid duplicate checks.
///
/// Always returns (smaller, larger) to ensure A-B and B-A map to the same key.
#[inline]
fn canonical_pair(a: u32, b: u32) -> (u32, u32) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Calculate the minimum distance between two axis-aligned bounding boxes.
///
/// Returns 0 if the AABBs touch or overlap.
/// Uses i128 intermediates to prevent overflow during distance calculation.
/// The gap between two boxes, which is the gap between two sharp corners when
/// the pair is diagonal.
///
/// Rounded copper is not measured here. A `roundrect`, an oblong, a circle
/// and a via are each a `Copper`, a core box grown by a radius, and
/// `copper_distance` takes the radii off the gap between the cores.
pub(crate) fn aabb_distance(a: &AABB<[i64; 2]>, b: &AABB<[i64; 2]>) -> i64 {
    // Calculate gap in each dimension
    // If boxes overlap in a dimension, the gap is 0
    let dx = (a.lower()[0].max(b.lower()[0]) - a.upper()[0].min(b.upper()[0])).max(0);
    let dy = (a.lower()[1].max(b.lower()[1]) - a.upper()[1].min(b.upper()[1])).max(0);

    // Euclidean distance using i128 to prevent overflow
    let dx_sq = (dx as i128) * (dx as i128);
    let dy_sq = (dy as i128) * (dy as i128);
    ((dx_sq + dy_sq) as f64).sqrt() as i64
}

/// Calculate the center point of an AABB.
/// Point halfway between two points.
fn midpoint(a: Point, b: Point) -> Point {
    Point::new(Nm((a.x.0 + b.x.0) / 2), Nm((a.y.0 + b.y.0) / 2))
}

fn aabb_center(aabb: &AABB<[i64; 2]>) -> Point {
    Point::new(
        Nm((aabb.lower()[0] + aabb.upper()[0]) / 2),
        Nm((aabb.lower()[1] + aabb.upper()[1]) / 2),
    )
}

// ============================================================================
// Trace-aware distance calculations
// ============================================================================

/// Pre-collected trace data for clearance checking.
pub(crate) struct TraceData {
    /// Half of the trace width in nanometers.
    pub(crate) half_width: i64,
    /// Segments as ([start_x, start_y], [end_x, end_y]).
    pub(crate) segments: Vec<([i64; 2], [i64; 2])>,
}

/// Minimum distance between two line segments.
///
/// Computes the exact minimum Euclidean distance between segment
/// (p1→p2) and segment (p3→p4). Handles parallel, perpendicular,
/// and endpoint-dominated cases correctly.
///
/// Uses i128 intermediates to prevent overflow with nanometer coordinates.
///
/// Algorithm derived from minimizing |P1 + s·D1 - P3 - t·D2|² subject
/// to s,t ∈ [0,1]. The unconstrained critical point is found first, then
/// clamped with recomputation to handle boundary cases.
pub fn segment_distance(p1: [i64; 2], p2: [i64; 2], p3: [i64; 2], p4: [i64; 2]) -> i64 {
    segment_closest(p1, p2, p3, p4).1
}

/// Closest point between two segments, and the distance across the gap.
///
/// The point is the midpoint of the two closest points, one on each segment -
/// the middle of the gap the checker is complaining about. A violation reported
/// anywhere else sends click-to-zoom to the wrong part of the board and gives
/// anything that consumes the report a coordinate it cannot act on.
pub fn segment_closest(p1: [i64; 2], p2: [i64; 2], p3: [i64; 2], p4: [i64; 2]) -> (Point, i64) {
    // Direction vectors
    let d1 = [p2[0] - p1[0], p2[1] - p1[1]];
    let d2 = [p4[0] - p3[0], p4[1] - p3[1]];
    // Vector from p1→p3
    let r = [p3[0] - p1[0], p3[1] - p1[1]];

    let a = dot128(d1, d1); // |d1|²
    let e = dot128(d2, d2); // |d2|²

    // Both segments degenerate to points
    if a == 0 && e == 0 {
        return (midpoint_raw(p1, p3), point_distance(p1, p3));
    }

    let c = dot128(d1, r); // D1 · r
    let f = dot128(d2, r); // D2 · r

    let mut s: f64;
    let mut t: f64;

    if a == 0 {
        // First segment degenerates to a point.
        //
        // Minimising |P1 - P3 - t·D2|² over t gives t = -(D2·r)/e, with
        // r = P3 - P1. This read `t = f/e` and dropped the sign, which walks
        // the closest point to the wrong end of the segment. The general
        // branch below computes `t = (b·s - f)/e`, which is -f/e at s = 0, so
        // the two disagreed. Nothing exercised it until a rule started asking
        // for point-to-segment distances - `point_to_segment_distance` was
        // marked dead code - and the silkscreen rule was under-reporting
        // because of it.
        s = 0.0;
        t = (-(f as f64) / e as f64).clamp(0.0, 1.0);
    } else if e == 0 {
        // Second segment degenerates to a point
        // Minimize |P1 + s·D1 - P3|²: s = c/a
        t = 0.0;
        s = (c as f64 / a as f64).clamp(0.0, 1.0);
    } else {
        // General case: two proper segments
        let b = dot128(d1, d2);
        let denom = a * e - b * b; // ≥ 0 by Cauchy-Schwarz

        // Unconstrained s from the linear system:
        //   a·s - b·t = c
        //  -b·s + e·t = -f
        // → s = (c·e - b·f) / denom
        if denom != 0 {
            s = ((c * e - b * f) as f64 / denom as f64).clamp(0.0, 1.0);
        } else {
            // Parallel segments — pick s=0, solve for t
            s = 0.0;
        }

        // Compute t from s: t = (b·s - f) / e
        t = (b as f64 * s - f as f64) / e as f64;

        // Clamp t and recompute s if needed
        if t < 0.0 {
            t = 0.0;
            // From a·s = c: s = c/a
            s = (c as f64 / a as f64).clamp(0.0, 1.0);
        } else if t > 1.0 {
            t = 1.0;
            // From a·s - b = c: s = (c + b)/a
            s = ((c + b) as f64 / a as f64).clamp(0.0, 1.0);
        }
    }

    // Closest points on each segment
    let closest1 = [
        p1[0] as f64 + s * d1[0] as f64,
        p1[1] as f64 + s * d1[1] as f64,
    ];
    let closest2 = [
        p3[0] as f64 + t * d2[0] as f64,
        p3[1] as f64 + t * d2[1] as f64,
    ];

    let dx = closest1[0] - closest2[0];
    let dy = closest1[1] - closest2[1];
    let location = Point::new(
        Nm(((closest1[0] + closest2[0]) / 2.0).round() as i64),
        Nm(((closest1[1] + closest2[1]) / 2.0).round() as i64),
    );
    (location, (dx * dx + dy * dy).sqrt() as i64)
}

/// Midpoint of two raw coordinate pairs.
#[inline]
fn midpoint_raw(a: [i64; 2], b: [i64; 2]) -> Point {
    Point::new(Nm((a[0] + b[0]) / 2), Nm((a[1] + b[1]) / 2))
}

/// Dot product using i128 to prevent overflow.
#[inline]
fn dot128(a: [i64; 2], b: [i64; 2]) -> i128 {
    (a[0] as i128) * (b[0] as i128) + (a[1] as i128) * (b[1] as i128)
}

/// Euclidean distance between two points.
#[inline]
fn point_distance(a: [i64; 2], b: [i64; 2]) -> i64 {
    let dx = (b[0] - a[0]) as i128;
    let dy = (b[1] - a[1]) as i128;
    ((dx * dx + dy * dy) as f64).sqrt() as i64
}

/// Minimum distance from a point to a line segment.
pub(crate) fn point_to_segment_distance(p: [i64; 2], s1: [i64; 2], s2: [i64; 2]) -> i64 {
    segment_distance(p, p, s1, s2)
}

/// The closest approach of every segment of `a` to every segment of `b`.
///
/// Per pair of segments, because a pair of segments is the one unit that is
/// the same however the copper is grouped into entities. The helper this
/// replaced measured each segment against the other trace as a whole and
/// claimed that made the count a property of the board; it did not. A segment
/// finds one closest point per entity the other net is cut into, so the same
/// board read back from its file, one entity per `path`, counted contacts the
/// router's one entity per net and layer did not: 46 shorts on
/// `esp32_starter` against 44, on the same segments.
///
/// `first` is the lower side. The pair is always measured that way round, and
/// each segment from its lower end, so a gap between two
/// parallel segments is reported at the same point whichever trace the loop
/// happened to hold and whichever way the file wrote the segment.
fn segment_pairs(first: &TraceData, second: &TraceData) -> Vec<(Stretch, Stretch, Point, i64)> {
    let mut out = Vec::with_capacity(first.segments.len() * second.segments.len());
    for s in &first.segments {
        let s = lower_end_first(s);
        for t in &second.segments {
            let t = lower_end_first(t);
            let (at, distance) = segment_closest(s.0, s.1, t.0, t.1);
            out.push((s, t, at, distance));
        }
    }
    out
}

/// A segment from its lower end, so a file that wrote it the other way round
/// measures the same point. Measured from its start, a segment lying along a
/// pad's edge reported the gap at whichever end it was written from:
/// `PA14` against U1 on `multi_ic` came out at one end of the pad's edge one
/// way and at the other end the other way.
fn lower_end_first(segment: &([i64; 2], [i64; 2])) -> ([i64; 2], [i64; 2]) {
    if segment.0 <= segment.1 {
        *segment
    } else {
        (segment.1, segment.0)
    }
}

/// Every segment of `trace` against every piece of copper it comes nearer than
/// `limit` to, with the stretch of the segment that is that near.
///
/// Every piece, not the nearest one. A segment running past two pads of a
/// connector too close to both is two gaps, and measured against its nearest
/// pad only it reported one - until a vertex in the middle of it split the
/// segment in two, and each half found its own pad: `CC1` against J1 on
/// `esp32_starter`, one row whole and two in halves.
fn per_segment_to_copper(trace: &TraceData, copper: &[&Copper], limit: i64) -> Vec<Measured> {
    let mut out = Vec::new();
    for seg in &trace.segments {
        let s = lower_end_first(seg);
        let one = TraceData {
            half_width: trace.half_width,
            segments: vec![s],
        };
        for piece in copper {
            let (at, distance) = trace_to_copper_distance(&one, piece);
            if distance >= limit {
                continue;
            }
            let run = stretch(s, limit, |p| copper_distance(&Copper::circle(p, 0), piece));
            out.push((at, distance, [run; 2], Some(piece.core.lower())));
        }
    }
    out
}

/// Minimum distance from trace centerlines to an AABB.
///
/// Computes the closest distance from any trace segment endpoint
/// or perpendicular projection to the AABB edges. For AABB-to-segment,
/// we test distance from each segment to each AABB edge segment.
pub(crate) fn trace_to_aabb_distance(trace: &TraceData, aabb: &AABB<[i64; 2]>) -> (Point, i64) {
    let lo = aabb.lower();
    let hi = aabb.upper();
    // AABB edge segments (4 sides)
    let edges: [([i64; 2], [i64; 2]); 4] = [
        ([lo[0], lo[1]], [hi[0], lo[1]]), // bottom
        ([hi[0], lo[1]], [hi[0], hi[1]]), // right
        ([hi[0], hi[1]], [lo[0], hi[1]]), // top
        ([lo[0], hi[1]], [lo[0], lo[1]]), // left
    ];
    let inside = |p: [i64; 2]| p[0] >= lo[0] && p[0] <= hi[0] && p[1] >= lo[1] && p[1] <= hi[1];

    let mut best = (Point::new(Nm(0), Nm(0)), i64::MAX);
    for seg in &trace.segments {
        // The centreline runs through the box: the overlap is the violation,
        // and the point that overlaps is where to report it.
        let mid = [(seg.0[0] + seg.1[0]) / 2, (seg.0[1] + seg.1[1]) / 2];
        if inside(mid) {
            return (midpoint_raw(mid, mid), 0);
        }
        if inside(seg.0) {
            return (midpoint_raw(seg.0, seg.0), 0);
        }
        if inside(seg.1) {
            return (midpoint_raw(seg.1, seg.1), 0);
        }

        for edge in &edges {
            let (at, distance) = segment_closest(seg.0, seg.1, edge.0, edge.1);
            if distance < best.1 {
                best = (at, distance);
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::*;
    use cypcb_core::{Nm, Point};
    use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
    use cypcb_world::components::NetId;
    use cypcb_world::Layer;
    use cypcb_world::SpatialEntry;

    use crate::ViolationKind;

    fn make_test_world_with_entries(entries: Vec<SpatialEntry>) -> BoardWorld {
        let mut world = BoardWorld::new();
        // Access the ECS world to directly populate the spatial index
        world
            .ecs_mut()
            .resource_mut::<cypcb_world::SpatialIndex>()
            .rebuild(entries);
        world
    }

    #[test]
    fn a_net_that_asks_for_more_clearance_gets_it() {
        // Two pads 0.2mm apart. The JLCPCB preset wants 0.127mm, so this board
        // is clean until a net says otherwise.
        let mut world = BoardWorld::new();
        let quiet = world.intern_net("SIG");
        let strict = world.intern_net("HV");

        let a = world.ecs_mut().spawn(quiet).id();
        let b = world.ecs_mut().spawn(strict).id();
        world
            .ecs_mut()
            .resource_mut::<cypcb_world::SpatialIndex>()
            .rebuild(vec![
                SpatialEntry::new(a, Point::from_mm(0.0, 0.0), Point::from_mm(1.0, 1.0), 0b01),
                SpatialEntry::new(b, Point::from_mm(1.2, 0.0), Point::from_mm(2.2, 1.0), 0b01),
            ]);

        let rules = DesignRules::jlcpcb_2layer();
        assert!(
            ClearanceRule.check(&mut world, &rules).is_empty(),
            "0.2mm clears the 0.127mm preset"
        );

        // The design states a rule the fab preset cannot know about.
        world.set_net_constraints(
            strict,
            cypcb_world::registry::NetConstraints {
                clearance: Some(Nm::from_mm(0.5)),
                ..Default::default()
            },
        );

        let violations = ClearanceRule.check(&mut world, &rules);
        assert_eq!(violations.len(), 1, "0.2mm does not clear a stated 0.5mm");
        assert!(
            violations[0].message.contains("0.50mm required"),
            "the reported requirement is the net's, not the preset's: {}",
            violations[0].message
        );
    }

    #[test]
    fn test_no_violation_when_far_apart() {
        // Two pads 10mm apart with 0.15mm clearance rule
        let entries = vec![
            SpatialEntry::new(
                Entity::from_raw(0),
                Point::from_mm(0.0, 0.0),
                Point::from_mm(1.0, 1.0),
                0b01,
            ),
            SpatialEntry::new(
                Entity::from_raw(1),
                Point::from_mm(10.0, 0.0),
                Point::from_mm(11.0, 1.0),
                0b01,
            ),
        ];

        let mut world = make_test_world_with_entries(entries);
        let rules = DesignRules::jlcpcb_2layer();
        let violations = ClearanceRule.check(&mut world, &rules);

        assert!(violations.is_empty(), "Should have no violations");
    }

    #[test]
    fn test_violation_when_too_close() {
        // Two pads 0.1mm apart with 0.15mm clearance rule
        let entries = vec![
            SpatialEntry::new(
                Entity::from_raw(0),
                Point::from_mm(0.0, 0.0),
                Point::from_mm(1.0, 1.0),
                0b01,
            ),
            SpatialEntry::new(
                Entity::from_raw(1),
                Point::from_mm(1.1, 0.0), // 0.1mm gap
                Point::from_mm(2.1, 1.0),
                0b01,
            ),
        ];

        let mut world = make_test_world_with_entries(entries);
        let rules = DesignRules::jlcpcb_2layer(); // 0.15mm clearance

        let violations = ClearanceRule.check(&mut world, &rules);

        assert_eq!(violations.len(), 1, "Should have one violation");
        assert_eq!(violations[0].kind, ViolationKind::Clearance);
    }

    #[test]
    fn point_to_segment_measures_the_perpendicular() {
        // A point beside the middle of a vertical segment. The answer is the
        // perpendicular distance, not the distance to whichever end the sign
        // of a dot product happened to pick.
        let point = [1_000_000i64, 5_000_000i64];
        let start = [0i64, 0i64];
        let end = [0i64, 10_000_000i64];

        assert_eq!(segment_distance(point, point, start, end), 1_000_000);

        // Beyond the end, the nearest point is the endpoint.
        let beyond = [0i64, 14_000_000i64];
        assert_eq!(segment_distance(beyond, beyond, start, end), 4_000_000);

        // Symmetric in the argument order.
        assert_eq!(
            segment_distance(start, end, point, point),
            segment_distance(point, point, start, end)
        );
    }

    #[test]
    fn contact_point_is_the_gap_not_a_centroid() {
        // A long trace running down the board, and a short one that comes
        // close to its far end only. The centroid of the two is near the
        // middle of the long trace, tens of millimetres from the actual
        // problem; the gap is at the far end.
        let long = [([0i64, 0i64], [0, 100_000_000])]; // 0 -> 100mm, vertical
        let short = [([100_000i64, 99_000_000i64], [5_000_000, 99_000_000])];

        let (at, distance) = segment_closest(long[0].0, long[0].1, short[0].0, short[0].1);

        assert_eq!(distance, 100_000, "0.1mm gap between the two");
        assert_eq!(
            at,
            Point::new(Nm(50_000), Nm(99_000_000)),
            "the contact sits in the middle of the gap, at the far end"
        );
        assert!(
            at.y.0 > 90_000_000,
            "a centroid would have landed near y=50mm"
        );
    }

    #[test]
    fn violation_is_reported_the_same_way_round_whatever_the_index_order() {
        // Same pair of pads, registered in both orders. The rule walks the
        // spatial index, whose order is not guaranteed run to run, so the two
        // must produce an identical violation - same names, same coordinate -
        // or the report is not reproducible.
        let a = SpatialEntry::new(
            Entity::from_raw(0),
            Point::from_mm(0.0, 0.0),
            Point::from_mm(1.0, 1.0),
            0b01,
        );
        let b = SpatialEntry::new(
            Entity::from_raw(1),
            Point::from_mm(1.1, 0.0), // 0.1mm gap, under the 0.15mm rule
            Point::from_mm(2.1, 1.0),
            0b01,
        );

        let rules = DesignRules::jlcpcb_2layer();
        let forward = ClearanceRule.check(
            &mut make_test_world_with_entries(vec![a.clone(), b.clone()]),
            &rules,
        );
        let reversed = ClearanceRule.check(&mut make_test_world_with_entries(vec![b, a]), &rules);

        assert_eq!(forward.len(), 1);
        assert_eq!(reversed.len(), 1);
        assert_eq!(forward[0].entity, reversed[0].entity);
        assert_eq!(forward[0].other_entity, reversed[0].other_entity);
        assert_eq!(forward[0].location, reversed[0].location);

        // Lower entity index first, and the location sits between the two.
        assert_eq!(forward[0].entity, Entity::from_raw(0));
        assert_eq!(forward[0].other_entity, Some(Entity::from_raw(1)));
        assert_eq!(forward[0].location, Point::from_mm(1.05, 0.5));
    }

    #[test]
    fn test_no_violation_different_layers() {
        // Two pads overlapping but on different layers
        let entries = vec![
            SpatialEntry::new(
                Entity::from_raw(0),
                Point::from_mm(0.0, 0.0),
                Point::from_mm(1.0, 1.0),
                0b01, // Top only
            ),
            SpatialEntry::new(
                Entity::from_raw(1),
                Point::from_mm(0.5, 0.5), // Overlapping position
                Point::from_mm(1.5, 1.5),
                0b10, // Bottom only
            ),
        ];

        let mut world = make_test_world_with_entries(entries);
        let rules = DesignRules::jlcpcb_2layer();

        let violations = ClearanceRule.check(&mut world, &rules);

        assert!(
            violations.is_empty(),
            "Different layers should not cause violation"
        );
    }

    #[test]
    fn test_no_duplicate_violations() {
        // Ensure A-B violation is not reported twice as B-A
        let entries = vec![
            SpatialEntry::new(
                Entity::from_raw(0),
                Point::from_mm(0.0, 0.0),
                Point::from_mm(1.0, 1.0),
                0b01,
            ),
            SpatialEntry::new(
                Entity::from_raw(1),
                Point::from_mm(1.05, 0.0), // Very close (0.05mm gap)
                Point::from_mm(2.05, 1.0),
                0b01,
            ),
        ];

        let mut world = make_test_world_with_entries(entries);
        let rules = DesignRules::jlcpcb_2layer();

        let violations = ClearanceRule.check(&mut world, &rules);

        assert_eq!(violations.len(), 1, "Should only report once");
    }

    #[test]
    fn test_aabb_distance_no_overlap() {
        let a = AABB::from_corners([0, 0], [100, 100]);
        let b = AABB::from_corners([200, 0], [300, 100]);

        let dist = aabb_distance(&a, &b);
        assert_eq!(dist, 100, "Distance should be 100");
    }

    #[test]
    fn test_aabb_distance_touching() {
        let a = AABB::from_corners([0, 0], [100, 100]);
        let b = AABB::from_corners([100, 0], [200, 100]);

        let dist = aabb_distance(&a, &b);
        assert_eq!(dist, 0, "Touching AABBs have zero distance");
    }

    #[test]
    fn test_aabb_distance_overlapping() {
        let a = AABB::from_corners([0, 0], [100, 100]);
        let b = AABB::from_corners([50, 50], [150, 150]);

        let dist = aabb_distance(&a, &b);
        assert_eq!(dist, 0, "Overlapping AABBs have zero distance");
    }

    #[test]
    fn test_aabb_distance_diagonal() {
        // Two AABBs separated diagonally
        let a = AABB::from_corners([0, 0], [100, 100]);
        let b = AABB::from_corners([200, 200], [300, 300]);

        let dist = aabb_distance(&a, &b);
        // Diagonal distance: sqrt(100^2 + 100^2) = sqrt(20000) = ~141
        let expected = ((100_i64 * 100 + 100 * 100) as f64).sqrt() as i64;
        assert_eq!(dist, expected, "Diagonal distance calculation");
    }

    #[test]
    fn test_canonical_pair_ordering() {
        assert_eq!(canonical_pair(1, 2), (1, 2));
        assert_eq!(canonical_pair(2, 1), (1, 2));
        assert_eq!(canonical_pair(5, 5), (5, 5));
    }

    #[test]
    fn test_aabb_center() {
        let aabb = AABB::from_corners([0, 0], [1000, 2000]);
        let center = aabb_center(&aabb);
        assert_eq!(center.x, Nm(500));
        assert_eq!(center.y, Nm(1000));
    }

    #[test]
    fn test_same_net_exemption() {
        // Two pads very close together but on the same net — should NOT violate
        let mut world = BoardWorld::new();
        let vcc = NetId::new(42);

        // Spawn real entities in the ECS with NetId components
        let e0 = world.ecs_mut().spawn(vcc).id();
        let e1 = world.ecs_mut().spawn(vcc).id();

        let entries = vec![
            SpatialEntry::new(e0, Point::from_mm(0.0, 0.0), Point::from_mm(1.0, 1.0), 0b01),
            SpatialEntry::new(
                e1,
                Point::from_mm(1.05, 0.0), // 0.05mm gap — would fail 0.15mm clearance
                Point::from_mm(2.05, 1.0),
                0b01,
            ),
        ];

        world
            .ecs_mut()
            .resource_mut::<cypcb_world::SpatialIndex>()
            .rebuild(entries);

        let rules = DesignRules::jlcpcb_2layer();
        let violations = ClearanceRule.check(&mut world, &rules);

        assert!(
            violations.is_empty(),
            "Same-net pads should be exempt from clearance check"
        );
    }

    #[test]
    fn test_different_net_still_violates() {
        // Two pads close together on DIFFERENT nets — should violate
        let mut world = BoardWorld::new();

        // Spawn real entities with different NetIds
        let e0 = world.ecs_mut().spawn(NetId::new(1)).id();
        let e1 = world.ecs_mut().spawn(NetId::new(2)).id();

        let entries = vec![
            SpatialEntry::new(e0, Point::from_mm(0.0, 0.0), Point::from_mm(1.0, 1.0), 0b01),
            SpatialEntry::new(
                e1,
                Point::from_mm(1.05, 0.0), // 0.05mm gap
                Point::from_mm(2.05, 1.0),
                0b01,
            ),
        ];

        world
            .ecs_mut()
            .resource_mut::<cypcb_world::SpatialIndex>()
            .rebuild(entries);

        let rules = DesignRules::jlcpcb_2layer();
        let violations = ClearanceRule.check(&mut world, &rules);

        assert_eq!(
            violations.len(),
            1,
            "Different-net pads should still violate"
        );
    }

    #[test]
    fn test_no_net_still_violates() {
        // Entities without NetId component — should still be checked (legacy behavior)
        let entries = vec![
            SpatialEntry::new(
                Entity::from_raw(0),
                Point::from_mm(0.0, 0.0),
                Point::from_mm(1.0, 1.0),
                0b01,
            ),
            SpatialEntry::new(
                Entity::from_raw(1),
                Point::from_mm(1.05, 0.0),
                Point::from_mm(2.05, 1.0),
                0b01,
            ),
        ];

        let mut world = make_test_world_with_entries(entries);
        let rules = DesignRules::jlcpcb_2layer();
        let violations = ClearanceRule.check(&mut world, &rules);

        assert_eq!(
            violations.len(),
            1,
            "Entities without nets should still be checked"
        );
    }

    // ========================================================================
    // Segment distance tests
    // ========================================================================

    #[test]
    fn test_segment_distance_parallel_horizontal() {
        // Two parallel horizontal segments, 1mm apart vertically
        let d = segment_distance(
            [0, 0],
            [10_000_000, 0],
            [0, 1_000_000],
            [10_000_000, 1_000_000],
        );
        assert_eq!(d, 1_000_000, "Parallel horizontal segments 1mm apart");
    }

    #[test]
    fn test_segment_distance_perpendicular() {
        // Perpendicular segments forming a T, 2mm gap
        // Horizontal: (0,0) → (10mm, 0)
        // Vertical:   (5mm, 2mm) → (5mm, 10mm)
        let d = segment_distance(
            [0, 0],
            [10_000_000, 0],
            [5_000_000, 2_000_000],
            [5_000_000, 10_000_000],
        );
        assert_eq!(d, 2_000_000, "Perpendicular with 2mm gap");
    }

    #[test]
    fn test_segment_distance_endpoint_closest() {
        // Two segments where the closest points are endpoints
        // Seg1: (0,0) → (1mm, 0)
        // Seg2: (2mm, 0) → (3mm, 0)  — gap is 1mm between endpoints
        let d = segment_distance([0, 0], [1_000_000, 0], [2_000_000, 0], [3_000_000, 0]);
        assert_eq!(d, 1_000_000, "Collinear with 1mm gap");
    }

    #[test]
    fn test_segment_distance_same_point() {
        // Degenerate: both segments are the same point
        let d = segment_distance([0, 0], [0, 0], [0, 0], [0, 0]);
        assert_eq!(d, 0, "Same point");
    }

    #[test]
    fn test_segment_distance_touching() {
        // Two segments sharing an endpoint
        let d = segment_distance([0, 0], [1_000_000, 0], [1_000_000, 0], [2_000_000, 0]);
        assert_eq!(d, 0, "Touching at endpoint");
    }

    #[test]
    fn test_segment_distance_crossing() {
        // Two segments that cross (X shape)
        let d = segment_distance(
            [0, 0],
            [10_000_000, 10_000_000],
            [0, 10_000_000],
            [10_000_000, 0],
        );
        assert_eq!(d, 0, "Crossing segments");
    }

    #[test]
    fn test_segment_distance_diagonal_gap() {
        // 3-4-5 triangle: endpoint distances
        let d = segment_distance(
            [0, 0],
            [0, 0], // degenerate to point at origin
            [3_000_000, 4_000_000],
            [3_000_000, 4_000_000], // degenerate to point
        );
        assert_eq!(d, 5_000_000, "3-4-5 triangle distance");
    }

    // ========================================================================
    // Trace-to-pad DRC tests
    // ========================================================================

    #[test]
    fn test_trace_to_pad_clearance_violation() {
        // A trace segment running 0.05mm from a pad — should violate 0.15mm clearance.
        // Pad AABB: (0, 0) to (1mm, 1mm) on top layer
        // Trace: horizontal at y=1.1mm (centerline), width=0.2mm, so edge at y=1.0mm
        //   → copper gap between pad edge and trace edge = 0.0mm → violation
        let mut world = BoardWorld::new();

        // Spawn pad entity (with NetId)
        let pad_entity = world.ecs_mut().spawn(NetId::new(1)).id();

        // Spawn trace entity (with Trace component + different NetId)
        let trace = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(0.0, 1.1),
                Point::from_mm(1.0, 1.1),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: NetId::new(2),
            locked: false,
            source: TraceSource::Autorouted,
        };
        let trace_entity = world.ecs_mut().spawn((trace, NetId::new(2))).id();

        // Build spatial index manually
        let entries = vec![
            // Pad
            SpatialEntry::new(
                pad_entity,
                Point::from_mm(0.0, 0.0),
                Point::from_mm(1.0, 1.0),
                Layer::TopCopper.to_copper_mask(),
            ),
            // Trace AABB (expanded by half width = 0.1mm)
            SpatialEntry::new(
                trace_entity,
                Point::from_mm(-0.1, 1.0), // 1.1 - 0.1
                Point::from_mm(1.1, 1.2),  // 1.1 + 0.1
                Layer::TopCopper.to_copper_mask(),
            ),
        ];
        world
            .ecs_mut()
            .resource_mut::<cypcb_world::SpatialIndex>()
            .rebuild(entries);

        let rules = DesignRules::jlcpcb_2layer();
        let violations = ClearanceRule.check(&mut world, &rules);

        assert!(
            !violations.is_empty(),
            "Trace 0.0mm from pad should violate 0.15mm clearance"
        );
        assert_eq!(violations[0].kind, ViolationKind::Clearance);
    }

    #[test]
    fn one_gap_is_one_violation_even_when_two_segments_share_it() {
        // A trace bends, and the corner is the closest point to a pad. Both
        // segments meeting there report that same gap, so the board gets two
        // violations at one coordinate with one message - which is what
        // led_blink prints today: `C1 <-> trace 'GND'` twice at
        // (19.431mm, 14.648mm). Per-segment reporting exists so a trace that
        // violates in three places counts three; one place counted twice is
        // not that, and every score in this project is charged per violation.
        let mut world = BoardWorld::new();

        let pad_entity = world.ecs_mut().spawn(NetId::new(1)).id();
        let trace = Trace {
            segments: vec![
                TraceSegment::new(Point::from_mm(-0.5, 1.6), Point::from_mm(0.5, 1.1)),
                TraceSegment::new(Point::from_mm(0.5, 1.1), Point::from_mm(1.5, 1.6)),
            ],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: NetId::new(2),
            locked: false,
            source: TraceSource::Autorouted,
        };
        let trace_entity = world.ecs_mut().spawn((trace, NetId::new(2))).id();

        let entries = vec![
            SpatialEntry::new(
                pad_entity,
                Point::from_mm(0.0, 0.0),
                Point::from_mm(1.0, 1.0),
                Layer::TopCopper.to_copper_mask(),
            ),
            SpatialEntry::new(
                trace_entity,
                Point::from_mm(-0.6, 1.0),
                Point::from_mm(1.6, 1.7),
                Layer::TopCopper.to_copper_mask(),
            ),
        ];
        world
            .ecs_mut()
            .resource_mut::<cypcb_world::SpatialIndex>()
            .rebuild(entries);

        let violations = ClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());

        assert_eq!(
            violations.len(),
            1,
            "one corner too close to one pad is one violation, got {:?}",
            violations
                .iter()
                .map(|v| (v.location.x.to_mm(), v.location.y.to_mm(), &v.message))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn two_traces_meeting_corner_to_corner_are_one_violation() {
        // The pad case is only half the shape. `segment_pairs` measures
        // every segment of one trace against every segment of the other, so
        // two bends facing each other find the same gap up to four times -
        // once per pair of segments that meets at the corner nearest.
        let mut world = BoardWorld::new();

        let lower = Trace {
            segments: vec![
                TraceSegment::new(Point::from_mm(-0.5, 0.5), Point::from_mm(0.5, 1.0)),
                TraceSegment::new(Point::from_mm(0.5, 1.0), Point::from_mm(1.5, 0.5)),
            ],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: NetId::new(1),
            locked: false,
            source: TraceSource::Autorouted,
        };
        let upper = Trace {
            segments: vec![
                TraceSegment::new(Point::from_mm(-0.5, 1.7), Point::from_mm(0.5, 1.2)),
                TraceSegment::new(Point::from_mm(0.5, 1.2), Point::from_mm(1.5, 1.7)),
            ],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: NetId::new(2),
            locked: false,
            source: TraceSource::Autorouted,
        };

        let lower_entity = world.ecs_mut().spawn((lower, NetId::new(1))).id();
        let upper_entity = world.ecs_mut().spawn((upper, NetId::new(2))).id();

        let entries = vec![
            SpatialEntry::new(
                lower_entity,
                Point::from_mm(-0.6, 0.4),
                Point::from_mm(1.6, 1.1),
                Layer::TopCopper.to_copper_mask(),
            ),
            SpatialEntry::new(
                upper_entity,
                Point::from_mm(-0.6, 1.1),
                Point::from_mm(1.6, 1.8),
                Layer::TopCopper.to_copper_mask(),
            ),
        ];
        world
            .ecs_mut()
            .resource_mut::<cypcb_world::SpatialIndex>()
            .rebuild(entries);

        let violations = ClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());

        assert_eq!(
            violations.len(),
            1,
            "two corners 0.2mm apart are one gap, got {:?}",
            violations
                .iter()
                .map(|v| (v.location.x.to_mm(), v.location.y.to_mm(), &v.message))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_trace_to_pad_no_violation_when_far() {
        // Trace is 2mm from pad — no violation
        let mut world = BoardWorld::new();

        let pad_entity = world.ecs_mut().spawn(NetId::new(1)).id();
        let trace = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(0.0, 5.0),
                Point::from_mm(10.0, 5.0),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: NetId::new(2),
            locked: false,
            source: TraceSource::Autorouted,
        };
        let trace_entity = world.ecs_mut().spawn((trace, NetId::new(2))).id();

        let entries = vec![
            SpatialEntry::new(
                pad_entity,
                Point::from_mm(0.0, 0.0),
                Point::from_mm(1.0, 1.0),
                Layer::TopCopper.to_copper_mask(),
            ),
            SpatialEntry::new(
                trace_entity,
                Point::from_mm(-0.1, 4.9),
                Point::from_mm(10.1, 5.1),
                Layer::TopCopper.to_copper_mask(),
            ),
        ];
        world
            .ecs_mut()
            .resource_mut::<cypcb_world::SpatialIndex>()
            .rebuild(entries);

        let rules = DesignRules::jlcpcb_2layer();
        let violations = ClearanceRule.check(&mut world, &rules);

        assert!(
            violations.is_empty(),
            "Trace 2mm from pad should not violate"
        );
    }

    // ========================================================================
    // Trace-to-trace DRC tests
    // ========================================================================

    #[test]
    fn test_trace_to_trace_clearance_violation() {
        // Two parallel traces too close together
        // Trace 1: horizontal at y=0, width=0.2mm → edge at y=0.1mm
        // Trace 2: horizontal at y=0.2mm, width=0.2mm → edge at y=0.1mm
        //   → copper gap = 0.0mm → violates 0.15mm clearance
        let mut world = BoardWorld::new();

        let t1 = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 0.0),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: NetId::new(1),
            locked: false,
            source: TraceSource::Autorouted,
        };
        let t2 = Trace {
            segments: vec![TraceSegment::new(
                Point::new(Nm(0), Nm(200_000)), // 0.2mm
                Point::new(Nm(10_000_000), Nm(200_000)),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: NetId::new(2),
            locked: false,
            source: TraceSource::Autorouted,
        };

        let e1 = world.ecs_mut().spawn((t1, NetId::new(1))).id();
        let e2 = world.ecs_mut().spawn((t2, NetId::new(2))).id();

        let hw = 100_000; // half width 0.1mm
        let entries = vec![
            SpatialEntry::from_raw(
                e1,
                -hw,
                -hw,
                10_000_000 + hw,
                hw,
                Layer::TopCopper.to_copper_mask(),
            ),
            SpatialEntry::from_raw(
                e2,
                -hw,
                200_000 - hw,
                10_000_000 + hw,
                200_000 + hw,
                Layer::TopCopper.to_copper_mask(),
            ),
        ];
        world
            .ecs_mut()
            .resource_mut::<cypcb_world::SpatialIndex>()
            .rebuild(entries);

        let rules = DesignRules::jlcpcb_2layer();
        let violations = ClearanceRule.check(&mut world, &rules);

        assert!(
            !violations.is_empty(),
            "Two traces 0mm copper gap should violate 0.15mm clearance"
        );
    }

    #[test]
    fn test_trace_to_trace_no_violation_when_far() {
        // Two traces 5mm apart — no violation
        let mut world = BoardWorld::new();

        let t1 = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 0.0),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: NetId::new(1),
            locked: false,
            source: TraceSource::Autorouted,
        };
        let t2 = Trace {
            segments: vec![TraceSegment::new(
                Point::new(Nm(0), Nm(5_000_000)), // 5mm
                Point::new(Nm(10_000_000), Nm(5_000_000)),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: NetId::new(2),
            locked: false,
            source: TraceSource::Autorouted,
        };

        let e1 = world.ecs_mut().spawn((t1, NetId::new(1))).id();
        let e2 = world.ecs_mut().spawn((t2, NetId::new(2))).id();

        let hw = 100_000;
        let entries = vec![
            SpatialEntry::from_raw(
                e1,
                -hw,
                -hw,
                10_000_000 + hw,
                hw,
                Layer::TopCopper.to_copper_mask(),
            ),
            SpatialEntry::from_raw(
                e2,
                -hw,
                5_000_000 - hw,
                10_000_000 + hw,
                5_000_000 + hw,
                Layer::TopCopper.to_copper_mask(),
            ),
        ];
        world
            .ecs_mut()
            .resource_mut::<cypcb_world::SpatialIndex>()
            .rebuild(entries);

        let rules = DesignRules::jlcpcb_2layer();
        let violations = ClearanceRule.check(&mut world, &rules);

        assert!(violations.is_empty(), "Traces 5mm apart should not violate");
    }

    // ========================================================================
    // Trace-to-component (NetConnections) same-net exemption tests
    // ========================================================================

    #[test]
    fn test_trace_touching_own_component_pad_no_violation() {
        // A trace on net VCC touching a component that has a pin on VCC.
        // This is the normal case: a routed trace connects to a pad.
        // Should NOT generate a clearance violation.
        let mut world = BoardWorld::new();
        let vcc = NetId::new(1);

        // Component entity with NetConnections (has a pin on VCC)
        let mut net_conns = cypcb_world::NetConnections::new();
        net_conns.add(cypcb_world::PinConnection::new("1", vcc));
        net_conns.add(cypcb_world::PinConnection::new("2", NetId::new(2))); // GND
        let comp_entity = world.ecs_mut().spawn(net_conns).id();

        // Trace entity on VCC net, touching the component's AABB
        let trace = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(1.0, 0.5),
                Point::from_mm(5.0, 0.5),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: vcc,
            locked: false,
            source: TraceSource::Autorouted,
        };
        let trace_entity = world.ecs_mut().spawn((trace, vcc)).id();

        // Component AABB: (0,0) to (1mm, 1mm), trace starts at x=1mm (touching)
        let entries = vec![
            SpatialEntry::new(
                comp_entity,
                Point::from_mm(0.0, 0.0),
                Point::from_mm(1.0, 1.0),
                Layer::TopCopper.to_copper_mask(),
            ),
            SpatialEntry::new(
                trace_entity,
                Point::from_mm(0.9, 0.4), // trace AABB (with half-width)
                Point::from_mm(5.1, 0.6),
                Layer::TopCopper.to_copper_mask(),
            ),
        ];
        world
            .ecs_mut()
            .resource_mut::<cypcb_world::SpatialIndex>()
            .rebuild(entries);

        let rules = DesignRules::jlcpcb_2layer();
        let violations = ClearanceRule.check(&mut world, &rules);

        assert!(
            violations.is_empty(),
            "Trace on VCC touching component with VCC pin should NOT violate clearance"
        );
    }

    #[test]
    fn test_trace_near_component_different_net_still_violates() {
        // A trace on net SIG too close to a component that has NO pin on SIG.
        // Should still generate a violation.
        let mut world = BoardWorld::new();

        let mut net_conns = cypcb_world::NetConnections::new();
        net_conns.add(cypcb_world::PinConnection::new("1", NetId::new(1))); // VCC
        net_conns.add(cypcb_world::PinConnection::new("2", NetId::new(2))); // GND
        let comp_entity = world.ecs_mut().spawn(net_conns).id();

        // Trace on net 3 (SIG) — not connected to this component
        let sig_net = NetId::new(3);
        let trace = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(1.0, 0.5),
                Point::from_mm(5.0, 0.5),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: sig_net,
            locked: false,
            source: TraceSource::Autorouted,
        };
        let trace_entity = world.ecs_mut().spawn((trace, sig_net)).id();

        let entries = vec![
            SpatialEntry::new(
                comp_entity,
                Point::from_mm(0.0, 0.0),
                Point::from_mm(1.0, 1.0),
                Layer::TopCopper.to_copper_mask(),
            ),
            SpatialEntry::new(
                trace_entity,
                Point::from_mm(0.9, 0.4),
                Point::from_mm(5.1, 0.6),
                Layer::TopCopper.to_copper_mask(),
            ),
        ];
        world
            .ecs_mut()
            .resource_mut::<cypcb_world::SpatialIndex>()
            .rebuild(entries);

        let rules = DesignRules::jlcpcb_2layer();
        let violations = ClearanceRule.check(&mut world, &rules);

        assert_eq!(
            violations.len(),
            1,
            "Trace on unrelated net near component should still violate"
        );
    }
}
