//! DRC rule definitions and implementations.
//!
//! This module defines the [`DrcRule`] trait that all rules implement.
//! Design rules configuration is defined in the [`presets`](crate::presets) module.

pub mod annular_ring;
pub mod area_off_board;
pub mod area_overlap;
pub mod assertion;
pub mod bend_radius;
pub mod clearance;
pub mod connectivity;
pub mod courtyard_clearance;
mod diff_pair;
pub mod drill_aspect_ratio;
pub mod drill_size;
pub mod edge_clearance;
pub mod empty_area;
pub mod flex_hole;
pub mod flex_trace_angle;
pub mod hole_to_edge;
pub mod hole_to_hole;
pub mod impedance;
pub mod mounting_hole_clearance;
mod neck_down;
pub mod pad_land;
pub mod paste_clearance;
pub mod pour_island;
pub mod silk_clearance;
pub mod slot_clearance;
pub mod solder_mask_bridge;
pub mod solid_pour_in_bend;
pub mod stackup;
mod trace_current;
pub mod trace_width;
pub mod unrouted_pin;
pub mod via_diameter;
pub mod via_drill;
pub mod via_span;
pub mod zone_overlap;

use cypcb_world::BoardWorld;

/// Placement geometry, from the crate that owns the model.
///
/// This was a private copy here, one of five across the workspace. Two of them
/// had already drifted: `cypcb-export`'s copper writer truncated the rotated
/// offset toward zero where its drill writer rounded it.
pub(crate) use cypcb_world::components::rotate_about_origin as rotate_point;

/// Where a layer sits in the copper sequence, top first.
///
/// `Layer::Inner` is zero-based and the copper sequence is not: the first
/// inner layer is copper entry 1, which is the off-by-one this project has
/// shipped three times. Two rules carried byte-identical private copies of
/// this and a third was about to want it - which is how an index error gets
/// fixed in one file and left standing in the other.
pub(crate) fn copper_index(layer: cypcb_world::Layer, copper_count: usize) -> Option<usize> {
    match layer {
        cypcb_world::Layer::TopCopper => Some(0),
        cypcb_world::Layer::BottomCopper => copper_count.checked_sub(1),
        cypcb_world::Layer::Inner(n) => {
            let index = usize::from(n) + 1;
            (index < copper_count).then_some(index)
        }
        _ => None,
    }
}

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

pub use annular_ring::AnnularRingRule;
pub use area_off_board::AreaOffBoardRule;
pub use area_overlap::AreaOverlapRule;
pub use assertion::AssertionRule;
pub use bend_radius::BendRadiusRule;
pub use clearance::ClearanceRule;
pub use connectivity::UnconnectedPinRule;
pub use courtyard_clearance::CourtyardClearanceRule;
pub use diff_pair::DiffPairSkewRule;
pub use drill_aspect_ratio::DrillAspectRatioRule;
pub use drill_size::MinDrillSizeRule;
pub use edge_clearance::EdgeClearanceRule;
pub use empty_area::EmptyAreaRule;
pub use flex_hole::FlexHoleRule;
pub use flex_trace_angle::FlexTraceAngleRule;
pub use hole_to_edge::HoleToEdgeRule;
pub use hole_to_hole::HoleToHoleRule;
pub use impedance::ImpedanceRule;
pub use mounting_hole_clearance::MountingHoleClearanceRule;
pub use neck_down::NeckDownRule;
pub use pad_land::PadLandRule;
pub use paste_clearance::PasteClearanceRule;
pub use pour_island::PourIslandRule;
pub use silk_clearance::SilkClearanceRule;
pub use slot_clearance::SlotClearanceRule;
pub use solder_mask_bridge::SolderMaskBridgeRule;
pub use solid_pour_in_bend::SolidPourInBendRule;
pub use stackup::StackupRule;
pub use trace_current::TraceCurrentRule;
pub use trace_width::MinTraceWidthRule;
pub use unrouted_pin::UnroutedPinRule;
pub use via_diameter::ViaDiameterRule;
pub use via_drill::ViaDrillRule;
pub use via_span::ViaSpanRule;
pub use zone_overlap::ZoneOverlapRule;

/// A single DRC rule that can be executed against a board.
///
/// Rules are implemented as structs that hold no state. Configuration
/// comes from the [`DesignRules`] struct passed to `check()`.
///
/// # Object Safety
///
/// This trait is designed to be object-safe, allowing rules to be
/// stored in a `Vec<Box<dyn DrcRule>>` for flexible rule composition.
///
/// # Examples
///
/// ```rust,ignore
/// impl DrcRule for ClearanceRule {
///     fn name(&self) -> &'static str {
///         "clearance"
///     }
///
///     fn check(&self, world: &BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
///         // Implementation...
///         vec![]
///     }
/// }
/// ```
pub trait DrcRule: Send + Sync {
    /// Rule identifier for error messages and filtering.
    fn name(&self) -> &'static str;

    /// Execute the rule check against the board world.
    ///
    /// Returns a list of violations (empty if rule passes).
    ///
    /// # Arguments
    ///
    /// * `world` - The board world to check (mutable for ECS queries)
    /// * `rules` - Design rules configuration
    ///
    /// Note: Takes `&mut BoardWorld` because bevy_ecs queries need to
    /// initialize their cache. No actual board data is modified.
    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation>;
}

/// Rule for checking components against keepout zones.
///
/// This rule checks if any component's position falls within a keepout zone.
/// Note: Currently only checks center point - a more complete implementation
/// would check the entire component footprint bounds.
///
/// # Examples
///
/// ```rust,ignore
/// use cypcb_drc::rules::KeepoutRule;
/// use cypcb_drc::rules::DrcRule;
///
/// let rule = KeepoutRule;
/// let violations = rule.check(&world, &rules);
/// for v in violations {
///     println!("Keepout violation: {}", v.message);
/// }
/// ```
pub struct KeepoutRule;

impl DrcRule for KeepoutRule {
    fn name(&self) -> &'static str {
        "keepout"
    }

    fn check(&self, world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        let mut violations = Vec::new();

        // Collect all keepout zones
        let zones: Vec<_> = world
            .zones()
            .into_iter()
            .filter(|(_, zone)| zone.is_keepout())
            .collect();

        // If no keepout zones, skip
        if zones.is_empty() {
            return violations;
        }

        // Collect all components
        let components = world.components();

        // Check each component against keepout zones
        for (entity, refdes, position) in components {
            for (zone_entity, zone) in &zones {
                // Check if component center is inside zone
                if zone.contains(position.0) {
                    violations.push(DrcViolation::keepout(
                        entity,
                        *zone_entity,
                        refdes.as_str(),
                        zone.name.as_deref(),
                        position.0,
                    ));
                }
            }
        }

        violations
    }
}

/// One hole in the board, as the shape the machine actually makes.
///
/// A drill leaves a circle. A slot is milled, so the bit travels and leaves a
/// capsule: a segment with a radius. Every rule that measures a hole against
/// something has to measure that shape, because a slot read as a circle of its
/// narrow dimension is wrong by up to its whole length - two 2.4mm slots end
/// to end with 0.3mm of laminate between them look 1.7mm apart if you only
/// know their centres and their bits.
///
/// The three rules that ask about holes - hole to hole, hole to the routed
/// edge, and how deep a hole is for its width - collected vias and drilled
/// pads separately, three times, with the same twenty lines. They share this.
pub(crate) struct Hole {
    /// The component or via this hole belongs to.
    pub entity: bevy_ecs::entity::Entity,
    /// Where the bit goes down. Equal to `end` for a drilled hole.
    pub start: cypcb_core::Point,
    /// Where the bit comes up.
    pub end: cypcb_core::Point,
    /// Half the narrow dimension: the radius of the bit that makes it.
    pub radius: i64,
    /// The layers this hole joins, which says which drill pass makes it.
    pub span: (
        cypcb_world::components::Layer,
        cypcb_world::components::Layer,
    ),
    /// Whether copper is plated down the barrel.
    pub plated: bool,
}

impl Hole {
    /// The diameter of the bit, which is what every fab number about a drill
    /// is stated against.
    #[inline]
    pub fn diameter(&self) -> cypcb_core::Nm {
        cypcb_core::Nm(self.radius * 2)
    }

    /// The middle of the hole, for saying where a violation is.
    #[inline]
    pub fn centre(&self) -> cypcb_core::Point {
        cypcb_core::Point::new(
            cypcb_core::Nm((self.start.x.0 + self.end.x.0) / 2),
            cypcb_core::Nm((self.start.y.0 + self.end.y.0) / 2),
        )
    }

    /// The box the hole occupies: `(min_x, min_y, max_x, max_y)`.
    #[inline]
    pub fn bounds(&self) -> (i64, i64, i64, i64) {
        (
            self.start.x.0.min(self.end.x.0) - self.radius,
            self.start.y.0.min(self.end.y.0) - self.radius,
            self.start.x.0.max(self.end.x.0) + self.radius,
            self.start.y.0.max(self.end.y.0) + self.radius,
        )
    }

    /// Laminate between this hole's wall and another's, which can be negative
    /// when they overlap.
    pub fn gap_to(&self, other: &Hole) -> i64 {
        segment_distance(self.start, self.end, other.start, other.end) - self.radius - other.radius
    }
}

/// The closest approach of two segments, in nanometres.
///
/// Either may be a single point, which is the common case: a drilled hole is a
/// segment of zero length, and for two of those this is the plain distance
/// between their centres.
///
/// Solved as the smallest of the four endpoint-to-segment distances, with
/// crossing segments answering zero. The closed-form parametric solution is
/// shorter and is wrong for the case this exists for: two parallel segments
/// leave its denominator at zero, and the fallback branch pins one parameter
/// at an endpoint - so two slots laid end to end in a row, which is exactly
/// how a connector's two anchors sit, measured 2.8mm apart when their ends
/// were 1.4mm apart.
pub(crate) fn segment_distance(
    a0: cypcb_core::Point,
    a1: cypcb_core::Point,
    b0: cypcb_core::Point,
    b1: cypcb_core::Point,
) -> i64 {
    if segments_cross(a0, a1, b0, b1) {
        return 0;
    }
    let candidates = [
        point_to_segment(a0, b0, b1),
        point_to_segment(a1, b0, b1),
        point_to_segment(b0, a0, a1),
        point_to_segment(b1, a0, a1),
    ];
    candidates
        .into_iter()
        .fold(f64::MAX, f64::min)
        .round()
        .max(0.0) as i64
}

/// Distance from a point to a segment, which is a segment of zero length away
/// from a plain point-to-point distance.
fn point_to_segment(p: cypcb_core::Point, s0: cypcb_core::Point, s1: cypcb_core::Point) -> f64 {
    let (px, py) = (p.x.0 as f64, p.y.0 as f64);
    let (sx, sy) = (s0.x.0 as f64, s0.y.0 as f64);
    let (dx, dy) = ((s1.x.0 - s0.x.0) as f64, (s1.y.0 - s0.y.0) as f64);
    let length_squared = dx * dx + dy * dy;
    let t = if length_squared > 0.0 {
        (((px - sx) * dx + (py - sy) * dy) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (nx, ny) = (sx + t * dx, sy + t * dy);
    ((px - nx).powi(2) + (py - ny).powi(2)).sqrt()
}

/// Whether two segments touch or cross, which makes their distance zero.
fn segments_cross(
    a0: cypcb_core::Point,
    a1: cypcb_core::Point,
    b0: cypcb_core::Point,
    b1: cypcb_core::Point,
) -> bool {
    let side = |p: cypcb_core::Point, q: cypcb_core::Point, r: cypcb_core::Point| -> f64 {
        let (qx, qy) = ((q.x.0 - p.x.0) as f64, (q.y.0 - p.y.0) as f64);
        let (rx, ry) = ((r.x.0 - p.x.0) as f64, (r.y.0 - p.y.0) as f64);
        qx * ry - qy * rx
    };
    let (d1, d2) = (side(a0, a1, b0), side(a0, a1, b1));
    let (d3, d4) = (side(b0, b1, a0), side(b0, b1, a1));
    (d1 * d2 < 0.0) && (d3 * d4 < 0.0)
}

/// Every hole in the board: vias, and the pads that are drilled.
///
/// A via joins the layers it says; a drilled pad goes through the board.
pub(crate) fn holes_of(world: &mut BoardWorld) -> Vec<Hole> {
    use cypcb_core::{Nm, Point};
    use cypcb_world::components::trace::Via;
    use cypcb_world::components::{FootprintRef, Layer, Position, Rotation};

    let mut holes: Vec<Hole> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Via)>();
        query
            .iter(ecs)
            .map(|(entity, via)| Hole {
                entity,
                start: via.position,
                end: via.position,
                radius: via.drill.0 / 2,
                span: (via.start_layer, via.end_layer),
                // A via that is not plated joins nothing.
                plated: true,
            })
            .collect()
    };

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

    // The board carries the table it was synced with, including any footprint
    // the source defined inline; building a fresh one here would see the
    // built-ins only.
    let library = world.footprints();
    for (entity, footprint_ref, position, rotation) in &components {
        let Some(footprint) = library.get(footprint_ref.as_str()) else {
            continue; // Unknown footprint - sync already reported it
        };
        let degrees = rotation.to_degrees();

        for pad in &footprint.pads {
            let Some(drill) = pad.drill else { continue };
            let half = pad.slot_half_travel().unwrap_or(Point::ORIGIN);
            let place = |dx: i64, dy: i64| {
                let offset = rotate_point(
                    Point::new(Nm(pad.position.x.0 + dx), Nm(pad.position.y.0 + dy)),
                    degrees,
                );
                Point::new(
                    Nm(position.0.x.0 + offset.x.0),
                    Nm(position.0.y.0 + offset.y.0),
                )
            };
            holes.push(Hole {
                entity: *entity,
                start: place(-half.x.0, -half.y.0),
                end: place(half.x.0, half.y.0),
                radius: drill.0 / 2,
                // A drilled pad goes through the board.
                span: (Layer::TopCopper, Layer::BottomCopper),
                plated: !pad.is_non_plated(),
            });
        }
    }

    holes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ViolationKind;
    use cypcb_core::{Point, Rect};
    use cypcb_world::components::zone::ZoneKind;
    use cypcb_world::components::{
        FootprintRef, NetConnections, Position, RefDes, Rotation, Value, Zone,
    };

    #[test]
    fn test_trait_object_safe() {
        // Verify that DrcRule can be used as a trait object
        fn _assert_object_safe(_: &dyn DrcRule) {}
    }

    #[test]
    fn test_rule_names() {
        assert_eq!(ClearanceRule.name(), "clearance");
        assert_eq!(MinDrillSizeRule.name(), "min-drill-size");
        assert_eq!(MinTraceWidthRule.name(), "min-trace-width");
        assert_eq!(UnconnectedPinRule.name(), "unconnected-pin");
        assert_eq!(KeepoutRule.name(), "keepout");
        assert_eq!(EdgeClearanceRule.name(), "edge-clearance");
        assert_eq!(AnnularRingRule.name(), "annular-ring");
        assert_eq!(HoleToHoleRule.name(), "hole-to-hole");
        assert_eq!(ViaDiameterRule.name(), "via-diameter");
        assert_eq!(CourtyardClearanceRule.name(), "courtyard-clearance");
        assert_eq!(SolderMaskBridgeRule.name(), "solder-mask-bridge");
        assert_eq!(SilkClearanceRule.name(), "silk-clearance");
    }

    #[test]
    fn test_rule_check_empty_world() {
        let mut world = BoardWorld::new();
        let rules = DesignRules::default();

        // All rules should return empty on empty world
        assert!(ClearanceRule.check(&mut world, &rules).is_empty());
        assert!(MinDrillSizeRule.check(&mut world, &rules).is_empty());
        assert!(MinTraceWidthRule.check(&mut world, &rules).is_empty());
        assert!(UnconnectedPinRule.check(&mut world, &rules).is_empty());
        assert!(KeepoutRule.check(&mut world, &rules).is_empty());
        assert!(EdgeClearanceRule.check(&mut world, &rules).is_empty());
        assert!(AnnularRingRule.check(&mut world, &rules).is_empty());
        assert!(HoleToHoleRule.check(&mut world, &rules).is_empty());
        assert!(ViaDiameterRule.check(&mut world, &rules).is_empty());
        assert!(CourtyardClearanceRule.check(&mut world, &rules).is_empty());
        assert!(SolderMaskBridgeRule.check(&mut world, &rules).is_empty());
        assert!(SilkClearanceRule.check(&mut world, &rules).is_empty());
    }

    #[test]
    fn test_rule_trait_object_vec() {
        // Verify rules can be collected into a Vec<Box<dyn DrcRule>>
        let rules: Vec<Box<dyn DrcRule>> = vec![
            Box::new(ClearanceRule),
            Box::new(MinDrillSizeRule),
            Box::new(MinTraceWidthRule),
            Box::new(UnconnectedPinRule),
            Box::new(KeepoutRule),
            Box::new(EdgeClearanceRule),
            Box::new(AnnularRingRule),
            Box::new(HoleToHoleRule),
            Box::new(ViaDiameterRule),
            Box::new(CourtyardClearanceRule),
            Box::new(SolderMaskBridgeRule),
            Box::new(SilkClearanceRule),
        ];
        assert_eq!(rules.len(), 12);
    }

    #[test]
    fn test_keepout_rule_detects_violation() {
        let mut world = BoardWorld::new();
        let rules = DesignRules::default();

        // Create a keepout zone
        let zone = Zone {
            bounds: Rect::new(Point::from_mm(10.0, 10.0), Point::from_mm(20.0, 20.0)),
            kind: ZoneKind::Keepout,
            layer_mask: 0xFFFFFFFF,
            name: Some("test_zone".to_string()),
            net: None,
        };
        world.ecs_mut().spawn(zone);

        // Create a component inside the zone
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(15.0, 15.0), // Inside zone
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        // Run the rule
        let violations = KeepoutRule.check(&mut world, &rules);
        assert_eq!(violations.len(), 1, "expected 1 violation");
        assert_eq!(violations[0].kind, ViolationKind::KeepoutViolation);
        assert!(violations[0].message.contains("R1"));
        assert!(violations[0].message.contains("test_zone"));
    }

    #[test]
    fn test_keepout_rule_no_violation_outside() {
        let mut world = BoardWorld::new();
        let rules = DesignRules::default();

        // Create a keepout zone
        let zone = Zone {
            bounds: Rect::new(Point::from_mm(10.0, 10.0), Point::from_mm(20.0, 20.0)),
            kind: ZoneKind::Keepout,
            layer_mask: 0xFFFFFFFF,
            name: Some("test_zone".to_string()),
            net: None,
        };
        world.ecs_mut().spawn(zone);

        // Create a component outside the zone
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(5.0, 5.0), // Outside zone
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        // Run the rule
        let violations = KeepoutRule.check(&mut world, &rules);
        assert!(violations.is_empty(), "expected no violations");
    }

    #[test]
    fn test_keepout_rule_ignores_copper_pour_zones() {
        let mut world = BoardWorld::new();
        let rules = DesignRules::default();

        // Create a copper pour zone (not keepout)
        let zone = Zone {
            bounds: Rect::new(Point::from_mm(0.0, 0.0), Point::from_mm(50.0, 50.0)),
            kind: ZoneKind::CopperPour,
            layer_mask: 0xFFFFFFFF,
            name: Some("gnd_pour".to_string()),
            net: None,
        };
        world.ecs_mut().spawn(zone);

        // Create a component inside the copper pour zone
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(15.0, 15.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        // Run the rule - should not detect violation (copper pour is not keepout)
        let violations = KeepoutRule.check(&mut world, &rules);
        assert!(
            violations.is_empty(),
            "copper pour zones should not trigger keepout violations"
        );
    }

    #[test]
    fn test_keepout_rule_multiple_components() {
        let mut world = BoardWorld::new();
        let rules = DesignRules::default();

        // Create a keepout zone
        let zone = Zone {
            bounds: Rect::new(Point::from_mm(10.0, 10.0), Point::from_mm(20.0, 20.0)),
            kind: ZoneKind::Keepout,
            layer_mask: 0xFFFFFFFF,
            name: None,
            net: None,
        };
        world.ecs_mut().spawn(zone);

        // Component inside
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(15.0, 15.0), // Inside
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        // Component outside
        world.spawn_component(
            RefDes::new("R2"),
            Value::new("10k"),
            Position::from_mm(5.0, 5.0), // Outside
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        // Another component inside
        world.spawn_component(
            RefDes::new("R3"),
            Value::new("10k"),
            Position::from_mm(12.0, 18.0), // Inside
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        // Run the rule
        let violations = KeepoutRule.check(&mut world, &rules);
        assert_eq!(violations.len(), 2, "expected 2 violations (R1 and R3)");

        // Verify both violations are for the right components
        let refdes_list: Vec<_> = violations.iter().map(|v| v.message.clone()).collect();
        assert!(refdes_list.iter().any(|m| m.contains("R1")));
        assert!(refdes_list.iter().any(|m| m.contains("R3")));
        assert!(!refdes_list.iter().any(|m| m.contains("R2")));
    }

    /// The distance every clearance measurement is built on, worked by hand.
    ///
    /// Four endpoint-to-segment distances and a crossing test, because the
    /// closed-form parametric solution is wrong for the case this exists for:
    /// two segments laid end to end leave its denominator at zero, and the
    /// fallback pins one parameter at an endpoint. Two slots in a row - which
    /// is how a connector's anchors sit - measured 2.8mm apart when their ends
    /// were 1.4mm apart.
    #[test]
    fn two_segments_are_as_far_apart_as_their_nearest_points() {
        use cypcb_core::Point;

        // Crossing: nothing between them.
        assert_eq!(
            segment_distance(
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 0.0),
                Point::from_mm(5.0, -5.0),
                Point::from_mm(5.0, 5.0),
            ),
            0
        );

        // Touching at one point is the same answer.
        assert_eq!(
            segment_distance(
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 0.0),
                Point::from_mm(10.0, 0.0),
                Point::from_mm(10.0, 5.0),
            ),
            0
        );

        // Parallel, 2mm apart along their whole length.
        assert_eq!(
            segment_distance(
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 0.0),
                Point::from_mm(0.0, 2.0),
                Point::from_mm(10.0, 2.0),
            ),
            2_000_000
        );

        // End to end on one line: the gap is between the ends, 1mm, and this
        // is the case the parametric form got wrong.
        assert_eq!(
            segment_distance(
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 0.0),
                Point::from_mm(11.0, 0.0),
                Point::from_mm(20.0, 0.0),
            ),
            1_000_000
        );

        // The nearest pair is an endpoint of the *second* segment against the
        // middle of the first: (5, 3) sits 3mm above the line, while either
        // end of the first segment is sqrt(25 + 9) = 5.83mm away. A solver
        // that only tried the first segment's endpoints would answer 5.831mm.
        assert_eq!(
            segment_distance(
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 0.0),
                Point::from_mm(5.0, 3.0),
                Point::from_mm(5.0, 9.0),
            ),
            3_000_000
        );

        // The nearest point is an endpoint rather than the foot of a
        // perpendicular: from (10, 0) to (15, 5) is sqrt(25 + 25) = 7.0711mm.
        assert_eq!(
            segment_distance(
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 0.0),
                Point::from_mm(15.0, 5.0),
                Point::from_mm(20.0, 10.0),
            ),
            7_071_068
        );
    }
}
