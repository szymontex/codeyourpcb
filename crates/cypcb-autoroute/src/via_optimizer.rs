//! Via optimizer: eliminates redundant via pairs where single-layer routing is DRC-clean.
//!
//! Scans for via pairs (down-via at A, up-via at B) with a single segment
//! between them on an alternate layer. If a direct segment on the original
//! layer from A→B is DRC-clean, both vias are eliminated and replaced
//! with a direct segment.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::rules::clearance::segment_distance;
use cypcb_router::types::{RouteSegment, ViaPlacement};
use cypcb_world::components::rotate_about_origin;
use cypcb_world::components::trace::{Trace, Via};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{BoardWorld, FootprintRef, Layer, NetConnections, NetId, Position, Rotation};

use crate::grid::{index_to_layer, layer_to_index};

/// Copper and keepouts the board had before routing, which a replacement
/// segment has to clear as much as the routed copper.
///
/// These are the things `RoutingGrid` marks as obstacles: pads, traces and
/// vias already on the board, keepout zones. Pours are left out for the reason
/// the grid leaves them out - they fill around traces rather than block them.
#[derive(Debug, Clone, Default)]
pub struct BoardObstacles {
    pads: Vec<PadCopper>,
    traces: Vec<RouteSegment>,
    keepouts: Vec<(Rect, u32)>,
}

/// A pad as the smallest circle that covers it, whatever its shape and
/// rotation. That is more copper than a rectangular pad has at its sides, which
/// can only keep a pair that could have gone.
#[derive(Debug, Clone)]
struct PadCopper {
    /// `None` for a pin no net connects: its copper is still copper.
    net: Option<NetId>,
    center: Point,
    radius: Nm,
    /// Empty for a hole with no copper, which no layer may cross.
    layers: Vec<Layer>,
}

impl BoardObstacles {
    /// Read the pads, placed vias, drawn traces and keepouts off the board.
    pub fn from_board(world: &mut BoardWorld, library: &FootprintLibrary) -> Self {
        let components: Vec<(Point, f64, String, Option<NetConnections>)> = {
            let ecs = world.ecs_mut();
            let mut query =
                ecs.query::<(&Position, &Rotation, &FootprintRef, Option<&NetConnections>)>();
            query
                .iter(ecs)
                .map(|(pos, rot, fp, nets)| {
                    (
                        pos.0,
                        rot.to_degrees(),
                        fp.as_str().to_string(),
                        nets.cloned(),
                    )
                })
                .collect()
        };

        let mut pads = Vec::new();
        for (comp_pos, rotation_deg, fp_name, nets) in &components {
            let Some(footprint) = library.get(fp_name) else {
                continue;
            };
            for pad in &footprint.pads {
                let offset = rotate_about_origin(pad.position, *rotation_deg);
                let (w, h) = (pad.size.0.raw() as f64, pad.size.1.raw() as f64);
                pads.push(PadCopper {
                    net: nets.as_ref().and_then(|n| n.pin_net(&pad.number)),
                    center: Point::new(
                        Nm::new(comp_pos.x.raw() + offset.x.raw()),
                        Nm::new(comp_pos.y.raw() + offset.y.raw()),
                    ),
                    radius: Nm::new(((w * w + h * h).sqrt() / 2.0).ceil() as i64),
                    layers: if pad.is_non_plated() {
                        Vec::new()
                    } else {
                        let copper = pad.copper_mask();
                        (0..32usize)
                            .filter(|index| copper & (1u32 << index) != 0)
                            .map(index_to_layer)
                            .collect()
                    },
                });
            }
        }

        // A via already on the board is a round pad of its net on every layer
        // its hole passes. Left out, the pair the router dropped to get past
        // it was taken away again and the direct segment ran through its ring.
        let vias: Vec<Via> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<&Via>();
            query.iter(ecs).copied().collect()
        };
        for via in &vias {
            let copper = via.copper_mask();
            let layers: Vec<Layer> = [Layer::TopCopper, Layer::BottomCopper]
                .into_iter()
                .chain((0..30).map(Layer::Inner))
                .filter(|layer| layer.to_copper_mask() & copper != 0)
                .collect();
            // No layers here means an unplated hole, which every layer must
            // clear; a via with no copper layer is not that.
            if layers.is_empty() {
                continue;
            }
            pads.push(PadCopper {
                net: Some(via.net_id),
                center: via.position,
                radius: Nm::new(via.outer_diameter.raw() / 2),
                layers,
            });
        }

        let traces: Vec<Trace> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<&Trace>();
            query.iter(ecs).cloned().collect()
        };
        let traces = traces
            .iter()
            .flat_map(|trace| {
                trace.segments.iter().enumerate().map(|(i, seg)| {
                    RouteSegment::new(
                        trace.net_id,
                        trace.layer,
                        trace.width_at(i),
                        seg.start,
                        seg.end,
                    )
                })
            })
            .collect();

        let keepouts = world
            .zones()
            .into_iter()
            .filter(|(_, zone)| zone.is_keepout())
            .map(|(_, zone)| (zone.bounds, zone.layer_mask))
            .collect();

        BoardObstacles {
            pads,
            traces,
            keepouts,
        }
    }
}

/// Optimize vias by eliminating redundant via pairs.
///
/// For each net, finds via pairs where:
/// 1. Via A transitions from layer L1 to L2 at point A
/// 2. Via B transitions from layer L2 back to L1 at point B
/// 3. A single segment exists between A and B on layer L2
/// 4. No other copper of the net meets A or B on L2
///
/// If a direct segment from A→B on L1 keeps `min_clearance` edge to edge from
/// every other net's segments, vias and pads, and from the traces and
/// keepouts in `board`, both vias are removed and the intermediate segment is
/// replaced with a direct segment on L1.
///
/// The other nets' copper is read from `segments` and `vias` themselves, as
/// they stand after each earlier elimination. It used to be a separate
/// argument, and every caller passed it empty, so the check had nothing to
/// check: on `led_blink` a GND pair was replaced by a diagonal that crosses
/// SW_OUT.
///
/// Condition 4 keeps the net connected. A via that another branch of the net
/// starts from on L2 is that branch's only way up; removing it leaves the
/// branch, and the pad at its far end, cut off from the rest of the net.
///
/// # Arguments
/// * `segments` - All route segments
/// * `vias` - All via placements
/// * `board` - What was on the board before routing
/// * `min_clearance` - Minimum clearance distance, edge to edge
///
/// # Returns
/// Tuple of (optimized segments, optimized vias)
pub fn optimize_vias(
    segments: Vec<RouteSegment>,
    vias: Vec<ViaPlacement>,
    board: &BoardObstacles,
    min_clearance: Nm,
) -> (Vec<RouteSegment>, Vec<ViaPlacement>) {
    if vias.len() < 2 {
        return (segments, vias);
    }

    // Group vias by net_id
    let mut net_ids: Vec<NetId> = vias.iter().map(|v| v.net_id).collect();
    net_ids.sort_by_key(|n| n.id());
    net_ids.dedup();

    let mut kept_segments = segments;
    let mut kept_vias = vias;
    let mut removed_via_indices: Vec<usize> = Vec::new();
    let mut removed_seg_indices: Vec<usize> = Vec::new();
    let mut added_segments: Vec<RouteSegment> = Vec::new();

    for net_id in &net_ids {
        // Collect vias for this net
        let net_vias: Vec<(usize, ViaPlacement)> = kept_vias
            .iter()
            .enumerate()
            .filter(|(_, v)| v.net_id == *net_id)
            .map(|(i, v)| (i, v.clone()))
            .collect();

        // Try to find eliminable via pairs
        for i in 0..net_vias.len() {
            for j in (i + 1)..net_vias.len() {
                let (idx_a, ref via_a) = net_vias[i];
                let (idx_b, ref via_b) = net_vias[j];

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

                // Any other copper of this net on the alternate layer that
                // touches either via still needs that via.
                let via_still_used = kept_segments.iter().enumerate().any(|(si, s)| {
                    si != between_seg_idx
                        && !removed_seg_indices.contains(&si)
                        && s.net_id == *net_id
                        && s.layer == alternate_layer
                        && (touches(s, via_a) || touches(s, via_b))
                });
                if via_still_used {
                    continue;
                }

                // The same holds for a pad. The router can reach a surface pad
                // from the far side by stepping up onto it and straight back
                // down, and then the short run between the two vias is the
                // pad's only copper. On `multi_ic` that was U1.63: VCC_3V3
                // came up onto the top-layer pad and went down again one cell
                // later, the pair was replaced by a bottom-layer segment under
                // the pad, and the pin was left with no copper at all.
                if pad_needs_the_hop(
                    board,
                    *net_id,
                    &kept_segments[between_seg_idx],
                    original_layer,
                    [via_a.position.x.0, via_a.position.y.0],
                    [via_b.position.x.0, via_b.position.y.0],
                ) {
                    continue;
                }

                // Check if a direct segment on the original layer is DRC-clean
                let direct_start = via_a.position;
                let direct_end = via_b.position;
                let width = kept_segments[between_seg_idx].width;

                let p1 = [direct_start.x.0, direct_start.y.0];
                let p2 = [direct_end.x.0, direct_end.y.0];

                let live_segments = kept_segments
                    .iter()
                    .enumerate()
                    .filter(|(si, _)| !removed_seg_indices.contains(si))
                    .map(|(_, s)| s)
                    .chain(added_segments.iter());
                let segments_clear = live_segments
                    .chain(board.traces.iter())
                    .filter(|s| s.net_id != *net_id && s.layer == original_layer)
                    .all(|other| {
                        let p3 = [other.start.x.0, other.start.y.0];
                        let p4 = [other.end.x.0, other.end.y.0];
                        let needed = min_clearance.0 + width.0 / 2 + other.width.0 / 2;
                        segment_distance(p1, p2, p3, p4) >= needed
                    });

                let vias_clear = kept_vias
                    .iter()
                    .enumerate()
                    // A via is read as copper on every layer. On a blind or
                    // buried via that is more copper than there is, which can
                    // only keep a pair that could have gone.
                    .filter(|(vi, v)| !removed_via_indices.contains(vi) && v.net_id != *net_id)
                    .all(|(_, other)| {
                        let p3 = [other.position.x.0, other.position.y.0];
                        let needed = min_clearance.0 + width.0 / 2 + other.outer_diameter.0 / 2;
                        segment_distance(p1, p2, p3, p3) >= needed
                    });

                let board_clear =
                    board_clear(board, *net_id, original_layer, p1, p2, width, min_clearance);

                if segments_clear && vias_clear && board_clear {
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

/// Whether a direct segment on `layer` clears the pads and keepouts on the
/// board. Drawn traces are checked with the routed segments.
fn board_clear(
    board: &BoardObstacles,
    net_id: NetId,
    layer: Layer,
    p1: [i64; 2],
    p2: [i64; 2],
    width: Nm,
    min_clearance: Nm,
) -> bool {
    let pads_clear = board
        .pads
        .iter()
        .filter(|pad| pad.net != Some(net_id))
        .filter(|pad| pad.layers.is_empty() || pad.layers.contains(&layer))
        .all(|pad| {
            let c = [pad.center.x.0, pad.center.y.0];
            segment_distance(p1, p2, c, c) >= min_clearance.0 + width.0 / 2 + pad.radius.0
        });

    let Some(index) = layer_to_index(layer) else {
        return pads_clear;
    };
    let keepouts_clear = board
        .keepouts
        .iter()
        .filter(|(_, mask)| mask & (1 << index) != 0)
        .all(|(rect, _)| {
            let (lo, hi) = ([rect.min.x.0, rect.min.y.0], [rect.max.x.0, rect.max.y.0]);
            let inside =
                |p: [i64; 2]| p[0] >= lo[0] && p[0] <= hi[0] && p[1] >= lo[1] && p[1] <= hi[1];
            let corners = [lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]];
            !inside(p1)
                && !inside(p2)
                && (0..4).all(|i| {
                    segment_distance(p1, p2, corners[i], corners[(i + 1) % 4]) >= width.0 / 2
                })
        });

    pads_clear && keepouts_clear
}

/// Whether a pad of the pair's own net is reached by the run between the two
/// vias and would not be reached by the direct segment that replaces it.
///
/// The pad is the covering circle `BoardObstacles` keeps, so a run that only
/// passes near a pad can read as reaching it. That errs toward keeping a pair,
/// which costs two vias; the other way costs a pin.
fn pad_needs_the_hop(
    board: &BoardObstacles,
    net_id: NetId,
    between: &RouteSegment,
    original_layer: Layer,
    p1: [i64; 2],
    p2: [i64; 2],
) -> bool {
    let a = [between.start.x.0, between.start.y.0];
    let b = [between.end.x.0, between.end.y.0];
    board
        .pads
        .iter()
        .filter(|pad| pad.net == Some(net_id) && pad.layers.contains(&between.layer))
        .any(|pad| {
            let c = [pad.center.x.0, pad.center.y.0];
            let reach = pad.radius.0 + between.width.0 / 2;
            let reached_now = segment_distance(a, b, c, c) <= reach;
            let reached_after =
                pad.layers.contains(&original_layer) && segment_distance(p1, p2, c, c) <= reach;
            reached_now && !reached_after
        })
}

/// Whether a segment's centreline passes through a via's centre.
fn touches(segment: &RouteSegment, via: &ViaPlacement) -> bool {
    let p = [via.position.x.0, via.position.y.0];
    let a = [segment.start.x.0, segment.start.y.0];
    let b = [segment.end.x.0, segment.end.y.0];
    segment_distance(p, p, a, b) == 0
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

        let (opt_segs, opt_vias) = optimize_vias(segments, vias, &BoardObstacles::default(), Nm(0));

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
            // Another net's copper on the top layer right in the path
            make_seg(99, Layer::TopCopper, 7.0, -0.05, 8.0, 0.05),
        ];
        let vias = vec![
            make_via(1, 5.0, 0.0, Layer::TopCopper, Layer::BottomCopper),
            make_via(1, 10.0, 0.0, Layer::BottomCopper, Layer::TopCopper),
        ];

        let (opt_segs, opt_vias) = optimize_vias(
            segments,
            vias,
            &BoardObstacles::default(),
            Nm::from_mm(0.15),
        );

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

        let (opt_segs, opt_vias) =
            optimize_vias(segments.clone(), vias, &BoardObstacles::default(), Nm(0));

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

        let (opt_segs, opt_vias) = optimize_vias(segments, vias, &BoardObstacles::default(), Nm(0));

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

        let (opt_segs, _) = optimize_vias(segments, vias, &BoardObstacles::default(), Nm(0));
        for s in &opt_segs {
            assert_eq!(
                s.net_id, net_id,
                "net_id must be preserved after via optimization"
            );
        }
    }

    /// GND on `led_blink` with the flag on, as the router hands it over: a
    /// trace into a via at (23.495, 18.161), a single bottom segment to a via
    /// at (30.353, 11.303), and SW_OUT running down x = 24.511 on the top
    /// layer between them. The direct top segment crosses SW_OUT at
    /// (24.511, 17.145), which is the short the checker reported.
    fn led_blink_gnd_pair() -> (Vec<RouteSegment>, Vec<ViaPlacement>) {
        let w = 0.127;
        let seg = |net: u32, layer, x1, y1, x2, y2| {
            RouteSegment::new(
                NetId::new(net),
                layer,
                Nm::from_mm(w),
                Point::from_mm(x1, y1),
                Point::from_mm(x2, y2),
            )
        };
        let segments = vec![
            seg(1, Layer::TopCopper, 20.447, 15.113, 23.495, 18.161),
            seg(1, Layer::BottomCopper, 23.495, 18.161, 30.353, 11.303),
            seg(1, Layer::TopCopper, 30.353, 11.303, 30.861, 10.795),
            seg(4, Layer::TopCopper, 18.161, 6.985, 24.511, 13.335),
            seg(4, Layer::TopCopper, 24.511, 13.335, 24.511, 22.987),
        ];
        let vias = vec![
            make_via(1, 23.495, 18.161, Layer::TopCopper, Layer::BottomCopper),
            make_via(1, 30.353, 11.303, Layer::BottomCopper, Layer::TopCopper),
        ];
        (segments, vias)
    }

    #[test]
    fn a_pair_whose_direct_path_crosses_another_net_is_kept() {
        let (segments, vias) = led_blink_gnd_pair();

        let (opt_segs, opt_vias) = optimize_vias(
            segments,
            vias,
            &BoardObstacles::default(),
            Nm::from_mm(0.127),
        );

        assert_eq!(
            opt_vias.len(),
            2,
            "the pair would have shorted GND to SW_OUT"
        );
        let gnd_on_top_crossing_sw_out = opt_segs.iter().any(|s| {
            s.net_id == NetId::new(1)
                && s.layer == Layer::TopCopper
                && s.start == Point::from_mm(23.495, 18.161)
                && s.end == Point::from_mm(30.353, 11.303)
        });
        assert!(!gnd_on_top_crossing_sw_out);
    }

    #[test]
    fn the_same_pair_goes_when_the_other_net_is_not_there() {
        // The control for the test above: without SW_OUT the pair is exactly
        // what the optimizer exists to remove, so a kept pair up there is the
        // other net's doing and not a pair the optimizer never takes.
        let (segments, vias) = led_blink_gnd_pair();
        let segments: Vec<_> = segments
            .into_iter()
            .filter(|s| s.net_id == NetId::new(1))
            .collect();

        let (_, opt_vias) = optimize_vias(
            segments,
            vias,
            &BoardObstacles::default(),
            Nm::from_mm(0.127),
        );

        assert!(opt_vias.is_empty());
    }

    #[test]
    fn a_via_another_branch_of_the_net_leaves_from_is_kept() {
        // With the flag on, a second GND connection on `led_blink` starts
        // from the first via on the bottom layer and runs to the via under
        // C2. Removing the first via leaves that branch, and C2's GND pad,
        // joined to nothing - a break the checker has no rule to report.
        let (segments, vias) = led_blink_gnd_pair();
        let mut segments: Vec<_> = segments
            .into_iter()
            .filter(|s| s.net_id == NetId::new(1))
            .collect();
        segments.push(make_seg(
            1,
            Layer::BottomCopper,
            23.495,
            18.161,
            25.527,
            20.193,
        ));

        let (_, opt_vias) = optimize_vias(
            segments,
            vias,
            &BoardObstacles::default(),
            Nm::from_mm(0.127),
        );

        assert_eq!(opt_vias.len(), 2);
    }

    #[test]
    fn clearance_is_measured_from_the_copper_edge() {
        // Centrelines 0.3 mm apart clear a 0.15 mm rule between centrelines
        // and fail it between edges: two 0.2 mm traces leave 0.1 mm of gap.
        let segments = vec![
            make_seg(1, Layer::TopCopper, 0.0, 0.0, 5.0, 0.0),
            make_seg(1, Layer::BottomCopper, 5.0, 0.0, 10.0, 0.0),
            make_seg(1, Layer::TopCopper, 10.0, 0.0, 15.0, 0.0),
            make_seg(99, Layer::TopCopper, 6.0, 0.3, 9.0, 0.3),
        ];
        let vias = vec![
            make_via(1, 5.0, 0.0, Layer::TopCopper, Layer::BottomCopper),
            make_via(1, 10.0, 0.0, Layer::BottomCopper, Layer::TopCopper),
        ];

        let (_, opt_vias) = optimize_vias(
            segments,
            vias,
            &BoardObstacles::default(),
            Nm::from_mm(0.15),
        );

        assert_eq!(opt_vias.len(), 2);
    }

    #[test]
    fn another_nets_via_in_the_path_keeps_the_pair() {
        let segments = vec![
            make_seg(1, Layer::TopCopper, 0.0, 0.0, 5.0, 0.0),
            make_seg(1, Layer::BottomCopper, 5.0, 0.0, 10.0, 0.0),
            make_seg(1, Layer::TopCopper, 10.0, 0.0, 15.0, 0.0),
        ];
        let vias = vec![
            make_via(1, 5.0, 0.0, Layer::TopCopper, Layer::BottomCopper),
            make_via(1, 10.0, 0.0, Layer::BottomCopper, Layer::TopCopper),
            make_via(99, 7.5, 0.0, Layer::TopCopper, Layer::BottomCopper),
        ];

        let (_, opt_vias) = optimize_vias(
            segments,
            vias,
            &BoardObstacles::default(),
            Nm::from_mm(0.15),
        );

        assert_eq!(opt_vias.len(), 3);
    }

    fn straight_pair() -> (Vec<RouteSegment>, Vec<ViaPlacement>) {
        let segments = vec![
            make_seg(1, Layer::TopCopper, 0.0, 0.0, 5.0, 0.0),
            make_seg(1, Layer::BottomCopper, 5.0, 0.0, 10.0, 0.0),
            make_seg(1, Layer::TopCopper, 10.0, 0.0, 15.0, 0.0),
        ];
        let vias = vec![
            make_via(1, 5.0, 0.0, Layer::TopCopper, Layer::BottomCopper),
            make_via(1, 10.0, 0.0, Layer::BottomCopper, Layer::TopCopper),
        ];
        (segments, vias)
    }

    fn board_with_pad(net: Option<u32>, layers: Vec<Layer>) -> BoardObstacles {
        BoardObstacles {
            pads: vec![PadCopper {
                net: net.map(NetId::new),
                center: Point::from_mm(7.5, 0.6),
                radius: Nm::from_mm(0.4),
                layers,
            }],
            ..BoardObstacles::default()
        }
    }

    #[test]
    fn another_nets_pad_beside_the_path_keeps_the_pair() {
        // The two shorts the fix left on `stm32_breakout` before pads were
        // read were both a direct segment run through a pad of another net.
        // Centre 0.6 mm off the line, 0.4 mm of pad: 0.1 mm of trace edge to
        // pad edge, under a 0.15 mm rule.
        let (segments, vias) = straight_pair();
        let board = board_with_pad(Some(2), vec![Layer::TopCopper]);

        let (_, opt_vias) = optimize_vias(segments, vias, &board, Nm::from_mm(0.15));

        assert_eq!(opt_vias.len(), 2);
    }

    #[test]
    fn a_pad_no_net_connects_is_still_copper() {
        let (segments, vias) = straight_pair();
        let board = board_with_pad(None, vec![Layer::TopCopper]);

        let (_, opt_vias) = optimize_vias(segments, vias, &board, Nm::from_mm(0.15));

        assert_eq!(opt_vias.len(), 2);
    }

    #[test]
    fn a_pad_on_the_other_layer_or_the_same_net_does_not_block() {
        for board in [
            board_with_pad(Some(2), vec![Layer::BottomCopper]),
            board_with_pad(Some(1), vec![Layer::TopCopper]),
        ] {
            let (segments, vias) = straight_pair();
            let (_, opt_vias) = optimize_vias(segments, vias, &board, Nm::from_mm(0.15));
            assert!(opt_vias.is_empty(), "{board:?}");
        }
    }

    /// U1.63 on `multi_ic`, reduced: a net comes up onto a top-layer pad from
    /// the bottom, runs one cell along it and goes back down. The run between
    /// the vias is the only copper on the pad, so the pair stays.
    fn a_hop_onto_a_surface_pad(
        pad_layers: Vec<Layer>,
    ) -> (Vec<RouteSegment>, Vec<ViaPlacement>, BoardObstacles) {
        let segments = vec![
            make_seg(1, Layer::BottomCopper, 0.0, 0.0, 5.0, 0.2),
            make_seg(1, Layer::TopCopper, 5.0, 0.2, 5.0, -0.2),
            make_seg(1, Layer::BottomCopper, 5.0, -0.2, 5.0, -5.0),
        ];
        let vias = vec![
            make_via(1, 5.0, 0.2, Layer::BottomCopper, Layer::TopCopper),
            make_via(1, 5.0, -0.2, Layer::TopCopper, Layer::BottomCopper),
        ];
        let board = BoardObstacles {
            pads: vec![PadCopper {
                net: Some(NetId::new(1)),
                center: Point::from_mm(4.8, 0.0),
                radius: Nm::from_mm(0.62),
                layers: pad_layers,
            }],
            ..BoardObstacles::default()
        };
        (segments, vias, board)
    }

    #[test]
    fn a_surface_pad_reached_only_between_the_vias_keeps_the_pair() {
        let (segments, vias, board) = a_hop_onto_a_surface_pad(vec![Layer::TopCopper]);

        let (opt_segs, opt_vias) = optimize_vias(segments, vias, &board, Nm::from_mm(0.1));

        assert_eq!(opt_vias.len(), 2, "the pad's only copper was replaced");
        assert!(opt_segs.iter().any(|s| s.layer == Layer::TopCopper));
    }

    #[test]
    fn a_pad_the_direct_segment_also_reaches_does_not_keep_it() {
        // The control: a through-hole pad is on the bottom too, the direct
        // segment still lands on it, and the pair is as redundant as ever.
        let (segments, vias, board) =
            a_hop_onto_a_surface_pad(vec![Layer::TopCopper, Layer::BottomCopper]);

        let (_, opt_vias) = optimize_vias(segments, vias, &board, Nm::from_mm(0.1));

        assert!(opt_vias.is_empty());
    }

    #[test]
    fn a_keepout_across_the_path_keeps_the_pair() {
        let (segments, vias) = straight_pair();
        let board = BoardObstacles {
            keepouts: vec![(
                Rect::new(Point::from_mm(7.0, -1.0), Point::from_mm(8.0, 1.0)),
                0b1,
            )],
            ..BoardObstacles::default()
        };

        let (_, opt_vias) = optimize_vias(segments, vias, &board, Nm::from_mm(0.15));

        assert_eq!(opt_vias.len(), 2);
    }
}
