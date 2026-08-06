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
        let pours: Vec<_> = world
            .zones()
            .into_iter()
            .filter(|(_, zone)| !zone.is_keepout())
            .collect();

        for i in 0..pours.len() {
            for j in (i + 1)..pours.len() {
                let (entity_a, a) = &pours[i];
                let (entity_b, b) = &pours[j];

                // Different layers cannot touch, and the same net is one plane
                // drawn twice rather than a short.
                if a.layer_mask & b.layer_mask == 0 {
                    continue;
                }
                if a.net.is_some() && a.net == b.net {
                    continue;
                }

                let overlap_x =
                    a.bounds.min.x.0 < b.bounds.max.x.0 && b.bounds.min.x.0 < a.bounds.max.x.0;
                let overlap_y =
                    a.bounds.min.y.0 < b.bounds.max.y.0 && b.bounds.min.y.0 < a.bounds.max.y.0;
                if !(overlap_x && overlap_y) {
                    continue;
                }

                // Report the middle of the contested copper, which is where a
                // person looking at the board will find the problem.
                let centre = Point::new(
                    Nm((a.bounds.min.x.0.max(b.bounds.min.x.0)
                        + a.bounds.max.x.0.min(b.bounds.max.x.0))
                        / 2),
                    Nm((a.bounds.min.y.0.max(b.bounds.min.y.0)
                        + a.bounds.max.y.0.min(b.bounds.max.y.0))
                        / 2),
                );

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
