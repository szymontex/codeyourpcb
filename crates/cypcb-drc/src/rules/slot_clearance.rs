//! Copper against a milled opening.
//!
//! D7, settled by the owner: `min_slot_clearance` is the routed-edge question
//! asked of a slot. JLCPCB publishes no slot-spacing figure of any kind; what
//! it publishes is copper clearance from a **routed** edge. A slot is a routed
//! opening made by the same mill on the same board, so copper against a slot
//! is the same physical question `min_edge_clearance` already answers about
//! the board outline.
//!
//! # The objection this rule has to survive
//!
//! A plated slot has its own annulus around it by construction, so a rule that
//! measures every piece of copper against every slot fires on every slotted
//! pad ever drawn. That is a rule-writing problem rather than a meaning
//! problem, and this checker already solves it twice: `HoleToHoleRule` skips
//! two pads of the same component, and every clearance rule distinguishes
//! foreign copper from a feature's own. **This rule measures other nets'
//! copper against the slot.** A slot's own component is skipped, and so is
//! anything on a net that component connects to.

use std::collections::HashMap;

use cypcb_core::{Nm, Point};
use cypcb_world::components::{NetConnections, NetId};
use cypcb_world::BoardWorld;

use super::{holes_of, segment_distance, DrcRule, Hole};
use crate::presets::DesignRules;
use crate::violation::DrcViolation;

/// Rule that checks copper clearance to a milled slot.
pub struct SlotClearanceRule;

impl DrcRule for SlotClearanceRule {
    fn name(&self) -> &'static str {
        "slot-clearance"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let mut violations = Vec::new();
        let required = rules.min_slot_clearance;

        // A drilled hole is a segment of zero length and a slot is one with
        // travel. Only the second is a milled opening, and this rule has
        // nothing to say about the first - `HoleToEdgeRule` and
        // `HoleToHoleRule` own that question.
        let slots: Vec<Hole> = holes_of(world)
            .into_iter()
            .filter(|hole| hole.start != hole.end)
            .collect();
        if slots.is_empty() {
            return violations;
        }

        // Which net a piece of copper carries, and which nets a component's
        // pins connect to. Both are needed: a trace has one net, and the
        // component a slot belongs to has as many as it has pins.
        let net_map: HashMap<u32, NetId> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &NetId)>();
            query.iter(ecs).map(|(e, n)| (e.index(), *n)).collect()
        };
        let net_connections_map: HashMap<u32, Vec<NetId>> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &NetConnections)>();
            query
                .iter(ecs)
                .map(|(e, nc)| (e.index(), nc.iter().map(|pc| pc.net).collect()))
                .collect()
        };

        let entries: Vec<_> = world.spatial().iter().cloned().collect();

        for slot in &slots {
            let own_nets: Vec<NetId> = net_connections_map
                .get(&slot.entity.index())
                .cloned()
                .or_else(|| net_map.get(&slot.entity.index()).map(|n| vec![*n]))
                .unwrap_or_default();

            for entry in &entries {
                // The slot's own component carries the annulus the slot is
                // plated with. Measuring that is measuring the slot against
                // itself.
                if entry.entity == slot.entity {
                    continue;
                }

                // Copper the slot's component is connected to is copper that
                // belongs there. A pin on GND beside a GND slot is the design
                // working, not a fault.
                let entry_nets: Vec<NetId> = net_connections_map
                    .get(&entry.entity.index())
                    .cloned()
                    .or_else(|| net_map.get(&entry.entity.index()).map(|n| vec![*n]))
                    .unwrap_or_default();
                if !own_nets.is_empty() && entry_nets.iter().any(|net| own_nets.contains(net)) {
                    continue;
                }

                let min_x = entry.envelope.lower()[0];
                let min_y = entry.envelope.lower()[1];
                let max_x = entry.envelope.upper()[0];
                let max_y = entry.envelope.upper()[1];

                let gap = box_to_slot_gap(slot, min_x, min_y, max_x, max_y);
                if gap < required.0 {
                    let location = Point::new(Nm((min_x + max_x) / 2), Nm((min_y + max_y) / 2));
                    violations.push(DrcViolation::slot_clearance(
                        entry.entity,
                        slot.entity,
                        Nm(gap.max(0)),
                        required,
                        location,
                    ));
                }
            }
        }

        violations
    }
}

/// Laminate between a bounding box and the wall of a milled slot.
///
/// The slot is a capsule: a segment with a radius, which is exactly what the
/// mill leaves. Each of the box's four sides is measured against the slot's
/// axis and the bit's radius is taken off, so a box beside the middle of a
/// long slot is measured against the slot's side rather than against its end.
fn box_to_slot_gap(slot: &Hole, min_x: i64, min_y: i64, max_x: i64, max_y: i64) -> i64 {
    let corners = [
        Point::new(Nm(min_x), Nm(min_y)),
        Point::new(Nm(max_x), Nm(min_y)),
        Point::new(Nm(max_x), Nm(max_y)),
        Point::new(Nm(min_x), Nm(max_y)),
    ];
    let mut nearest = i64::MAX;
    for i in 0..4 {
        let a = corners[i];
        let b = corners[(i + 1) % 4];
        nearest = nearest.min(segment_distance(a, b, slot.start, slot.end));
    }
    nearest - slot.radius
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_world::components::Layer;

    /// A 2.4 x 1.0mm slot centred on the origin: the bit centres sit 0.7mm
    /// either side and the bit's radius is 0.5mm.
    fn slot(start: (f64, f64), end: (f64, f64)) -> Hole {
        Hole {
            entity: bevy_ecs::entity::Entity::from_raw(1),
            start: Point::from_mm(start.0, start.1),
            end: Point::from_mm(end.0, end.1),
            radius: Nm::from_mm(0.5).0,
            span: (Layer::TopCopper, Layer::BottomCopper),
            plated: true,
        }
    }

    #[test]
    fn the_gap_is_measured_from_the_slot_wall_not_its_centre() {
        // A box whose near edge is 1.6mm out along the slot's own length. The
        // axis ends at 0.7, so 0.9mm to the axis and 0.4mm to the wall.
        // Measured from the centre it would read 1.6mm, which is the mistake
        // this rule exists not to make.
        let gap = box_to_slot_gap(
            &slot((-0.7, 0.0), (0.7, 0.0)),
            Nm::from_mm(1.6).0,
            Nm::from_mm(-0.5).0,
            Nm::from_mm(2.6).0,
            Nm::from_mm(0.5).0,
        );
        assert_eq!(gap, Nm::from_mm(0.4).0, "measured {gap}nm");
    }

    #[test]
    fn a_box_beside_the_middle_is_measured_against_the_side() {
        // A long slot is a wall, not a point: a box level with its middle is
        // 1.0mm from the axis and 0.5mm from the wall, however far the ends
        // are.
        let gap = box_to_slot_gap(
            &slot((-5.0, 0.0), (5.0, 0.0)),
            Nm::from_mm(-0.5).0,
            Nm::from_mm(1.0).0,
            Nm::from_mm(0.5).0,
            Nm::from_mm(2.0).0,
        );
        assert_eq!(gap, Nm::from_mm(0.5).0, "measured {gap}nm");
    }

    #[test]
    fn copper_overlapping_the_slot_reads_as_negative_rather_than_far() {
        // A box straddling the slot has no laminate at all. The caller clamps
        // to zero for the report; what must not happen is a positive number,
        // which is what a distance that forgot the radius would give.
        let gap = box_to_slot_gap(
            &slot((-0.7, 0.0), (0.7, 0.0)),
            Nm::from_mm(-0.2).0,
            Nm::from_mm(-0.2).0,
            Nm::from_mm(0.2).0,
            Nm::from_mm(0.2).0,
        );
        assert!(
            gap < 0,
            "copper over the slot must not read as clear: {gap}nm"
        );
    }

    #[test]
    fn a_board_with_no_slot_has_nothing_to_measure() {
        // Zero travel is a drill, and `HoleToEdgeRule` owns that question.
        // Without the filter this rule would report against every through-hole
        // pad on every board.
        let mut world = BoardWorld::new();
        world.set_board("plain".into(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);
        let violations = SlotClearanceRule.check(&mut world, &DesignRules::jlcpcb_2layer());
        assert!(violations.is_empty(), "{violations:?}");
    }
}
