//! Clearance checking rule.
//!
//! Detects copper features that are too close together for manufacturing.
//! Uses the spatial index for efficient O(log n) candidate selection.

use cypcb_core::{Nm, Point};
use cypcb_world::BoardWorld;
use cypcb_world::components::NetId;
use cypcb_world::components::trace::Trace;
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
                        .map(|s| {
                            ([s.start.x.0, s.start.y.0], [s.end.x.0, s.end.y.0])
                        })
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

        for entry in &entries {
            // Expand bounding box by min_clearance to find candidates
            let query_min = Point::new(
                Nm(entry.envelope.lower()[0] - min_clearance.0),
                Nm(entry.envelope.lower()[1] - min_clearance.0),
            );
            let query_max = Point::new(
                Nm(entry.envelope.upper()[0] + min_clearance.0),
                Nm(entry.envelope.upper()[1] + min_clearance.0),
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
                // belong to the same net (they're electrically connected)
                if let (Some(net_a), Some(net_b)) = (
                    net_map.get(&entry.entity.index()),
                    net_map.get(&candidate.entity.index()),
                ) {
                    if net_a == net_b {
                        continue;
                    }
                }

                // Phase 2: Calculate actual distance.
                // If either entity is a trace, use refined segment-based
                // distance instead of raw AABB distance.
                let trace_a = trace_map.get(&entry.entity.index());
                let trace_b = trace_map.get(&candidate.entity.index());

                let distance = match (trace_a, trace_b) {
                    // Both are traces: segment-to-segment distance minus both half-widths
                    (Some(ta), Some(tb)) => {
                        let seg_dist = trace_to_trace_distance(ta, tb);
                        (seg_dist - ta.half_width - tb.half_width).max(0)
                    }
                    // One is a trace, the other is a pad/component AABB
                    (Some(t), None) => {
                        let seg_dist = trace_to_aabb_distance(t, &candidate.envelope);
                        (seg_dist - t.half_width).max(0)
                    }
                    (None, Some(t)) => {
                        let seg_dist = trace_to_aabb_distance(t, &entry.envelope);
                        (seg_dist - t.half_width).max(0)
                    }
                    // Neither is a trace: use AABB distance (original behavior)
                    (None, None) => aabb_distance(&entry.envelope, &candidate.envelope),
                };

                if distance < min_clearance.0 {
                    let location = aabb_center(&entry.envelope);
                    violations.push(DrcViolation::clearance(
                        entry.entity,
                        candidate.entity,
                        Nm(distance),
                        min_clearance,
                        location,
                    ));
                }
            }
        }

        violations
    }
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
pub fn segment_distance(
    p1: [i64; 2],
    p2: [i64; 2],
    p3: [i64; 2],
    p4: [i64; 2],
) -> i64 {
    // Direction vectors
    let d1 = [p2[0] - p1[0], p2[1] - p1[1]];
    let d2 = [p4[0] - p3[0], p4[1] - p3[1]];
    // Vector from p1→p3
    let r = [p3[0] - p1[0], p3[1] - p1[1]];

    let a = dot128(d1, d1); // |d1|²
    let e = dot128(d2, d2); // |d2|²

    // Both segments degenerate to points
    if a == 0 && e == 0 {
        return point_distance(p1, p3);
    }

    let c = dot128(d1, r); // D1 · r
    let f = dot128(d2, r); // D2 · r

    let mut s: f64;
    let mut t: f64;

    if a == 0 {
        // First segment degenerates to a point
        // Minimize |P1 - P3 - t·D2|²: t = f/e
        s = 0.0;
        t = (f as f64 / e as f64).clamp(0.0, 1.0);
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
    (dx * dx + dy * dy).sqrt() as i64
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
fn point_to_segment_distance(p: [i64; 2], s1: [i64; 2], s2: [i64; 2]) -> i64 {
    segment_distance(p, p, s1, s2)
}

/// Minimum distance between trace centerlines (segment-to-segment).
fn trace_to_trace_distance(a: &TraceData, b: &TraceData) -> i64 {
    let mut min_dist = i64::MAX;
    for seg_a in &a.segments {
        for seg_b in &b.segments {
            let d = segment_distance(seg_a.0, seg_a.1, seg_b.0, seg_b.1);
            min_dist = min_dist.min(d);
        }
    }
    min_dist
}

/// Minimum distance from trace centerlines to an AABB.
///
/// Computes the closest distance from any trace segment endpoint
/// or perpendicular projection to the AABB edges. For AABB-to-segment,
/// we test distance from each segment to each AABB edge segment.
fn trace_to_aabb_distance(trace: &TraceData, aabb: &AABB<[i64; 2]>) -> i64 {
    let lo = aabb.lower();
    let hi = aabb.upper();
    // AABB edge segments (4 sides)
    let edges: [([i64; 2], [i64; 2]); 4] = [
        ([lo[0], lo[1]], [hi[0], lo[1]]), // bottom
        ([hi[0], lo[1]], [hi[0], hi[1]]), // right
        ([hi[0], hi[1]], [lo[0], hi[1]]), // top
        ([lo[0], hi[1]], [lo[0], lo[1]]), // left
    ];

    let mut min_dist = i64::MAX;
    for seg in &trace.segments {
        // Check if the segment center is inside the AABB (overlap)
        let mid_x = (seg.0[0] + seg.1[0]) / 2;
        let mid_y = (seg.0[1] + seg.1[1]) / 2;
        if mid_x >= lo[0] && mid_x <= hi[0] && mid_y >= lo[1] && mid_y <= hi[1] {
            return 0; // Centerline passes through AABB
        }

        // Check endpoints inside AABB
        if seg.0[0] >= lo[0] && seg.0[0] <= hi[0] && seg.0[1] >= lo[1] && seg.0[1] <= hi[1] {
            return 0;
        }
        if seg.1[0] >= lo[0] && seg.1[0] <= hi[0] && seg.1[1] >= lo[1] && seg.1[1] <= hi[1] {
            return 0;
        }

        for edge in &edges {
            let d = segment_distance(seg.0, seg.1, edge.0, edge.1);
            min_dist = min_dist.min(d);
        }
    }
    min_dist
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::*;
    use cypcb_core::{Nm, Point};
    use cypcb_world::SpatialEntry;
    use cypcb_world::components::NetId;
    use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource, Via};
    use cypcb_world::Layer;

    use crate::ViolationKind;

    fn make_test_world_with_entries(entries: Vec<SpatialEntry>) -> BoardWorld {
        let mut world = BoardWorld::new();
        // Access the ECS world to directly populate the spatial index
        world.ecs_mut().resource_mut::<cypcb_world::SpatialIndex>().rebuild(entries);
        world
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
            SpatialEntry::new(
                e0,
                Point::from_mm(0.0, 0.0),
                Point::from_mm(1.0, 1.0),
                0b01,
            ),
            SpatialEntry::new(
                e1,
                Point::from_mm(1.05, 0.0), // 0.05mm gap — would fail 0.15mm clearance
                Point::from_mm(2.05, 1.0),
                0b01,
            ),
        ];

        world.ecs_mut().resource_mut::<cypcb_world::SpatialIndex>().rebuild(entries);

        let rules = DesignRules::jlcpcb_2layer();
        let violations = ClearanceRule.check(&mut world, &rules);

        assert!(violations.is_empty(), "Same-net pads should be exempt from clearance check");
    }

    #[test]
    fn test_different_net_still_violates() {
        // Two pads close together on DIFFERENT nets — should violate
        let mut world = BoardWorld::new();

        // Spawn real entities with different NetIds
        let e0 = world.ecs_mut().spawn(NetId::new(1)).id();
        let e1 = world.ecs_mut().spawn(NetId::new(2)).id();

        let entries = vec![
            SpatialEntry::new(
                e0,
                Point::from_mm(0.0, 0.0),
                Point::from_mm(1.0, 1.0),
                0b01,
            ),
            SpatialEntry::new(
                e1,
                Point::from_mm(1.05, 0.0), // 0.05mm gap
                Point::from_mm(2.05, 1.0),
                0b01,
            ),
        ];

        world.ecs_mut().resource_mut::<cypcb_world::SpatialIndex>().rebuild(entries);

        let rules = DesignRules::jlcpcb_2layer();
        let violations = ClearanceRule.check(&mut world, &rules);

        assert_eq!(violations.len(), 1, "Different-net pads should still violate");
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

        assert_eq!(violations.len(), 1, "Entities without nets should still be checked");
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
        let d = segment_distance(
            [0, 0],
            [1_000_000, 0],
            [2_000_000, 0],
            [3_000_000, 0],
        );
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
        let d = segment_distance(
            [0, 0],
            [1_000_000, 0],
            [1_000_000, 0],
            [2_000_000, 0],
        );
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
                Point::from_mm(-0.1, 1.0),   // 1.1 - 0.1
                Point::from_mm(1.1, 1.2),    // 1.1 + 0.1
                Layer::TopCopper.to_copper_mask(),
            ),
        ];
        world.ecs_mut().resource_mut::<cypcb_world::SpatialIndex>().rebuild(entries);

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
        world.ecs_mut().resource_mut::<cypcb_world::SpatialIndex>().rebuild(entries);

        let rules = DesignRules::jlcpcb_2layer();
        let violations = ClearanceRule.check(&mut world, &rules);

        assert!(violations.is_empty(), "Trace 2mm from pad should not violate");
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
            SpatialEntry::from_raw(e1, -hw, -hw, 10_000_000 + hw, hw, Layer::TopCopper.to_copper_mask()),
            SpatialEntry::from_raw(e2, -hw, 200_000 - hw, 10_000_000 + hw, 200_000 + hw, Layer::TopCopper.to_copper_mask()),
        ];
        world.ecs_mut().resource_mut::<cypcb_world::SpatialIndex>().rebuild(entries);

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
            SpatialEntry::from_raw(e1, -hw, -hw, 10_000_000 + hw, hw, Layer::TopCopper.to_copper_mask()),
            SpatialEntry::from_raw(e2, -hw, 5_000_000 - hw, 10_000_000 + hw, 5_000_000 + hw, Layer::TopCopper.to_copper_mask()),
        ];
        world.ecs_mut().resource_mut::<cypcb_world::SpatialIndex>().rebuild(entries);

        let rules = DesignRules::jlcpcb_2layer();
        let violations = ClearanceRule.check(&mut world, &rules);

        assert!(violations.is_empty(), "Traces 5mm apart should not violate");
    }
}
