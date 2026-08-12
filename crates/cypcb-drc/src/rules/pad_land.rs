//! The land a fab will put around a hole.
//!
//! D6, settled by the owner from two published rows rather than a preference:
//! `min_pad_size` is a via land and a through-hole pad minimum, **not** an SMD
//! land. JLCPCB publishes both and they are different rows -
//! `Min. Via hole size/diameter: 0.15mm / 0.25mm`, a hole paired with a
//! diameter, and `Minimum SMD pad: 0.25mm x 0.25mm` separately.
//!
//! `ViaDiameterRule` already asks this of a via. Nothing asked it of a drilled
//! pad, which is the same copper around the same kind of hole, so a connector
//! whose land is under the fab's floor was reported by nobody.
//!
//! # Not the same question as the annular ring
//!
//! `AnnularRingRule` measures `(land - drill) / 2` against the ring the fab
//! needs to keep copper attached once the hole is drilled off-centre. This
//! measures the land itself against the smallest one the fab will make. A pad
//! can pass one and fail the other: a 0.4mm land on a 0.1mm drill has a
//! comfortable 0.15mm ring and is still smaller than anything JLCPCB will
//! image.

use cypcb_core::{Nm, Point};
use cypcb_world::components::{FootprintRef, Position, RefDes};
use cypcb_world::BoardWorld;

use super::DrcRule;
use crate::presets::DesignRules;
use crate::violation::DrcViolation;

/// Rule that checks the land around a drilled pad.
pub struct PadLandRule;

impl DrcRule for PadLandRule {
    fn name(&self) -> &'static str {
        "pad-land"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let mut violations = Vec::new();
        let min_land = rules.min_pad_size;

        let components: Vec<_> = {
            let ecs = world.ecs_mut();
            let mut query =
                ecs.query::<(bevy_ecs::entity::Entity, &RefDes, &FootprintRef, &Position)>();
            query
                .iter(ecs)
                .map(|(e, r, f, p)| (e, r.clone(), f.clone(), *p))
                .collect()
        };

        // The board carries the table it was synced with, including any
        // footprint the source defined inline; a fresh library would see the
        // built-ins only.
        let library = world.footprints();
        for (entity, refdes, footprint_ref, position) in components {
            let Some(footprint) = library.get(footprint_ref.as_str()) else {
                continue; // Unknown footprint - sync already reported it
            };

            for pad in &footprint.pads {
                // A mounting hole has no copper around it on purpose, and an
                // SMD pad has no hole for a land to be around. Neither is this
                // rule's subject, and reporting either was the exact mistake
                // D6 was raised to settle: the measurement that refuted the
                // SMD reading flagged 16 USB-C pads at 0.250mm, which is
                // JLCPCB's published SMD floor to the micrometre.
                if pad.is_non_plated() {
                    continue;
                }
                let Some(drill) = pad.drill else {
                    continue;
                };

                // The narrow way, because a land is only as small as its
                // smallest dimension: an oblong pad 2.0 x 0.3mm images at
                // 0.3mm however long it is.
                let land = pad.size.0.min(pad.size.1);
                if land < min_land {
                    let location = Point::new(
                        Nm(position.0.x.0 + pad.position.x.0),
                        Nm(position.0.y.0 + pad.position.y.0),
                    );
                    violations.push(DrcViolation::pad_land(
                        entity,
                        format!("{}.{}", refdes.as_str(), pad.number),
                        land,
                        drill,
                        min_land,
                        location,
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
    use crate::ViolationKind;
    use cypcb_world::components::{Layer, NetConnections, PadShape, Rotation, Value};
    use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};

    /// A board with one part whose single drilled pad is `land` wide.
    fn board_with_land(land_mm: f64, drill_mm: f64) -> BoardWorld {
        let mut world = BoardWorld::new();
        world.set_board("land".into(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);

        let mut library = FootprintLibrary::new();
        let base = library
            .get("0402")
            .expect("the library has an 0402")
            .clone();
        library.register_design(Footprint {
            name: "pin".to_string(),
            pads: vec![PadDef {
                number: "1".to_string(),
                shape: PadShape::Circle,
                position: Point::ORIGIN,
                size: (Nm::from_mm(land_mm), Nm::from_mm(land_mm)),
                drill: Some(Nm::from_mm(drill_mm)),
                slot: None,
                layers: vec![Layer::TopCopper, Layer::BottomCopper],
            }],
            ..base
        });
        world.set_footprints(library);

        world.spawn_component(
            RefDes::new("J1"),
            Value::new(""),
            Position::from_mm(10.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("pin"),
            NetConnections::new(),
        );
        world
    }

    #[test]
    fn a_land_under_the_fabs_floor_is_reported() {
        // JLCPCB will not image a land under 0.5mm around a hole.
        let mut world = board_with_land(0.4, 0.2);
        let violations = PadLandRule.check(&mut world, &DesignRules::jlcpcb_2layer());
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert_eq!(violations[0].kind, ViolationKind::PadLand);
        assert!(
            violations[0].message.contains("J1.1"),
            "the message names the pin: {}",
            violations[0].message
        );
    }

    #[test]
    fn a_land_at_the_floor_passes() {
        // Exactly at the published figure is legal. A rule that fails here
        // fails every part built to the fab's own minimum.
        let mut world = board_with_land(0.5, 0.2);
        let violations = PadLandRule.check(&mut world, &DesignRules::jlcpcb_2layer());
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn a_generous_ring_does_not_excuse_a_small_land() {
        // The distinction this rule exists for. A 0.4mm land on a 0.1mm drill
        // has a 0.15mm ring, which `AnnularRingRule` is happy with - and the
        // land is still smaller than anything the fab will image.
        let mut world = board_with_land(0.4, 0.1);
        let rules = DesignRules::jlcpcb_2layer();
        let ring = (Nm::from_mm(0.4).0 - Nm::from_mm(0.1).0) / 2;
        assert!(
            Nm(ring) >= rules.min_annular_ring,
            "the premise of this test: {ring}nm of ring against {}nm required",
            rules.min_annular_ring.0
        );
        assert_eq!(PadLandRule.check(&mut world, &rules).len(), 1);
    }

    #[test]
    fn an_smd_pad_has_no_land_to_measure() {
        // What D6 settled, at the number that settled it. The measurement
        // which refuted the SMD reading flagged 16 USB-C pads at 0.250mm -
        // JLCPCB's published SMD floor to the micrometre - against a figure
        // that governs holes.
        //
        // The pad below is that 0.250mm, which is half the 0.5mm this rule
        // enforces, so a rule that measured SMD lands would certainly report
        // it. The built-in 0402 cannot make this point: its own land is not
        // under 0.5mm, so it would pass either way. That was the first version
        // of this test and a mutation letting SMD pads through did not fail it.
        let mut world = BoardWorld::new();
        world.set_board("smd".into(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);

        let mut library = FootprintLibrary::new();
        let base = library
            .get("0402")
            .expect("the library has an 0402")
            .clone();
        library.register_design(Footprint {
            name: "usbc_pad".to_string(),
            pads: vec![PadDef {
                number: "A1".to_string(),
                shape: PadShape::Rect,
                position: Point::ORIGIN,
                size: (Nm::from_mm(0.25), Nm::from_mm(1.0)),
                drill: None,
                slot: None,
                layers: vec![Layer::TopCopper],
            }],
            ..base
        });
        world.set_footprints(library);
        world.spawn_component(
            RefDes::new("J9"),
            Value::new("USB-C"),
            Position::from_mm(10.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("usbc_pad"),
            NetConnections::new(),
        );

        let rules = DesignRules::jlcpcb_2layer();
        assert!(
            Nm::from_mm(0.25) < rules.min_pad_size,
            "the premise: 0.25mm is under the {}nm this rule enforces",
            rules.min_pad_size.0
        );
        let violations = PadLandRule.check(&mut world, &rules);
        assert!(
            violations.is_empty(),
            "an SMD pad has no hole, so it has no land this figure governs: {violations:?}"
        );
    }
}
