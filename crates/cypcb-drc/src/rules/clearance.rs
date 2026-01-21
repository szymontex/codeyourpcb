//! Clearance checking rule.
//!
//! Detects copper features that are too close together for manufacturing.
//! Uses the spatial index for efficient O(log n) candidate selection.

use cypcb_core::{Nm, Point};
use cypcb_world::{BoardWorld, SpatialEntry};
use hashbrown::HashSet;
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

        // Track checked pairs to avoid A-B and B-A duplicates
        let mut checked_pairs: HashSet<(u32, u32)> = HashSet::new();

        // Collect all entries first to avoid borrowing issues
        let entries: Vec<SpatialEntry> = world.spatial().iter().cloned().collect();

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
                // Skip self
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

                // TODO: Check if same net (exempt from clearance)
                // For now, we check all pairs

                // Phase 2: Calculate actual distance between AABBs
                let distance = aabb_distance(&entry.envelope, &candidate.envelope);

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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::*;
    use cypcb_core::Point;
    use cypcb_world::SpatialEntry;

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
}
