//! Unconnected pin detection rule (DRC-04).
//!
//! Validates that all component pins have net connections, catching
//! incomplete designs before manufacturing.

use cypcb_world::BoardWorld;
use cypcb_world::components::{FootprintRef, NetConnections, Position, RefDes};
use cypcb_world::footprint::FootprintLibrary;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;
use super::DrcRule;

/// Rule that checks all component pins have net connections.
///
/// Iterates through all components with NetConnections, looks up their footprints,
/// and checks that each pin in the footprint has a corresponding net connection.
///
/// # Examples
///
/// ```rust,ignore
/// use cypcb_drc::rules::{UnconnectedPinRule, DrcRule};
/// use cypcb_drc::presets::DesignRules;
///
/// let rule = UnconnectedPinRule;
/// let mut world = BoardWorld::new();
/// // ... add components with incomplete nets ...
/// let rules = DesignRules::default();
/// let violations = rule.check(&mut world, &rules);
///
/// for v in &violations {
///     println!("Unconnected: {}", v.message);
/// }
/// ```
pub struct UnconnectedPinRule;

impl DrcRule for UnconnectedPinRule {
    fn name(&self) -> &'static str {
        "unconnected-pin"
    }

    fn check(&self, world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        let mut violations = Vec::new();
        let lib = FootprintLibrary::new();

        // Collect components first to avoid borrow issues
        let components: Vec<_> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(
                bevy_ecs::entity::Entity,
                &RefDes,
                &FootprintRef,
                &NetConnections,
                &Position,
            )>();
            query
                .iter(ecs)
                .map(|(e, r, f, n, p)| (e, r.clone(), f.clone(), n.clone(), *p))
                .collect()
        };

        for (entity, refdes, footprint_ref, nets, position) in components {
            // Look up footprint in library
            let Some(footprint) = lib.get(footprint_ref.as_str()) else {
                continue; // Unknown footprint - skip (already caught by sync)
            };

            // Check each pad has a net connection
            for pad in &footprint.pads {
                if nets.pin_net(&pad.number).is_none() {
                    // Calculate pad's absolute position for click-to-zoom
                    let pad_location = cypcb_core::Point::new(
                        cypcb_core::Nm(position.0.x.0 + pad.position.x.0),
                        cypcb_core::Nm(position.0.y.0 + pad.position.y.0),
                    );
                    violations.push(DrcViolation::unconnected_pin(
                        entity,
                        &pad.number,
                        refdes.as_str(),
                        pad_location,
                    ));
                }
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::Nm;
    use cypcb_world::components::{NetId, PinConnection, Rotation, Value};

    #[test]
    fn test_rule_name() {
        assert_eq!(UnconnectedPinRule.name(), "unconnected-pin");
    }

    #[test]
    fn test_unconnected_pin_detected() {
        // 0402 has 2 pins, only connect one
        let mut world = BoardWorld::new();
        let vcc = world.intern_net("VCC");

        let mut nets = NetConnections::new();
        nets.add(PinConnection::new("1", vcc)); // Only pin 1 connected

        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 20.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            nets,
        );

        let rules = DesignRules::default();
        let violations = UnconnectedPinRule.check(&mut world, &rules);

        assert_eq!(violations.len(), 1, "Should detect 1 unconnected pin");
        assert_eq!(violations[0].kind, crate::ViolationKind::UnconnectedPin);
        assert!(violations[0].message.contains("R1.2"), "Should report pin 2");
    }

    #[test]
    fn test_fully_connected_no_violation() {
        // 0402 has 2 pins, connect both
        let mut world = BoardWorld::new();
        let vcc = world.intern_net("VCC");
        let gnd = world.intern_net("GND");

        let mut nets = NetConnections::new();
        nets.add(PinConnection::new("1", vcc));
        nets.add(PinConnection::new("2", gnd));

        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 20.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            nets,
        );

        let rules = DesignRules::default();
        let violations = UnconnectedPinRule.check(&mut world, &rules);

        assert!(violations.is_empty(), "Fully connected component should pass");
    }

    #[test]
    fn test_all_pins_unconnected() {
        // No net connections at all
        let mut world = BoardWorld::new();
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 20.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let rules = DesignRules::default();
        let violations = UnconnectedPinRule.check(&mut world, &rules);

        assert_eq!(violations.len(), 2, "Both pins should be unconnected");
    }

    #[test]
    fn test_empty_world_no_violations() {
        let mut world = BoardWorld::new();
        let rules = DesignRules::default();
        let violations = UnconnectedPinRule.check(&mut world, &rules);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_unknown_footprint_skipped() {
        let mut world = BoardWorld::new();
        world.spawn_component(
            RefDes::new("X1"),
            Value::new("Unknown"),
            Position::from_mm(10.0, 20.0),
            Rotation::ZERO,
            FootprintRef::new("NONEXISTENT-FOOTPRINT"),
            NetConnections::new(),
        );

        let rules = DesignRules::default();
        let violations = UnconnectedPinRule.check(&mut world, &rules);
        assert!(violations.is_empty(), "Unknown footprints should be skipped");
    }

    #[test]
    fn test_multiple_components() {
        let mut world = BoardWorld::new();
        let vcc = world.intern_net("VCC");
        let gnd = world.intern_net("GND");

        // R1: Fully connected
        let mut nets1 = NetConnections::new();
        nets1.add(PinConnection::new("1", vcc));
        nets1.add(PinConnection::new("2", gnd));
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            nets1,
        );

        // R2: Only pin 1 connected
        let mut nets2 = NetConnections::new();
        nets2.add(PinConnection::new("1", vcc));
        world.spawn_component(
            RefDes::new("R2"),
            Value::new("4.7k"),
            Position::from_mm(20.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            nets2,
        );

        // R3: Completely unconnected
        world.spawn_component(
            RefDes::new("R3"),
            Value::new("1k"),
            Position::from_mm(30.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let rules = DesignRules::default();
        let violations = UnconnectedPinRule.check(&mut world, &rules);

        // R1: 0 violations, R2: 1 violation (pin 2), R3: 2 violations (pins 1 and 2)
        assert_eq!(violations.len(), 3, "Expected 3 violations total");

        let messages: Vec<_> = violations.iter().map(|v| v.message.clone()).collect();
        assert!(messages.iter().any(|m| m.contains("R2.2")));
        assert!(messages.iter().any(|m| m.contains("R3.1")));
        assert!(messages.iter().any(|m| m.contains("R3.2")));
        // R1 should NOT appear
        assert!(!messages.iter().any(|m| m.contains("R1")));
    }

    #[test]
    fn test_ic_footprint_many_pins() {
        // SOIC-8 has 8 pins, connect only 4
        let mut world = BoardWorld::new();
        let vcc = world.intern_net("VCC");
        let gnd = world.intern_net("GND");
        let sig1 = world.intern_net("SIG1");
        let sig2 = world.intern_net("SIG2");

        let mut nets = NetConnections::new();
        nets.add(PinConnection::new("1", sig1));
        nets.add(PinConnection::new("4", gnd));
        nets.add(PinConnection::new("5", sig2));
        nets.add(PinConnection::new("8", vcc));
        // Pins 2, 3, 6, 7 are unconnected

        world.spawn_component(
            RefDes::new("U1"),
            Value::new("ATtiny85"),
            Position::from_mm(50.0, 50.0),
            Rotation::ZERO,
            FootprintRef::new("SOIC-8"),
            nets,
        );

        let rules = DesignRules::default();
        let violations = UnconnectedPinRule.check(&mut world, &rules);

        assert_eq!(violations.len(), 4, "Should detect 4 unconnected pins");
        let messages: Vec<_> = violations.iter().map(|v| v.message.clone()).collect();
        assert!(messages.iter().any(|m| m.contains("U1.2")));
        assert!(messages.iter().any(|m| m.contains("U1.3")));
        assert!(messages.iter().any(|m| m.contains("U1.6")));
        assert!(messages.iter().any(|m| m.contains("U1.7")));
    }
}
