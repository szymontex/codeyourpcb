//! Coordinate types for PCB layout.
//!
//! All coordinates are stored internally as nanometers using i64. This provides:
//! - Deterministic precision (no floating-point accumulation errors)
//! - Sufficient range (i64 max = ~9.2 billion meters, more than enough for any PCB)
//! - Efficient comparison and hashing
//!
//! # Coordinate System
//!
//! The coordinate system uses:
//! - Origin: bottom-left of the board
//! - X-axis: positive right
//! - Y-axis: positive up (mathematical convention)
//!
//! # Unit Conversions
//!
//! - 1 mm = 1,000,000 nm
//! - 1 mil = 25,400 nm
//! - 1 inch = 25,400,000 nm

use serde::{Deserialize, Serialize};
use std::hash::Hash;
use std::ops::{Add, Div, Mul, Neg, Sub};

/// Conversion constant: nanometers per millimeter
pub const NM_PER_MM: i64 = 1_000_000;

/// Conversion constant: nanometers per mil (thousandth of an inch)
pub const NM_PER_MIL: i64 = 25_400;

/// Conversion constant: nanometers per inch
pub const NM_PER_INCH: i64 = 25_400_000;

/// A coordinate value in nanometers.
///
/// This newtype provides type safety and prevents accidentally mixing
/// different unit systems. All internal PCB coordinates use this type.
///
/// # Examples
///
/// ```
/// use cypcb_core::Nm;
///
/// let distance = Nm::from_mm(10.5);
/// assert_eq!(distance.0, 10_500_000);
///
/// let in_mm = distance.to_mm();
/// assert!((in_mm - 10.5).abs() < 1e-6);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Nm(pub i64);

impl Nm {
    /// Zero nanometers.
    pub const ZERO: Nm = Nm(0);

    /// Maximum representable value.
    pub const MAX: Nm = Nm(i64::MAX);

    /// Minimum representable value.
    pub const MIN: Nm = Nm(i64::MIN);

    /// Create a new Nm value from raw nanometers.
    #[inline]
    pub const fn new(nm: i64) -> Self {
        Nm(nm)
    }

    /// Convert from millimeters to nanometers.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::Nm;
    ///
    /// let nm = Nm::from_mm(1.0);
    /// assert_eq!(nm.0, 1_000_000);
    /// ```
    #[inline]
    pub fn from_mm(mm: f64) -> Self {
        Nm((mm * NM_PER_MM as f64).round() as i64)
    }

    /// Convert from mils (thousandths of an inch) to nanometers.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::Nm;
    ///
    /// let nm = Nm::from_mil(1.0);
    /// assert_eq!(nm.0, 25_400);
    /// ```
    #[inline]
    pub fn from_mil(mil: f64) -> Self {
        Nm((mil * NM_PER_MIL as f64).round() as i64)
    }

    /// Convert from inches to nanometers.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::Nm;
    ///
    /// let nm = Nm::from_inch(1.0);
    /// assert_eq!(nm.0, 25_400_000);
    /// ```
    #[inline]
    pub fn from_inch(inch: f64) -> Self {
        Nm((inch * NM_PER_INCH as f64).round() as i64)
    }

    /// Convert to millimeters.
    #[inline]
    pub fn to_mm(self) -> f64 {
        self.0 as f64 / NM_PER_MM as f64
    }

    /// Convert to mils (thousandths of an inch).
    #[inline]
    pub fn to_mil(self) -> f64 {
        self.0 as f64 / NM_PER_MIL as f64
    }

    /// Convert to inches.
    #[inline]
    pub fn to_inch(self) -> f64 {
        self.0 as f64 / NM_PER_INCH as f64
    }

    /// Get the absolute value.
    #[inline]
    pub fn abs(self) -> Self {
        Nm(self.0.abs())
    }

    /// Get the raw nanometer value.
    #[inline]
    pub const fn raw(self) -> i64 {
        self.0
    }
}

impl Add for Nm {
    type Output = Nm;

    #[inline]
    fn add(self, rhs: Nm) -> Nm {
        Nm(self.0 + rhs.0)
    }
}

impl Sub for Nm {
    type Output = Nm;

    #[inline]
    fn sub(self, rhs: Nm) -> Nm {
        Nm(self.0 - rhs.0)
    }
}

impl Mul<i64> for Nm {
    type Output = Nm;

    #[inline]
    fn mul(self, rhs: i64) -> Nm {
        Nm(self.0 * rhs)
    }
}

impl Div<i64> for Nm {
    type Output = Nm;

    #[inline]
    fn div(self, rhs: i64) -> Nm {
        Nm(self.0 / rhs)
    }
}

impl Neg for Nm {
    type Output = Nm;

    #[inline]
    fn neg(self) -> Nm {
        Nm(-self.0)
    }
}

impl std::fmt::Display for Nm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}nm", self.0)
    }
}

/// A 2D point in nanometers.
///
/// Represents a position on the PCB with integer precision.
/// Origin is at the bottom-left of the board, Y increases upward.
///
/// # Examples
///
/// ```
/// use cypcb_core::{Point, Nm};
///
/// let p = Point::from_mm(10.0, 20.0);
/// assert_eq!(p.x.0, 10_000_000);
/// assert_eq!(p.y.0, 20_000_000);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Point {
    /// X coordinate in nanometers (positive right).
    pub x: Nm,
    /// Y coordinate in nanometers (positive up).
    pub y: Nm,
}

impl Point {
    /// Origin point (0, 0).
    pub const ORIGIN: Point = Point { x: Nm::ZERO, y: Nm::ZERO };

    /// Create a new point from Nm values.
    #[inline]
    pub const fn new(x: Nm, y: Nm) -> Self {
        Point { x, y }
    }

    /// Create a point from raw nanometer values.
    #[inline]
    pub const fn from_raw(x: i64, y: i64) -> Self {
        Point { x: Nm(x), y: Nm(y) }
    }

    /// Create a point from millimeters.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::Point;
    ///
    /// let p = Point::from_mm(1.0, 2.0);
    /// assert_eq!(p.x.0, 1_000_000);
    /// assert_eq!(p.y.0, 2_000_000);
    /// ```
    #[inline]
    pub fn from_mm(x: f64, y: f64) -> Self {
        Point {
            x: Nm::from_mm(x),
            y: Nm::from_mm(y),
        }
    }

    /// Create a point from mils.
    #[inline]
    pub fn from_mil(x: f64, y: f64) -> Self {
        Point {
            x: Nm::from_mil(x),
            y: Nm::from_mil(y),
        }
    }

    /// Create a point from inches.
    #[inline]
    pub fn from_inch(x: f64, y: f64) -> Self {
        Point {
            x: Nm::from_inch(x),
            y: Nm::from_inch(y),
        }
    }

    /// Calculate the squared distance to another point.
    ///
    /// Uses i128 for the intermediate calculation to avoid overflow
    /// when dealing with large coordinates.
    ///
    /// Returns the squared distance to avoid the cost of sqrt.
    /// Use this for distance comparisons.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::Point;
    ///
    /// let p1 = Point::from_mm(0.0, 0.0);
    /// let p2 = Point::from_mm(3.0, 4.0);
    ///
    /// // 3^2 + 4^2 = 25 (in mm), which is 25 * 10^12 in nm^2
    /// let dist_sq = p1.distance_squared(p2);
    /// assert_eq!(dist_sq, 25_000_000_000_000i128);
    /// ```
    #[inline]
    pub fn distance_squared(self, other: Point) -> i128 {
        let dx = (self.x.0 - other.x.0) as i128;
        let dy = (self.y.0 - other.y.0) as i128;
        dx * dx + dy * dy
    }

    /// Calculate the Manhattan distance to another point.
    ///
    /// This is |dx| + |dy|, useful for grid-based routing.
    #[inline]
    pub fn manhattan_distance(self, other: Point) -> Nm {
        let dx = (self.x.0 - other.x.0).abs();
        let dy = (self.y.0 - other.y.0).abs();
        Nm(dx + dy)
    }

    /// Add an offset to this point.
    #[inline]
    pub fn offset(self, dx: Nm, dy: Nm) -> Self {
        Point {
            x: Nm(self.x.0 + dx.0),
            y: Nm(self.y.0 + dy.0),
        }
    }
}

impl Add for Point {
    type Output = Point;

    #[inline]
    fn add(self, rhs: Point) -> Point {
        Point {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Point {
    type Output = Point;

    #[inline]
    fn sub(self, rhs: Point) -> Point {
        Point {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl std::fmt::Display for Point {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nm_from_mm() {
        assert_eq!(Nm::from_mm(1.0).0, 1_000_000);
        assert_eq!(Nm::from_mm(0.5).0, 500_000);
        assert_eq!(Nm::from_mm(0.001).0, 1_000);
    }

    #[test]
    fn test_nm_from_mil() {
        assert_eq!(Nm::from_mil(1.0).0, 25_400);
        assert_eq!(Nm::from_mil(10.0).0, 254_000);
    }

    #[test]
    fn test_nm_from_inch() {
        assert_eq!(Nm::from_inch(1.0).0, 25_400_000);
        assert_eq!(Nm::from_inch(0.1).0, 2_540_000);
    }

    #[test]
    fn test_nm_round_trip_mm() {
        let original = 10.5;
        let nm = Nm::from_mm(original);
        let back = nm.to_mm();
        assert!((back - original).abs() < 1e-6);
    }

    #[test]
    fn test_nm_round_trip_mil() {
        let original = 100.0;
        let nm = Nm::from_mil(original);
        let back = nm.to_mil();
        assert!((back - original).abs() < 1e-6);
    }

    #[test]
    fn test_nm_round_trip_inch() {
        let original = 2.5;
        let nm = Nm::from_inch(original);
        let back = nm.to_inch();
        assert!((back - original).abs() < 1e-6);
    }

    #[test]
    fn test_nm_arithmetic() {
        let a = Nm(100);
        let b = Nm(50);

        assert_eq!(a + b, Nm(150));
        assert_eq!(a - b, Nm(50));
        assert_eq!(a * 2, Nm(200));
        assert_eq!(a / 2, Nm(50));
        assert_eq!(-a, Nm(-100));
    }

    #[test]
    fn test_point_from_mm() {
        let p = Point::from_mm(1.0, 2.0);
        assert_eq!(p.x, Nm(1_000_000));
        assert_eq!(p.y, Nm(2_000_000));
    }

    #[test]
    fn test_point_distance_squared() {
        let p1 = Point::from_mm(0.0, 0.0);
        let p2 = Point::from_mm(3.0, 4.0);

        // 3^2 + 4^2 = 25 mm^2 = 25 * 10^12 nm^2
        let dist_sq = p1.distance_squared(p2);
        assert_eq!(dist_sq, 25_000_000_000_000i128);
    }

    #[test]
    fn test_point_manhattan_distance() {
        let p1 = Point::from_mm(0.0, 0.0);
        let p2 = Point::from_mm(3.0, 4.0);

        // |3| + |4| = 7 mm = 7,000,000 nm
        let dist = p1.manhattan_distance(p2);
        assert_eq!(dist, Nm(7_000_000));
    }

    #[test]
    fn test_conversion_constants() {
        // 1 inch = 25.4 mm
        assert_eq!(NM_PER_INCH, 25_400_000);

        // 1 mil = 0.001 inch = 0.0254 mm
        assert_eq!(NM_PER_MIL, 25_400);

        // 1 mm = 1,000,000 nm
        assert_eq!(NM_PER_MM, 1_000_000);

        // Verify relationship: 1 inch = 1000 mils
        assert_eq!(NM_PER_INCH, NM_PER_MIL * 1000);

        // Verify relationship: 1 inch = 25.4 mm
        assert_eq!(NM_PER_INCH, (NM_PER_MM as f64 * 25.4) as i64);
    }
}
