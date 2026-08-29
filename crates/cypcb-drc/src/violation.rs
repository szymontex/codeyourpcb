//! DRC violation types.
//!
//! This module defines the types used to represent design rule violations.

use bevy_ecs::entity::Entity;
use cypcb_core::{Nm, Point, Rect};
use cypcb_parser::ast::Span;

/// A design rule violation.
///
/// Captures all information needed to display the violation to the user
/// and allow click-to-zoom functionality in the viewer.
#[derive(Debug, Clone)]
pub struct DrcViolation {
    /// Type of violation.
    pub kind: ViolationKind,
    /// Location on the board (for click-to-zoom).
    pub location: Point,
    /// Primary entity involved.
    pub entity: Entity,
    /// Secondary entity (for clearance violations).
    pub other_entity: Option<Entity>,
    /// Source span in the DSL file (if available).
    pub source_span: Option<Span>,
    /// Human-readable description.
    pub message: String,
    /// The distance measured, where the rule measures one.
    ///
    /// Carried as a number rather than only inside `message`, because the
    /// difference between 0.00mm and 0.05mm is the difference between copper
    /// touching copper and a gap under spec - a board that cannot work and a
    /// board a fab may still build. Anything ranking violations has to be
    /// able to tell them apart without parsing a sentence.
    pub actual: Option<Nm>,
    /// What the rule demanded, where it demands a distance.
    pub required: Option<Nm>,
    /// The copper the violation is about, where it is an area rather than a
    /// point.
    ///
    /// A clearance fault happens at a place, and a coordinate is the whole
    /// story. An orphaned pour island is a *sheet*: the designer has to see
    /// which copper is stranded, and zooming to its centre shows a plane that
    /// looks like every other part of the plane.
    pub area: Option<Rect>,
}

/// Categories of design rule violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationKind {
    /// Clearance between two copper features is too small.
    Clearance,
    /// Trace width is below minimum.
    TraceWidth,
    /// Drill hole size is below minimum.
    DrillSize,
    /// Pin has no net connection.
    UnconnectedPin,
    /// Via drill is below minimum.
    ViaDrill,
    /// Via outer diameter is below minimum.
    ViaDiameter,
    /// Annular ring is below minimum.
    AnnularRing,
    /// Component placed in a keepout zone.
    KeepoutViolation,
    /// Copper feature too close to board edge.
    EdgeClearance,
    /// Drill holes too close together (edge-to-edge).
    HoleToHole,
    /// Solder mask bridge between pads too narrow.
    SolderMaskBridge,
    /// Silkscreen overlaps copper pad.
    SilkClearance,
    /// Trace is too narrow for the current its net declares.
    TraceCurrent,
    /// A net stated an impedance the stack does not deliver, or one this
    /// stack cannot be asked about at all.
    Impedance,
    /// A declared neck that does not describe a neck.
    NeckDown,
    /// Copper in a pour that reaches no pad of its own net.
    PourIsland,
    /// A pin the design connects to a net that no copper reaches.
    UnroutedPin,
    /// A claim the design makes about itself does not hold.
    Assertion,
    /// Component courtyards overlap.
    CourtyardClearance,
    /// The two halves of a differential pair are not the same length.
    DiffPairSkew,
    /// A declared stackup contradicts the rest of the design.
    Stackup,
    /// Two paste stencil openings leave a web too thin to hold.
    PasteClearance,
    /// A drilled hole sits too close to the routed board edge.
    HoleToEdge,
    /// A hole too deep for its width for the plating to reach the middle.
    DrillAspectRatio,
    /// Another net's copper too close to a milled slot.
    SlotClearance,
    /// The land around a drilled hole is smaller than the fab will image.
    PadLand,
    /// A via joins two layers by a route the build does not make.
    ViaSpan,
    /// A drilled hole sits where the board bends.
    FlexHole,
    /// A declared area has no width, no height, or neither.
    EmptyArea,
    /// A declared area hangs off the board, or sits nowhere near it.
    AreaOffBoard,
    /// Two areas the stack states a build for cover the same strip of board.
    AreaOverlap,
    /// A flexible region is folded tighter than its own thickness takes.
    BendRadius,
    /// Copper in a bend runs along the fold rather than across it.
    FlexTraceAngle,
}

/// The two features a clearance message is about.
///
/// `U1 <-> trace 'GND': Clearance violation: ...` - everything before the
/// first colon names the pair, and it is the same string however many segments
/// of the same two features report it.
///
/// One copy, in the crate that owns the message. `cypcb check` and the
/// language server each had their own before this, and a third was about to be
/// written in the router's scorer.
pub fn pair_of(message: &str) -> &str {
    message
        .split_once(':')
        .map(|(pair, _)| pair.trim())
        .unwrap_or(message)
}

/// How many pairs of features the clearance violations describe.
///
/// The clearance rule reports per pair of features that come too close, and a
/// trace is a chain of segments: two features touching along a run report once
/// for each segment that takes part. On the shipped benchmarks that is 759
/// rows for 484 contacts, and one pair accounts for 24 of them.
///
/// **Decided 2026-08-23: the rule keeps counting pairs of segments, and this
/// number is published beside it.** A violation is a place, and two segments
/// of one trace touching a pad at two points are two places a fabricator's
/// etch can fail; collapsing them in the model loses the locations. The counts
/// are also regression ratchets, and a per-contact count is the coarser
/// number - it would mask a routing change that a per-segment count catches.
/// What a reader needed was never a different count but a second one, which is
/// this.
pub fn clearance_contacts(violations: &[DrcViolation]) -> usize {
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for violation in violations {
        if violation.kind == ViolationKind::Clearance {
            seen.insert(pair_of(&violation.message));
        }
    }
    seen.len()
}

/// How far under its rule a violation is, as a fraction of what the rule asked
/// for.
///
/// A short is 1.0 - the copper measured nothing at all - and a trace 0.100mm
/// wide against a 0.127mm floor is 0.213. The fraction rather than the
/// difference, because the difference ranks a 0.300mm miss on a 0.500mm net
/// above a short on a 0.127mm rule, and a short is the worse board every time.
///
/// `None` where the rule measures nothing: an unrouted pin and an assertion
/// are faults with no distance in them, and a number invented for them here
/// would sort them among the ones that have one.
pub fn shortfall(violation: &DrcViolation) -> Option<f64> {
    let (actual, required) = (violation.actual?, violation.required?);
    if required <= Nm::ZERO {
        return None;
    }
    let missing = (required.raw() - actual.raw()) as f64 / required.raw() as f64;
    Some(missing.clamp(0.0, 1.0))
}

/// Copper touching copper, counted.
///
/// Three call sites counted this as "any violation whose measured distance is
/// zero" - `cypcb route`, `cypcb export` and the autorouter's own score - and
/// `check` counted the same thing until a paste stencil web of 0.000mm turned
/// up in the total. A stencil aperture with no web left is a torn stencil, not
/// a short, and the difference is what the sentence beside the number claims.
///
/// The kind is the guard. Every rule that measures a distance now carries it
/// as a number, so "actual is zero" alone would count a via with no annular
/// ring and a trace of no width as copper touching copper.
pub fn shorts(violations: &[DrcViolation]) -> usize {
    violations
        .iter()
        .filter(|violation| violation.kind == ViolationKind::Clearance)
        .filter(|violation| violation.actual == Some(Nm::ZERO))
        .count()
}

impl std::fmt::Display for ViolationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViolationKind::Clearance => write!(f, "clearance"),
            ViolationKind::TraceWidth => write!(f, "trace-width"),
            ViolationKind::DrillSize => write!(f, "drill-size"),
            ViolationKind::UnconnectedPin => write!(f, "unconnected-pin"),
            ViolationKind::ViaDrill => write!(f, "via-drill"),
            ViolationKind::ViaDiameter => write!(f, "via-diameter"),
            ViolationKind::AnnularRing => write!(f, "annular-ring"),
            ViolationKind::KeepoutViolation => write!(f, "keepout-violation"),
            ViolationKind::EdgeClearance => write!(f, "edge-clearance"),
            ViolationKind::HoleToHole => write!(f, "hole-to-hole"),
            ViolationKind::SolderMaskBridge => write!(f, "solder-mask-bridge"),
            ViolationKind::SilkClearance => write!(f, "silk-clearance"),
            ViolationKind::TraceCurrent => write!(f, "trace-current"),
            ViolationKind::Impedance => write!(f, "impedance"),
            ViolationKind::NeckDown => write!(f, "neck-down"),
            ViolationKind::PourIsland => write!(f, "pour-island"),
            ViolationKind::UnroutedPin => write!(f, "unrouted-pin"),
            ViolationKind::Assertion => write!(f, "assertion"),
            ViolationKind::CourtyardClearance => write!(f, "courtyard-clearance"),
            ViolationKind::DiffPairSkew => write!(f, "diff-pair-skew"),
            ViolationKind::Stackup => write!(f, "stackup"),
            ViolationKind::PasteClearance => write!(f, "paste-clearance"),
            ViolationKind::HoleToEdge => write!(f, "hole-to-edge"),
            ViolationKind::DrillAspectRatio => write!(f, "drill-aspect-ratio"),
            ViolationKind::SlotClearance => write!(f, "slot-clearance"),
            ViolationKind::PadLand => write!(f, "pad-land"),
            ViolationKind::ViaSpan => write!(f, "via-span"),
            ViolationKind::FlexHole => write!(f, "flex-hole"),
            ViolationKind::EmptyArea => write!(f, "empty-area"),
            ViolationKind::AreaOffBoard => write!(f, "area-off-board"),
            ViolationKind::AreaOverlap => write!(f, "area-overlap"),
            ViolationKind::BendRadius => write!(f, "bend-radius"),
            ViolationKind::FlexTraceAngle => write!(f, "flex-trace-angle"),
        }
    }
}

/// The smallest hole a fab plates through a board of this thickness.
///
/// Aspect ratio is depth over width, so the smallest drill that still reaches
/// the published ratio is the thickness divided by it. Rounded up: a hole one
/// nanometre under the answer is over the ratio, and this is the number the
/// violation tells a person to drill.
///
/// Returns zero when the fab published no ratio, which reads as "no limit" and
/// keeps the rule silent rather than failing every hole on the board.
pub(crate) fn smallest_platable_drill(thickness: Nm, max_ratio_x100: u32) -> Nm {
    if max_ratio_x100 == 0 {
        return Nm(0);
    }
    let ratio = i64::from(max_ratio_x100);
    Nm((thickness.0 * 100 + ratio - 1) / ratio)
}

impl DrcViolation {
    /// Create a violation for a pin no copper reaches.
    ///
    /// The design says the pin is on a net; the board says nothing is joined
    /// to it. `UnconnectedPinRule` asks whether the schematic named a net -
    /// this asks whether anybody laid the copper.
    pub fn unrouted_pin(component: Entity, pin: &str, refdes: &str, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::UnroutedPin,
            actual: None,
            required: None,
            area: None,
            location,
            entity: component,
            other_entity: None,
            source_span: None,
            message: format!("{refdes}.{pin} is on a net that no copper reaches"),
        }
    }

    /// Create a violation for pour copper that connects to nothing.
    ///
    /// A plane cut into pieces by the copper it flows around can leave a piece
    /// no pad of its own net bridges to. It is not a short and it breaks no
    /// clearance - it is copper that does nothing, and it looks exactly like
    /// the rest of the plane in every preview.
    pub fn pour_island(zone: Entity, area: Rect) -> Self {
        let location = Point::new(
            Nm((area.min.x.0 + area.max.x.0) / 2),
            Nm((area.min.y.0 + area.max.y.0) / 2),
        );
        DrcViolation {
            kind: ViolationKind::PourIsland,
            actual: None,
            required: None,
            area: Some(area),
            location,
            entity: zone,
            other_entity: None,
            source_span: None,
            message: "Pour island: this copper reaches no pad of its own net".to_string(),
        }
    }

    /// Create a clearance violation.
    ///
    /// # Arguments
    ///
    /// * `entity` - Primary entity involved
    /// * `other` - Secondary entity (the other item too close)
    /// * `actual` - Actual clearance distance
    /// * `required` - Required minimum clearance
    /// * `location` - Board location for click-to-zoom
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy_ecs::entity::Entity;
    /// use cypcb_core::{Nm, Point};
    /// use cypcb_drc::DrcViolation;
    ///
    /// let violation = DrcViolation::clearance(
    ///     Entity::from_raw(1),
    ///     Entity::from_raw(2),
    ///     Nm::from_mm(0.1),
    ///     Nm::from_mm(0.15),
    ///     Point::from_mm(10.0, 20.0),
    /// );
    /// assert_eq!(violation.kind, cypcb_drc::ViolationKind::Clearance);
    /// ```
    pub fn clearance(
        entity: Entity,
        other: Entity,
        actual: Nm,
        required: Nm,
        location: Point,
    ) -> Self {
        DrcViolation {
            kind: ViolationKind::Clearance,
            actual: Some(actual),
            required: Some(required),
            area: None,
            location,
            entity,
            other_entity: Some(other),
            source_span: None,
            message: format!(
                "Clearance violation: {:.2}mm actual, {:.2}mm required",
                actual.to_mm(),
                required.to_mm(),
            ),
        }
    }

    /// Create a drill size violation.
    ///
    /// # Arguments
    ///
    /// * `entity` - Entity with the undersized drill
    /// * `actual` - Actual drill size
    /// * `required` - Required minimum drill size
    /// * `location` - Board location for click-to-zoom
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy_ecs::entity::Entity;
    /// use cypcb_core::{Nm, Point};
    /// use cypcb_drc::DrcViolation;
    ///
    /// let violation = DrcViolation::drill_size(
    ///     Entity::from_raw(1),
    ///     Nm::from_mm(0.2),
    ///     Nm::from_mm(0.3),
    ///     Point::from_mm(10.0, 20.0),
    /// );
    /// assert_eq!(violation.kind, cypcb_drc::ViolationKind::DrillSize);
    /// ```
    pub fn drill_size(entity: Entity, actual: Nm, required: Nm, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::DrillSize,
            actual: Some(actual),
            required: Some(required),
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message: format!(
                "Drill size violation: {:.2}mm actual, {:.2}mm minimum",
                actual.to_mm(),
                required.to_mm(),
            ),
        }
    }

    /// Create a trace width violation.
    ///
    /// # Arguments
    ///
    /// * `entity` - Trace entity that is too narrow
    /// * `actual` - Width the trace was drawn at
    /// * `required` - Minimum width the fab allows
    /// * `location` - Board location for click-to-zoom
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy_ecs::entity::Entity;
    /// use cypcb_core::{Nm, Point};
    /// use cypcb_drc::DrcViolation;
    ///
    /// let violation = DrcViolation::trace_width(
    ///     Entity::from_raw(1),
    ///     Nm::from_mm(0.1),
    ///     Nm::from_mm(0.127),
    ///     Point::from_mm(10.0, 20.0),
    /// );
    /// assert_eq!(violation.kind, cypcb_drc::ViolationKind::TraceWidth);
    /// ```
    pub fn trace_width(entity: Entity, actual: Nm, required: Nm, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::TraceWidth,
            actual: Some(actual),
            required: Some(required),
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message: format!(
                "Trace width violation: {:.3}mm actual, {:.3}mm minimum",
                actual.to_mm(),
                required.to_mm(),
            ),
        }
    }

    /// Create an unconnected pin violation.
    ///
    /// # Arguments
    ///
    /// * `entity` - Component entity with the unconnected pin
    /// * `pin` - Pin identifier (number or name)
    /// * `refdes` - Component reference designator
    /// * `location` - Board location for click-to-zoom
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy_ecs::entity::Entity;
    /// use cypcb_core::Point;
    /// use cypcb_drc::DrcViolation;
    ///
    /// let violation = DrcViolation::unconnected_pin(
    ///     Entity::from_raw(1),
    ///     "1",
    ///     "R1",
    ///     Point::from_mm(10.0, 20.0),
    /// );
    /// assert_eq!(violation.kind, cypcb_drc::ViolationKind::UnconnectedPin);
    /// assert!(violation.message.contains("R1.1"));
    /// ```
    pub fn unconnected_pin(entity: Entity, pin: &str, refdes: &str, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::UnconnectedPin,
            actual: None,
            required: None,
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message: format!("Unconnected pin: {}.{}", refdes, pin),
        }
    }

    /// Create a keepout violation.
    ///
    /// # Arguments
    ///
    /// * `entity` - Component entity in the keepout zone
    /// * `zone_entity` - Zone entity being violated
    /// * `refdes` - Component reference designator
    /// * `zone_name` - Optional zone name (for error message)
    /// * `location` - Board location for click-to-zoom
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy_ecs::entity::Entity;
    /// use cypcb_core::Point;
    /// use cypcb_drc::DrcViolation;
    ///
    /// let violation = DrcViolation::keepout(
    ///     Entity::from_raw(1),
    ///     Entity::from_raw(2),
    ///     "R1",
    ///     Some("antenna_clearance"),
    ///     Point::from_mm(15.0, 15.0),
    /// );
    /// assert_eq!(violation.kind, cypcb_drc::ViolationKind::KeepoutViolation);
    /// assert!(violation.message.contains("R1"));
    /// ```
    pub fn keepout(
        entity: Entity,
        zone_entity: Entity,
        refdes: &str,
        zone_name: Option<&str>,
        location: Point,
    ) -> Self {
        let zone_desc = zone_name
            .map(|n| format!("keepout zone '{}'", n))
            .unwrap_or_else(|| "keepout zone".to_string());
        DrcViolation {
            kind: ViolationKind::KeepoutViolation,
            actual: None,
            required: None,
            area: None,
            location,
            entity,
            other_entity: Some(zone_entity),
            source_span: None,
            message: format!("Component {} placed in {}", refdes, zone_desc),
        }
    }

    /// Create an edge clearance violation.
    ///
    /// # Arguments
    ///
    /// * `entity` - Entity too close to the board edge
    /// * `actual` - Actual distance to nearest edge
    /// * `required` - Required minimum edge clearance
    /// * `location` - Board location for click-to-zoom
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy_ecs::entity::Entity;
    /// use cypcb_core::{Nm, Point};
    /// use cypcb_drc::DrcViolation;
    ///
    /// let violation = DrcViolation::edge_clearance(
    ///     Entity::from_raw(1),
    ///     Nm::from_mm(0.1),
    ///     Nm::from_mm(0.3),
    ///     Point::from_mm(0.5, 10.0),
    /// );
    /// assert_eq!(violation.kind, cypcb_drc::ViolationKind::EdgeClearance);
    /// ```
    pub fn edge_clearance(entity: Entity, actual: Nm, required: Nm, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::EdgeClearance,
            actual: Some(actual),
            required: Some(required),
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message: format!(
                "Edge clearance violation: {:.2}mm actual, {:.2}mm required",
                actual.to_mm(),
                required.to_mm(),
            ),
        }
    }

    /// Create an annular ring violation.
    ///
    /// # Arguments
    ///
    /// * `entity` - Entity with insufficient annular ring
    /// * `actual` - Actual annular ring width
    /// * `required` - Required minimum annular ring
    /// * `location` - Board location for click-to-zoom
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy_ecs::entity::Entity;
    /// use cypcb_core::{Nm, Point};
    /// use cypcb_drc::DrcViolation;
    ///
    /// let violation = DrcViolation::annular_ring(
    ///     Entity::from_raw(1),
    ///     Nm::from_mm(0.1),
    ///     Nm::from_mm(0.15),
    ///     Point::from_mm(10.0, 20.0),
    /// );
    /// assert_eq!(violation.kind, cypcb_drc::ViolationKind::AnnularRing);
    /// ```
    pub fn annular_ring(entity: Entity, actual: Nm, required: Nm, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::AnnularRing,
            actual: Some(actual),
            required: Some(required),
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message: format!(
                "Annular ring violation: {:.3}mm actual, {:.3}mm required",
                actual.to_mm(),
                required.to_mm(),
            ),
        }
    }

    /// Create a hole-to-hole clearance violation.
    pub fn hole_to_hole(
        entity: Entity,
        other: Entity,
        actual: Nm,
        required: Nm,
        location: Point,
    ) -> Self {
        DrcViolation {
            kind: ViolationKind::HoleToHole,
            actual: None,
            required: None,
            area: None,
            location,
            entity,
            other_entity: Some(other),
            source_span: None,
            message: format!(
                "Hole-to-hole violation: {:.2}mm actual, {:.2}mm required",
                actual.to_mm(),
                required.to_mm(),
            ),
        }
    }

    /// Create a via diameter violation.
    pub fn via_diameter(entity: Entity, actual: Nm, required: Nm, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::ViaDiameter,
            actual: Some(actual),
            required: Some(required),
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message: format!(
                "Via diameter violation: {:.2}mm actual, {:.2}mm required",
                actual.to_mm(),
                required.to_mm(),
            ),
        }
    }

    /// Create a via-drill violation.
    ///
    /// The hole through the via, as distinct from the copper ring around it -
    /// a fab that quotes 0.2mm cannot drill 0.1mm whatever the ring looks like.
    pub fn via_drill(entity: Entity, actual: Nm, required: Nm, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::ViaDrill,
            actual: Some(actual),
            required: Some(required),
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message: format!(
                "Via drill violation: {:.2}mm actual, {:.2}mm required",
                actual.to_mm(),
                required.to_mm(),
            ),
        }
    }

    /// Create a solder mask bridge violation.
    pub fn solder_mask_bridge(
        entity: Entity,
        other: Entity,
        actual: Nm,
        required: Nm,
        location: Point,
    ) -> Self {
        DrcViolation {
            kind: ViolationKind::SolderMaskBridge,
            actual: None,
            required: None,
            area: None,
            location,
            entity,
            other_entity: Some(other),
            source_span: None,
            message: format!(
                "Solder mask bridge violation: {:.2}mm actual, {:.2}mm required",
                actual.to_mm(),
                required.to_mm(),
            ),
        }
    }

    /// Two paste stencil openings with too little steel between them.
    ///
    /// The mask bridge rule's twin: same geometry, a different sheet. Where
    /// the web tears the two openings become one and the pads bridge with
    /// solder on reflow.
    pub fn paste_clearance(
        entity: Entity,
        other: Entity,
        actual: Nm,
        required: Nm,
        location: Point,
    ) -> Self {
        DrcViolation {
            kind: ViolationKind::PasteClearance,
            actual: Some(actual),
            required: Some(required),
            area: None,
            location,
            entity,
            other_entity: Some(other),
            source_span: None,
            message: format!(
                "Paste stencil web is {:.3}mm, {:.3}mm required",
                actual.to_mm(),
                required.to_mm(),
            ),
        }
    }

    /// A hole too deep for its width for the plating to reach the middle.
    ///
    /// `actual` is the drill the design asked for and `required` the smallest
    /// one this fab plates through a board this thick, because widening the
    /// hole is the fix a person can act on - the ratio itself is in the
    /// message, where the two numbers it came from are named.
    pub fn drill_aspect_ratio(
        entity: Entity,
        drill: Nm,
        thickness: Nm,
        max_ratio_x100: u32,
        location: Point,
    ) -> Self {
        let smallest = smallest_platable_drill(thickness, max_ratio_x100);
        DrcViolation {
            kind: ViolationKind::DrillAspectRatio,
            actual: Some(drill),
            required: Some(smallest),
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message: format!(
                "A {:.3}mm hole through a {:.2}mm board is {:.1}:1, more than the {:.1}:1 this fab plates - {:.3}mm is the smallest that reaches",
                drill.to_mm(),
                thickness.to_mm(),
                thickness.0 as f64 / drill.0.max(1) as f64,
                f64::from(max_ratio_x100) / 100.0,
                smallest.to_mm(),
            ),
        }
    }

    /// A drilled hole too close to the routed board edge.
    ///
    /// The bit that cuts the board out of the panel follows the outline; a
    /// hole nearer than the fab allows comes out open on one side.
    pub fn hole_to_edge(entity: Entity, actual: Nm, required: Nm, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::HoleToEdge,
            actual: Some(actual),
            required: Some(required),
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message: format!(
                "Hole is {:.3}mm from the board edge, {:.3}mm required",
                actual.to_mm(),
                required.to_mm(),
            ),
        }
    }

    /// The land around a drilled hole is smaller than the fab will image.
    ///
    /// D6. Distinct from the annular ring, which is `(land - drill) / 2` and
    /// asks whether copper stays attached when the hole wanders; this asks
    /// whether the land exists at all at the fab's smallest size.
    pub fn pad_land(
        entity: Entity,
        pin: String,
        actual: Nm,
        drill: Nm,
        required: Nm,
        location: Point,
    ) -> Self {
        DrcViolation {
            kind: ViolationKind::PadLand,
            actual: Some(actual),
            required: Some(required),
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message: format!(
                "{pin}: land is {:.3}mm around a {:.3}mm hole, {:.3}mm required",
                actual.to_mm(),
                drill.to_mm(),
                required.to_mm(),
            ),
        }
    }

    /// Another net's copper too close to a milled slot.
    ///
    /// The same physical question `edge_clearance` asks about the board
    /// outline, asked of an opening cut inside it by the same mill. `entity`
    /// is the copper; `other_entity` is the part the slot belongs to, so a
    /// reader can name both sides the way every other pair rule does.
    pub fn slot_clearance(
        entity: Entity,
        slot_owner: Entity,
        actual: Nm,
        required: Nm,
        location: Point,
    ) -> Self {
        DrcViolation {
            kind: ViolationKind::SlotClearance,
            actual: Some(actual),
            required: Some(required),
            area: None,
            location,
            entity,
            other_entity: Some(slot_owner),
            source_span: None,
            message: format!(
                "Copper is {:.3}mm from a milled slot, {:.3}mm required",
                actual.to_mm(),
                required.to_mm(),
            ),
        }
    }

    /// Create an assertion violation.
    ///
    /// The message is written by the rule, which knows what was claimed and
    /// what the board actually is.
    /// Create a differential-pair skew violation.
    ///
    /// `actual` is how far apart the two halves ended up and `required` is the
    /// fab's length-match tolerance, so the message reads like every other
    /// measured rule: what the board has against what it may have.
    pub fn diff_pair_skew(
        entity: Entity,
        message: String,
        actual: Option<Nm>,
        required: Option<Nm>,
        location: Point,
    ) -> Self {
        DrcViolation {
            kind: ViolationKind::DiffPairSkew,
            actual,
            required,
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message,
        }
    }

    /// A declared stackup that contradicts the rest of the design.
    ///
    /// No `actual`/`required` pair: what is wrong is a disagreement between
    /// two statements the design makes, not a measurement against a limit.
    /// A hole drilled where the board bends.
    ///
    /// No `actual`/`required` pair: nothing was measured against a limit. A
    /// hole is either in the bend or it is not.
    /// Copper following a fold instead of crossing it.
    pub fn flex_trace_angle(entity: Entity, message: String, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::FlexTraceAngle,
            actual: None,
            required: None,
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message,
        }
    }

    /// A fold tighter than the ribbon's own thickness takes.
    ///
    /// `actual` is the radius the design states and `required` the smallest
    /// the house publishes for that ribbon, so a reader gets both numbers
    /// without reading the sentence.
    pub fn bend_radius(
        entity: Entity,
        radius: cypcb_core::Nm,
        least: cypcb_core::Nm,
        message: String,
        location: Point,
    ) -> Self {
        DrcViolation {
            kind: ViolationKind::BendRadius,
            actual: Some(radius),
            required: Some(least),
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message,
        }
    }

    /// Two stack-bearing areas over the same strip of board.
    ///
    /// The location is the corner of the overlap nearest the origin - the
    /// contested strip itself rather than either area, because that is what
    /// the designer has to look at.
    pub fn area_overlap(entity: Entity, other: Entity, message: String, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::AreaOverlap,
            actual: None,
            required: None,
            area: None,
            location,
            entity,
            other_entity: Some(other),
            source_span: None,
            message,
        }
    }

    /// An area whose rectangle is not on the board.
    ///
    /// The location is the corner the design wrote first, the same one
    /// [`DrcViolation::empty_area`] sends a reader to: it is the number in the
    /// file, whether or not it is on the panel.
    pub fn area_off_board(entity: Entity, message: String, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::AreaOffBoard,
            actual: None,
            required: None,
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message,
        }
    }

    /// An area whose rectangle has no area.
    ///
    /// The location is the corner the design wrote first, which is the only
    /// point of a collapsed rectangle a reader can be sent to.
    pub fn empty_area(entity: Entity, message: String, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::EmptyArea,
            actual: None,
            required: None,
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message,
        }
    }

    pub fn flex_hole(entity: Entity, message: String, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::FlexHole,
            actual: None,
            required: None,
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message,
        }
    }

    /// A via whose span the build does not make.
    ///
    /// No `actual`/`required` pair, for the reason [`Self::stackup`] has none:
    /// a via that goes from the top layer to an inner one is not a measurement
    /// that missed a limit, it is a hole nobody drills on this build.
    pub fn via_span(entity: Entity, message: String, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::ViaSpan,
            actual: None,
            required: None,
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message,
        }
    }

    pub fn stackup(entity: Entity, message: String, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::Stackup,
            actual: None,
            required: None,
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message,
        }
    }

    pub fn assertion(entity: Entity, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::Assertion,
            actual: None,
            required: None,
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message: String::new(),
        }
    }

    /// Create a trace-current violation.
    ///
    /// The trace is narrower than the current its net declares needs, per
    /// IPC-2221. Distinct from `TraceWidth`, which is the fabricator's floor:
    /// a trace can clear the fab minimum and still be too thin for its load.
    pub fn trace_current(entity: Entity, actual: Nm, required: Nm, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::TraceCurrent,
            actual: Some(actual),
            required: Some(required),
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message: format!(
                "Trace current violation: {:.3}mm actual, {:.3}mm required",
                actual.0 as f64 / 1_000_000.0,
                required.0 as f64 / 1_000_000.0
            ),
        }
    }

    /// Create an impedance violation.
    ///
    /// Neither `actual` nor `required` is set: both are impedances in ohms and
    /// every other field of this type is a length in nanometres. A reader
    /// comparing them would be comparing two different quantities, so the
    /// numbers stay in the message where their unit is written beside them.
    pub fn impedance(entity: Entity, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::Impedance,
            actual: None,
            required: None,
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message: String::new(),
        }
    }

    /// Create a neck-down violation.
    ///
    /// The numbers stay in the message: a neck compares a width against a
    /// width in one case and a length against a length in another, and one
    /// pair of `actual`/`required` fields cannot say which.
    pub fn neck_down(entity: Entity, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::NeckDown,
            actual: None,
            required: None,
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message: String::new(),
        }
    }

    /// Create a silkscreen clearance violation.
    pub fn silk_clearance(entity: Entity, actual: Nm, required: Nm, location: Point) -> Self {
        DrcViolation {
            kind: ViolationKind::SilkClearance,
            actual: Some(actual),
            required: Some(required),
            area: None,
            location,
            entity,
            other_entity: None,
            source_span: None,
            message: format!(
                "Silk-to-pad clearance violation: {:.2}mm actual, {:.2}mm required",
                actual.to_mm(),
                required.to_mm(),
            ),
        }
    }

    /// Create a courtyard clearance violation.
    pub fn courtyard_clearance(
        entity: Entity,
        other: Entity,
        actual: Nm,
        required: Nm,
        location: Point,
    ) -> Self {
        DrcViolation {
            kind: ViolationKind::CourtyardClearance,
            actual: Some(actual),
            required: Some(required),
            area: None,
            location,
            entity,
            other_entity: Some(other),
            source_span: None,
            message: format!(
                "Courtyard overlap: {:.2}mm actual, {:.2}mm required",
                actual.to_mm(),
                required.to_mm(),
            ),
        }
    }

    /// Set the source span for this violation.
    ///
    /// This enables the DSL error display to show the exact source location.
    pub fn with_source_span(mut self, span: Span) -> Self {
        self.source_span = Some(span);
        self
    }

    /// Add pad information to a violation message.
    ///
    /// Updates the message to include component refdes and pad number.
    /// Works for DrillSize and AnnularRing violations.
    pub fn with_pad_info(mut self, refdes: &str, pad_number: &str) -> Self {
        match self.kind {
            ViolationKind::DrillSize => {
                if let Some(rest) = self.message.strip_prefix("Drill size violation: ") {
                    self.message = format!(
                        "Drill size violation at {}.{}: {}",
                        refdes, pad_number, rest
                    );
                }
            }
            ViolationKind::AnnularRing => {
                if let Some(rest) = self.message.strip_prefix("Annular ring violation: ") {
                    self.message = format!(
                        "Annular ring violation at {}.{}: {}",
                        refdes, pad_number, rest
                    );
                }
            }
            _ => {}
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_violation_kind_display() {
        assert_eq!(format!("{}", ViolationKind::Clearance), "clearance");
        assert_eq!(format!("{}", ViolationKind::DrillSize), "drill-size");
        assert_eq!(
            format!("{}", ViolationKind::UnconnectedPin),
            "unconnected-pin"
        );
    }

    #[test]
    fn test_clearance_violation() {
        let v = DrcViolation::clearance(
            Entity::from_raw(1),
            Entity::from_raw(2),
            Nm::from_mm(0.1),
            Nm::from_mm(0.15),
            Point::ORIGIN,
        );
        assert_eq!(v.kind, ViolationKind::Clearance);
        assert!(v.other_entity.is_some());
        assert!(v.message.contains("0.10"));
        assert!(v.message.contains("0.15"));
    }

    #[test]
    fn test_drill_size_violation() {
        let v = DrcViolation::drill_size(
            Entity::from_raw(1),
            Nm::from_mm(0.2),
            Nm::from_mm(0.3),
            Point::ORIGIN,
        );
        assert_eq!(v.kind, ViolationKind::DrillSize);
        assert!(v.other_entity.is_none());
        assert!(v.message.contains("0.20"));
        assert!(v.message.contains("0.30"));
    }

    #[test]
    fn test_unconnected_pin_violation() {
        let v = DrcViolation::unconnected_pin(Entity::from_raw(1), "1", "R1", Point::ORIGIN);
        assert_eq!(v.kind, ViolationKind::UnconnectedPin);
        assert!(v.message.contains("R1.1"));
    }

    #[test]
    fn test_with_source_span() {
        let v = DrcViolation::unconnected_pin(Entity::from_raw(1), "1", "R1", Point::ORIGIN)
            .with_source_span(Span::new(10, 20));

        assert!(v.source_span.is_some());
        assert_eq!(v.source_span.unwrap().start, 10);
        assert_eq!(v.source_span.unwrap().end, 20);
    }

    #[test]
    fn test_keepout_violation() {
        let v = DrcViolation::keepout(
            Entity::from_raw(1),
            Entity::from_raw(2),
            "R1",
            Some("antenna_area"),
            Point::from_mm(15.0, 15.0),
        );
        assert_eq!(v.kind, ViolationKind::KeepoutViolation);
        assert!(v.other_entity.is_some());
        assert!(v.message.contains("R1"));
        assert!(v.message.contains("antenna_area"));
    }

    #[test]
    fn test_keepout_violation_no_name() {
        let v = DrcViolation::keepout(
            Entity::from_raw(1),
            Entity::from_raw(2),
            "U1",
            None,
            Point::ORIGIN,
        );
        assert_eq!(v.kind, ViolationKind::KeepoutViolation);
        assert!(v.message.contains("U1"));
        assert!(v.message.contains("keepout zone"));
    }

    #[test]
    fn test_violation_kind_display_keepout() {
        assert_eq!(
            format!("{}", ViolationKind::KeepoutViolation),
            "keepout-violation"
        );
    }

    /// The number the drill-aspect rule divides by, worked by hand.
    ///
    /// Every `drill-aspect-ratio` violation prints it - `0.200mm is the
    /// smallest that reaches` - and the rule stays silent whenever it comes
    /// back zero. Nothing named this function in a test until now.
    #[test]
    fn the_smallest_platable_drill_is_the_thickness_over_the_ratio() {
        // 1.6mm of board at 8:1 is 0.2mm exactly.
        assert_eq!(
            smallest_platable_drill(Nm::from_mm(1.6), 800),
            Nm::from_mm(0.2)
        );

        // The same board at 12:1, which is JLCPCB's advanced process: 1.6 / 12
        // is 0.13333..., and the answer is rounded **up**, because a hole one
        // nanometre under is a hole over the ratio.
        assert_eq!(
            smallest_platable_drill(Nm::from_mm(1.6), 1_200),
            Nm(133_334)
        );

        // A thin flexible build, where the same drill is a quarter as deep.
        assert_eq!(
            smallest_platable_drill(Nm::from_mm(0.4), 800),
            Nm::from_mm(0.05)
        );

        // 1mm at 3:1 is 333_333.33nm, and rounding down would publish a width
        // the fab would refuse.
        assert_eq!(smallest_platable_drill(Nm::from_mm(1.0), 300), Nm(333_334));
    }

    #[test]
    fn a_fab_that_publishes_no_ratio_puts_no_floor_under_a_drill() {
        // Zero reads as "no limit" and keeps the rule silent, rather than
        // failing every hole on the board.
        assert_eq!(smallest_platable_drill(Nm::from_mm(1.6), 0), Nm(0));
    }
}
