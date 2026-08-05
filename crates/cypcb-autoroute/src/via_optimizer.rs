//! Via optimizer: eliminates redundant via pairs where single-layer routing is DRC-clean.
//!
//! Scans for via pairs (down-via at A, up-via at B) with a single segment
//! between them on an alternate layer. If a direct segment on the original
//! layer from A→B is DRC-clean, both vias are eliminated and replaced
//! with a direct segment.

use cypcb_core::Nm;
use cypcb_drc::rules::clearance::segment_distance;
use cypcb_router::types::{RouteSegment, ViaPlacement};
use cypcb_world::NetId;

/// Optimize vias by eliminating redundant via pairs.
///
/// For each net, finds via pairs where:
/// 1. Via A transitions from layer L1 to L2 at point A
/// 2. Via B transitions from layer L2 back to L1 at point B
/// 3. A single segment exists between A and B on layer L2
///
/// If a direct segment from A→B on L1 is DRC-clean (no clearance violations
/// against other-net segments), both vias are removed and the intermediate
/// segment is replaced with a direct segment on L1.
///
/// # Arguments
/// * `segments` - All route segments
/// * `vias` - All via placements
/// * `other_net_segments` - Segments from other nets for DRC checking
/// * `min_clearance` - Minimum clearance distance
///
/// # Returns
/// Tuple of (optimized segments, optimized vias)
pub fn optimize_vias(
    segments: Vec<RouteSegment>,
    vias: Vec<ViaPlacement>,
    other_net_segments: &[RouteSegment],
    min_clearance: Nm,
) -> (Vec<RouteSegment>, Vec<ViaPlacement>) {
    if vias.len() < 2 {
        return (segments, vias);
    }

    // Group vias by net_id
    let mut net_ids: Vec<NetId> = vias.iter().map(|v| v.net_id).collect();
    net_ids.sort_by_key(|n| n.id());
    net_ids.dedup();

    let mut kept_segments = segments.clone();
    let mut kept_vias = vias.clone();
    let mut removed_via_indices: Vec<usize> = Vec::new();
    let mut removed_seg_indices: Vec<usize> = Vec::new();
    let mut added_segments: Vec<RouteSegment> = Vec::new();

    for net_id in &net_ids {
        // Collect vias for this net
        let net_vias: Vec<(usize, &ViaPlacement)> = kept_vias
            .iter()
            .enumerate()
            .filter(|(_, v)| v.net_id == *net_id)
            .collect();

        // Try to find eliminable via pairs
        for i in 0..net_vias.len() {
            for j in (i + 1)..net_vias.len() {
                let (idx_a, via_a) = net_vias[i];
                let (idx_b, via_b) = net_vias[j];

                // Check if already marked for removal
                if removed_via_indices.contains(&idx_a) || removed_via_indices.contains(&idx_b) {
                    continue;
                }

                // Check if they form a complementary pair:
                // via_a goes L1→L2, via_b goes L2→L1 (or vice versa)
                let is_complementary =
                    via_a.start_layer == via_b.end_layer && via_a.end_layer == via_b.start_layer;

                if !is_complementary {
                    continue;
                }

                let original_layer = via_a.start_layer;
                let alternate_layer = via_a.end_layer;

                // Find the segment between via_a and via_b on the alternate layer
                let between_seg_idx = kept_segments.iter().enumerate().position(|(si, s)| {
                    !removed_seg_indices.contains(&si)
                        && s.net_id == *net_id
                        && s.layer == alternate_layer
                        && ((s.start == via_a.position && s.end == via_b.position)
                            || (s.start == via_b.position && s.end == via_a.position))
                });

                let between_seg_idx = match between_seg_idx {
                    Some(idx) => idx,
                    None => continue,
                };

                // Check if a direct segment on the original layer is DRC-clean
                let direct_start = via_a.position;
                let direct_end = via_b.position;

                // Filter other-net segments to the original layer
                let p1 = [direct_start.x.0, direct_start.y.0];
                let p2 = [direct_end.x.0, direct_end.y.0];

                let drc_ok = other_net_segments
                    .iter()
                    .filter(|s| s.layer == original_layer)
                    .all(|other| {
                        let p3 = [other.start.x.0, other.start.y.0];
                        let p4 = [other.end.x.0, other.end.y.0];
                        let dist = segment_distance(p1, p2, p3, p4);
                        dist >= min_clearance.0
                    });

                if drc_ok {
                    tracing::info!(
                        net_id = net_id.id(),
                        from = ?format!("({},{})", direct_start.x.0, direct_start.y.0),
                        to = ?format!("({},{})", direct_end.x.0, direct_end.y.0),
                        "eliminated via pair — direct single-layer path is DRC-clean"
                    );

                    removed_via_indices.push(idx_a);
                    removed_via_indices.push(idx_b);
                    removed_seg_indices.push(between_seg_idx);

                    // Add direct segment on original layer
                    let width = kept_segments[between_seg_idx].width;
                    added_segments.push(RouteSegment::new(
                        *net_id,
                        original_layer,
                        width,
                        direct_start,
                        direct_end,
                    ));
                }
            }
        }
    }

    // Apply removals (in reverse order to preserve indices)
    removed_via_indices.sort_unstable();
    removed_via_indices.dedup();
    for idx in removed_via_indices.iter().rev() {
        kept_vias.remove(*idx);
    }

    removed_seg_indices.sort_unstable();
    removed_seg_indices.dedup();
    for idx in removed_seg_indices.iter().rev() {
        kept_segments.remove(*idx);
    }

    kept_segments.extend(added_segments);

    (kept_segments, kept_vias)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::Point;
    use cypcb_world::Layer;

    fn make_seg(net_id: u32, layer: Layer, x1: f64, y1: f64, x2: f64, y2: f64) -> RouteSegment {
        RouteSegment::new(
            NetId::new(net_id),
            layer,
            Nm::from_mm(0.2),
            Point::from_mm(x1, y1),
            Point::from_mm(x2, y2),
        )
    }

    fn make_via(net_id: u32, x: f64, y: f64, start: Layer, end: Layer) -> ViaPlacement {
        ViaPlacement::new(
            NetId::new(net_id),
            Point::from_mm(x, y),
            Nm::from_mm(0.3),
            start,
            end,
        )
    }

    #[test]
    fn via_pair_eliminated_when_drc_clean() {
        // Net 1: seg on top → via down at (5,0) → seg on bottom → via up at (10,0) → seg on top
        let segments = vec![
            make_seg(1, Layer::TopCopper, 0.0, 0.0, 5.0, 0.0),
            make_seg(1, Layer::BottomCopper, 5.0, 0.0, 10.0, 0.0), // between vias
            make_seg(1, Layer::TopCopper, 10.0, 0.0, 15.0, 0.0),
        ];
        let vias = vec![
            make_via(1, 5.0, 0.0, Layer::TopCopper, Layer::BottomCopper),
            make_via(1, 10.0, 0.0, Layer::BottomCopper, Layer::TopCopper),
        ];

        let (opt_segs, opt_vias) = optimize_vias(segments, vias, &[], Nm(0));

        assert_eq!(opt_vias.len(), 0, "both vias should be eliminated");
        // Should have: original top seg + direct top seg replacing bottom + original top seg
        assert!(
            opt_segs.len() >= 3,
            "should have at least 3 segments after optimization"
        );
        // All segments should be on top layer
        for s in &opt_segs {
            assert_eq!(
                s.layer,
                Layer::TopCopper,
                "all segments should be on top layer"
            );
        }
    }

    #[test]
    fn via_pair_not_eliminated_when_drc_blocked() {
        let segments = vec![
            make_seg(1, Layer::TopCopper, 0.0, 0.0, 5.0, 0.0),
            make_seg(1, Layer::BottomCopper, 5.0, 0.0, 10.0, 0.0),
            make_seg(1, Layer::TopCopper, 10.0, 0.0, 15.0, 0.0),
        ];
        let vias = vec![
            make_via(1, 5.0, 0.0, Layer::TopCopper, Layer::BottomCopper),
            make_via(1, 10.0, 0.0, Layer::BottomCopper, Layer::TopCopper),
        ];

        // Place an obstacle on the top layer right in the path
        let obstacle = make_seg(99, Layer::TopCopper, 7.0, -0.05, 8.0, 0.05);

        let (opt_segs, opt_vias) = optimize_vias(segments, vias, &[obstacle], Nm::from_mm(0.15));

        assert_eq!(
            opt_vias.len(),
            2,
            "vias should be kept when DRC blocks direct path"
        );
        assert!(
            opt_segs.iter().any(|s| s.layer == Layer::BottomCopper),
            "bottom layer segment should be preserved"
        );
    }

    #[test]
    fn no_vias_no_change() {
        let segments = vec![make_seg(1, Layer::TopCopper, 0.0, 0.0, 10.0, 0.0)];
        let vias: Vec<ViaPlacement> = vec![];

        let (opt_segs, opt_vias) = optimize_vias(segments.clone(), vias, &[], Nm(0));

        assert_eq!(opt_segs.len(), 1);
        assert!(opt_vias.is_empty());
    }

    #[test]
    fn single_via_no_pair() {
        let segments = vec![
            make_seg(1, Layer::TopCopper, 0.0, 0.0, 5.0, 0.0),
            make_seg(1, Layer::BottomCopper, 5.0, 0.0, 10.0, 0.0),
        ];
        let vias = vec![make_via(1, 5.0, 0.0, Layer::TopCopper, Layer::BottomCopper)];

        let (opt_segs, opt_vias) = optimize_vias(segments, vias, &[], Nm(0));

        assert_eq!(opt_vias.len(), 1, "single via cannot form a pair");
        assert_eq!(opt_segs.len(), 2);
    }

    #[test]
    fn via_optimization_preserves_net_id() {
        let net_id = NetId::new(7);
        let segments = vec![
            RouteSegment::new(
                net_id,
                Layer::TopCopper,
                Nm::from_mm(0.25),
                Point::from_mm(0.0, 0.0),
                Point::from_mm(5.0, 0.0),
            ),
            RouteSegment::new(
                net_id,
                Layer::BottomCopper,
                Nm::from_mm(0.25),
                Point::from_mm(5.0, 0.0),
                Point::from_mm(10.0, 0.0),
            ),
            RouteSegment::new(
                net_id,
                Layer::TopCopper,
                Nm::from_mm(0.25),
                Point::from_mm(10.0, 0.0),
                Point::from_mm(15.0, 0.0),
            ),
        ];
        let vias = vec![
            ViaPlacement::new(
                net_id,
                Point::from_mm(5.0, 0.0),
                Nm::from_mm(0.3),
                Layer::TopCopper,
                Layer::BottomCopper,
            ),
            ViaPlacement::new(
                net_id,
                Point::from_mm(10.0, 0.0),
                Nm::from_mm(0.3),
                Layer::BottomCopper,
                Layer::TopCopper,
            ),
        ];

        let (opt_segs, _) = optimize_vias(segments, vias, &[], Nm(0));
        for s in &opt_segs {
            assert_eq!(
                s.net_id, net_id,
                "net_id must be preserved after via optimization"
            );
        }
    }
}
