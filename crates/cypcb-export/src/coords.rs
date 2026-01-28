//! Coordinate conversion utilities for Gerber and Excellon export.
//!
//! Converts internal nanometer coordinates to decimal format for manufacturing files.
//! Uses integer arithmetic to avoid floating-point precision loss.

use serde::{Deserialize, Serialize};

/// Unit system for coordinate output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    /// Millimeters (most common for modern PCBs)
    Millimeters,
    /// Inches (legacy format, still supported)
    Inches,
}

/// Coordinate format specification for Gerber/Excellon files.
///
/// Defines the number of integer and decimal places for coordinate values.
/// Format is typically specified as N.M where N is integer places and M is decimal places.
///
/// # Examples
///
/// ```
/// use cypcb_export::coords::CoordinateFormat;
///
/// // 2.6 format: 2 integer places, 6 decimal places (mm)
/// let format = CoordinateFormat::FORMAT_MM_2_6;
/// assert_eq!(format.integer_places, 2);
/// assert_eq!(format.decimal_places, 6);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoordinateFormat {
    /// Unit system (millimeters or inches)
    pub unit: Unit,
    /// Number of integer places (typically 2-4)
    pub integer_places: u8,
    /// Number of decimal places (typically 4-6)
    pub decimal_places: u8,
}

impl CoordinateFormat {
    /// Standard format: 2 integer, 6 decimal places in millimeters.
    ///
    /// This is the most common format for modern PCB fabrication.
    /// Provides 0.001mm (1µm) precision.
    pub const FORMAT_MM_2_6: CoordinateFormat = CoordinateFormat {
        unit: Unit::Millimeters,
        integer_places: 2,
        decimal_places: 6,
    };

    /// Legacy format: 2 integer, 4 decimal places in inches.
    ///
    /// Older format, less common but still supported.
    /// Provides 0.0001 inch precision.
    pub const FORMAT_INCH_2_4: CoordinateFormat = CoordinateFormat {
        unit: Unit::Inches,
        integer_places: 2,
        decimal_places: 4,
    };
}

/// Convert nanometers to Gerber/Excellon decimal format string.
///
/// Uses integer arithmetic to avoid floating-point precision loss.
/// Handles negative coordinates correctly.
///
/// # Arguments
///
/// * `nm` - Coordinate value in nanometers
/// * `format` - Coordinate format specification
///
/// # Examples
///
/// ```
/// use cypcb_export::coords::{nm_to_gerber, CoordinateFormat};
///
/// let format = CoordinateFormat::FORMAT_MM_2_6;
///
/// // 1mm = 1,000,000 nm
/// assert_eq!(nm_to_gerber(1_000_000, &format), "1.000000");
///
/// // 0 nm
/// assert_eq!(nm_to_gerber(0, &format), "0.000000");
///
/// // Negative coordinate
/// assert_eq!(nm_to_gerber(-1_000_000, &format), "-1.000000");
/// ```
pub fn nm_to_gerber(nm: i64, format: &CoordinateFormat) -> String {
    // Conversion factor from nanometers to unit
    let nm_per_unit = match format.unit {
        Unit::Millimeters => 1_000_000i64,  // 1mm = 1,000,000 nm
        Unit::Inches => 25_400_000i64,       // 1 inch = 25,400,000 nm
    };

    // Separate sign from magnitude for cleaner arithmetic
    let is_negative = nm < 0;
    let abs_nm = nm.abs();

    // Integer part: divide by conversion factor
    let integer_part = abs_nm / nm_per_unit;

    // Fractional part: remainder * 10^decimal_places / nm_per_unit
    let remainder = abs_nm % nm_per_unit;
    let scale = 10i64.pow(format.decimal_places as u32);
    let fractional_part = (remainder * scale) / nm_per_unit;

    // Format with zero-padding for fractional part
    let sign = if is_negative { "-" } else { "" };
    format!(
        "{}{}.{:0width$}",
        sign,
        integer_part,
        fractional_part,
        width = format.decimal_places as usize
    )
}

/// Generate Gerber format declaration string.
///
/// Returns the %FS...% format string that declares coordinate format in Gerber files.
///
/// # Arguments
///
/// * `format` - Coordinate format specification
///
/// # Examples
///
/// ```
/// use cypcb_export::coords::{gerber_format_string, CoordinateFormat};
///
/// let format = CoordinateFormat::FORMAT_MM_2_6;
/// assert_eq!(gerber_format_string(&format), "%FSLAX26Y26*%");
/// ```
pub fn gerber_format_string(format: &CoordinateFormat) -> String {
    // Format: %FSLAX{int}{dec}Y{int}{dec}*%
    // FS = Format Statement
    // L = Leading zeros omitted
    // A = Absolute coordinates
    // X{int}{dec} = X coordinate format
    // Y{int}{dec} = Y coordinate format
    format!(
        "%FSLAX{}{}Y{}{}*%",
        format.integer_places,
        format.decimal_places,
        format.integer_places,
        format.decimal_places
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nm_to_gerber_zero() {
        let format = CoordinateFormat::FORMAT_MM_2_6;
        assert_eq!(nm_to_gerber(0, &format), "0.000000");
    }

    #[test]
    fn test_nm_to_gerber_one_mm() {
        let format = CoordinateFormat::FORMAT_MM_2_6;
        // 1mm = 1,000,000 nm
        assert_eq!(nm_to_gerber(1_000_000, &format), "1.000000");
    }

    #[test]
    fn test_nm_to_gerber_fractional_mm() {
        let format = CoordinateFormat::FORMAT_MM_2_6;
        // 1.5mm = 1,500,000 nm
        assert_eq!(nm_to_gerber(1_500_000, &format), "1.500000");
        // 0.123456mm = 123,456 nm
        assert_eq!(nm_to_gerber(123_456, &format), "0.123456");
    }

    #[test]
    fn test_nm_to_gerber_negative() {
        let format = CoordinateFormat::FORMAT_MM_2_6;
        assert_eq!(nm_to_gerber(-1_000_000, &format), "-1.000000");
        assert_eq!(nm_to_gerber(-1_500_000, &format), "-1.500000");
    }

    #[test]
    fn test_nm_to_gerber_inches() {
        let format = CoordinateFormat::FORMAT_INCH_2_4;
        // 1 inch = 25,400,000 nm
        assert_eq!(nm_to_gerber(25_400_000, &format), "1.0000");
        // 0.5 inch = 12,700,000 nm
        assert_eq!(nm_to_gerber(12_700_000, &format), "0.5000");
    }

    #[test]
    fn test_nm_to_gerber_precision() {
        let format = CoordinateFormat::FORMAT_MM_2_6;
        // Test edge case: very small value
        // 1nm should round to 0.000001mm
        assert_eq!(nm_to_gerber(1, &format), "0.000001");
        // 999,999 nm = 0.999999mm
        assert_eq!(nm_to_gerber(999_999, &format), "0.999999");
    }

    #[test]
    fn test_nm_to_gerber_large_values() {
        let format = CoordinateFormat::FORMAT_MM_2_6;
        // 100mm = 100,000,000 nm
        assert_eq!(nm_to_gerber(100_000_000, &format), "100.000000");
        // 99.999999mm = 99,999,999 nm
        assert_eq!(nm_to_gerber(99_999_999, &format), "99.999999");
    }

    #[test]
    fn test_gerber_format_string_mm() {
        let format = CoordinateFormat::FORMAT_MM_2_6;
        assert_eq!(gerber_format_string(&format), "%FSLAX26Y26*%");
    }

    #[test]
    fn test_gerber_format_string_inch() {
        let format = CoordinateFormat::FORMAT_INCH_2_4;
        assert_eq!(gerber_format_string(&format), "%FSLAX24Y24*%");
    }

    #[test]
    fn test_custom_format() {
        let format = CoordinateFormat {
            unit: Unit::Millimeters,
            integer_places: 3,
            decimal_places: 5,
        };
        assert_eq!(gerber_format_string(&format), "%FSLAX35Y35*%");
        // 10mm with 5 decimal places
        assert_eq!(nm_to_gerber(10_000_000, &format), "10.00000");
    }

    #[test]
    fn test_format_constants() {
        // Verify constants are correctly defined
        assert_eq!(CoordinateFormat::FORMAT_MM_2_6.unit, Unit::Millimeters);
        assert_eq!(CoordinateFormat::FORMAT_MM_2_6.integer_places, 2);
        assert_eq!(CoordinateFormat::FORMAT_MM_2_6.decimal_places, 6);

        assert_eq!(CoordinateFormat::FORMAT_INCH_2_4.unit, Unit::Inches);
        assert_eq!(CoordinateFormat::FORMAT_INCH_2_4.integer_places, 2);
        assert_eq!(CoordinateFormat::FORMAT_INCH_2_4.decimal_places, 4);
    }
}
