//! IPC-2221 Trace Width Calculator
//!
//! Calculates minimum trace width based on current carrying requirements using the
//! IPC-2221 standard formulas. The calculator accounts for:
//!
//! - Current capacity
//! - Temperature rise
//! - Copper thickness (weight in oz/ft²)
//! - Layer position (external vs internal)
//!
//! # IPC-2221 Formula
//!
//! The standard formula is: `I = k * dT^0.44 * A^0.725`
//!
//! Where:
//! - `I` = current in amperes
//! - `k` = constant (0.048 for external layers, 0.024 for internal)
//! - `dT` = temperature rise above ambient in Celsius
//! - `A` = cross-sectional area in mils²
//!
//! We solve for area: `A = (I / (k * dT^0.44))^(1/0.725)`
//!
//! Then convert to width: `width = A / (thickness_mils)`
//! Where `thickness_mils = copper_oz * 1.378`
//!
//! # Example
//!
//! ```
//! use cypcb_calc::{TraceWidthCalculator, TraceWidthParams};
//!
//! // Calculate trace width for 1A, 1oz copper, external layer, 10C rise
//! let params = TraceWidthParams::new(1.0);
//! let result = TraceWidthCalculator::calculate(&params);
//!
//! // Should be approximately 0.25mm (10 mils)
//! assert!(result.width.to_mm() > 0.2 && result.width.to_mm() < 0.35);
//! ```
//!
//! # Accuracy Limits
//!
//! IPC-2221 formulas are approximations accurate under specific conditions:
//! - Current: 0-35A (accuracy degrades above 35A)
//! - Temperature rise: 10-100C
//! - Trace width: up to ~400 mils (10mm)
//!
//! The calculator generates warnings when inputs fall outside these ranges.

use cypcb_core::Nm;

/// IPC-2221 coefficient for external (outer) copper layers.
///
/// External layers dissipate heat more efficiently due to air convection.
const K_EXTERNAL: f64 = 0.048;

/// IPC-2221 coefficient for internal copper layers.
///
/// Internal layers are surrounded by FR4 which is a poor thermal conductor.
const K_INTERNAL: f64 = 0.024;

/// Copper thickness in mils per oz/ft² of copper weight.
///
/// 1 oz copper = 1.378 mils = 35μm thickness.
const MILS_PER_OZ: f64 = 1.378;

/// Conversion: nanometers per mil.
const NM_PER_MIL: f64 = 25_400.0;

/// Maximum current for accurate IPC-2221 calculations (amps).
const MAX_CURRENT_ACCURATE: f64 = 35.0;

/// Minimum recommended temperature rise (Celsius).
const MIN_TEMP_RISE: f64 = 10.0;

/// Maximum recommended temperature rise (Celsius).
const MAX_TEMP_RISE: f64 = 100.0;

/// Maximum recommended trace width in mm before considering multiple traces.
const MAX_WIDTH_MM: f64 = 10.0;

/// Standard copper weights (oz/ft²) for manufacturing.
const STANDARD_COPPER_WEIGHTS: &[f64] = &[0.5, 1.0, 2.0, 3.0];

/// Parameters for trace width calculation.
///
/// # Example
///
/// ```
/// use cypcb_calc::TraceWidthParams;
///
/// // Create with defaults (1oz copper, 10C rise, external layer)
/// let params = TraceWidthParams::new(2.0);
/// assert_eq!(params.current_amps, 2.0);
/// assert_eq!(params.copper_oz, 1.0);
/// assert!(params.is_external);
///
/// // Create with custom parameters
/// let params = TraceWidthParams::new(5.0)
///     .with_temp_rise(20.0)
///     .with_copper_oz(2.0)
///     .internal();
/// assert_eq!(params.temp_rise_c, 20.0);
/// assert!(!params.is_external);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TraceWidthParams {
    /// Current in amperes (required)
    pub current_amps: f64,
    /// Allowed temperature rise in Celsius (default: 10.0)
    pub temp_rise_c: f64,
    /// Copper weight in oz/ft² (default: 1.0, typical range 0.5-3.0)
    pub copper_oz: f64,
    /// External layer (true) or internal layer (false)
    pub is_external: bool,
    /// Ambient temperature in Celsius (informational, default: 25.0)
    pub ambient_temp_c: f64,
}

impl TraceWidthParams {
    /// Create new parameters with required current value.
    ///
    /// Uses defaults:
    /// - Temperature rise: 10C
    /// - Copper weight: 1oz
    /// - External layer: true
    /// - Ambient temperature: 25C
    pub fn new(current_amps: f64) -> Self {
        Self {
            current_amps,
            temp_rise_c: 10.0,
            copper_oz: 1.0,
            is_external: true,
            ambient_temp_c: 25.0,
        }
    }

    /// Set custom temperature rise.
    pub fn with_temp_rise(mut self, temp_rise_c: f64) -> Self {
        self.temp_rise_c = temp_rise_c;
        self
    }

    /// Set custom copper weight.
    pub fn with_copper_oz(mut self, copper_oz: f64) -> Self {
        self.copper_oz = copper_oz;
        self
    }

    /// Set to internal layer (less heat dissipation).
    pub fn internal(mut self) -> Self {
        self.is_external = false;
        self
    }

    /// Set to external layer (better heat dissipation).
    pub fn external(mut self) -> Self {
        self.is_external = true;
        self
    }

    /// Set ambient temperature (informational).
    pub fn with_ambient_temp(mut self, ambient_temp_c: f64) -> Self {
        self.ambient_temp_c = ambient_temp_c;
        self
    }
}

impl Default for TraceWidthParams {
    fn default() -> Self {
        Self::new(1.0)
    }
}

/// Warnings about calculation accuracy or design concerns.
///
/// These warnings indicate that the calculation may be less accurate
/// or that the design might have issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TraceWidthWarning {
    /// Current >35A - formula accuracy degrades.
    ///
    /// IPC-2221 formulas were derived from data up to ~35A.
    /// Higher currents may need experimental validation.
    CurrentTooHigh,

    /// Temperature rise <10C may not be achievable.
    ///
    /// Very low temperature rise requires extremely wide traces
    /// and may not be practical.
    TempRiseTooLow,

    /// Temperature rise >100C risks delamination.
    ///
    /// High temperatures can cause FR4 to delaminate and damage
    /// solder joints. Consider heat sinking or thicker copper.
    TempRiseTooHigh,

    /// Width >10mm - consider multiple traces.
    ///
    /// Very wide traces may cause manufacturing issues and uneven
    /// current distribution. Multiple parallel traces are better.
    WidthExceedsMax,

    /// Non-standard copper weight.
    ///
    /// Standard weights are 0.5, 1.0, 2.0, and 3.0 oz/ft².
    /// Non-standard weights may have limited fab availability.
    CopperWeightNonStandard,
}

impl std::fmt::Display for TraceWidthWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CurrentTooHigh => {
                write!(f, "Current >35A: IPC-2221 accuracy degrades, consider experimental validation")
            }
            Self::TempRiseTooLow => {
                write!(f, "Temperature rise <10C: may require impractically wide traces")
            }
            Self::TempRiseTooHigh => {
                write!(f, "Temperature rise >100C: risk of delamination and solder joint damage")
            }
            Self::WidthExceedsMax => {
                write!(f, "Width >10mm: consider multiple parallel traces for better current distribution")
            }
            Self::CopperWeightNonStandard => {
                write!(f, "Non-standard copper weight: may have limited fab availability")
            }
        }
    }
}

/// Result of trace width calculation.
///
/// Contains the calculated width, cross-sectional area, and any warnings.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceWidthResult {
    /// Calculated minimum trace width in nanometers.
    pub width: Nm,

    /// Cross-sectional area in mm².
    ///
    /// This is the copper cross-section perpendicular to current flow.
    /// Useful for verifying calculations or comparing to wire gauges.
    pub cross_section_mm2: f64,

    /// Any warnings about the calculation.
    ///
    /// Empty if all parameters are within recommended ranges.
    pub warnings: Vec<TraceWidthWarning>,
}

impl TraceWidthResult {
    /// Check if the calculation has any warnings.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Get the width in millimeters.
    pub fn width_mm(&self) -> f64 {
        self.width.to_mm()
    }

    /// Get the width in mils (thousandths of an inch).
    pub fn width_mil(&self) -> f64 {
        self.width.to_mil()
    }
}

/// IPC-2221 trace width calculator.
///
/// Calculates minimum trace width for given current requirements using
/// the IPC-2221 standard formulas.
///
/// # Example
///
/// ```
/// use cypcb_calc::{TraceWidthCalculator, TraceWidthParams};
///
/// // Simple calculation with defaults
/// let width = TraceWidthCalculator::min_width_for_current(1.0, true);
/// println!("1A external: {:.2}mm", width.to_mm());
///
/// // Full calculation with custom parameters
/// let params = TraceWidthParams::new(3.0)
///     .with_temp_rise(20.0)
///     .with_copper_oz(2.0);
/// let result = TraceWidthCalculator::calculate(&params);
/// println!("3A, 20C rise, 2oz: {:.2}mm", result.width.to_mm());
/// ```
pub struct TraceWidthCalculator;

impl TraceWidthCalculator {
    /// Calculate minimum trace width from parameters.
    ///
    /// This is the full calculation method that returns cross-sectional
    /// area and any warnings about the parameters.
    ///
    /// # Formula
    ///
    /// IPC-2221: `I = k * dT^0.44 * A^0.725`
    ///
    /// Solving for area: `A = (I / (k * dT^0.44))^(1/0.725)`
    ///
    /// Converting to width: `width = A / thickness`
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_calc::{TraceWidthCalculator, TraceWidthParams, TraceWidthWarning};
    ///
    /// let params = TraceWidthParams::new(1.0);
    /// let result = TraceWidthCalculator::calculate(&params);
    ///
    /// // Should be ~0.25mm for 1A, 1oz, external, 10C rise
    /// assert!(result.width.to_mm() > 0.2);
    /// assert!(result.width.to_mm() < 0.35);
    /// assert!(result.warnings.is_empty());
    ///
    /// // High current triggers warning
    /// let params = TraceWidthParams::new(50.0);
    /// let result = TraceWidthCalculator::calculate(&params);
    /// assert!(result.warnings.contains(&TraceWidthWarning::CurrentTooHigh));
    /// ```
    pub fn calculate(params: &TraceWidthParams) -> TraceWidthResult {
        let mut warnings = Vec::new();

        // Check for warnings
        if params.current_amps > MAX_CURRENT_ACCURATE {
            warnings.push(TraceWidthWarning::CurrentTooHigh);
        }

        if params.temp_rise_c < MIN_TEMP_RISE {
            warnings.push(TraceWidthWarning::TempRiseTooLow);
        }

        if params.temp_rise_c > MAX_TEMP_RISE {
            warnings.push(TraceWidthWarning::TempRiseTooHigh);
        }

        // Check for standard copper weights
        let is_standard = STANDARD_COPPER_WEIGHTS
            .iter()
            .any(|&w| (w - params.copper_oz).abs() < 0.01);
        if !is_standard {
            warnings.push(TraceWidthWarning::CopperWeightNonStandard);
        }

        // Select k constant based on layer position
        let k = if params.is_external {
            K_EXTERNAL
        } else {
            K_INTERNAL
        };

        // Calculate cross-sectional area in mils²
        // Formula: A = (I / (k * dT^0.44))^(1/0.725)
        let area_mils2 = (params.current_amps / (k * params.temp_rise_c.powf(0.44)))
            .powf(1.0 / 0.725);

        // Calculate copper thickness in mils
        let thickness_mils = params.copper_oz * MILS_PER_OZ;

        // Calculate width in mils
        let width_mils = area_mils2 / thickness_mils;

        // Convert to mm for the warning check
        let width_mm = width_mils * 0.0254; // 1 mil = 0.0254 mm

        if width_mm > MAX_WIDTH_MM {
            warnings.push(TraceWidthWarning::WidthExceedsMax);
        }

        // Convert cross-sectional area to mm²
        // 1 mil² = 0.0254² mm² = 0.00064516 mm²
        let cross_section_mm2 = area_mils2 * 0.00064516;

        // Convert width to nanometers
        let width = Nm::new((width_mils * NM_PER_MIL) as i64);

        TraceWidthResult {
            width,
            cross_section_mm2,
            warnings,
        }
    }

    /// Quick calculation for minimum trace width.
    ///
    /// Uses defaults:
    /// - Temperature rise: 10C
    /// - Copper weight: 1oz
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_calc::TraceWidthCalculator;
    ///
    /// // External layer (typical)
    /// let width = TraceWidthCalculator::min_width_for_current(1.0, true);
    /// println!("1A external: {:.2}mm", width.to_mm());
    ///
    /// // Internal layer (needs wider trace)
    /// let width = TraceWidthCalculator::min_width_for_current(1.0, false);
    /// println!("1A internal: {:.2}mm", width.to_mm());
    /// ```
    pub fn min_width_for_current(current_amps: f64, is_external: bool) -> Nm {
        let params = if is_external {
            TraceWidthParams::new(current_amps)
        } else {
            TraceWidthParams::new(current_amps).internal()
        };

        Self::calculate(&params).width
    }

    /// Create calculator with default settings.
    ///
    /// Returns a `TraceWidthParams` configured with common defaults
    /// that can be customized via builder methods.
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_calc::TraceWidthCalculator;
    ///
    /// let params = TraceWidthCalculator::with_defaults()
    ///     .with_temp_rise(20.0)
    ///     .with_copper_oz(2.0);
    /// ```
    pub fn with_defaults() -> TraceWidthParams {
        TraceWidthParams::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to check if a value is within percentage of expected.
    fn within_percent(actual: f64, expected: f64, percent: f64) -> bool {
        let tolerance = expected * (percent / 100.0);
        (actual - expected).abs() <= tolerance
    }

    #[test]
    fn test_1a_external_10c_rise() {
        // 1A, 1oz copper, external, 10C rise should give ~10 mils (~0.25mm)
        let params = TraceWidthParams::new(1.0);
        let result = TraceWidthCalculator::calculate(&params);

        // IPC-2221 reference: ~10 mils (0.254mm)
        // Allow 10% tolerance for rounding differences
        let width_mil = result.width.to_mil();
        assert!(
            width_mil > 8.0 && width_mil < 13.0,
            "Expected ~10 mils, got {} mils",
            width_mil
        );
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_1a_internal_needs_wider() {
        // Internal layers need wider traces due to poor heat dissipation
        let external = TraceWidthCalculator::min_width_for_current(1.0, true);
        let internal = TraceWidthCalculator::min_width_for_current(1.0, false);

        // Internal should be roughly 2x wider (k_internal = k_external / 2)
        assert!(
            internal.to_mm() > external.to_mm() * 1.5,
            "Internal ({:.2}mm) should be much wider than external ({:.2}mm)",
            internal.to_mm(),
            external.to_mm()
        );
    }

    #[test]
    fn test_10a_current() {
        // 10A current, external, 10C rise
        let params = TraceWidthParams::new(10.0);
        let result = TraceWidthCalculator::calculate(&params);

        // Higher current needs wider trace
        // IPC-2221 for 10A, 1oz, 10C rise: ~280 mils (~7mm)
        // This is intentionally wide - 10A at only 10C rise is demanding
        let width_mm = result.width.to_mm();
        assert!(
            width_mm > 5.0 && width_mm < 10.0,
            "Expected ~7mm for 10A at 10C rise, got {}mm",
            width_mm
        );
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_higher_temp_rise_narrower_trace() {
        // Higher temperature rise allows narrower trace
        let low_rise = TraceWidthParams::new(5.0).with_temp_rise(10.0);
        let high_rise = TraceWidthParams::new(5.0).with_temp_rise(40.0);

        let low_result = TraceWidthCalculator::calculate(&low_rise);
        let high_result = TraceWidthCalculator::calculate(&high_rise);

        assert!(
            high_result.width.to_mm() < low_result.width.to_mm(),
            "Higher temp rise ({:.2}mm) should allow narrower trace than low ({:.2}mm)",
            high_result.width.to_mm(),
            low_result.width.to_mm()
        );
    }

    #[test]
    fn test_thicker_copper_narrower_trace() {
        // Thicker copper (2oz vs 1oz) allows narrower trace
        let thin = TraceWidthParams::new(5.0).with_copper_oz(1.0);
        let thick = TraceWidthParams::new(5.0).with_copper_oz(2.0);

        let thin_result = TraceWidthCalculator::calculate(&thin);
        let thick_result = TraceWidthCalculator::calculate(&thick);

        assert!(
            thick_result.width.to_mm() < thin_result.width.to_mm(),
            "Thicker copper ({:.2}mm) should allow narrower trace than thin ({:.2}mm)",
            thick_result.width.to_mm(),
            thin_result.width.to_mm()
        );
    }

    #[test]
    fn test_warning_current_too_high() {
        let params = TraceWidthParams::new(50.0);
        let result = TraceWidthCalculator::calculate(&params);

        assert!(
            result.warnings.contains(&TraceWidthWarning::CurrentTooHigh),
            "Should warn about high current"
        );
    }

    #[test]
    fn test_warning_temp_rise_too_low() {
        let params = TraceWidthParams::new(1.0).with_temp_rise(5.0);
        let result = TraceWidthCalculator::calculate(&params);

        assert!(
            result.warnings.contains(&TraceWidthWarning::TempRiseTooLow),
            "Should warn about low temp rise"
        );
    }

    #[test]
    fn test_warning_temp_rise_too_high() {
        let params = TraceWidthParams::new(1.0).with_temp_rise(150.0);
        let result = TraceWidthCalculator::calculate(&params);

        assert!(
            result.warnings.contains(&TraceWidthWarning::TempRiseTooHigh),
            "Should warn about high temp rise"
        );
    }

    #[test]
    fn test_warning_width_exceeds_max() {
        // Very high current with low temp rise = very wide trace
        let params = TraceWidthParams::new(100.0).with_temp_rise(10.0);
        let result = TraceWidthCalculator::calculate(&params);

        assert!(
            result.warnings.contains(&TraceWidthWarning::WidthExceedsMax),
            "Should warn about excessive width"
        );
    }

    #[test]
    fn test_warning_non_standard_copper() {
        let params = TraceWidthParams::new(1.0).with_copper_oz(1.5);
        let result = TraceWidthCalculator::calculate(&params);

        assert!(
            result.warnings.contains(&TraceWidthWarning::CopperWeightNonStandard),
            "Should warn about non-standard copper weight"
        );
    }

    #[test]
    fn test_standard_copper_no_warning() {
        for &copper_oz in STANDARD_COPPER_WEIGHTS {
            let params = TraceWidthParams::new(1.0).with_copper_oz(copper_oz);
            let result = TraceWidthCalculator::calculate(&params);

            assert!(
                !result.warnings.contains(&TraceWidthWarning::CopperWeightNonStandard),
                "Should not warn for standard {}oz copper",
                copper_oz
            );
        }
    }

    #[test]
    fn test_cross_section_area() {
        let params = TraceWidthParams::new(1.0);
        let result = TraceWidthCalculator::calculate(&params);

        // Cross-section should be positive and reasonable
        assert!(result.cross_section_mm2 > 0.0);
        assert!(result.cross_section_mm2 < 1.0); // Less than 1mm² for 1A
    }

    #[test]
    fn test_builder_pattern() {
        let params = TraceWidthParams::new(5.0)
            .with_temp_rise(20.0)
            .with_copper_oz(2.0)
            .internal()
            .with_ambient_temp(40.0);

        assert_eq!(params.current_amps, 5.0);
        assert_eq!(params.temp_rise_c, 20.0);
        assert_eq!(params.copper_oz, 2.0);
        assert!(!params.is_external);
        assert_eq!(params.ambient_temp_c, 40.0);
    }

    #[test]
    fn test_with_defaults() {
        let params = TraceWidthCalculator::with_defaults();
        assert_eq!(params.current_amps, 1.0);
        assert_eq!(params.temp_rise_c, 10.0);
        assert_eq!(params.copper_oz, 1.0);
        assert!(params.is_external);
    }

    #[test]
    fn test_min_width_for_current_convenience() {
        let direct = TraceWidthCalculator::min_width_for_current(2.0, true);
        let params = TraceWidthParams::new(2.0);
        let via_params = TraceWidthCalculator::calculate(&params).width;

        assert_eq!(direct, via_params);
    }

    #[test]
    fn test_result_helpers() {
        let params = TraceWidthParams::new(1.0);
        let result = TraceWidthCalculator::calculate(&params);

        // width_mm and width_mil should match direct conversions
        assert!((result.width_mm() - result.width.to_mm()).abs() < 0.0001);
        assert!((result.width_mil() - result.width.to_mil()).abs() < 0.0001);

        // No warnings
        assert!(!result.has_warnings());
    }

    #[test]
    fn test_ipc2221_reference_values() {
        // Verify against IPC-2221 reference table values
        // These are approximate values from the standard

        // 1A, 1oz, external, 10C rise: ~10 mils
        let result = TraceWidthCalculator::calculate(&TraceWidthParams::new(1.0));
        assert!(
            within_percent(result.width_mil(), 10.0, 30.0),
            "1A should be ~10 mils, got {} mils",
            result.width_mil()
        );

        // 2A, 1oz, external, 10C rise: ~30 mils
        let result = TraceWidthCalculator::calculate(&TraceWidthParams::new(2.0));
        assert!(
            within_percent(result.width_mil(), 30.0, 30.0),
            "2A should be ~30 mils, got {} mils",
            result.width_mil()
        );

        // 3A, 1oz, external, 10C rise: ~50 mils
        let result = TraceWidthCalculator::calculate(&TraceWidthParams::new(3.0));
        assert!(
            within_percent(result.width_mil(), 50.0, 30.0),
            "3A should be ~50 mils, got {} mils",
            result.width_mil()
        );

        // 5A, 1oz, external, 10C rise: ~110 mils
        let result = TraceWidthCalculator::calculate(&TraceWidthParams::new(5.0));
        assert!(
            within_percent(result.width_mil(), 110.0, 30.0),
            "5A should be ~110 mils, got {} mils",
            result.width_mil()
        );
    }

    #[test]
    fn test_warning_display() {
        // Ensure Display trait works for warnings
        let warnings = vec![
            TraceWidthWarning::CurrentTooHigh,
            TraceWidthWarning::TempRiseTooLow,
            TraceWidthWarning::TempRiseTooHigh,
            TraceWidthWarning::WidthExceedsMax,
            TraceWidthWarning::CopperWeightNonStandard,
        ];

        for warning in warnings {
            let msg = format!("{}", warning);
            assert!(!msg.is_empty());
        }
    }
}
