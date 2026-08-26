//! Edge clearance rule.
//!
//! Validates that all copper features maintain minimum distance from
//! the board edge. Manufacturing requires this clearance to prevent
//! copper exposure when the board is routed (cut to shape).

use cypcb_core::{Nm, Point};
use cypcb_world::components::{BoardOutline, BoardSize};
use cypcb_world::BoardWorld;

use super::DrcRule;
use crate::presets::DesignRules;
use crate::violation::DrcViolation;

/// Rule that checks minimum copper-to-board-edge clearance.
///
/// For each entry in the spatial index, checks that its bounding box is
/// at least `min_edge_clearance` away from all four board edges. The board
/// is assumed to be rectangular with origin at (0, 0).
///
/// If no board size is defined, this rule silently passes (no violations).
///
/// # Examples
///
/// ```rust,ignore
/// use cypcb_drc::rules::{EdgeClearanceRule, DrcRule};
/// use cypcb_drc::presets::DesignRules;
/// use cypcb_world::BoardWorld;
///
/// let mut world = BoardWorld::new();
/// world.set_board("test".into(), (Nm::from_mm(50.0), Nm::from_mm(50.0)), 2);
/// // ... add copper features ...
///
/// let rules = DesignRules::jlcpcb_2layer(); // 0.3mm edge clearance
/// let violations = EdgeClearanceRule.check(&mut world, &rules);
/// ```
pub struct EdgeClearanceRule;

impl DrcRule for EdgeClearanceRule {
    fn name(&self) -> &'static str {
        "edge-clearance"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let mut violations = Vec::new();
        let min_edge = rules.min_edge_clearance;

        // Get board size — if no board is defined, skip
        let board_entity = match world.board_entity() {
            Some(e) => e,
            None => return violations,
        };
        let board_size = match world.ecs().get::<BoardSize>(board_entity) {
            Some(bs) => *bs,
            None => return violations,
        };

        // The real edge when the board states one; otherwise the rectangle
        // `BoardSize` describes. A cutout, a slot and a chamfer all live inside
        // the same bounding box, so measuring against the box passes copper
        // that sits outside the actual edge.
        let outline = world.ecs().get::<BoardOutline>(board_entity).cloned();

        // Board edges are at x=0, x=width, y=0, y=height
        let board_w = board_size.width.0;
        let board_h = board_size.height.0;

        // Check each spatial entry against the four edges.
        //
        // Copper pours are measured too, and they are not in the index: a zone
        // covers every pad inside it, so indexing one would have the clearance
        // rule report a violation against each. The edge is the one question a
        // zone's own outline can answer, and a plane hanging off the board is
        // copper the router will cut through - which nothing reported before,
        // because nothing looked.
        let mut entries: Vec<_> = world.spatial().iter().cloned().collect();
        for (entity, zone) in world.zones() {
            // `is_copper_pour`, not `!is_keepout`: this measures copper against
            // the board edge, and only a pour is copper. A flexible region is
            // an area the board bends in - it reaches the edge by definition,
            // being the full width of the ribbon, so measuring one reported
            // every rigid-flex board as having copper 0.00mm from its own
            // outline.
            if !zone.is_copper_pour() {
                continue;
            }
            entries.push(cypcb_world::SpatialEntry::new(
                entity,
                zone.bounds.min,
                zone.bounds.max,
                zone.layer_mask,
            ));
        }

        for entry in &entries {
            let min_x = entry.envelope.lower()[0];
            let min_y = entry.envelope.lower()[1];
            let max_x = entry.envelope.upper()[0];
            let max_y = entry.envelope.upper()[1];

            let min_dist = match &outline {
                Some(outline) => distance_to_outline(outline, min_x, min_y, max_x, max_y),
                None => {
                    // Distance to each edge (negative means outside board)
                    let dist_left = min_x; // distance from left edge (x=0)
                    let dist_bottom = min_y; // distance from bottom edge (y=0)
                    let dist_right = board_w - max_x; // distance from right edge
                    let dist_top = board_h - max_y; // distance from top edge
                    dist_left.min(dist_bottom).min(dist_right).min(dist_top)
                }
            };

            if min_dist < min_edge.0 {
                let center = Point::new(Nm((min_x + max_x) / 2), Nm((min_y + max_y) / 2));
                violations.push(DrcViolation::edge_clearance(
                    entry.entity,
                    Nm(min_dist.max(0)), // clamp negative to 0
                    min_edge,
                    center,
                ));
            }
        }

        violations
    }
}

/// How far a bounding box sits from the board's edge.
///
/// The box's own four sides are measured against every edge of the ring, which
/// is exact for two convex shapes that do not overlap and gives zero when they
/// do. A box whose centre falls outside the ring reads as zero rather than a
/// positive distance: copper off the board is not "well clear of the edge".
pub(crate) fn distance_to_outline(
    outline: &BoardOutline,
    min_x: i64,
    min_y: i64,
    max_x: i64,
    max_y: i64,
) -> i64 {
    let centre = Point::new(Nm((min_x + max_x) / 2), Nm((min_y + max_y) / 2));
    if !outline.contains(centre) {
        return 0;
    }

    let box_edges = [
        ([min_x, min_y], [max_x, min_y]),
        ([max_x, min_y], [max_x, max_y]),
        ([max_x, max_y], [min_x, max_y]),
        ([min_x, max_y], [min_x, min_y]),
    ];

    let mut nearest = i64::MAX;
    for (a, b) in outline.edges() {
        let edge = ([a.x.raw(), a.y.raw()], [b.x.raw(), b.y.raw()]);
        for (p, q) in &box_edges {
            nearest = nearest.min(crate::rules::clearance::segment_distance(
                *p, *q, edge.0, edge.1,
            ));
        }
    }
    nearest
}

#[cfg(test)]
mod tests {

    #[test]
    fn an_l_shaped_board_is_measured_against_its_own_edge() {
        use cypcb_world::components::BoardOutline;

        // An L: 40x40 with the top-right quarter removed. A pad at 30mm, 30mm
        // sits inside the bounding box and outside the board.
        let mut world = BoardWorld::new();
        world.set_board("l".to_string(), (Nm::from_mm(40.0), Nm::from_mm(40.0)), 2);
        let board = world.board_entity().unwrap();
        let outline = BoardOutline::new(vec![
            Point::from_mm(0.0, 0.0),
            Point::from_mm(40.0, 0.0),
            Point::from_mm(40.0, 20.0),
            Point::from_mm(20.0, 20.0),
            Point::from_mm(20.0, 40.0),
            Point::from_mm(0.0, 40.0),
        ])
        .expect("a ring");
        assert!(outline.contains(Point::from_mm(10.0, 10.0)));
        assert!(!outline.contains(Point::from_mm(30.0, 30.0)));
        world.ecs_mut().entity_mut(board).insert(outline);

        let in_the_notch = world.ecs_mut().spawn(()).id();
        let well_inside = world.ecs_mut().spawn(()).id();
        world
            .ecs_mut()
            .resource_mut::<cypcb_world::SpatialIndex>()
            .rebuild(vec![
                SpatialEntry::new(
                    in_the_notch,
                    Point::from_mm(29.0, 29.0),
                    Point::from_mm(31.0, 31.0),
                    0b01,
                ),
                SpatialEntry::new(
                    well_inside,
                    Point::from_mm(9.0, 9.0),
                    Point::from_mm(11.0, 11.0),
                    0b01,
                ),
            ]);

        let violations = EdgeClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());

        assert_eq!(
            violations.len(),
            1,
            "only the pad in the removed corner is off the board: {violations:?}"
        );
        assert_eq!(violations[0].entity, in_the_notch);
        assert_eq!(violations[0].kind, ViolationKind::EdgeClearance);
    }
    use super::*;
    use crate::ViolationKind;
    use bevy_ecs::prelude::*;
    use cypcb_world::SpatialEntry;

    fn make_world_with_board_and_entries(
        board_w_mm: f64,
        board_h_mm: f64,
        entries: Vec<SpatialEntry>,
    ) -> BoardWorld {
        let mut world = BoardWorld::new();
        world.set_board(
            "test".into(),
            (Nm::from_mm(board_w_mm), Nm::from_mm(board_h_mm)),
            2,
        );
        world
            .ecs_mut()
            .resource_mut::<cypcb_world::SpatialIndex>()
            .rebuild(entries);
        world
    }

    #[test]
    fn test_no_violation_centered() {
        // Pad well inside a 50mm board
        let entries = vec![SpatialEntry::new(
            Entity::from_raw(0),
            Point::from_mm(20.0, 20.0),
            Point::from_mm(21.0, 21.0),
            0b01,
        )];
        let mut world = make_world_with_board_and_entries(50.0, 50.0, entries);
        let rules = DesignRules::jlcpcb_2layer(); // 0.3mm edge clearance

        let violations = EdgeClearanceRule.check(&mut world, &rules);
        assert!(violations.is_empty(), "Centered pad should pass");
    }

    #[test]
    fn test_violation_too_close_to_left_edge() {
        // Pad 0.1mm from left edge, rule requires 0.3mm
        let entries = vec![SpatialEntry::new(
            Entity::from_raw(0),
            Point::from_mm(0.1, 20.0),
            Point::from_mm(1.1, 21.0),
            0b01,
        )];
        let mut world = make_world_with_board_and_entries(50.0, 50.0, entries);
        let rules = DesignRules::jlcpcb_2layer();

        let violations = EdgeClearanceRule.check(&mut world, &rules);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].kind, ViolationKind::EdgeClearance);
    }

    #[test]
    fn test_violation_too_close_to_right_edge() {
        // Pad 0.1mm from right edge of 50mm board
        let entries = vec![SpatialEntry::new(
            Entity::from_raw(0),
            Point::from_mm(49.0, 20.0),
            Point::from_mm(49.9, 21.0),
            0b01,
        )];
        let mut world = make_world_with_board_and_entries(50.0, 50.0, entries);
        let rules = DesignRules::jlcpcb_2layer();

        let violations = EdgeClearanceRule.check(&mut world, &rules);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].kind, ViolationKind::EdgeClearance);
    }

    #[test]
    fn test_violation_too_close_to_top_edge() {
        let entries = vec![SpatialEntry::new(
            Entity::from_raw(0),
            Point::from_mm(20.0, 49.0),
            Point::from_mm(21.0, 49.9),
            0b01,
        )];
        let mut world = make_world_with_board_and_entries(50.0, 50.0, entries);
        let rules = DesignRules::jlcpcb_2layer();

        let violations = EdgeClearanceRule.check(&mut world, &rules);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_violation_too_close_to_bottom_edge() {
        let entries = vec![SpatialEntry::new(
            Entity::from_raw(0),
            Point::from_mm(20.0, 0.05),
            Point::from_mm(21.0, 1.05),
            0b01,
        )];
        let mut world = make_world_with_board_and_entries(50.0, 50.0, entries);
        let rules = DesignRules::jlcpcb_2layer();

        let violations = EdgeClearanceRule.check(&mut world, &rules);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_no_board_no_violations() {
        // No board defined — rule should pass silently
        let mut world = BoardWorld::new();
        let rules = DesignRules::jlcpcb_2layer();
        let violations = EdgeClearanceRule.check(&mut world, &rules);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_exactly_at_clearance_passes() {
        // Pad exactly at min_edge_clearance from left edge (0.3mm)
        let entries = vec![SpatialEntry::new(
            Entity::from_raw(0),
            Point::from_mm(0.3, 20.0),
            Point::from_mm(1.3, 21.0),
            0b01,
        )];
        let mut world = make_world_with_board_and_entries(50.0, 50.0, entries);
        let rules = DesignRules::jlcpcb_2layer();

        let violations = EdgeClearanceRule.check(&mut world, &rules);
        assert!(violations.is_empty(), "Exactly at clearance should pass");
    }

    #[test]
    fn test_multiple_violations() {
        // Two pads too close to edges
        let entries = vec![
            SpatialEntry::new(
                Entity::from_raw(0),
                Point::from_mm(0.1, 20.0), // too close to left
                Point::from_mm(1.1, 21.0),
                0b01,
            ),
            SpatialEntry::new(
                Entity::from_raw(1),
                Point::from_mm(20.0, 49.9), // too close to top
                Point::from_mm(21.0, 50.0),
                0b01,
            ),
            SpatialEntry::new(
                Entity::from_raw(2),
                Point::from_mm(20.0, 20.0), // well inside — OK
                Point::from_mm(21.0, 21.0),
                0b01,
            ),
        ];
        let mut world = make_world_with_board_and_entries(50.0, 50.0, entries);
        let rules = DesignRules::jlcpcb_2layer();

        let violations = EdgeClearanceRule.check(&mut world, &rules);
        assert_eq!(violations.len(), 2, "Expected 2 violations");
    }

    #[test]
    fn test_empty_spatial_index() {
        let mut world = BoardWorld::new();
        world.set_board("test".into(), (Nm::from_mm(50.0), Nm::from_mm(50.0)), 2);
        let rules = DesignRules::default();
        let violations = EdgeClearanceRule.check(&mut world, &rules);
        assert!(violations.is_empty());
    }

    /// The measurement behind every edge-clearance number on a board whose
    /// outline is not a rectangle.
    ///
    /// `distance_to_outline` takes a bounding box and the ring, and answers
    /// how far one sits from the other - measuring the box's four sides
    /// against every edge of the ring rather than its centre against the
    /// nearest one. Nothing named it in a test.
    #[test]
    fn a_box_is_measured_from_its_own_sides_to_the_ring() {
        use cypcb_world::components::BoardOutline;

        // The U from `examples/cutout.cypcb`: 40 by 30 with a slot cut down
        // from the top edge between x = 15 and x = 25.
        let u = BoardOutline::new(vec![
            Point::from_mm(0.0, 0.0),
            Point::from_mm(40.0, 0.0),
            Point::from_mm(40.0, 30.0),
            Point::from_mm(25.0, 30.0),
            Point::from_mm(25.0, 10.0),
            Point::from_mm(15.0, 10.0),
            Point::from_mm(15.0, 30.0),
            Point::from_mm(0.0, 30.0),
        ])
        .expect("a ring");

        let mm = |v: f64| Nm::from_mm(v).raw();

        // A 1mm box in the left arm, its centre 5mm from the left edge: the
        // nearest side of the box is 4.5mm from it, not 5mm.
        assert_eq!(
            distance_to_outline(&u, mm(4.5), mm(19.5), mm(5.5), mm(20.5)),
            mm(4.5)
        );

        // The same box in the middle of the arm, equidistant from the left
        // edge and the slot wall.
        assert_eq!(
            distance_to_outline(&u, mm(7.0), mm(14.5), mm(8.0), mm(15.5)),
            mm(7.0)
        );

        // Copper in the slot is not copper well clear of the edge: a box whose
        // centre falls outside the ring reads zero.
        assert_eq!(
            distance_to_outline(&u, mm(19.5), mm(19.5), mm(20.5), mm(20.5)),
            0
        );

        // And a box straddling the slot wall touches the ring.
        assert_eq!(
            distance_to_outline(&u, mm(12.0), mm(18.0), mm(16.0), mm(22.0)),
            0
        );
    }
}
