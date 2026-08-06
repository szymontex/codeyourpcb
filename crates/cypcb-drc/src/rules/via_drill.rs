//! Via drill size against the fabricator's smallest bit.
//!
//! `ViaDiameterRule` checks the copper ring. This checks the hole through it,
//! which is a different limit and the one that decides whether the board can
//! be drilled at all: a fab that quotes 0.2mm cannot make a 0.1mm hole,
//! whatever the ring around it looks like.
//!
//! `ViolationKind::ViaDrill` and `DesignRules::min_via_drill` both existed with
//! nothing enforcing them. The sandbox test that should have caught that ran
//! the checker and threw the result away - `let _ = result;` - so a via with
//! half the minimum drill passed for as long as the rule was missing.

use cypcb_world::components::trace::Via;
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for checking a via's drill against the minimum the fab can make.
pub struct ViaDrillRule;

impl DrcRule for ViaDrillRule {
    fn name(&self) -> &'static str {
        "via-drill"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let min_drill = rules.min_via_drill;

        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Via)>();
        let vias: Vec<_> = query
            .iter(ecs)
            .map(|(entity, via)| (entity, via.position, via.drill))
            .collect();

        vias.into_iter()
            .filter(|(_, _, drill)| *drill < min_drill)
            .map(|(entity, position, drill)| {
                DrcViolation::via_drill(entity, drill, min_drill, position)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ViolationKind;
    use cypcb_core::{Nm, Point};
    use cypcb_world::components::Layer;

    fn board_with_via(drill_mm: f64) -> BoardWorld {
        let mut world = BoardWorld::new();
        world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);
        let net = world.intern_net("VCC");
        world.ecs_mut().spawn(Via {
            position: Point::from_mm(10.0, 10.0),
            drill: Nm::from_mm(drill_mm),
            outer_diameter: Nm::from_mm(drill_mm * 2.0),
            start_layer: Layer::TopCopper,
            end_layer: Layer::BottomCopper,
            net_id: net,
            locked: false,
        });
        world
    }

    #[test]
    fn a_hole_the_fab_cannot_drill_is_reported() {
        // JLCPCB's minimum via drill is 0.3mm.
        let mut world = board_with_via(0.1);
        let violations = ViaDrillRule.check(&mut world, &DesignRules::jlcpcb_2layer());

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].kind, ViolationKind::ViaDrill);
        assert!(
            violations[0].message.contains("0.10mm") && violations[0].message.contains("0.30mm"),
            "the message states what was drawn and what is possible: {}",
            violations[0].message
        );
    }

    #[test]
    fn a_hole_the_fab_can_drill_is_not() {
        let mut world = board_with_via(0.4);
        assert!(ViaDrillRule
            .check(&mut world, &DesignRules::jlcpcb_2layer())
            .is_empty());
    }

    #[test]
    fn exactly_the_minimum_passes() {
        // The rule is "at least this", not "more than this" - a fab that
        // quotes 0.3mm drills 0.3mm.
        let mut world = board_with_via(0.3);
        assert!(ViaDrillRule
            .check(&mut world, &DesignRules::jlcpcb_2layer())
            .is_empty());
    }
}
