//! The stencil has to survive being made.
//!
//! A solder paste stencil is a steel sheet with a hole cut for every SMD pad.
//! Where two holes come closer than [`DesignRules::min_paste_clearance`], the
//! web of steel between them is too thin to hold: it tears, the two openings
//! become one, and the parts bridge with solder on reflow. Fine-pitch parts
//! are where this bites, which is the same place the solder mask bridge rule
//! bites and for the same physical reason - a thin web of something.
//!
//! Every fab preset in this project has published a paste clearance since the
//! tables were written, and until now **nothing read it**: the field appeared
//! thirteen times inside `cypcb-rules` and nowhere else in the workspace.
//!
//! A paste opening is the pad itself. `MaskPasteConfig::paste_reduction` is
//! 0.0 - aperture equals pad - and a reduction is a stencil design decision
//! rather than a number a fabricator publishes, so this rule does not invent
//! one.

use cypcb_core::{Nm, Point};
use cypcb_world::components::{FootprintRef, Layer, Position, Rotation};
use cypcb_world::BoardWorld;

use super::{rotate_point, DrcRule};
use crate::presets::DesignRules;
use crate::violation::DrcViolation;

/// One pad's paste opening, as an axis-aligned rectangle.
struct PasteOpening {
    entity: bevy_ecs::entity::Entity,
    center: Point,
    half_width: i64,
    half_height: i64,
    /// Which face of the board the stencil is for.
    top_side: bool,
}

/// Rule for checking the web between two paste stencil openings.
pub struct PasteClearanceRule;

impl DrcRule for PasteClearanceRule {
    fn name(&self) -> &'static str {
        "paste-clearance"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let min_web = rules.min_paste_clearance.0;

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
        let mut openings: Vec<PasteOpening> = Vec::new();

        for (entity, footprint_ref, position, rotation) in &components {
            let Some(footprint) = lib.get(footprint_ref.as_str()) else {
                continue; // Unknown footprint - sync already reported it
            };
            let degrees = rotation.to_degrees();
            let quarter_turn = (degrees.rem_euclid(180.0) - 90.0).abs() < 0.001;

            for pad in &footprint.pads {
                // A through-hole pad is soldered by wave or by hand and gets
                // no stencil aperture at all.
                if !pad.is_smd() {
                    continue;
                }

                let offset = rotate_point(pad.position, degrees);
                let center = Point::new(
                    Nm(position.0.x.0 + offset.x.0),
                    Nm(position.0.y.0 + offset.y.0),
                );
                let (w, h) = if quarter_turn {
                    (pad.size.1 .0, pad.size.0 .0)
                } else {
                    (pad.size.0 .0, pad.size.1 .0)
                };

                for top_side in [true, false] {
                    let layer = if top_side {
                        Layer::TopCopper
                    } else {
                        Layer::BottomCopper
                    };
                    if !pad.layers.contains(&layer) {
                        continue;
                    }
                    openings.push(PasteOpening {
                        entity: *entity,
                        center,
                        // The aperture is the pad: no expansion, no reduction.
                        half_width: w / 2,
                        half_height: h / 2,
                        top_side,
                    });
                }
            }
        }

        // Swept by x, the same way the mask bridge rule is, so a dense board
        // does not turn into an n-squared scan.
        openings.sort_by_key(|o| o.center.x.0);
        let mut violations = Vec::new();

        for (i, a) in openings.iter().enumerate() {
            for b in openings[i + 1..].iter() {
                let dx = b.center.x.0 - a.center.x.0;
                if dx - a.half_width - b.half_width >= min_web {
                    break;
                }
                if a.top_side != b.top_side {
                    continue; // Two stencils, one per face
                }

                let gap_x = dx.abs() - a.half_width - b.half_width;
                let gap_y = (b.center.y.0 - a.center.y.0).abs() - a.half_height - b.half_height;
                // Two rectangles clear each other along whichever axis
                // separates them, so the wider gap is the web that has to
                // hold.
                let web = gap_x.max(gap_y);

                if web < min_web {
                    let location = Point::new(
                        Nm((a.center.x.0 + b.center.x.0) / 2),
                        Nm((a.center.y.0 + b.center.y.0) / 2),
                    );
                    violations.push(DrcViolation::paste_clearance(
                        a.entity,
                        b.entity,
                        Nm(web.max(0)),
                        Nm(min_web),
                        location,
                    ));
                }
            }
        }

        violations
    }
}
