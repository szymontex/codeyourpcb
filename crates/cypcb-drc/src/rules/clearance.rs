//! Clearance checking rule.
//!
//! Detects copper features that are too close together for manufacturing.
//! Uses the spatial index for efficient O(log n) candidate selection.

use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::Trace;
use cypcb_world::components::{NetConnections, NetId};
use cypcb_world::BoardWorld;
use hashbrown::{HashMap, HashSet};
use rstar::AABB;

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
            let query_min = Point::new(
                Nm(entry.envelope.lower()[0] - widest.0),
                Nm(entry.envelope.lower()[1] - widest.0),
            );
            let query_max = Point::new(
                Nm(entry.envelope.upper()[0] + widest.0),
                Nm(entry.envelope.upper()[1] + widest.0),
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

                let same_net = match (net_a, net_b) {
                    // Case 1: both have a single NetId
                    (Some(na), Some(nb)) => na == nb,
                    // Case 2a: A has NetId, B is a component
                    (Some(na), None) => nc_b.is_some_and(|nets| nets.contains(na)),
                    // Case 2b: B has NetId, A is a component
                    (None, Some(nb)) => nc_a.is_some_and(|nets| nets.contains(nb)),
                    // Case 3: both are components — share a common net
                    (None, None) => match (nc_a, nc_b) {
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
                let copper_of = |idx: u32, mask: u32| -> Option<Vec<&AABB<[i64; 2]>>> {
                    let pads = pad_map.get(&idx)?;
                    Some(
                        pads.iter()
                            .filter(|pad| pad.layer_mask & mask != 0)
                            .map(|pad| &pad.box_)
                            .collect(),
                    )
                };

                let mut no_copper_in_reach = false;
                let (contact, distance) = match (trace_a, trace_b) {
                    // Both are traces: segment-to-segment distance minus both half-widths
                    (Some(ta), Some(tb)) => {
                        let (at, seg_dist) = trace_to_trace_distance(ta, tb);
                        (at, (seg_dist - ta.half_width - tb.half_width).max(0))
                    }
                    // One is a trace, the other is a via or a component
                    (Some(t), None) => {
                        let (at, seg_dist) = match copper_of(b_idx, entry.layer_mask) {
                            Some(pads) if pads.is_empty() => {
                                no_copper_in_reach = true;
                                (Point::ORIGIN, i64::MAX)
                            }
                            Some(pads) => trace_to_nearest(t, &pads),
                            None => trace_to_aabb_distance(t, &candidate.envelope),
                        };
                        (at, (seg_dist - t.half_width).max(0))
                    }
                    (None, Some(t)) => {
                        let (at, seg_dist) = match copper_of(a_idx, candidate.layer_mask) {
                            Some(pads) if pads.is_empty() => {
                                no_copper_in_reach = true;
                                (Point::ORIGIN, i64::MAX)
                            }
                            Some(pads) => trace_to_nearest(t, &pads),
                            None => trace_to_aabb_distance(t, &entry.envelope),
                        };
                        (at, (seg_dist - t.half_width).max(0))
                    }
                    // Neither is a trace: vias and pads. A component stands for
                    // its pads; anything else stands for its own box.
                    (None, None) => {
                        let a_boxes = copper_of(a_idx, candidate.layer_mask);
                        let b_boxes = copper_of(b_idx, entry.layer_mask);
                        if a_boxes.as_ref().is_some_and(|p| p.is_empty())
                            || b_boxes.as_ref().is_some_and(|p| p.is_empty())
                        {
                            no_copper_in_reach = true;
                            (Point::ORIGIN, i64::MAX)
                        } else {
                            let a_list = a_boxes.unwrap_or_else(|| vec![&entry.envelope]);
                            let b_list = b_boxes.unwrap_or_else(|| vec![&candidate.envelope]);
                            nearest_pair(&a_list, &b_list)
                        }
                    }
                };

                if no_copper_in_reach {
                    continue;
                }

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

                if distance < required.0 {
                    // Report the pair the same way round however the loop
                    // reached it, and point at the gap between the two
                    // features rather than at whichever one the outer loop
                    // happened to be holding. The outer loop walks the spatial
                    // index, whose order is not guaranteed run to run, so
                    // without this the same board reports the same violation
                    // with a different name order and a different coordinate
                    // on every run. Measured on stm32_breakout: 308 violations
                    // both runs, 152 of the printed lines different.
                    let (primary, secondary) = if a_idx <= b_idx {
                        (entry, candidate)
                    } else {
                        (candidate, entry)
                    };
                    violations.push(DrcViolation::clearance(
                        primary.entity,
                        secondary.entity,
                        Nm(distance),
                        required,
                        contact,
                    ));
                }
            }
        }

        violations
    }
}

/// One pad's copper, in board coordinates, and the layers it is on.
struct PadBox {
    box_: AABB<[i64; 2]>,
    layer_mask: u32,
}

/// Every component's pad copper, keyed by entity index.
///
/// Pads are placed the way the exporter and the renderer place them: the pad
/// offset is rotated around the component origin and added to its position. A
/// pad rotated off the axes is boxed by the extent of the rotated rectangle,
/// which is never smaller than the copper - a checker may over-report, and may
/// not under-report.
fn component_pads(world: &mut BoardWorld) -> HashMap<u32, Vec<PadBox>> {
    use cypcb_world::components::{FootprintRef, Position, Rotation};

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

        let radians = degrees.to_radians();
        let (sin, cos) = radians.sin_cos();

        let boxes = footprint
            .pads
            .iter()
            .map(|pad| {
                let px = pad.position.x.0 as f64;
                let py = pad.position.y.0 as f64;
                let cx = position.x.0 + (px * cos - py * sin).round() as i64;
                let cy = position.y.0 + (px * sin + py * cos).round() as i64;

                let half_w = pad.size.0 .0 as f64 / 2.0;
                let half_h = pad.size.1 .0 as f64 / 2.0;
                let extent_x = (half_w * cos.abs() + half_h * sin.abs()).round() as i64;
                let extent_y = (half_w * sin.abs() + half_h * cos.abs()).round() as i64;

                let layer_mask = pad
                    .layers
                    .iter()
                    .fold(0u32, |mask, layer| mask | layer.to_copper_mask());

                PadBox {
                    box_: AABB::from_corners(
                        [cx - extent_x, cy - extent_y],
                        [cx + extent_x, cy + extent_y],
                    ),
                    layer_mask,
                }
            })
            .collect();

        out.insert(index, boxes);
    }

    out
}

/// Closest approach between a trace and the nearest of several boxes.
fn trace_to_nearest(trace: &TraceData, boxes: &[&AABB<[i64; 2]>]) -> (Point, i64) {
    boxes
        .iter()
        .map(|b| trace_to_aabb_distance(trace, b))
        .min_by_key(|(_, distance)| *distance)
        .unwrap_or((Point::ORIGIN, i64::MAX))
}

/// Closest approach between two sets of boxes, and where it happens.
fn nearest_pair(a: &[&AABB<[i64; 2]>], b: &[&AABB<[i64; 2]>]) -> (Point, i64) {
    a.iter()
        .flat_map(|box_a| {
            b.iter().map(move |box_b| {
                (
                    midpoint(aabb_center(box_a), aabb_center(box_b)),
                    aabb_distance(box_a, box_b),
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
fn aabb_distance(a: &AABB<[i64; 2]>, b: &AABB<[i64; 2]>) -> i64 {
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
struct TraceData {
    /// Half of the trace width in nanometers.
    half_width: i64,
    /// Segments as ([start_x, start_y], [end_x, end_y]).
    segments: Vec<([i64; 2], [i64; 2])>,
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
#[allow(dead_code)] // Kept for future DRC rules (e.g., pad-to-trace clearance)
fn point_to_segment_distance(p: [i64; 2], s1: [i64; 2], s2: [i64; 2]) -> i64 {
    segment_distance(p, p, s1, s2)
}

/// Minimum distance between trace centerlines (segment-to-segment).
fn trace_to_trace_distance(a: &TraceData, b: &TraceData) -> (Point, i64) {
    let mut best = (Point::new(Nm(0), Nm(0)), i64::MAX);
    for seg_a in &a.segments {
        for seg_b in &b.segments {
            let (at, distance) = segment_closest(seg_a.0, seg_a.1, seg_b.0, seg_b.1);
            if distance < best.1 {
                best = (at, distance);
            }
        }
    }
    best
}

/// Minimum distance from trace centerlines to an AABB.
///
/// Computes the closest distance from any trace segment endpoint
/// or perpendicular projection to the AABB edges. For AABB-to-segment,
/// we test distance from each segment to each AABB edge segment.
fn trace_to_aabb_distance(trace: &TraceData, aabb: &AABB<[i64; 2]>) -> (Point, i64) {
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
