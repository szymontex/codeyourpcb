//! Path post-processing: simplification and output conversion.
//!
//! Converts raw grid paths from the A* pathfinder into minimal
//! [`RouteSegment`] and [`ViaPlacement`] output for `apply_routes()`.
//!
//! # Pipeline
//!
//! 1. [`simplify_path`] — merge collinear grid steps into minimal straight segments
//! 2. [`convert_to_route_segments`] — convert grid coordinates to Nm, apply trace widths

use cypcb_router::types::{RouteSegment, ViaPlacement};
use cypcb_rules::RoutingRuleSet;
use cypcb_world::NetId;

use crate::grid::{index_to_layer, RoutingGrid};
use crate::pathfinder::GridNode;

/// A straight segment in grid coordinates (same layer, same direction).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathSegment {
    /// Start grid node.
    pub start: GridNode,
    /// End grid node.
    pub end: GridNode,
    /// Layer index (redundant with start/end but convenient).
    pub layer: u8,
}

/// A layer transition (via) detected in the grid path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerTransition {
    /// Grid position where the via is placed.
    pub position: GridNode,
    /// Source layer index.
    pub from_layer: u8,
    /// Destination layer index.
    pub to_layer: u8,
}

/// Result of simplifying a grid path.
#[derive(Debug, Clone)]
pub struct SimplifiedPath {
    /// Merged straight segments.
    pub segments: Vec<PathSegment>,
    /// Layer transitions (vias).
    pub transitions: Vec<LayerTransition>,
}

/// Simplify a raw grid path by merging collinear steps.
///
/// Adjacent nodes moving in the same direction (same dx, dy signs) on
/// the same layer collapse into a single [`PathSegment`]. Layer changes
/// produce [`LayerTransition`] entries.
///
/// A path of N collinear horizontal steps becomes 1 segment.
/// An L-shaped path becomes 2 segments.
/// A path crossing layers produces segments + transitions.
pub fn simplify_path(path: &[GridNode]) -> SimplifiedPath {
    let mut segments = Vec::new();
    let mut transitions = Vec::new();

    if path.len() < 2 {
        return SimplifiedPath {
            segments,
            transitions,
        };
    }

    let mut seg_start = path[0];
    let mut prev = path[0];

    for &node in &path[1..] {
        if node.2 != prev.2 {
            // Layer change — emit segment up to prev, then record transition
            if seg_start != prev {
                segments.push(PathSegment {
                    start: seg_start,
                    end: prev,
                    layer: seg_start.2,
                });
            }

            transitions.push(LayerTransition {
                position: prev,
                from_layer: prev.2,
                to_layer: node.2,
            });

            seg_start = node;
        } else {
            // Same layer — check if direction changed
            let dx1 = prev.0 as i32 - seg_start.0 as i32;
            let dy1 = prev.1 as i32 - seg_start.1 as i32;
            let dx2 = node.0 as i32 - prev.0 as i32;
            let dy2 = node.1 as i32 - prev.1 as i32;

            let dir_changed = if dx1 == 0 && dy1 == 0 {
                false // seg_start == prev, no direction yet
            } else {
                dx1.signum() != dx2.signum() || dy1.signum() != dy2.signum()
            };

            if dir_changed {
                segments.push(PathSegment {
                    start: seg_start,
                    end: prev,
                    layer: seg_start.2,
                });
                seg_start = prev;
            }
        }

        prev = node;
    }

    // Emit final segment
    if seg_start != prev {
        segments.push(PathSegment {
            start: seg_start,
            end: prev,
            layer: seg_start.2,
        });
    }

    SimplifiedPath {
        segments,
        transitions,
    }
}

/// Convert simplified grid path segments into [`RouteSegment`] and [`ViaPlacement`] output.
///
/// - Grid coordinates are converted to Nm via `grid.grid_to_nm()`
/// - Layer indices map to `Layer` enum (0→TopCopper, 1→BottomCopper, 2+→Inner)
/// - Trace width comes from `width_override` when the design states one for
///   this net, floored at the preset minimum; otherwise from the preset
/// - Via drill size comes from `rules.constraints_for_net(net_id).min_via_drill`
pub fn convert_to_route_segments(
    simplified: &SimplifiedPath,
    grid: &RoutingGrid,
    net_id: NetId,
    rules: &dyn RoutingRuleSet,
    width_override: Option<cypcb_core::Nm>,
) -> (Vec<RouteSegment>, Vec<ViaPlacement>) {
    let constraints = rules.constraints_for_net(net_id.id());
    // A design that writes `net VCC [width 0.3mm]` has said something the fab
    // preset cannot know. Its number wins, and never goes below the preset
    // minimum - a design cannot ask for copper thinner than the board house
    // will etch.
    let trace_width = width_override
        .map(|width| width.max(constraints.min_trace_width))
        .unwrap_or(constraints.min_trace_width);
    let via_drill = constraints.min_via_drill;

    let mut segments = Vec::with_capacity(simplified.segments.len());
    let mut vias = Vec::with_capacity(simplified.transitions.len());

    for seg in &simplified.segments {
        let start = grid.grid_to_nm(seg.start.0 as u32, seg.start.1 as u32);
        let end = grid.grid_to_nm(seg.end.0 as u32, seg.end.1 as u32);
        let layer = index_to_layer(seg.layer as usize);
        segments.push(RouteSegment::new(net_id, layer, trace_width, start, end));
    }

    for tr in &simplified.transitions {
        let pos = grid.grid_to_nm(tr.position.0 as u32, tr.position.1 as u32);
        let from = index_to_layer(tr.from_layer as usize);
        let to = index_to_layer(tr.to_layer as usize);
        vias.push(ViaPlacement::new(net_id, pos, via_drill, from, to));
    }

    (segments, vias)
}

/// Convert multiple raw grid paths for a net into final output.
///
/// Convenience wrapper: calls `simplify_path` + `convert_to_route_segments`
/// for each path, collecting all results. Logs post-processing stats.
pub fn paths_to_output(
    grid: &RoutingGrid,
    net_id: NetId,
    paths: &[Vec<GridNode>],
    rules: &dyn RoutingRuleSet,
    width_override: Option<cypcb_core::Nm>,
) -> (Vec<RouteSegment>, Vec<ViaPlacement>) {
    let mut all_segments = Vec::new();
    let mut all_vias = Vec::new();
    let mut raw_steps = 0usize;

    for path in paths {
        raw_steps += path.len();
        let simplified = simplify_path(path);
        let (segs, vias) =
            convert_to_route_segments(&simplified, grid, net_id, rules, width_override);
        all_segments.extend(segs);

        // Filter out vias at pad positions — THT pads already connect layers,
        // so a via on a pad is redundant and visually confusing.
        for via in vias {
            let grid_pos = grid.nm_to_grid(via.position);
            let is_on_pad = grid.cell(grid_pos.0, grid_pos.1, 0) & super::grid::CELL_PAD != 0
                || grid.cell(grid_pos.0, grid_pos.1, 1) & super::grid::CELL_PAD != 0;
            if !is_on_pad {
                all_vias.push(via);
            }
        }
    }

    let filtered_count = raw_steps; // for logging
    tracing::info!(
        net_id = net_id.id(),
        raw_steps = filtered_count,
        segments = all_segments.len(),
        vias = all_vias.len(),
        "post-processing: {} raw steps -> {} segments, {} vias",
        filtered_count,
        all_segments.len(),
        all_vias.len()
    );

    (all_segments, all_vias)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::make_test_grid;
    use cypcb_core::Nm;
    use cypcb_rules::constraints::DesignConstraints;
    use cypcb_rules::signal_class::{SignalClass, SignalClassConstraints};
    use cypcb_rules::RoutingRuleSet;
    use cypcb_world::{Layer, NetId};

    struct TestRules {
        base: DesignConstraints,
    }

    impl TestRules {
        fn new() -> Self {
            Self {
                base: DesignConstraints::default(),
            }
        }
    }

    impl RoutingRuleSet for TestRules {
        fn constraints_for_net(&self, _net_id: u32) -> &DesignConstraints {
            &self.base
        }
        fn constraints_for_class(&self, class: SignalClass) -> SignalClassConstraints {
            class.default_constraints()
        }
        fn via_cost(&self, from_layer: u8, to_layer: u8) -> f64 {
            let span = (from_layer as i16 - to_layer as i16).unsigned_abs() as f64;
            span * 2.0
        }
        fn layer_change_cost(&self, layer: u8) -> f64 {
            if layer == 0 {
                0.1
            } else {
                0.5
            }
        }
        fn clearance_between(&self, _net_a: u32, _net_b: u32) -> Nm {
            self.base.min_clearance
        }
    }

    #[test]
    fn collinear_merge_horizontal() {
        // 5 horizontal steps → 1 segment
        let path: Vec<GridNode> = (0..6).map(|x| (x, 5, 0)).collect();
        let simplified = simplify_path(&path);

        assert_eq!(
            simplified.segments.len(),
            1,
            "5 horizontal steps should merge into 1 segment"
        );
        assert_eq!(simplified.segments[0].start, (0, 5, 0));
        assert_eq!(simplified.segments[0].end, (5, 5, 0));
        assert_eq!(simplified.segments[0].layer, 0);
        assert!(simplified.transitions.is_empty());
    }

    #[test]
    fn collinear_merge_vertical() {
        let path: Vec<GridNode> = (0..8).map(|y| (3, y, 0)).collect();
        let simplified = simplify_path(&path);

        assert_eq!(simplified.segments.len(), 1);
        assert_eq!(simplified.segments[0].start, (3, 0, 0));
        assert_eq!(simplified.segments[0].end, (3, 7, 0));
    }

    #[test]
    fn collinear_merge_diagonal() {
        // Diagonal steps (1,1), (2,2), (3,3), (4,4) — same direction
        let path: Vec<GridNode> = (0..5).map(|i| (i, i, 0)).collect();
        let simplified = simplify_path(&path);

        assert_eq!(simplified.segments.len(), 1);
        assert_eq!(simplified.segments[0].start, (0, 0, 0));
        assert_eq!(simplified.segments[0].end, (4, 4, 0));
    }

    #[test]
    fn l_shaped_path_two_segments() {
        // Horizontal then vertical: L-shape
        let mut path: Vec<GridNode> = Vec::new();
        // Horizontal: (0,5) -> (5,5)
        for x in 0..=5 {
            path.push((x, 5, 0));
        }
        // Vertical: (5,5) -> (5,10)
        for y in 6..=10 {
            path.push((5, y, 0));
        }

        let simplified = simplify_path(&path);
        assert_eq!(
            simplified.segments.len(),
            2,
            "L-shaped path should produce 2 segments"
        );
        assert_eq!(simplified.segments[0].start, (0, 5, 0));
        assert_eq!(simplified.segments[0].end, (5, 5, 0));
        assert_eq!(simplified.segments[1].start, (5, 5, 0));
        assert_eq!(simplified.segments[1].end, (5, 10, 0));
        assert!(simplified.transitions.is_empty());
    }

    #[test]
    fn path_with_via_produces_transition() {
        // Horizontal on layer 0, then via, then horizontal on layer 1
        let path: Vec<GridNode> = vec![
            (0, 5, 0),
            (1, 5, 0),
            (2, 5, 0),
            (3, 5, 0),
            (3, 5, 1), // via
            (4, 5, 1),
            (5, 5, 1),
        ];

        let simplified = simplify_path(&path);
        assert_eq!(
            simplified.segments.len(),
            2,
            "Should have 2 segments (one per layer)"
        );
        assert_eq!(
            simplified.transitions.len(),
            1,
            "Should have 1 layer transition"
        );

        let via = &simplified.transitions[0];
        assert_eq!(via.position, (3, 5, 0));
        assert_eq!(via.from_layer, 0);
        assert_eq!(via.to_layer, 1);

        // Segment before via
        assert_eq!(simplified.segments[0].start, (0, 5, 0));
        assert_eq!(simplified.segments[0].end, (3, 5, 0));
        assert_eq!(simplified.segments[0].layer, 0);

        // Segment after via
        assert_eq!(simplified.segments[1].start, (3, 5, 1));
        assert_eq!(simplified.segments[1].end, (5, 5, 1));
        assert_eq!(simplified.segments[1].layer, 1);
    }

    #[test]
    fn coordinate_conversion_accuracy() {
        let grid = make_test_grid(100, 100, 63_500, 2); // 63.5µm resolution
        let rules = TestRules::new();
        let net_id = NetId::new(1);

        // Create a path from (10,20) to (50,20) on layer 0
        let path: Vec<GridNode> = (10..=50).map(|x| (x, 20, 0)).collect();
        let simplified = simplify_path(&path);
        let (segments, _vias) = convert_to_route_segments(&simplified, &grid, net_id, &rules, None);

        assert_eq!(segments.len(), 1);
        let seg = &segments[0];

        // Verify endpoint coordinates are within 1 grid cell of expected Nm
        let expected_start_x = 10 * 63_500 + 63_500 / 2; // center of cell 10
        let expected_end_x = 50 * 63_500 + 63_500 / 2; // center of cell 50
        let expected_y = 20 * 63_500 + 63_500 / 2;

        let err_start_x = (seg.start.x.raw() - expected_start_x).abs();
        let err_end_x = (seg.end.x.raw() - expected_end_x).abs();
        let err_y = (seg.start.y.raw() - expected_y).abs();

        assert!(
            err_start_x <= grid.resolution(),
            "Start X error {} exceeds resolution {}",
            err_start_x,
            grid.resolution()
        );
        assert!(
            err_end_x <= grid.resolution(),
            "End X error {} exceeds resolution {}",
            err_end_x,
            grid.resolution()
        );
        assert!(
            err_y <= grid.resolution(),
            "Y error {} exceeds resolution {}",
            err_y,
            grid.resolution()
        );

        // Layer should be TopCopper (index 0)
        assert_eq!(seg.layer, Layer::TopCopper);

        // Width should match rules
        assert_eq!(seg.width, rules.constraints_for_net(1).min_trace_width);
    }

    #[test]
    fn via_placement_correct_drill_and_layers() {
        let grid = make_test_grid(50, 50, 100_000, 2);
        let rules = TestRules::new();
        let net_id = NetId::new(5);

        // Path with a via at (25, 25)
        let path: Vec<GridNode> = vec![
            (20, 25, 0),
            (21, 25, 0),
            (22, 25, 0),
            (23, 25, 0),
            (24, 25, 0),
            (25, 25, 0),
            (25, 25, 1), // via
            (26, 25, 1),
            (27, 25, 1),
            (28, 25, 1),
            (29, 25, 1),
            (30, 25, 1),
        ];

        let simplified = simplify_path(&path);
        let (segments, vias) = convert_to_route_segments(&simplified, &grid, net_id, &rules, None);

        assert_eq!(segments.len(), 2);
        assert_eq!(vias.len(), 1);

        let via = &vias[0];
        assert_eq!(via.net_id, net_id);
        assert_eq!(via.drill, rules.constraints_for_net(5).min_via_drill);
        assert_eq!(via.start_layer, Layer::TopCopper);
        assert_eq!(via.end_layer, Layer::BottomCopper);

        // Via position should be at grid cell (25, 25) center
        let expected_x = 25 * 100_000 + 100_000 / 2;
        let expected_y = 25 * 100_000 + 100_000 / 2;
        assert_eq!(via.position.x.raw(), expected_x);
        assert_eq!(via.position.y.raw(), expected_y);
    }

    #[test]
    fn empty_and_single_node_paths() {
        let empty: Vec<GridNode> = vec![];
        let s = simplify_path(&empty);
        assert!(s.segments.is_empty());
        assert!(s.transitions.is_empty());

        let single: Vec<GridNode> = vec![(5, 5, 0)];
        let s = simplify_path(&single);
        assert!(s.segments.is_empty());
        assert!(s.transitions.is_empty());
    }

    #[test]
    fn two_node_path() {
        let path: Vec<GridNode> = vec![(0, 0, 0), (1, 0, 0)];
        let s = simplify_path(&path);
        assert_eq!(s.segments.len(), 1);
        assert_eq!(s.segments[0].start, (0, 0, 0));
        assert_eq!(s.segments[0].end, (1, 0, 0));
    }

    #[test]
    fn all_segments_get_correct_width() {
        let grid = make_test_grid(30, 30, 100_000, 2);
        let rules = TestRules::new();
        let net_id = NetId::new(3);
        let expected_width = rules.constraints_for_net(3).min_trace_width;

        // Z-shaped path: horizontal + diagonal + horizontal
        let mut path: Vec<GridNode> = Vec::new();
        for x in 0..=5 {
            path.push((x, 5, 0));
        }
        for i in 1..=5 {
            path.push((5 + i, 5 + i, 0));
        }
        for x in 11..=15 {
            path.push((x, 10, 0));
        }

        let simplified = simplify_path(&path);
        let (segments, _) = convert_to_route_segments(&simplified, &grid, net_id, &rules, None);

        assert_eq!(segments.len(), 3, "Z-shaped path should produce 3 segments");
        for seg in &segments {
            assert_eq!(
                seg.width, expected_width,
                "Every segment must have the rule-defined trace width"
            );
            assert_eq!(seg.net_id, net_id);
        }
    }
}
