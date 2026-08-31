//! Solder mask bridge rule.
//!
//! The mask opening around a pad is the pad grown by
//! [`DesignRules::solder_mask_expansion`] on every side, unless the pad asks
//! for its own - `mask 0.1016mm` in this language, `(solder_mask_margin ...)`
//! in KiCad's. Where two openings come closer than
//! [`DesignRules::min_solder_mask_bridge`], the fab cannot hold a web of mask
//! between them and the pads bridge with solder during reflow. Fine-pitch
//! parts are where this bites.
//!
//! Measured 2026-08-31: of this crate's 37 rules, **this is the only one that
//! measures a mask opening at all**, and it grew every pad by the board's
//! figure. A pad asking for more than the fabricator's default - 124 pads in
//! this repository's KiCad files ask for 4 mil, against a 2 mil default -
//! therefore had its web measured wider than it is, and the checker passed a
//! board the exporter then made with openings that touch.

use cypcb_core::{Nm, Point};
use cypcb_world::components::{FootprintRef, Layer, Position, Rotation};
use cypcb_world::BoardWorld;

use super::{rotate_point, DrcRule};
use crate::presets::DesignRules;
use crate::violation::DrcViolation;

/// One pad's solder mask opening, as an axis-aligned rectangle.
struct MaskOpening {
    entity: bevy_ecs::entity::Entity,
    center: Point,
    half_width: i64,
    half_height: i64,
    /// Which side of the board the opening is on.
    top_side: bool,
}

/// Rule for checking minimum solder mask bridge width.
pub struct SolderMaskBridgeRule;

impl DrcRule for SolderMaskBridgeRule {
    fn name(&self) -> &'static str {
        "solder-mask-bridge"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let board_expansion = rules.solder_mask_expansion.0;
        let min_bridge = rules.min_solder_mask_bridge.0;

        let components: Vec<_> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(
                bevy_ecs::entity::Entity,
                &FootprintRef,
                &Position,
                &Rotation,
            )>();
            query
                .iter(ecs)
                .map(|(e, f, p, r)| (e, f.clone(), *p, *r))
                .collect()
        };

        let lib = world.footprints();
        let mut openings: Vec<MaskOpening> = Vec::new();

        for (entity, footprint_ref, position, rotation) in &components {
            let Some(footprint) = lib.get(footprint_ref.as_str()) else {
                continue; // Unknown footprint - sync already reported it
            };
            let degrees = rotation.to_degrees();
            let quarter_turn = is_quarter_turn(degrees);

            for pad in &footprint.pads {
                let offset = rotate_point(pad.position, degrees);
                let center = Point::new(
                    Nm(position.0.x.0 + offset.x.0),
                    Nm(position.0.y.0 + offset.y.0),
                );
                // A quarter turn swaps the pad's own axes.
                let (w, h) = if quarter_turn {
                    (pad.size.1 .0, pad.size.0 .0)
                } else {
                    (pad.size.0 .0, pad.size.1 .0)
                };

                // The opening this pad gets made with: its own margin where
                // it asks for one, the board's where it does not. The same
                // question the mask exporter asks, so the checker measures the
                // board the exporter writes. `None` is the board's figure and
                // is not a zero.
                let expansion = pad.mask_margin.map_or(board_expansion, |m| m.0);

                for top_side in [true, false] {
                    let layer = if top_side {
                        Layer::TopCopper
                    } else {
                        Layer::BottomCopper
                    };
                    if !pad.layers.contains(&layer) {
                        continue;
                    }
                    openings.push(MaskOpening {
                        entity: *entity,
                        center,
                        half_width: w / 2 + expansion,
                        half_height: h / 2 + expansion,
                        top_side,
                    });
                }
            }
        }

        // Sweep by x so a dense board does not turn into an n-squared scan.
        openings.sort_by_key(|o| o.center.x.0);
        let mut violations = Vec::new();

        for (i, a) in openings.iter().enumerate() {
            for b in openings[i + 1..].iter() {
                let dx = b.center.x.0 - a.center.x.0;
                // Sorted by x: once the horizontal gap alone clears the rule,
                // every later opening clears it too.
                if dx - a.half_width - b.half_width >= min_bridge {
                    break;
                }
                if a.top_side != b.top_side {
                    continue; // Opposite sides of the board
                }

                let gap_x = dx.abs() - a.half_width - b.half_width;
                let gap_y = (b.center.y.0 - a.center.y.0).abs() - a.half_height - b.half_height;
                // Two rectangles clear each other along whichever axis separates
                // them, so the wider of the two gaps is the actual mask web.
                let web = gap_x.max(gap_y);

                if web < min_bridge {
                    let location = Point::new(
                        Nm((a.center.x.0 + b.center.x.0) / 2),
                        Nm((a.center.y.0 + b.center.y.0) / 2),
                    );
                    violations.push(DrcViolation::solder_mask_bridge(
                        a.entity,
                        b.entity,
                        Nm(web.max(0)),
                        Nm(min_bridge),
                        location,
                    ));
                }
            }
        }

        violations
    }
}

/// Is this rotation a quarter turn, where the pad's own axes swap?
fn is_quarter_turn(degrees: f64) -> bool {
    (degrees.rem_euclid(180.0) - 90.0).abs() < 0.001
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_world::components::{NetConnections, RefDes, Value};

    fn spawn_0402(world: &mut BoardWorld, refdes: &str, x_mm: f64) {
        world.spawn_component(
            RefDes::new(refdes),
            Value::new("10k"),
            Position::from_mm(x_mm, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );
    }

    #[test]
    fn rule_name() {
        assert_eq!(SolderMaskBridgeRule.name(), "solder-mask-bridge");
    }

    #[test]
    fn a_lone_footprints_own_pads_keep_their_web() {
        // An 0402's two pads are 0.9mm apart with 0.5mm of copper between them;
        // the mask web survives a 0.05mm expansion on each side.
        let mut world = BoardWorld::new();
        spawn_0402(&mut world, "R1", 10.0);
        let rules = DesignRules::jlcpcb_2layer();
        assert!(SolderMaskBridgeRule.check(&mut world, &rules).is_empty());
    }

    #[test]
    fn parts_placed_far_apart_are_fine() {
        let mut world = BoardWorld::new();
        spawn_0402(&mut world, "R1", 10.0);
        spawn_0402(&mut world, "R2", 15.0);
        let rules = DesignRules::jlcpcb_2layer();
        assert!(SolderMaskBridgeRule.check(&mut world, &rules).is_empty());
    }

    #[test]
    fn overlapping_neighbours_are_reported() {
        // Half a millimetre apart: the facing pads' openings run into each other.
        let mut world = BoardWorld::new();
        spawn_0402(&mut world, "R1", 10.0);
        spawn_0402(&mut world, "R2", 10.5);

        let rules = DesignRules::jlcpcb_2layer();
        let violations = SolderMaskBridgeRule.check(&mut world, &rules);
        assert!(!violations.is_empty());
        assert_eq!(violations[0].kind, crate::ViolationKind::SolderMaskBridge);
        assert!(violations[0].message.contains("0.10mm required"));
    }

    #[test]
    fn a_wider_expansion_eats_the_web() {
        // 0402 pads are 0.6mm wide on a 1.0mm span, so two parts 2.0mm apart
        // leave 0.3mm of web at the default 0.05mm expansion - and none at all
        // once the fab opens the mask by 0.3mm. The rule has to follow the
        // preset, not a constant.
        let mut world = BoardWorld::new();
        spawn_0402(&mut world, "R1", 10.0);
        spawn_0402(&mut world, "R2", 12.0);

        let tight = DesignRules::jlcpcb_2layer();
        assert!(SolderMaskBridgeRule.check(&mut world, &tight).is_empty());

        let generous = DesignRules {
            solder_mask_expansion: Nm::from_mm(0.3),
            ..tight
        };
        assert!(!SolderMaskBridgeRule.check(&mut world, &generous).is_empty());
    }
}
