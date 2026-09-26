//! A footprint whose courtyard does not hold its own pads.
//!
//! The courtyard is the one box every other question about a part's space
//! reads: the spatial index files the part under it, `courtyard-clearance`
//! keeps parts apart by it, and the placer packs by it. Per IPC-7351 it
//! encloses the land pattern. A footprint that states a courtyard smaller than
//! its pads, or one centred somewhere else, has copper none of those readers
//! can see, and nothing said so: a benchmark connector carried pads outside
//! its courtyard until `ClearanceRule` grew a workaround for it.
//!
//! The fault is in the footprint, so a footprint is reported once however many
//! parts use it, at the first of its pads that reaches out on the first part
//! by reference designator.

use cypcb_world::in_build_order;
use std::collections::BTreeMap;

use cypcb_world::components::{place_pad, FootprintRef, Position, RefDes, Rotation};
use cypcb_world::BoardWorld;

use super::DrcRule;
use crate::presets::DesignRules;
use crate::violation::DrcViolation;

/// Rule that checks a footprint's courtyard holds every one of its pads.
pub struct LandOutsideCourtyardRule;

impl DrcRule for LandOutsideCourtyardRule {
    fn name(&self) -> &'static str {
        "land-outside-courtyard"
    }

    fn check(&self, world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        let mut components: Vec<_> = {
            let ecs = world.ecs_mut();
            in_build_order::<(
                bevy_ecs::entity::Entity,
                &RefDes,
                &FootprintRef,
                &Position,
                Option<&Rotation>,
            )>(ecs)
            .into_iter()
            .map(|(e, r, f, p, rot)| {
                (
                    e,
                    r.as_str().to_string(),
                    f.as_str().to_string(),
                    p.0,
                    rot.copied().unwrap_or(Rotation::ZERO),
                )
            })
            .collect()
        };

        // First part per footprint, by reference designator, so the row lands
        // in the same place on every run.
        components.sort_by(|a, b| a.1.cmp(&b.1));
        let mut first = BTreeMap::new();
        for (entity, refdes, name, position, rotation) in components {
            first
                .entry(name)
                .or_insert((entity, refdes, position, rotation));
        }

        let library = world.footprints();
        let mut violations = Vec::new();
        for (name, (entity, refdes, position, rotation)) in first {
            let Some(footprint) = library.get(&name) else {
                continue; // Unknown footprint - sync already reported it
            };
            let outside = footprint.pads_outside_courtyard();
            let Some(&(pad, reach)) = outside.first() else {
                continue;
            };
            violations.push(DrcViolation::land_outside_courtyard(
                entity,
                name.clone(),
                refdes,
                pad.number.clone(),
                outside.len(),
                footprint.pads.len(),
                reach,
                place_pad(position, pad.position, rotation),
            ));
        }
        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ViolationKind;
    use cypcb_core::{Nm, Point, Rect};
    use cypcb_world::components::{Layer, NetConnections, PadShape, Value};
    use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};

    fn pad(number: &str, x_mm: f64) -> PadDef {
        PadDef {
            number: number.to_string(),
            shape: PadShape::Rect,
            position: Point::from_mm(x_mm, 0.0),
            size: (Nm::from_mm(0.6), Nm::from_mm(0.6)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
        }
    }

    /// Two pads 6mm apart; the courtyard is `width_mm` wide about the origin.
    fn bar(name: &str, width_mm: f64) -> Footprint {
        let courtyard =
            Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(width_mm), Nm::from_mm(1.0)));
        Footprint {
            name: name.to_string(),
            description: String::new(),
            pads: vec![pad("1", -3.0), pad("2", 3.0)],
            bounds: courtyard,
            courtyard,
            silk: Vec::new(),
        }
    }

    fn board(parts: &[(&str, &str, f64)]) -> BoardWorld {
        let mut world = BoardWorld::new();
        world.set_board("t".into(), (Nm::from_mm(40.0), Nm::from_mm(40.0)), 2);
        let mut library = FootprintLibrary::new();
        library.register(bar("WHOLE", 7.0));
        // 6.4mm reaches 3.2mm each way; a pad's outer edge is at 3.3mm.
        library.register(bar("SHORT", 6.4));
        world.set_footprints(library);
        for (i, (refdes, footprint, degrees)) in parts.iter().enumerate() {
            world.spawn_component(
                RefDes::new(*refdes),
                Value::new(""),
                Position::from_mm(10.0 + 10.0 * i as f64, 10.0),
                Rotation::from_degrees(*degrees),
                FootprintRef::new(*footprint),
                NetConnections::new(),
            );
        }
        world
    }

    #[test]
    fn a_courtyard_that_holds_its_pads_says_nothing() {
        let mut world = board(&[("U1", "WHOLE", 0.0)]);
        let rows = LandOutsideCourtyardRule.check(&mut world, &DesignRules::default());
        assert!(rows.is_empty(), "{rows:?}");
    }

    #[test]
    fn a_footprint_is_reported_once_at_its_first_part() {
        let mut world = board(&[("U2", "SHORT", 0.0), ("U1", "SHORT", 90.0)]);
        let rows = LandOutsideCourtyardRule.check(&mut world, &DesignRules::default());
        assert_eq!(rows.len(), 1, "{rows:?}");
        let row = &rows[0];
        assert_eq!(row.kind, ViolationKind::LandOutsideCourtyard);
        assert_eq!(row.actual, Some(Nm::from_mm(0.1)));
        assert!(row.message.contains("2 of 2 pads"), "{}", row.message);
        assert!(row.message.contains("on U1"), "{}", row.message);
        // U1 is the second part, at x = 20mm, turned 90 degrees: its pad 1
        // sits 3mm below its centre rather than 3mm to the left.
        assert_eq!(row.location, Point::from_mm(20.0, 7.0));
    }
}
