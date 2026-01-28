//! Component Placement List (CPL) / Pick-and-Place export functionality.
//!
//! Provides CPL generation in CSV format compatible with JLCPCB and other
//! pick-and-place machines. Used for automated PCB assembly.

pub mod csv;

pub use csv::export_cpl;

use serde::{Deserialize, Serialize};

/// A single component placement entry.
///
/// Represents the physical position and rotation of a component for
/// pick-and-place machine programming.
///
/// # Examples
///
/// ```
/// use cypcb_export::cpl::CplEntry;
///
/// let entry = CplEntry {
///     designator: "U1".to_string(),
///     x_mm: 50.8,
///     y_mm: 30.48,
///     layer: "Top".to_string(),
///     rotation: 90.0,
/// };
///
/// assert_eq!(entry.designator, "U1");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CplEntry {
    /// Component designator (e.g., "U1", "R2").
    pub designator: String,
    /// X coordinate of component center in millimeters.
    pub x_mm: f64,
    /// Y coordinate of component center in millimeters.
    pub y_mm: f64,
    /// Layer ("Top" or "Bottom").
    pub layer: String,
    /// Rotation angle in degrees (0-359.999), counterclockwise from 0° (east).
    pub rotation: f64,
}

/// Configuration for CPL export.
///
/// Allows customization of rotation offsets and coordinate systems
/// to match different pick-and-place machine requirements.
///
/// # Examples
///
/// ```
/// use cypcb_export::cpl::CplConfig;
///
/// // Default configuration
/// let config = CplConfig::default();
/// assert_eq!(config.rotation_offset, 0.0);
/// assert!(!config.flip_y);
///
/// // Custom configuration for machine with 90° offset
/// let custom = CplConfig {
///     rotation_offset: 90.0,
///     flip_y: false,
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CplConfig {
    /// Rotation offset added to all components (in degrees).
    ///
    /// Some pick-and-place machines use different rotation conventions.
    /// This offset is added to every component's rotation angle.
    pub rotation_offset: f64,

    /// Flip Y-axis (for machines using Y-down coordinate systems).
    ///
    /// By default, Y increases upward (standard math convention).
    /// Some machines use Y-down (screen coordinates).
    pub flip_y: bool,
}

impl Default for CplConfig {
    fn default() -> Self {
        CplConfig {
            rotation_offset: 0.0,
            flip_y: false,
        }
    }
}

impl CplConfig {
    /// Create a configuration with a rotation offset.
    #[inline]
    pub fn with_rotation_offset(offset: f64) -> Self {
        CplConfig {
            rotation_offset: offset,
            flip_y: false,
        }
    }

    /// Create a configuration with Y-axis flipped.
    #[inline]
    pub fn with_flipped_y() -> Self {
        CplConfig {
            rotation_offset: 0.0,
            flip_y: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpl_entry_creation() {
        let entry = CplEntry {
            designator: "R1".to_string(),
            x_mm: 10.5,
            y_mm: 20.3,
            layer: "Top".to_string(),
            rotation: 45.0,
        };

        assert_eq!(entry.designator, "R1");
        assert_eq!(entry.x_mm, 10.5);
        assert_eq!(entry.y_mm, 20.3);
        assert_eq!(entry.layer, "Top");
        assert_eq!(entry.rotation, 45.0);
    }

    #[test]
    fn test_cpl_config_default() {
        let config = CplConfig::default();
        assert_eq!(config.rotation_offset, 0.0);
        assert!(!config.flip_y);
    }

    #[test]
    fn test_cpl_config_with_rotation_offset() {
        let config = CplConfig::with_rotation_offset(90.0);
        assert_eq!(config.rotation_offset, 90.0);
        assert!(!config.flip_y);
    }

    #[test]
    fn test_cpl_config_with_flipped_y() {
        let config = CplConfig::with_flipped_y();
        assert_eq!(config.rotation_offset, 0.0);
        assert!(config.flip_y);
    }
}
