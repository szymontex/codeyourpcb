//! Manufacturer preset design rules.
//!
//! This module provides pre-configured design rules for common PCB manufacturers.
//! Use the [`Preset`] enum for name-based lookup or call the constructor methods
//! directly on [`DesignRules`].
//!
//! # Supported Manufacturers
//!
//! - **JLCPCB**: Popular Chinese manufacturer with competitive pricing
//!   - 2-layer: Standard hobbyist option with 6mil minimum
//!   - 4-layer: Tighter tolerances available (4mil)
//!
//! - **PCBWay**: Alternative Chinese manufacturer with similar capabilities
//!
//! - **Prototype**: Relaxed rules for prototyping and hand assembly
//!
//! # Examples
//!
//! ```
//! use cypcb_drc::presets::{DesignRules, Preset};
//!
//! // Use preset enum for dynamic lookup
//! let preset = Preset::from_name("jlcpcb").unwrap();
//! let rules = preset.rules();
//!
//! // Or use constructor directly
//! let rules = DesignRules::jlcpcb_2layer();
//! ```

mod jlcpcb;
mod pcbway;

use cypcb_core::Nm;

/// Complete set of design rules for a board.
///
/// Contains minimum values for various design parameters that the DRC engine
/// validates against. Use the factory methods for manufacturer presets, or
/// create custom rules by constructing directly.
///
/// # Fields
///
/// - `min_clearance`: Minimum distance between copper features on the same layer
/// - `min_trace_width`: Minimum width of copper traces
/// - `min_drill_size`: Minimum mechanical drill hole diameter
/// - `min_via_drill`: Minimum via drill hole diameter
/// - `min_annular_ring`: Minimum width of copper ring around drill holes
/// - `min_silk_width`: Minimum silkscreen line width
/// - `min_edge_clearance`: Minimum distance from copper to board edge
///
/// # Examples
///
/// ```
/// use cypcb_drc::presets::DesignRules;
/// use cypcb_core::Nm;
///
/// // Use a manufacturer preset
/// let jlcpcb = DesignRules::jlcpcb_2layer();
/// assert_eq!(jlcpcb.min_clearance, Nm::from_mm(0.15));
///
/// // Or create custom rules
/// let custom = DesignRules {
///     min_clearance: Nm::from_mm(0.2),
///     min_trace_width: Nm::from_mm(0.25),
///     min_drill_size: Nm::from_mm(0.4),
///     min_via_drill: Nm::from_mm(0.3),
///     min_annular_ring: Nm::from_mm(0.2),
///     min_silk_width: Nm::from_mm(0.2),
///     min_edge_clearance: Nm::from_mm(0.5),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignRules {
    /// Minimum clearance between copper features.
    pub min_clearance: Nm,
    /// Minimum trace width.
    pub min_trace_width: Nm,
    /// Minimum drill size (mechanical drilling).
    pub min_drill_size: Nm,
    /// Minimum via drill size.
    pub min_via_drill: Nm,
    /// Minimum annular ring width.
    pub min_annular_ring: Nm,
    /// Minimum silkscreen line width.
    pub min_silk_width: Nm,
    /// Minimum copper to board edge clearance.
    pub min_edge_clearance: Nm,
}

impl Default for DesignRules {
    /// Default rules use JLCPCB 2-layer values.
    ///
    /// JLCPCB is chosen as default because it's the most commonly used
    /// manufacturer for hobbyist PCB fabrication.
    fn default() -> Self {
        Self::jlcpcb_2layer()
    }
}

/// Manufacturer preset identifiers.
///
/// Use [`from_name`](Preset::from_name) for string-based lookup (useful for
/// DSL parsing) or match directly on the enum variants.
///
/// # Examples
///
/// ```
/// use cypcb_drc::presets::Preset;
///
/// // From string (e.g., parsed from DSL)
/// let preset = Preset::from_name("jlcpcb").unwrap();
/// assert_eq!(preset, Preset::Jlcpcb2Layer);
///
/// // Get rules from preset
/// let rules = preset.rules();
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Preset {
    /// JLCPCB standard 2-layer board.
    Jlcpcb2Layer,
    /// JLCPCB 4-layer board with tighter tolerances.
    Jlcpcb4Layer,
    /// PCBWay standard capabilities.
    PcbwayStandard,
    /// Relaxed rules for prototyping.
    Prototype,
}

impl Preset {
    /// Get the design rules for this preset.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_drc::presets::Preset;
    /// use cypcb_core::Nm;
    ///
    /// let rules = Preset::Jlcpcb2Layer.rules();
    /// assert_eq!(rules.min_clearance, Nm::from_mm(0.15));
    /// ```
    pub fn rules(self) -> DesignRules {
        match self {
            Preset::Jlcpcb2Layer => DesignRules::jlcpcb_2layer(),
            Preset::Jlcpcb4Layer => DesignRules::jlcpcb_4layer(),
            Preset::PcbwayStandard => DesignRules::pcbway_standard(),
            Preset::Prototype => DesignRules::prototype(),
        }
    }

    /// Parse a preset from a string name.
    ///
    /// Accepts various aliases for convenience:
    /// - `"jlcpcb"` or `"jlcpcb_2layer"` -> [`Jlcpcb2Layer`](Preset::Jlcpcb2Layer)
    /// - `"jlcpcb_4layer"` -> [`Jlcpcb4Layer`](Preset::Jlcpcb4Layer)
    /// - `"pcbway"` or `"pcbway_standard"` -> [`PcbwayStandard`](Preset::PcbwayStandard)
    /// - `"prototype"` -> [`Prototype`](Preset::Prototype)
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_drc::presets::Preset;
    ///
    /// assert_eq!(Preset::from_name("jlcpcb"), Some(Preset::Jlcpcb2Layer));
    /// assert_eq!(Preset::from_name("jlcpcb_2layer"), Some(Preset::Jlcpcb2Layer));
    /// assert_eq!(Preset::from_name("pcbway"), Some(Preset::PcbwayStandard));
    /// assert_eq!(Preset::from_name("unknown"), None);
    /// ```
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "jlcpcb" | "jlcpcb_2layer" => Some(Preset::Jlcpcb2Layer),
            "jlcpcb_4layer" => Some(Preset::Jlcpcb4Layer),
            "pcbway" | "pcbway_standard" => Some(Preset::PcbwayStandard),
            "prototype" => Some(Preset::Prototype),
            _ => None,
        }
    }

    /// Get the canonical name for this preset.
    ///
    /// Returns the primary string identifier used in DSL files.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_drc::presets::Preset;
    ///
    /// assert_eq!(Preset::Jlcpcb2Layer.name(), "jlcpcb_2layer");
    /// assert_eq!(Preset::PcbwayStandard.name(), "pcbway_standard");
    /// ```
    pub fn name(self) -> &'static str {
        match self {
            Preset::Jlcpcb2Layer => "jlcpcb_2layer",
            Preset::Jlcpcb4Layer => "jlcpcb_4layer",
            Preset::PcbwayStandard => "pcbway_standard",
            Preset::Prototype => "prototype",
        }
    }

    /// Get all available presets.
    ///
    /// Useful for generating documentation or CLI help text.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_drc::presets::Preset;
    ///
    /// let presets = Preset::all();
    /// assert_eq!(presets.len(), 4);
    /// ```
    pub fn all() -> &'static [Preset] {
        &[
            Preset::Jlcpcb2Layer,
            Preset::Jlcpcb4Layer,
            Preset::PcbwayStandard,
            Preset::Prototype,
        ]
    }
}

impl std::fmt::Display for Preset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jlcpcb_2layer_values() {
        let rules = DesignRules::jlcpcb_2layer();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.15));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.15));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.3));
        assert_eq!(rules.min_via_drill, Nm::from_mm(0.2));
        assert_eq!(rules.min_annular_ring, Nm::from_mm(0.15));
        assert_eq!(rules.min_silk_width, Nm::from_mm(0.15));
        assert_eq!(rules.min_edge_clearance, Nm::from_mm(0.3));
    }

    #[test]
    fn test_jlcpcb_4layer_values() {
        let rules = DesignRules::jlcpcb_4layer();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.1));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.1));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.2));
        assert_eq!(rules.min_annular_ring, Nm::from_mm(0.125));
    }

    #[test]
    fn test_pcbway_standard_values() {
        let rules = DesignRules::pcbway_standard();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.15));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.2));
        assert_eq!(rules.min_silk_width, Nm::from_mm(0.22));
    }

    #[test]
    fn test_prototype_values() {
        let rules = DesignRules::prototype();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.2));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.25));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.4));
    }

    #[test]
    fn test_preset_from_name() {
        assert_eq!(Preset::from_name("jlcpcb"), Some(Preset::Jlcpcb2Layer));
        assert_eq!(Preset::from_name("jlcpcb_2layer"), Some(Preset::Jlcpcb2Layer));
        assert_eq!(Preset::from_name("jlcpcb_4layer"), Some(Preset::Jlcpcb4Layer));
        assert_eq!(Preset::from_name("pcbway"), Some(Preset::PcbwayStandard));
        assert_eq!(Preset::from_name("pcbway_standard"), Some(Preset::PcbwayStandard));
        assert_eq!(Preset::from_name("prototype"), Some(Preset::Prototype));
        assert_eq!(Preset::from_name("unknown"), None);
    }

    #[test]
    fn test_preset_rules_accessor() {
        let rules = Preset::Jlcpcb2Layer.rules();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.15));

        let rules = Preset::PcbwayStandard.rules();
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.2));
    }

    #[test]
    fn test_default_is_jlcpcb() {
        let default = DesignRules::default();
        let jlcpcb = DesignRules::jlcpcb_2layer();
        assert_eq!(default, jlcpcb);
    }

    #[test]
    fn test_prototype_has_larger_margins() {
        let proto = DesignRules::prototype();
        let jlcpcb = DesignRules::jlcpcb_2layer();
        assert!(proto.min_clearance > jlcpcb.min_clearance);
        assert!(proto.min_trace_width > jlcpcb.min_trace_width);
        assert!(proto.min_drill_size > jlcpcb.min_drill_size);
    }

    #[test]
    fn test_preset_name() {
        assert_eq!(Preset::Jlcpcb2Layer.name(), "jlcpcb_2layer");
        assert_eq!(Preset::Jlcpcb4Layer.name(), "jlcpcb_4layer");
        assert_eq!(Preset::PcbwayStandard.name(), "pcbway_standard");
        assert_eq!(Preset::Prototype.name(), "prototype");
    }

    #[test]
    fn test_preset_display() {
        assert_eq!(format!("{}", Preset::Jlcpcb2Layer), "jlcpcb_2layer");
        assert_eq!(format!("{}", Preset::PcbwayStandard), "pcbway_standard");
    }

    #[test]
    fn test_preset_all() {
        let all = Preset::all();
        assert_eq!(all.len(), 4);
        assert!(all.contains(&Preset::Jlcpcb2Layer));
        assert!(all.contains(&Preset::Jlcpcb4Layer));
        assert!(all.contains(&Preset::PcbwayStandard));
        assert!(all.contains(&Preset::Prototype));
    }

    #[test]
    fn test_preset_roundtrip() {
        // Verify name() -> from_name() roundtrips
        for preset in Preset::all() {
            let name = preset.name();
            let parsed = Preset::from_name(name).unwrap();
            assert_eq!(*preset, parsed);
        }
    }

    #[test]
    fn test_4layer_tighter_than_2layer() {
        let two = DesignRules::jlcpcb_2layer();
        let four = DesignRules::jlcpcb_4layer();
        assert!(four.min_clearance < two.min_clearance);
        assert!(four.min_trace_width < two.min_trace_width);
        assert!(four.min_drill_size < two.min_drill_size);
    }
}
