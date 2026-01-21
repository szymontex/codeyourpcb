//! DRC violation types.
//!
//! This module defines the types used to represent design rule violations.

use bevy_ecs::entity::Entity;
use cypcb_core::{Nm, Point};
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
}

/// Categories of design rule violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationKind {
    /// Clearance between two copper features is too small.
    Clearance,
    /// Trace width is below minimum (placeholder for future).
    TraceWidth,
    /// Drill hole size is below minimum.
    DrillSize,
    /// Pin has no net connection.
    UnconnectedPin,
    /// Via drill is below minimum (placeholder for future).
    ViaDrill,
    /// Annular ring is below minimum (placeholder for future).
    AnnularRing,
}

impl std::fmt::Display for ViolationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViolationKind::Clearance => write!(f, "clearance"),
            ViolationKind::TraceWidth => write!(f, "trace-width"),
            ViolationKind::DrillSize => write!(f, "drill-size"),
            ViolationKind::UnconnectedPin => write!(f, "unconnected-pin"),
            ViolationKind::ViaDrill => write!(f, "via-drill"),
            ViolationKind::AnnularRing => write!(f, "annular-ring"),
        }
    }
}

impl DrcViolation {
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
            location,
            entity,
            other_entity: None,
            source_span: None,
            message: format!("Unconnected pin: {}.{}", refdes, pin),
        }
    }

    /// Set the source span for this violation.
    ///
    /// This enables the DSL error display to show the exact source location.
    pub fn with_source_span(mut self, span: Span) -> Self {
        self.source_span = Some(span);
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
        assert_eq!(format!("{}", ViolationKind::UnconnectedPin), "unconnected-pin");
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
        let v = DrcViolation::unconnected_pin(
            Entity::from_raw(1),
            "1",
            "R1",
            Point::ORIGIN,
        );
        assert_eq!(v.kind, ViolationKind::UnconnectedPin);
        assert!(v.message.contains("R1.1"));
    }

    #[test]
    fn test_with_source_span() {
        let v = DrcViolation::unconnected_pin(
            Entity::from_raw(1),
            "1",
            "R1",
            Point::ORIGIN,
        ).with_source_span(Span::new(10, 20));

        assert!(v.source_span.is_some());
        assert_eq!(v.source_span.unwrap().start, 10);
        assert_eq!(v.source_span.unwrap().end, 20);
    }
}
