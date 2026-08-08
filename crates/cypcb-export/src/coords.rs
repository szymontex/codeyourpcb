//! Coordinate conversion utilities for Gerber and Excellon export.
//!
//! Two shapes of number leave here. Gerber coordinate data has no decimal
//! point, because the file's `%FS` line already declared where it is; sizes
//! and drill data keep theirs, because nothing declared it for them. See
//! [`nm_to_gerber`] and [`nm_to_decimal`].
//!
//! Integer arithmetic throughout, so nothing is rounded on the way out.

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

/// Convert nanometres to a Gerber coordinate.
///
/// Gerber coordinate data carries no decimal point: the file's `%FS` line
/// declares where it goes. `%FSLAX26Y26*%` is two integer digits and six
/// decimal ones with the point implied, and the `L` suppresses leading zeros,
/// so 1mm is `1000000` and 0.5mm is `500000`.
///
/// For a number that is *not* coordinate data - an aperture size, a drill
/// diameter, a hole position in an Excellon file - use [`nm_to_decimal`],
/// which keeps the point. Writing a coordinate with a point, or a size
/// without one, makes a file that argues with its own header.
///
/// Integer arithmetic throughout, so no value is rounded on the way out.
///
/// # Arguments
///
/// * `nm` - Coordinate value in nanometers
/// * `format` - Coordinate format specification
///
/// # Examples
///
/// ```
/// use cypcb_export::coords::{nm_to_gerber, nm_to_decimal, CoordinateFormat};
///
/// let format = CoordinateFormat::FORMAT_MM_2_6;
///
/// // 1mm = 1,000,000 nm, and six decimals are implied.
/// assert_eq!(nm_to_gerber(1_000_000, &format), "1000000");
///
/// // Leading zeros are suppressed, so zero is a single digit.
/// assert_eq!(nm_to_gerber(0, &format), "0");
///
/// // Negative coordinate
/// assert_eq!(nm_to_gerber(-1_000_000, &format), "-1000000");
///
/// // The same value as a size, which keeps its point.
/// assert_eq!(nm_to_decimal(1_000_000, &format), "1.000000");
/// ```
pub fn nm_to_gerber(nm: i64, format: &CoordinateFormat) -> String {
    // Conversion factor from nanometers to unit
    let nm_per_unit = match format.unit {
        Unit::Millimeters => 1_000_000i64, // 1mm = 1,000,000 nm
        Unit::Inches => 25_400_000i64,     // 1 inch = 25,400,000 nm
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

    // No decimal point: the header already said where it goes.
    //
    // `%FSLAX26Y26*%` declares two integer digits and six decimal ones, with
    // the point implied - that is what the `26` means. Writing `X3.730000`
    // under that declaration is a file arguing with its own header, and every
    // Gerber this project has produced did it. A CAM tool that reads the
    // format line and then the coordinates is entitled to take `3.730000` as
    // something other than 3.73mm.
    //
    // The fractional part stays padded to `decimal_places`, because those
    // digits are positional now rather than decorative: dropping a trailing
    // zero moves the point.
    // The `L` in `%FSLAX26Y26*%` is leading-zero suppression, so 0.5mm is
    // `500000` rather than `0500000`, and zero is `0`. Trailing zeros stay:
    // they carry the decimal point's position now.
    let sign = if is_negative { "-" } else { "" };
    let digits = format!(
        "{}{:0width$}",
        integer_part,
        fractional_part,
        width = format.decimal_places as usize
    );
    let significant = digits.trim_start_matches('0');
    let significant = if significant.is_empty() {
        "0"
    } else {
        significant
    };
    format!("{sign}{significant}")
}

/// The same value as a plain decimal number, for everything that is not a
/// Gerber coordinate.
///
/// Two callers need this. A drill file's header here is `METRIC,TZ`, which
/// names no digit count at all, so an integer in its body would have no
/// declared scale and a reader assuming the usual 3.3 would put a hole at
/// 3730mm. And a Gerber aperture definition - `%ADD10C,1.500000*%` - is a
/// size in the file's units, not coordinate data: written without its point
/// it becomes a 1.5-metre aperture.
///
/// So the rule is not "Gerber drops the point". It is that each number is
/// written the way the line it sits on declares, and only coordinate data has
/// a `%FS` declaration telling a reader where the point went.
pub fn nm_to_decimal(nm: i64, format: &CoordinateFormat) -> String {
    let nm_per_unit = match format.unit {
        Unit::Millimeters => 1_000_000i64,
        Unit::Inches => 25_400_000i64,
    };
    let sign = if nm < 0 { "-" } else { "" };
    let abs_nm = nm.abs();
    let scale = 10i64.pow(format.decimal_places as u32);
    format!(
        "{}{}.{:0width$}",
        sign,
        abs_nm / nm_per_unit,
        ((abs_nm % nm_per_unit) * scale) / nm_per_unit,
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
        format.integer_places, format.decimal_places, format.integer_places, format.decimal_places
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Coordinate data: no decimal point, because `%FS` already said where it
    // is, and no leading zeros, because the `L` said they are suppressed. Each
    // case is paired with the decimal form of the same value, which is what
    // apertures and drill files carry.
    #[test]
    fn test_nm_to_gerber_zero() {
        let format = CoordinateFormat::FORMAT_MM_2_6;
        assert_eq!(nm_to_gerber(0, &format), "0");
        assert_eq!(nm_to_decimal(0, &format), "0.000000");
    }

    #[test]
    fn test_nm_to_gerber_one_mm() {
        let format = CoordinateFormat::FORMAT_MM_2_6;
        // 1mm = 1,000,000 nm
        assert_eq!(nm_to_gerber(1_000_000, &format), "1000000");
        assert_eq!(nm_to_decimal(1_000_000, &format), "1.000000");
    }

    #[test]
    fn test_nm_to_gerber_fractional_mm() {
        let format = CoordinateFormat::FORMAT_MM_2_6;
        // 1.5mm = 1,500,000 nm
        assert_eq!(nm_to_gerber(1_500_000, &format), "1500000");
        // 0.123456mm: the integer zero is suppressed, the six digits are not.
        assert_eq!(nm_to_gerber(123_456, &format), "123456");
        assert_eq!(nm_to_decimal(123_456, &format), "0.123456");
    }

    #[test]
    fn test_nm_to_gerber_negative() {
        let format = CoordinateFormat::FORMAT_MM_2_6;
        assert_eq!(nm_to_gerber(-1_000_000, &format), "-1000000");
        assert_eq!(nm_to_gerber(-1_500_000, &format), "-1500000");
        assert_eq!(nm_to_decimal(-1_500_000, &format), "-1.500000");
    }

    #[test]
    fn test_nm_to_gerber_inches() {
        let format = CoordinateFormat::FORMAT_INCH_2_4;
        // 1 inch = 25,400,000 nm, four decimals here rather than six.
        assert_eq!(nm_to_gerber(25_400_000, &format), "10000");
        // 0.5 inch = 12,700,000 nm
        assert_eq!(nm_to_gerber(12_700_000, &format), "5000");
        assert_eq!(nm_to_decimal(12_700_000, &format), "0.5000");
    }

    #[test]
    fn test_nm_to_gerber_precision() {
        let format = CoordinateFormat::FORMAT_MM_2_6;
        // 1nm is 0.000001mm: every leading zero goes, so a single digit is
        // left and the reader recovers the value from the declared six.
        assert_eq!(nm_to_gerber(1, &format), "1");
        assert_eq!(nm_to_decimal(1, &format), "0.000001");
        // 999,999 nm = 0.999999mm
        assert_eq!(nm_to_gerber(999_999, &format), "999999");
    }

    #[test]
    fn test_nm_to_gerber_large_values() {
        let format = CoordinateFormat::FORMAT_MM_2_6;
        // 100mm = 100,000,000 nm. Nine digits under a format that declared
        // two integer places: the declaration bounds what a reader must
        // accept, not what the exporter may write, and a board this size is
        // ordinary.
        assert_eq!(nm_to_gerber(100_000_000, &format), "100000000");
        // 99.999999mm = 99,999,999 nm
        assert_eq!(nm_to_gerber(99_999_999, &format), "99999999");
        assert_eq!(nm_to_decimal(99_999_999, &format), "99.999999");
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
        // 10mm under five declared decimals. The declaration and the data have
        // to move together: this is the pairing that a reader relies on, and
        // the one the exporter got wrong for its whole life.
        assert_eq!(nm_to_gerber(10_000_000, &format), "1000000");
        assert_eq!(nm_to_decimal(10_000_000, &format), "10.00000");
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
