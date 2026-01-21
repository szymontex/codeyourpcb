//! Position and rotation components for board entities.
//!
//! All coordinates use integer nanometers from cypcb-core for deterministic precision.

use bevy_ecs::prelude::*;
use cypcb_core::Point;
use serde::{Deserialize, Serialize};

/// Position in nanometers from board origin (bottom-left).
///
/// Wraps a [`Point`] from cypcb-core, providing the ECS Component derive.
/// The origin is at the bottom-left corner of the board, with Y increasing upward.
///
/// # Examples
///
/// ```
/// use cypcb_world::Position;
/// use cypcb_core::Point;
///
/// let pos = Position(Point::from_mm(10.0, 20.0));
/// assert_eq!(pos.0.x.0, 10_000_000);
/// ```
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Position(pub Point);

impl Position {
    /// Create a position at the origin (0, 0).
    pub const ORIGIN: Position = Position(Point::ORIGIN);

    /// Create a new position from a Point.
    #[inline]
    pub const fn new(point: Point) -> Self {
        Position(point)
    }

    /// Create a position from millimeter coordinates.
    #[inline]
    pub fn from_mm(x: f64, y: f64) -> Self {
        Position(Point::from_mm(x, y))
    }

    /// Create a position from mil coordinates.
    #[inline]
    pub fn from_mil(x: f64, y: f64) -> Self {
        Position(Point::from_mil(x, y))
    }

    /// Get the underlying Point.
    #[inline]
    pub const fn point(&self) -> Point {
        self.0
    }
}

impl From<Point> for Position {
    fn from(point: Point) -> Self {
        Position(point)
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Position({})", self.0)
    }
}

/// Rotation in millidegrees (0-359999).
///
/// Using integer millidegrees provides:
/// - 0.001 degree precision (sufficient for PCB work)
/// - Deterministic comparisons
/// - Efficient storage and hashing
///
/// The value is automatically normalized to [0, 360000) range.
///
/// # Examples
///
/// ```
/// use cypcb_world::Rotation;
///
/// let rot = Rotation::from_degrees(45.0);
/// assert_eq!(rot.0, 45_000);
/// assert!((rot.to_degrees() - 45.0).abs() < 0.001);
///
/// // Automatic normalization
/// let rot2 = Rotation::from_degrees(450.0);
/// assert_eq!(rot2.0, 90_000);
/// ```
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Rotation(pub i32);

impl Rotation {
    /// Zero rotation.
    pub const ZERO: Rotation = Rotation(0);

    /// 90 degrees.
    pub const DEG_90: Rotation = Rotation(90_000);

    /// 180 degrees.
    pub const DEG_180: Rotation = Rotation(180_000);

    /// 270 degrees.
    pub const DEG_270: Rotation = Rotation(270_000);

    /// Create a rotation from degrees (f64).
    ///
    /// The value is normalized to [0, 360) degrees.
    #[inline]
    pub fn from_degrees(deg: f64) -> Self {
        let millideg = (deg * 1000.0).round() as i32;
        Self(millideg.rem_euclid(360_000))
    }

    /// Create a rotation from millidegrees directly.
    ///
    /// The value is normalized to [0, 360000) range.
    #[inline]
    pub fn from_millidegrees(millideg: i32) -> Self {
        Self(millideg.rem_euclid(360_000))
    }

    /// Convert to degrees.
    #[inline]
    pub fn to_degrees(&self) -> f64 {
        self.0 as f64 / 1000.0
    }

    /// Get the raw millidegree value.
    #[inline]
    pub const fn millidegrees(&self) -> i32 {
        self.0
    }

    /// Check if this is a 90-degree multiple (0, 90, 180, or 270).
    #[inline]
    pub fn is_orthogonal(&self) -> bool {
        self.0 % 90_000 == 0
    }
}

impl std::fmt::Display for Rotation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}deg", self.to_degrees())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_from_mm() {
        let pos = Position::from_mm(10.0, 20.0);
        assert_eq!(pos.0.x.0, 10_000_000);
        assert_eq!(pos.0.y.0, 20_000_000);
    }

    #[test]
    fn test_rotation_from_degrees() {
        let rot = Rotation::from_degrees(45.0);
        assert_eq!(rot.0, 45_000);
    }

    #[test]
    fn test_rotation_normalization() {
        // Positive overflow
        let rot = Rotation::from_degrees(450.0);
        assert_eq!(rot.0, 90_000);

        // Negative
        let rot2 = Rotation::from_degrees(-90.0);
        assert_eq!(rot2.0, 270_000);

        // Large negative
        let rot3 = Rotation::from_degrees(-450.0);
        assert_eq!(rot3.0, 270_000);
    }

    #[test]
    fn test_rotation_round_trip() {
        let original = 123.456;
        let rot = Rotation::from_degrees(original);
        let back = rot.to_degrees();
        assert!((back - original).abs() < 0.001);
    }

    #[test]
    fn test_rotation_orthogonal() {
        assert!(Rotation::ZERO.is_orthogonal());
        assert!(Rotation::DEG_90.is_orthogonal());
        assert!(Rotation::DEG_180.is_orthogonal());
        assert!(Rotation::DEG_270.is_orthogonal());
        assert!(!Rotation::from_degrees(45.0).is_orthogonal());
    }
}
