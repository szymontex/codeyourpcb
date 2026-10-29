//! Two copper pours on different nets, over the same ground.
//!
//! A pour is cut against pads, traces and vias, and not against another pour -
//! so two planes declared over the same area both fill it, and the board has a
//! ground plane shorted to a supply plane over their whole overlap.
//!
//! The exporter does not pick a winner, because there is no right answer to
//! pick: which plane owns the contested copper is a decision only the designer
//! can make. What a checker can do is refuse to let it pass quietly.

use cypcb_core::{Nm, Point};
use cypcb_world::components::zone::{Zone, ZoneOutline};
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for copper pours on different nets that overlap.
pub struct ZoneOverlapRule;

impl DrcRule for ZoneOverlapRule {
    fn name(&self) -> &'static str {
        "zone-overlap"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let mut violations = Vec::new();

        // Keepouts pour nothing, so they cannot short anything.
        let mut pours: Vec<(_, Zone, Option<ZoneOutline>)> = world
            .zones()
            .into_iter()
            // `is_copper_pour`, not `!is_keepout`: this rule is about two
            // pours of different nets meeting, and a flexible region is not a
            // pour.
            .filter(|(_, zone)| zone.is_copper_pour())
            .map(|(entity, zone)| (entity, zone, None))
            .collect();
        // The pours drawn as a polygon follow, so a board with none reads in the
        // order it always did.
        pours.extend(
            world
                .outlined_zones()
                .into_iter()
                .filter(|(_, zone, _)| zone.is_copper_pour())
                .map(|(entity, zone, ring)| (entity, zone, Some(ring))),
        );

        for i in 0..pours.len() {
            for j in (i + 1)..pours.len() {
                let (entity_a, a, ring_a) = &pours[i];
                let (entity_b, b, ring_b) = &pours[j];

                // Different layers cannot touch, and the same net is one plane
                // drawn twice rather than a short.
                if a.layer_mask & b.layer_mask == 0 {
                    continue;
                }
                if a.net.is_some() && a.net == b.net {
                    continue;
                }

                // Two boxes overlap when they share area: meeting along an edge
                // or at a corner is not a short. A ring is measured as the ring,
                // by the same rule, in convex pieces.
                let centre = match (ring_a, ring_b) {
                    (None, None) => {
                        let overlap_x = a.bounds.min.x.0 < b.bounds.max.x.0
                            && b.bounds.min.x.0 < a.bounds.max.x.0;
                        let overlap_y = a.bounds.min.y.0 < b.bounds.max.y.0
                            && b.bounds.min.y.0 < a.bounds.max.y.0;
                        if !(overlap_x && overlap_y) {
                            continue;
                        }
                        // Report the middle of the contested copper, which is where a
                        // person looking at the board will find the problem.
                        Point::new(
                            Nm((a.bounds.min.x.0.max(b.bounds.min.x.0)
                                + a.bounds.max.x.0.min(b.bounds.max.x.0))
                                / 2),
                            Nm((a.bounds.min.y.0.max(b.bounds.min.y.0)
                                + a.bounds.max.y.0.min(b.bounds.max.y.0))
                                / 2),
                        )
                    }
                    _ => {
                        let Some(centre) = ring_overlap(a, ring_a.as_ref(), b, ring_b.as_ref())
                        else {
                            continue;
                        };
                        centre
                    }
                };

                violations.push(DrcViolation::clearance(
                    *entity_a,
                    *entity_b,
                    Nm::ZERO,
                    rules.min_clearance,
                    centre,
                ));
            }
        }

        violations
    }
}

/// The convex pieces of a pour: its ring cut by slabs, or its box.
fn pieces_of(zone: &Zone, ring: Option<&ZoneOutline>) -> Vec<Vec<Point>> {
    match ring {
        Some(ring) => cypcb_core::pour_shape::convex_pieces(ring.corners()),
        None => vec![vec![
            zone.bounds.min,
            Point::new(zone.bounds.max.x, zone.bounds.min.y),
            zone.bounds.max,
            Point::new(zone.bounds.min.x, zone.bounds.max.y),
        ]],
    }
}

/// Where two pours, at least one a ring, share area: the middle of the box the
/// two overlapping pieces share that is the largest, or `None` when they share
/// none. Touching along an edge or at a corner is not sharing area.
fn ring_overlap(
    a: &Zone,
    ring_a: Option<&ZoneOutline>,
    b: &Zone,
    ring_b: Option<&ZoneOutline>,
) -> Option<Point> {
    use cypcb_core::pour_shape::{convex_interiors_overlap, shape_bounds};
    let (pieces_a, pieces_b) = (pieces_of(a, ring_a), pieces_of(b, ring_b));
    let mut best: Option<(i128, Point)> = None;
    for pa in &pieces_a {
        for pb in &pieces_b {
            if !convex_interiors_overlap(pa, pb) {
                continue;
            }
            let (ba, bb) = (shape_bounds(pa), shape_bounds(pb));
            let (lo_x, hi_x) = (ba.min.x.0.max(bb.min.x.0), ba.max.x.0.min(bb.max.x.0));
            let (lo_y, hi_y) = (ba.min.y.0.max(bb.min.y.0), ba.max.y.0.min(bb.max.y.0));
            let area = i128::from((hi_x - lo_x).max(0)) * i128::from((hi_y - lo_y).max(0));
            let centre = Point::new(Nm((lo_x + hi_x) / 2), Nm((lo_y + hi_y) / 2));
            if best.is_none_or(|(most, _)| area > most) {
                best = Some((area, centre));
            }
        }
    }
    best.map(|(_, centre)| centre)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::Rect;
    use cypcb_world::components::zone::{Zone, ZoneKind};
    use cypcb_world::components::{Layer, NetId};

    fn pour(world: &mut BoardWorld, centre: (f64, f64), size: (f64, f64), net: u32, layer: Layer) {
        world.spawn_entity(Zone {
            bounds: Rect::from_center_size(
                Point::from_mm(centre.0, centre.1),
                (Nm::from_mm(size.0), Nm::from_mm(size.1)),
            ),
            kind: ZoneKind::CopperPour,
            layer_mask: layer.to_copper_mask(),
            name: None,
            net: Some(NetId::new(net)),
        });
    }

    fn board() -> BoardWorld {
        let mut world = BoardWorld::new();
        world.set_board("t".to_string(), (Nm::from_mm(40.0), Nm::from_mm(40.0)), 2);
        world
    }

    #[test]
    fn two_pours_on_different_nets_over_the_same_copper_are_a_short() {
        let mut world = board();
        pour(&mut world, (10.0, 10.0), (12.0, 12.0), 1, Layer::TopCopper);
        pour(&mut world, (14.0, 10.0), (12.0, 12.0), 2, Layer::TopCopper);

        let violations = ZoneOverlapRule.check(&mut world, &DesignRules::jlcpcb_2layer());
        assert_eq!(
            violations.len(),
            1,
            "a ground plane over a supply plane is a short across their whole overlap"
        );
    }

    #[test]
    fn the_same_net_twice_is_one_plane_drawn_twice() {
        let mut world = board();
        pour(&mut world, (10.0, 10.0), (12.0, 12.0), 1, Layer::TopCopper);
        pour(&mut world, (14.0, 10.0), (12.0, 12.0), 1, Layer::TopCopper);

        assert!(ZoneOverlapRule
            .check(&mut world, &DesignRules::jlcpcb_2layer())
            .is_empty());
    }

    #[test]
    fn planes_on_different_layers_do_not_touch() {
        let mut world = board();
        pour(&mut world, (10.0, 10.0), (12.0, 12.0), 1, Layer::TopCopper);
        pour(
            &mut world,
            (10.0, 10.0),
            (12.0, 12.0),
            2,
            Layer::BottomCopper,
        );

        assert!(ZoneOverlapRule
            .check(&mut world, &DesignRules::jlcpcb_2layer())
            .is_empty());
    }

    #[test]
    fn planes_that_only_meet_at_an_edge_are_not_overlapping() {
        // Touching bounds share no area. A pour that stops exactly where the
        // next begins is a designer being precise, not a short.
        let mut world = board();
        pour(&mut world, (10.0, 10.0), (10.0, 10.0), 1, Layer::TopCopper);
        pour(&mut world, (20.0, 10.0), (10.0, 10.0), 2, Layer::TopCopper);

        assert!(ZoneOverlapRule
            .check(&mut world, &DesignRules::jlcpcb_2layer())
            .is_empty());
    }
}
