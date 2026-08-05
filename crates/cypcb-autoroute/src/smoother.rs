//! Trace smoother: converts raw grid-aligned autorouter output into clean 45°/90° traces.
//!
//! Operates on `Vec<RouteSegment>` (Nm coordinates) grouped by (net_id, layer),
//! applying three passes:
//! 1. Staircase-to-diagonal collapse
//! 2. Corner chamfering (90° → 45° bends)
//! 3. Collinear segment merge
//!
//! Each move is validated against other-net segments using `segment_distance()`
//! to ensure DRC safety.

use cypcb_core::{Nm, Point};
use cypcb_drc::rules::clearance::segment_distance;
use cypcb_router::types::RouteSegment;
use cypcb_world::{Layer, NetId};

/// Check that a segment angle is a multiple of 45°.
///
/// Uses exact integer direction patterns (dx==0, dy==0, |dx|==|dy|) to avoid
/// floating-point ambiguity. Returns true for 0°, 45°, 90°, 135° and their
/// reflections/negations. Also returns true for zero-length segments.
pub fn is_valid_angle(start: Point, end: Point) -> bool {
    let dx = (end.x.0 - start.x.0).abs();
    let dy = (end.y.0 - start.y.0).abs();

    // Zero-length segment is valid
    if dx == 0 && dy == 0 {
        return true;
    }

    // 0° or 90° (horizontal or vertical)
    if dx == 0 || dy == 0 {
        return true;
    }

    // 45° diagonal: |dx| == |dy|
    dx == dy
}

/// Smooth all route segments, grouping by (net_id, layer) and smoothing each group independently.
///
/// # Arguments
/// * `segments` - All route segments for one or more nets on one or more layers
/// * `other_net_segments` - Segments from other nets (used for DRC clearance checking)
/// * `min_clearance` - Minimum clearance distance in nanometers
///
/// # Returns
/// A new vector of smoothed segments with only 0°/45°/90°/135° angles.
pub fn smooth_routes(
    segments: &[RouteSegment],
    other_net_segments: &[RouteSegment],
    min_clearance: Nm,
    roundness: f64,
) -> Vec<RouteSegment> {
    if segments.is_empty() {
        return Vec::new();
    }

    // Group segments by (net_id, layer)
    let mut groups: Vec<(NetId, Layer, Vec<&RouteSegment>)> = Vec::new();

    for seg in segments {
        let key = (seg.net_id, seg.layer);
        if let Some(group) = groups
            .iter_mut()
            .find(|(n, l, _)| *n == key.0 && *l == key.1)
        {
            group.2.push(seg);
        } else {
            groups.push((key.0, key.1, vec![seg]));
        }
    }

    let before_count = segments.len();
    let mut result = Vec::new();

    for (net_id, layer, group) in &groups {
        let group_segs: Vec<RouteSegment> = group.iter().map(|s| (*s).clone()).collect();
        let smoothed =
            smooth_net_layer_group(&group_segs, other_net_segments, min_clearance, roundness);
        tracing::debug!(
            net_id = net_id.id(),
            layer = ?layer,
            before = group_segs.len(),
            after = smoothed.len(),
            "smoothed net-layer group"
        );
        result.extend(smoothed);
    }

    tracing::info!(
        before = before_count,
        after = result.len(),
        "smooth_routes complete"
    );

    result
}

/// Smooth a group of segments belonging to the same (net_id, layer).
///
/// Applies three passes in order:
/// 1. Staircase-to-diagonal collapse
/// 2. Corner chamfering
/// 3. Collinear segment merge
fn smooth_net_layer_group(
    group: &[RouteSegment],
    others: &[RouteSegment],
    min_clearance: Nm,
    roundness: f64,
) -> Vec<RouteSegment> {
    if group.is_empty() {
        return Vec::new();
    }

    let net_id = group[0].net_id;
    let layer = group[0].layer;
    let width = group[0].width;

    // Filter other-net segments to same layer for DRC checking
    let same_layer_others: Vec<&RouteSegment> =
        others.iter().filter(|s| s.layer == layer).collect();

    // Pass 1: Staircase-to-diagonal collapse
    let after_staircase = collapse_staircases(group, &same_layer_others, min_clearance);

    // Pass 2: Corner chamfering
    let after_chamfer = chamfer_corners(
        &after_staircase,
        &same_layer_others,
        min_clearance,
        roundness,
    );

    // Pass 3: Collinear segment merge
    let after_merge = merge_collinear(&after_chamfer);

    // Validate all output angles
    for seg in &after_merge {
        debug_assert!(
            is_valid_angle(seg.start, seg.end),
            "smoother produced invalid angle: {:?} -> {:?}",
            seg.start,
            seg.end
        );
        debug_assert_eq!(seg.net_id, net_id, "net_id must be preserved");
        debug_assert_eq!(seg.layer, layer, "layer must be preserved");
        debug_assert_eq!(seg.width, width, "width must be preserved");
    }

    after_merge
}

/// Direction classification for segment analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Horizontal,
    Vertical,
    Diagonal,
    Other,
}

fn classify_direction(start: Point, end: Point) -> Direction {
    let dx = end.x.0 - start.x.0;
    let dy = end.y.0 - start.y.0;
    if dx == 0 && dy == 0 {
        return Direction::Other;
    }
    if dy == 0 {
        Direction::Horizontal
    } else if dx == 0 {
        Direction::Vertical
    } else if dx.abs() == dy.abs() {
        Direction::Diagonal
    } else {
        Direction::Other
    }
}

/// Check if a proposed segment is DRC-clean against other-net segments.
fn is_drc_clean(
    new_start: Point,
    new_end: Point,
    others: &[&RouteSegment],
    min_clearance: Nm,
) -> bool {
    let p1 = [new_start.x.0, new_start.y.0];
    let p2 = [new_end.x.0, new_end.y.0];

    for other in others {
        let p3 = [other.start.x.0, other.start.y.0];
        let p4 = [other.end.x.0, other.end.y.0];
        let dist = segment_distance(p1, p2, p3, p4);
        if dist < min_clearance.0 {
            tracing::debug!(
                clearance = dist,
                min_clearance = min_clearance.0,
                new_seg = ?format!("({},{})→({},{})", new_start.x.0, new_start.y.0, new_end.x.0, new_end.y.0),
                other_seg = ?format!("({},{})→({},{})", other.start.x.0, other.start.y.0, other.end.x.0, other.end.y.0),
                "DRC rejection: smoothing move too close to other-net segment"
            );
            return false;
        }
    }
    true
}

/// Pass 1: Collapse staircase patterns into diagonal + orthogonal segments.
///
/// Detects alternating H-V or V-H sequences and replaces them with
/// a single 45° diagonal plus (optionally) an orthogonal tail.
fn collapse_staircases(
    segments: &[RouteSegment],
    others: &[&RouteSegment],
    min_clearance: Nm,
) -> Vec<RouteSegment> {
    if segments.len() < 2 {
        return segments.to_vec();
    }

    let net_id = segments[0].net_id;
    let layer = segments[0].layer;
    let width = segments[0].width;

    let mut result: Vec<RouteSegment> = Vec::new();
    let mut i = 0;

    while i < segments.len() {
        // Try to find a staircase starting at i
        // A staircase is ≥2 segments alternating between H and V directions
        let first_dir = classify_direction(segments[i].start, segments[i].end);

        if first_dir != Direction::Horizontal && first_dir != Direction::Vertical {
            result.push(segments[i].clone());
            i += 1;
            continue;
        }

        // Scan forward for alternating H/V pattern with connected endpoints
        let mut stair_end = i;
        let mut prev_dir = first_dir;

        for j in (i + 1)..segments.len() {
            let dir = classify_direction(segments[j].start, segments[j].end);
            let connected = segments[j].start == segments[j - 1].end;

            if !connected {
                break;
            }

            let alternates = (prev_dir == Direction::Horizontal && dir == Direction::Vertical)
                || (prev_dir == Direction::Vertical && dir == Direction::Horizontal);

            if alternates {
                stair_end = j;
                prev_dir = dir;
            } else {
                break;
            }
        }

        let stair_len = stair_end - i + 1;
        if stair_len < 3 {
            // Not enough segments for a staircase (need at least 3 alternating)
            result.push(segments[i].clone());
            i += 1;
            continue;
        }

        // We have a staircase from segments[i..=stair_end]
        let stair_start = segments[i].start;
        let stair_finish = segments[stair_end].end;

        let dx = stair_finish.x.0 - stair_start.x.0;
        let dy = stair_finish.y.0 - stair_start.y.0;

        let abs_dx = dx.abs();
        let abs_dy = dy.abs();

        // The diagonal covers min(|dx|, |dy|) in both axes
        let diag_len = abs_dx.min(abs_dy);

        if diag_len == 0 {
            // Degenerate — just keep originals
            result.extend_from_slice(&segments[i..=stair_end]);
            i = stair_end + 1;
            continue;
        }

        let diag_dx = if dx > 0 { diag_len } else { -diag_len };
        let diag_dy = if dy > 0 { diag_len } else { -diag_len };

        let diag_end = Point::new(Nm(stair_start.x.0 + diag_dx), Nm(stair_start.y.0 + diag_dy));

        // Verify the diagonal is a valid 45° angle
        debug_assert!(is_valid_angle(stair_start, diag_end));

        // Check DRC safety of the diagonal
        if is_drc_clean(stair_start, diag_end, others, min_clearance) {
            // Emit the diagonal
            result.push(RouteSegment::new(
                net_id,
                layer,
                width,
                stair_start,
                diag_end,
            ));

            // If there's remaining distance, emit an orthogonal segment
            if diag_end != stair_finish {
                debug_assert!(is_valid_angle(diag_end, stair_finish));
                if is_drc_clean(diag_end, stair_finish, others, min_clearance) {
                    result.push(RouteSegment::new(
                        net_id,
                        layer,
                        width,
                        diag_end,
                        stair_finish,
                    ));
                } else {
                    // Orthogonal tail not safe — keep original staircase
                    result.pop(); // remove the diagonal we just added
                    result.extend_from_slice(&segments[i..=stair_end]);
                }
            }
        } else {
            // Diagonal not DRC-safe — keep original staircase
            result.extend_from_slice(&segments[i..=stair_end]);
        }

        i = stair_end + 1;
    }

    result
}

/// Pass 2: Chamfer 90° corners with 45° bevel segments.
///
/// For each 90° bend, inserts a short 45° chamfer. The chamfer length
/// is scaled by `roundness` (0.0–1.0): at 0.0, no chamfering occurs;
/// at 1.0, chamfer uses the full `min(len_A, len_B) / 3`, capped by max_chamfer.
fn chamfer_corners(
    segments: &[RouteSegment],
    others: &[&RouteSegment],
    min_clearance: Nm,
    roundness: f64,
) -> Vec<RouteSegment> {
    if segments.len() < 2 {
        return segments.to_vec();
    }

    let net_id = segments[0].net_id;
    let layer = segments[0].layer;
    let width = segments[0].width;

    let max_chamfer = (Nm::from_mm(1.0).0 as f64 * roundness.clamp(0.0, 1.0)) as i64; // roundness scales cap

    let mut result: Vec<RouteSegment> = Vec::new();
    let mut i = 0;

    while i < segments.len() {
        if i + 1 >= segments.len() {
            result.push(segments[i].clone());
            i += 1;
            continue;
        }

        let seg_a = &segments[i];
        let seg_b = &segments[i + 1];

        // Check if they're connected and form a 90° bend
        if seg_a.end != seg_b.start {
            result.push(seg_a.clone());
            i += 1;
            continue;
        }

        let dir_a = classify_direction(seg_a.start, seg_a.end);
        let dir_b = classify_direction(seg_b.start, seg_b.end);

        let is_90_bend = (dir_a == Direction::Horizontal && dir_b == Direction::Vertical)
            || (dir_a == Direction::Vertical && dir_b == Direction::Horizontal);

        if !is_90_bend {
            result.push(seg_a.clone());
            i += 1;
            continue;
        }

        let len_a = seg_a.length().0;
        let len_b = seg_b.length().0;

        // Chamfer length scaled by roundness: roundness=0 → no chamfer, roundness=1 → full chamfer
        let chamfer_len =
            ((len_a.min(len_b) as f64 * roundness.clamp(0.0, 1.0) / 3.0) as i64).min(max_chamfer);

        if chamfer_len < 1000 {
            // Too short to chamfer (< 1µm) — skip
            result.push(seg_a.clone());
            i += 1;
            continue;
        }

        // Compute chamfer points
        // Shorten seg_a: move its endpoint back by chamfer_len
        let dx_a = seg_a.end.x.0 - seg_a.start.x.0;
        let dy_a = seg_a.end.y.0 - seg_a.start.y.0;
        let a_len_actual = ((dx_a as f64).powi(2) + (dy_a as f64).powi(2)).sqrt();
        if a_len_actual < 1.0 {
            result.push(seg_a.clone());
            i += 1;
            continue;
        }
        let ratio_a = (a_len_actual - chamfer_len as f64) / a_len_actual;
        let chamfer_start = Point::new(
            Nm(seg_a.start.x.0 + (dx_a as f64 * ratio_a).round() as i64),
            Nm(seg_a.start.y.0 + (dy_a as f64 * ratio_a).round() as i64),
        );

        // Advance seg_b start by chamfer_len
        let dx_b = seg_b.end.x.0 - seg_b.start.x.0;
        let dy_b = seg_b.end.y.0 - seg_b.start.y.0;
        let b_len_actual = ((dx_b as f64).powi(2) + (dy_b as f64).powi(2)).sqrt();
        if b_len_actual < 1.0 {
            result.push(seg_a.clone());
            i += 1;
            continue;
        }
        let ratio_b = chamfer_len as f64 / b_len_actual;
        let chamfer_end = Point::new(
            Nm(seg_b.start.x.0 + (dx_b as f64 * ratio_b).round() as i64),
            Nm(seg_b.start.y.0 + (dy_b as f64 * ratio_b).round() as i64),
        );

        // For H+V or V+H, the chamfer is a 45° segment.
        // Since seg_a is H and seg_b is V (or vice versa), moving chamfer_len
        // back on A (in its direction) and forward on B (in its direction)
        // produces a diagonal connecting them. For H→V: chamfer_start is
        // (corner.x - chamfer_len, corner.y) and chamfer_end is
        // (corner.x, corner.y + chamfer_len), so the diagonal has |dx|=|dy|=chamfer_len → 45°.

        // Verify the chamfer is a valid 45° angle
        if !is_valid_angle(chamfer_start, chamfer_end) {
            // Rounding made it invalid — skip chamfering this corner
            result.push(seg_a.clone());
            i += 1;
            continue;
        }

        // Check DRC safety of the chamfer segment
        if !is_drc_clean(chamfer_start, chamfer_end, others, min_clearance) {
            result.push(seg_a.clone());
            i += 1;
            continue;
        }

        // Emit shortened seg_a + chamfer + shortened seg_b
        if chamfer_start != seg_a.start {
            result.push(RouteSegment::new(
                net_id,
                layer,
                width,
                seg_a.start,
                chamfer_start,
            ));
        }
        result.push(RouteSegment::new(
            net_id,
            layer,
            width,
            chamfer_start,
            chamfer_end,
        ));
        if chamfer_end != seg_b.end {
            result.push(RouteSegment::new(
                net_id,
                layer,
                width,
                chamfer_end,
                seg_b.end,
            ));
        }

        // Skip seg_b since we've consumed it
        i += 2;
    }

    result
}

/// Pass 3: Merge consecutive collinear segments into single segments.
fn merge_collinear(segments: &[RouteSegment]) -> Vec<RouteSegment> {
    if segments.is_empty() {
        return Vec::new();
    }

    let mut result: Vec<RouteSegment> = Vec::new();
    result.push(segments[0].clone());

    for seg in &segments[1..] {
        let last = result.last().unwrap();

        // Check if connected and collinear
        if last.end == seg.start && are_collinear(last.start, last.end, seg.end) {
            // Extend the last segment
            let extended =
                RouteSegment::new(last.net_id, last.layer, last.width, last.start, seg.end);
            *result.last_mut().unwrap() = extended;
        } else {
            result.push(seg.clone());
        }
    }

    result
}

/// Check if three points are collinear (same direction vector).
fn are_collinear(a: Point, b: Point, c: Point) -> bool {
    let dx1 = b.x.0 - a.x.0;
    let dy1 = b.y.0 - a.y.0;
    let dx2 = c.x.0 - b.x.0;
    let dy2 = c.y.0 - b.y.0;

    // Zero-length first segment — treat as collinear
    if dx1 == 0 && dy1 == 0 {
        return true;
    }
    // Zero-length second segment — treat as collinear
    if dx2 == 0 && dy2 == 0 {
        return true;
    }

    // Same direction: signs must match and cross product must be zero
    let same_sign_x = dx1.signum() == dx2.signum();
    let same_sign_y = dy1.signum() == dy2.signum();

    if !same_sign_x || !same_sign_y {
        return false;
    }

    // Cross product (exact, using i128 to prevent overflow)
    let cross = (dx1 as i128) * (dy2 as i128) - (dy1 as i128) * (dx2 as i128);
    cross == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(x1: i64, y1: i64, x2: i64, y2: i64) -> RouteSegment {
        RouteSegment::new(
            NetId::new(1),
            Layer::TopCopper,
            Nm::from_mm(0.2),
            Point::new(Nm(x1), Nm(y1)),
            Point::new(Nm(x2), Nm(y2)),
        )
    }

    fn seg_mm(x1: f64, y1: f64, x2: f64, y2: f64) -> RouteSegment {
        RouteSegment::new(
            NetId::new(1),
            Layer::TopCopper,
            Nm::from_mm(0.2),
            Point::from_mm(x1, y1),
            Point::from_mm(x2, y2),
        )
    }

    // ====== Angle validation tests ======

    #[test]
    fn angle_valid_horizontal() {
        assert!(is_valid_angle(
            Point::from_mm(0.0, 0.0),
            Point::from_mm(5.0, 0.0)
        ));
    }

    #[test]
    fn angle_valid_vertical() {
        assert!(is_valid_angle(
            Point::from_mm(0.0, 0.0),
            Point::from_mm(0.0, 3.0)
        ));
    }

    #[test]
    fn angle_valid_45_diagonal() {
        assert!(is_valid_angle(
            Point::from_mm(0.0, 0.0),
            Point::from_mm(3.0, 3.0)
        ));
        assert!(is_valid_angle(
            Point::from_mm(0.0, 0.0),
            Point::from_mm(-3.0, 3.0)
        ));
    }

    #[test]
    fn angle_valid_zero_length() {
        assert!(is_valid_angle(
            Point::from_mm(1.0, 1.0),
            Point::from_mm(1.0, 1.0)
        ));
    }

    #[test]
    fn angle_invalid_arbitrary() {
        // 30° angle — not a multiple of 45°
        assert!(!is_valid_angle(
            Point::from_mm(0.0, 0.0),
            Point::from_mm(3.0, 1.0)
        ));
    }

    // ====== Staircase collapse tests ======

    #[test]
    fn staircase_collapse_3_step() {
        // 3 alternating H/V segments forming a staircase
        let step = 1_000_000; // 1mm
        let segments = vec![
            seg(0, 0, step, 0),              // H
            seg(step, 0, step, step),        // V
            seg(step, step, 2 * step, step), // H
        ];
        let result = smooth_routes(&segments, &[], Nm(0), 0.5);
        // Should collapse to 1 diagonal + maybe 1 orthogonal
        assert!(
            result.len() <= 2,
            "3-step staircase should collapse to ≤2 segments, got {}",
            result.len()
        );
        // Verify all angles are valid
        for s in &result {
            assert!(is_valid_angle(s.start, s.end), "invalid angle in output");
        }
        // Endpoints preserved
        assert_eq!(result.first().unwrap().start, Point::new(Nm(0), Nm(0)));
        assert_eq!(
            result.last().unwrap().end,
            Point::new(Nm(2 * step), Nm(step))
        );
    }

    #[test]
    fn staircase_collapse_5_step() {
        // 5 alternating H/V segments
        let s = 500_000; // 0.5mm
        let segments = vec![
            seg(0, 0, s, 0),
            seg(s, 0, s, s),
            seg(s, s, 2 * s, s),
            seg(2 * s, s, 2 * s, 2 * s),
            seg(2 * s, 2 * s, 3 * s, 2 * s),
        ];
        let result = smooth_routes(&segments, &[], Nm(0), 0.5);
        assert!(
            result.len() <= 3,
            "5-step staircase should collapse to ≤3 segments, got {}",
            result.len()
        );
        assert_eq!(result.first().unwrap().start, Point::new(Nm(0), Nm(0)));
        assert_eq!(result.last().unwrap().end, Point::new(Nm(3 * s), Nm(2 * s)));
        for s in &result {
            assert!(is_valid_angle(s.start, s.end));
        }
    }

    #[test]
    fn staircase_collapse_10_step_produces_few_segments() {
        // 10 alternating H/V steps — should produce ≤3 output segments
        let s = 100_000; // 0.1mm
        let mut segments = Vec::new();
        for i in 0..10 {
            if i % 2 == 0 {
                // Horizontal step
                let x = (i / 2) as i64 * s;
                let y = (i / 2) as i64 * s;
                segments.push(seg(x, y, x + s, y));
            } else {
                // Vertical step
                let x = ((i / 2) + 1) as i64 * s;
                let y = (i / 2) as i64 * s;
                segments.push(seg(x, y, x, y + s));
            }
        }
        let result = smooth_routes(&segments, &[], Nm(0), 0.5);
        assert!(
            result.len() <= 3,
            "10-step staircase should produce ≤3 segments, got {}",
            result.len()
        );
    }

    #[test]
    fn staircase_irregular_not_alternating() {
        // Two horizontal segments followed by vertical — not a proper staircase
        let s = 1_000_000;
        let segments = vec![
            seg(0, 0, s, 0),
            seg(s, 0, 2 * s, 0), // same direction as previous — not alternating
            seg(2 * s, 0, 2 * s, s),
        ];
        let result = smooth_routes(&segments, &[], Nm(0), 0.5);
        // Should still be valid but may not collapse as aggressively
        for s in &result {
            assert!(is_valid_angle(s.start, s.end));
        }
    }

    // ====== Corner chamfer tests ======

    #[test]
    fn chamfer_90_degree_bend() {
        // An L-shaped path: horizontal then vertical
        let segments = vec![
            seg_mm(0.0, 0.0, 6.0, 0.0), // 6mm horizontal
            seg_mm(6.0, 0.0, 6.0, 6.0), // 6mm vertical
        ];
        let result = smooth_routes(&segments, &[], Nm(0), 0.5);
        // Should have 3 segments after chamfering: shortened H + 45° chamfer + shortened V
        assert_eq!(
            result.len(),
            3,
            "90° bend should be chamfered into 3 segments"
        );
        // Middle segment should be diagonal
        assert!(is_valid_angle(result[1].start, result[1].end));
        let dx = (result[1].end.x.0 - result[1].start.x.0).abs();
        let dy = (result[1].end.y.0 - result[1].start.y.0).abs();
        assert_eq!(dx, dy, "chamfer should be 45° diagonal");
    }

    #[test]
    fn chamfer_already_45_no_op() {
        // A segment that's already 45° — should not be modified
        let segments = vec![
            seg_mm(0.0, 0.0, 3.0, 3.0), // 45° diagonal
        ];
        let result = smooth_routes(&segments, &[], Nm(0), 0.5);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start, segments[0].start);
        assert_eq!(result[0].end, segments[0].end);
    }

    // ====== Collinear merge tests ======

    #[test]
    fn merge_collinear_segments() {
        // Two horizontal segments end-to-end in the same direction
        let segments = vec![seg_mm(0.0, 0.0, 3.0, 0.0), seg_mm(3.0, 0.0, 7.0, 0.0)];
        let result = smooth_routes(&segments, &[], Nm(0), 0.5);
        assert_eq!(result.len(), 1, "collinear segments should merge");
        assert_eq!(result[0].start, Point::from_mm(0.0, 0.0));
        assert_eq!(result[0].end, Point::from_mm(7.0, 0.0));
    }

    // ====== DRC rejection tests ======

    #[test]
    fn drc_rejection_staircase_blocked() {
        // A staircase that would violate clearance if collapsed
        let s = 1_000_000; // 1mm steps
        let staircase = vec![seg(0, 0, s, 0), seg(s, 0, s, s), seg(s, s, 2 * s, s)];

        // Place an obstacle segment right where the diagonal would go
        let obstacle = RouteSegment::new(
            NetId::new(99), // different net
            Layer::TopCopper,
            Nm::from_mm(0.2),
            Point::new(Nm(s / 2), Nm(s / 2 - 50_000)), // very close to diagonal path
            Point::new(Nm(s / 2), Nm(s / 2 + 50_000)),
        );

        let result = smooth_routes(&staircase, &[obstacle], Nm(200_000), 0.5); // 0.2mm clearance
                                                                               // Should keep original staircase since diagonal violates clearance
        assert!(
            result.len() >= 3,
            "should keep original staircase when diagonal violates DRC, got {} segments",
            result.len()
        );
    }

    // ====== Edge cases ======

    #[test]
    fn empty_input() {
        let result = smooth_routes(&[], &[], Nm(0), 0.5);
        assert!(result.is_empty());
    }

    #[test]
    fn single_segment() {
        let segments = vec![seg_mm(0.0, 0.0, 5.0, 0.0)];
        let result = smooth_routes(&segments, &[], Nm(0), 0.5);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start, segments[0].start);
        assert_eq!(result[0].end, segments[0].end);
    }

    #[test]
    fn zero_length_segment() {
        let segments = vec![seg_mm(1.0, 1.0, 1.0, 1.0)];
        let result = smooth_routes(&segments, &[], Nm(0), 0.5);
        // Zero-length segment passes through (valid angle)
        assert_eq!(result.len(), 1);
    }

    // ====== Preservation tests ======

    #[test]
    fn net_id_layer_width_preserved() {
        let net = NetId::new(42);
        let layer = Layer::BottomCopper;
        let width = Nm::from_mm(0.3);
        let segments = vec![
            RouteSegment::new(
                net,
                layer,
                width,
                Point::from_mm(0.0, 0.0),
                Point::from_mm(3.0, 0.0),
            ),
            RouteSegment::new(
                net,
                layer,
                width,
                Point::from_mm(3.0, 0.0),
                Point::from_mm(3.0, 3.0),
            ),
        ];
        let result = smooth_routes(&segments, &[], Nm(0), 0.5);
        for s in &result {
            assert_eq!(s.net_id, net, "net_id must be preserved");
            assert_eq!(s.layer, layer, "layer must be preserved");
            assert_eq!(s.width, width, "width must be preserved");
        }
    }
}
