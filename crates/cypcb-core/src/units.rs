//! Unit types for dimension parsing.
//!
//! This module provides the [`Unit`] enum for representing and converting
//! between different measurement units used in PCB design.

use crate::coords::{Nm, NM_PER_INCH, NM_PER_MIL, NM_PER_MM};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

/// A unit of measurement for PCB dimensions.
///
/// Used for parsing dimensions from the DSL and converting to internal
/// nanometer representation.
///
/// # Examples
///
/// ```
/// use cypcb_core::{Unit, Nm};
///
/// let unit = Unit::Mm;
/// let nm = unit.to_nm(10.0);
/// assert_eq!(nm.0, 10_000_000);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    /// Millimeters (1 mm = 1,000,000 nm)
    Mm,
    /// Mils / thousandths of an inch (1 mil = 25,400 nm)
    Mil,
    /// Inches (1 inch = 25,400,000 nm)
    Inch,
    /// Nanometers (native unit)
    Nm,
}

impl Unit {
    /// Convert a value in this unit to nanometers.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::Unit;
    ///
    /// assert_eq!(Unit::Mm.to_nm(1.0).0, 1_000_000);
    /// assert_eq!(Unit::Mil.to_nm(1.0).0, 25_400);
    /// assert_eq!(Unit::Inch.to_nm(1.0).0, 25_400_000);
    /// assert_eq!(Unit::Nm.to_nm(100.0).0, 100);
    /// ```
    #[inline]
    pub fn to_nm(self, value: f64) -> Nm {
        let nm = match self {
            Unit::Mm => (value * NM_PER_MM as f64).round() as i64,
            Unit::Mil => (value * NM_PER_MIL as f64).round() as i64,
            Unit::Inch => (value * NM_PER_INCH as f64).round() as i64,
            Unit::Nm => value.round() as i64,
        };
        Nm(nm)
    }

    /// Convert a nanometer value to this unit.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::{Unit, Nm};
    ///
    /// let nm = Nm(1_000_000);
    /// assert!((Unit::Mm.from_nm(nm) - 1.0).abs() < 1e-9);
    /// ```
    #[inline]
    pub fn from_nm(self, nm: Nm) -> f64 {
        match self {
            Unit::Mm => nm.0 as f64 / NM_PER_MM as f64,
            Unit::Mil => nm.0 as f64 / NM_PER_MIL as f64,
            Unit::Inch => nm.0 as f64 / NM_PER_INCH as f64,
            Unit::Nm => nm.0 as f64,
        }
    }

    /// Get the suffix string for this unit.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::Unit;
    ///
    /// assert_eq!(Unit::Mm.suffix(), "mm");
    /// assert_eq!(Unit::Mil.suffix(), "mil");
    /// ```
    pub const fn suffix(self) -> &'static str {
        match self {
            Unit::Mm => "mm",
            Unit::Mil => "mil",
            Unit::Inch => "in",
            Unit::Nm => "nm",
        }
    }

    /// Get the nanometers per unit.
    ///
    /// Returns 1 for nanometers.
    pub const fn nm_per_unit(self) -> i64 {
        match self {
            Unit::Mm => NM_PER_MM,
            Unit::Mil => NM_PER_MIL,
            Unit::Inch => NM_PER_INCH,
            Unit::Nm => 1,
        }
    }
}

impl Default for Unit {
    fn default() -> Self {
        Unit::Mm
    }
}

impl std::fmt::Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.suffix())
    }
}

/// Error type for unit parsing.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("unknown unit: '{0}' (expected: mm, mil, in, nm)")]
pub struct ParseUnitError(pub String);

impl FromStr for Unit {
    type Err = ParseUnitError;

    /// Parse a unit from a string.
    ///
    /// Accepts: "mm", "mil", "in", "inch", "nm"
    /// Case-insensitive.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_core::Unit;
    ///
    /// assert_eq!("mm".parse::<Unit>().unwrap(), Unit::Mm);
    /// assert_eq!("mil".parse::<Unit>().unwrap(), Unit::Mil);
    /// assert_eq!("in".parse::<Unit>().unwrap(), Unit::Inch);
    /// assert_eq!("inch".parse::<Unit>().unwrap(), Unit::Inch);
    /// assert_eq!("nm".parse::<Unit>().unwrap(), Unit::Nm);
    ///
    /// // Case insensitive
    /// assert_eq!("MM".parse::<Unit>().unwrap(), Unit::Mm);
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mm" => Ok(Unit::Mm),
            "mil" | "mils" => Ok(Unit::Mil),
            "in" | "inch" | "inches" => Ok(Unit::Inch),
            "nm" => Ok(Unit::Nm),
            _ => Err(ParseUnitError(s.to_string())),
        }
    }
}

/// A dimension value with an associated unit.
///
/// This is useful for preserving the original unit in the source
/// while still being able to convert to nanometers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Dimension {
    /// The numeric value in the original unit.
    pub value: f64,
    /// The unit of measurement.
    pub unit: Unit,
}

impl Dimension {
    /// Create a new dimension.
    pub const fn new(value: f64, unit: Unit) -> Self {
        Dimension { value, unit }
    }

    /// Create a dimension in millimeters.
    pub const fn mm(value: f64) -> Self {
        Dimension { value, unit: Unit::Mm }
    }

    /// Create a dimension in mils.
    pub const fn mil(value: f64) -> Self {
        Dimension { value, unit: Unit::Mil }
    }

    /// Create a dimension in inches.
    pub const fn inch(value: f64) -> Self {
        Dimension { value, unit: Unit::Inch }
    }

    /// Create a dimension in nanometers.
    pub const fn nm(value: f64) -> Self {
        Dimension { value, unit: Unit::Nm }
    }

    /// Convert this dimension to nanometers.
    pub fn to_nm(self) -> Nm {
        self.unit.to_nm(self.value)
    }
}

impl std::fmt::Display for Dimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.value, self.unit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unit_to_nm() {
        assert_eq!(Unit::Mm.to_nm(1.0).0, 1_000_000);
        assert_eq!(Unit::Mil.to_nm(1.0).0, 25_400);
        assert_eq!(Unit::Inch.to_nm(1.0).0, 25_400_000);
        assert_eq!(Unit::Nm.to_nm(1.0).0, 1);
    }

    #[test]
    fn test_unit_from_nm() {
        let nm = Nm(1_000_000);
        assert!((Unit::Mm.from_nm(nm) - 1.0).abs() < 1e-9);

        let nm = Nm(25_400);
        assert!((Unit::Mil.from_nm(nm) - 1.0).abs() < 1e-9);

        let nm = Nm(25_400_000);
        assert!((Unit::Inch.from_nm(nm) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_unit_round_trip() {
        for unit in [Unit::Mm, Unit::Mil, Unit::Inch] {
            let original = 10.5;
            let nm = unit.to_nm(original);
            let back = unit.from_nm(nm);
            assert!((back - original).abs() < 0.001, "Round-trip failed for {:?}", unit);
        }
    }

    #[test]
    fn test_unit_parse() {
        assert_eq!("mm".parse::<Unit>().unwrap(), Unit::Mm);
        assert_eq!("MM".parse::<Unit>().unwrap(), Unit::Mm);
        assert_eq!("mil".parse::<Unit>().unwrap(), Unit::Mil);
        assert_eq!("mils".parse::<Unit>().unwrap(), Unit::Mil);
        assert_eq!("in".parse::<Unit>().unwrap(), Unit::Inch);
        assert_eq!("inch".parse::<Unit>().unwrap(), Unit::Inch);
        assert_eq!("inches".parse::<Unit>().unwrap(), Unit::Inch);
        assert_eq!("nm".parse::<Unit>().unwrap(), Unit::Nm);
    }

    #[test]
    fn test_unit_parse_error() {
        let result = "cm".parse::<Unit>();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().0, "cm");
    }

    #[test]
    fn test_dimension_to_nm() {
        let dim = Dimension::mm(10.0);
        assert_eq!(dim.to_nm().0, 10_000_000);

        let dim = Dimension::mil(100.0);
        assert_eq!(dim.to_nm().0, 2_540_000);
    }

    #[test]
    fn test_unit_suffix() {
        assert_eq!(Unit::Mm.suffix(), "mm");
        assert_eq!(Unit::Mil.suffix(), "mil");
        assert_eq!(Unit::Inch.suffix(), "in");
        assert_eq!(Unit::Nm.suffix(), "nm");
    }
}
