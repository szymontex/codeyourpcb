//! Hole-to-hole clearance rule.
//!
//! Checks minimum distance between drill holes (edge-to-edge).
//! Applies to through-hole pads, vias, and mounting holes.

use cypcb_core::{Nm, Point};
use cypcb_world::components::Layer;
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// How deep a layer sits, for comparing one span against another.
///
/// The board's own count is not needed: the top face is above every inner
/// layer and the bottom face below all of them, whatever the stack.
fn depth(layer: Layer) -> u16 {
    match layer {
        Layer::TopCopper => 0,
        Layer::Inner(n) => n as u16 + 1,
        _ => u16::MAX,
    }
}

/// Whether two holes are made in the same drill pass.
///
/// A hole joins a range of the stack. Two ranges that overlap are drilled
/// through the same material and have to keep their distance; two that do not
/// are made on different sub-stacks, before the board is pressed together.
fn passes_overlap(a: (Layer, Layer), b: (Layer, Layer)) -> bool {
    let (a_start, a_end) = (depth(a.0).min(depth(a.1)), depth(a.0).max(depth(a.1)));
    let (b_start, b_end) = (depth(b.0).min(depth(b.1)), depth(b.0).max(depth(b.1)));
    a_start < b_end && b_start < a_end
}

/// Rule for checking minimum hole-to-hole clearance.
pub struct HoleToHoleRule;

impl DrcRule for HoleToHoleRule {
    fn name(&self) -> &'static str {
        "hole-to-hole"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let mut violations = Vec::new();
        let min_distance = rules.min_hole_to_hole;

        // Every drilled feature on the board: vias and through-hole pads. A via
        // 0.2mm from a connector pin is as unmanufacturable as two vias that
        // close, so checking only via-to-via missed most of the real cases.
        //
        // Each hole is the shape the machine leaves rather than a point: a
        // drill gives a circle and a milled slot a capsule, and two slots end
        // to end are as close as their ends, not as their centres.
        let holes = super::holes_of(world);

        for i in 0..holes.len() {
            for j in (i + 1)..holes.len() {
                let (a, b) = (&holes[i], &holes[j]);

                // Two pads of the same component are placed by the footprint,
                // not by the designer, so a footprint's own pitch is not a
                // board defect to report here.
                if a.entity == b.entity {
                    continue;
                }

                // Each hole carries the layers it joins. Two holes only meet
                // if the drill passes that make them overlap: a via buried
                // between In1 and In2 and one between In3 and the bottom are
                // made in different passes on different sub-stacks, and
                // reporting them as too close is a fault the board does not
                // have.
                if !passes_overlap(a.span, b.span) {
                    continue;
                }

                let gap = a.gap_to(b);
                if gap < min_distance.0 {
                    let centre_a = a.centre();
                    let centre_b = b.centre();
                    let location = Point::new(
                        Nm((centre_a.x.0 + centre_b.x.0) / 2),
                        Nm((centre_a.y.0 + centre_b.y.0) / 2),
                    );
                    violations.push(DrcViolation::hole_to_hole(
                        a.entity,
                        b.entity,
                        Nm(gap.max(0)),
                        min_distance,
                        location,
                    ));
                }
            }
        }

        violations
    }
}

#[cfg(test)]
mod drill_pass_tests {
    use super::*;

    #[test]
    fn a_through_hole_shares_a_pass_with_everything() {
        let through = (Layer::TopCopper, Layer::BottomCopper);
        assert!(passes_overlap(through, through));
        assert!(passes_overlap(through, (Layer::TopCopper, Layer::Inner(0))));
        assert!(passes_overlap(through, (Layer::Inner(0), Layer::Inner(1))));
    }

    #[test]
    fn two_buried_vias_on_different_sub_stacks_never_meet() {
        // In1..In2 is drilled before the board is pressed together with the
        // half that carries In3..Bottom. Reporting them as too close is a
        // fault the board does not have.
        let upper = (Layer::Inner(0), Layer::Inner(1));
        let lower = (Layer::Inner(2), Layer::BottomCopper);
        assert!(!passes_overlap(upper, lower));
    }

    #[test]
    fn spans_that_only_touch_at_a_layer_are_not_one_pass() {
        // Top..In1 and In1..Bottom meet at In1 and are still two passes.
        assert!(!passes_overlap(
            (Layer::TopCopper, Layer::Inner(0)),
            (Layer::Inner(0), Layer::BottomCopper),
        ));
    }

    #[test]
    fn overlapping_spans_share_a_pass() {
        assert!(passes_overlap(
            (Layer::TopCopper, Layer::Inner(1)),
            (Layer::Inner(0), Layer::BottomCopper),
        ));
    }
}
