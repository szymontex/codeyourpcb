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
//!   - Advanced 2-layer: 3.5mil process, micro-drill
//!   - Advanced 4-layer: Tightest tolerances, controlled impedance
//!
//! - **OSHPark**: US-based service with purple boards and ENIG finish
//!   - 2-layer: 6mil trace/space, 10mil minimum drill
//!   - 4-layer: 5mil trace/space, controlled impedance available
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
mod oshpark;
mod pcbway;

use cypcb_core::Nm;
use cypcb_rules::DesignConstraints;

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
/// - `min_via_diameter`: Minimum via outer diameter (annulus)
/// - `min_annular_ring`: Minimum width of copper ring around drill holes
/// - `min_silk_width`: Minimum silkscreen line width
/// - `min_edge_clearance`: Minimum distance from copper to board edge
/// - `min_hole_to_hole`: Minimum distance between drill holes (edge-to-edge)
/// - `min_solder_mask_bridge`: Minimum solder mask web between pads
/// - `min_silk_clearance`: Minimum silkscreen to copper clearance
/// - `min_courtyard_clearance`: Minimum courtyard clearance between components
///
/// # Examples
///
/// ```
/// use cypcb_drc::presets::DesignRules;
/// use cypcb_core::Nm;
///
/// // Use a manufacturer preset
/// let jlcpcb = DesignRules::jlcpcb_2layer();
/// assert_eq!(jlcpcb.min_clearance, Nm::from_mm(0.127));
///
/// // Or create custom rules
/// let custom = DesignRules {
///     min_clearance: Nm::from_mm(0.2),
///     min_trace_width: Nm::from_mm(0.25),
///     min_drill_size: Nm::from_mm(0.4),
///     min_via_drill: Nm::from_mm(0.3),
///     min_via_diameter: Nm::from_mm(0.6),
///     min_annular_ring: Nm::from_mm(0.2),
///     min_silk_width: Nm::from_mm(0.2),
///     min_edge_clearance: Nm::from_mm(0.5),
///     min_hole_to_hole: Nm::from_mm(0.5),
///     min_solder_mask_bridge: Nm::from_mm(0.1),
///     solder_mask_expansion: Nm::from_mm(0.05),
///     min_silk_clearance: Nm::from_mm(0.15),
///     min_courtyard_clearance: Nm::from_mm(0.25),
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
    /// Minimum via outer diameter (copper annulus).
    pub min_via_diameter: Nm,
    /// Minimum annular ring width.
    pub min_annular_ring: Nm,
    /// Minimum silkscreen line width.
    pub min_silk_width: Nm,
    /// Minimum copper to board edge clearance.
    pub min_edge_clearance: Nm,
    /// Minimum distance between drill holes (edge-to-edge).
    pub min_hole_to_hole: Nm,
    /// Minimum solder mask bridge between pads.
    pub min_solder_mask_bridge: Nm,
    /// How far the mask opening extends beyond the pad on every side.
    pub solder_mask_expansion: Nm,
    /// Minimum silkscreen to copper pad clearance.
    pub min_silk_clearance: Nm,
    /// Minimum courtyard clearance between components.
    pub min_courtyard_clearance: Nm,
}

impl DesignRules {
    /// Build DRC rules from a manufacturer's routing constraints.
    ///
    /// `cypcb-rules` is the single source of fabrication numbers: the autorouter
    /// routes against it, so the checker has to grade against the same table.
    /// They used to be two hand-maintained copies, and they disagreed - the
    /// JLCPCB router preset allowed 0.127mm clearance while the JLCPCB DRC
    /// preset demanded 0.15mm, so a correctly routed board failed its own check.
    ///
    /// Three assembly-side rules have no counterpart in the routing constraints
    /// and are derived here: the via diameter comes from the drill plus two
    /// annular rings, silk clearance follows the silk width, and courtyard
    /// clearance takes a conservative IPC-style value.
    ///
    /// Hole-to-hole used to be a fourth, pinned at 0.5mm for every fab while
    /// the constraints carried the real number - so the checker allowed 0.5mm
    /// between holes on OSHPark, which requires **0.635mm**, and a board that
    /// passed `cypcb check --preset oshpark` could come back from the fab
    /// refused. It demanded 0.5mm on JLCPCB's advanced process, which allows
    /// 0.4mm, and failed boards that were fine. The value comes from the fab
    /// now.
    pub fn from_constraints(c: &DesignConstraints) -> Self {
        DesignRules {
            min_clearance: c.min_clearance,
            min_trace_width: c.min_trace_width,
            min_drill_size: c.min_drill_size,
            min_via_drill: c.min_via_drill,
            min_via_diameter: Nm(c.min_via_drill.0 + 2 * c.min_via_annular_ring.0),
            min_annular_ring: c.min_annular_ring,
            min_silk_width: c.min_silk_width,
            min_edge_clearance: c.min_edge_clearance,
            min_hole_to_hole: c.min_hole_to_hole,
            min_solder_mask_bridge: c.min_solder_mask_bridge,
            solder_mask_expansion: c.solder_mask_expansion,
            min_silk_clearance: c.min_silk_width,
            min_courtyard_clearance: Nm::from_mm(0.25),
        }
    }
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
    /// JLCPCB advanced 2-layer (tighter tolerances, higher cost).
    JlcpcbAdvanced2Layer,
    /// JLCPCB advanced 4-layer (tightest tolerances).
    JlcpcbAdvanced4Layer,
    /// OSHPark 2-layer (US-based, purple boards, ENIG).
    OshPark2Layer,
    /// OSHPark 4-layer (controlled impedance available).
    OshPark4Layer,
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
    /// assert_eq!(rules.min_clearance, Nm::from_mm(0.127));
    /// ```
    pub fn rules(self) -> DesignRules {
        match self {
            Preset::Jlcpcb2Layer => DesignRules::jlcpcb_2layer(),
            Preset::Jlcpcb4Layer => DesignRules::jlcpcb_4layer(),
            Preset::JlcpcbAdvanced2Layer => DesignRules::jlcpcb_advanced_2layer(),
            Preset::JlcpcbAdvanced4Layer => DesignRules::jlcpcb_advanced_4layer(),
            Preset::OshPark2Layer => DesignRules::oshpark_2layer(),
            Preset::OshPark4Layer => DesignRules::oshpark_4layer(),
            Preset::PcbwayStandard => DesignRules::pcbway_standard(),
            Preset::Prototype => DesignRules::prototype(),
        }
    }

    /// Parse a preset from a string name.
    ///
    /// Accepts various aliases for convenience. Names are normalized:
    /// lowercase, hyphens converted to underscores.
    ///
    /// | Input | Preset |
    /// |-------|--------|
    /// | `"jlcpcb"`, `"jlcpcb_2layer"` | `Jlcpcb2Layer` |
    /// | `"jlcpcb_4layer"` | `Jlcpcb4Layer` |
    /// | `"jlcpcb_advanced"`, `"jlcpcb_advanced_2layer"` | `JlcpcbAdvanced2Layer` |
    /// | `"jlcpcb_advanced_4layer"` | `JlcpcbAdvanced4Layer` |
    /// | `"oshpark"`, `"oshpark_2layer"` | `OshPark2Layer` |
    /// | `"oshpark_4layer"` | `OshPark4Layer` |
    /// | `"pcbway"`, `"pcbway_standard"` | `PcbwayStandard` |
    /// | `"prototype"` | `Prototype` |
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_drc::presets::Preset;
    ///
    /// assert_eq!(Preset::from_name("jlcpcb"), Some(Preset::Jlcpcb2Layer));
    /// assert_eq!(Preset::from_name("oshpark"), Some(Preset::OshPark2Layer));
    /// assert_eq!(Preset::from_name("jlcpcb_advanced"), Some(Preset::JlcpcbAdvanced2Layer));
    /// assert_eq!(Preset::from_name("unknown"), None);
    /// ```
    pub fn from_name(name: &str) -> Option<Self> {
        // Normalize: lowercase, hyphens to underscores
        let normalized = name.to_lowercase().replace('-', "_");
        match normalized.as_str() {
            "jlcpcb" | "jlcpcb_2layer" => Some(Preset::Jlcpcb2Layer),
            "jlcpcb_4layer" => Some(Preset::Jlcpcb4Layer),
            "jlcpcb_advanced" | "jlcpcb_advanced_2layer" => Some(Preset::JlcpcbAdvanced2Layer),
            "jlcpcb_advanced_4layer" => Some(Preset::JlcpcbAdvanced4Layer),
            "oshpark" | "oshpark_2layer" => Some(Preset::OshPark2Layer),
            "oshpark_4layer" => Some(Preset::OshPark4Layer),
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
            Preset::JlcpcbAdvanced2Layer => "jlcpcb_advanced_2layer",
            Preset::JlcpcbAdvanced4Layer => "jlcpcb_advanced_4layer",
            Preset::OshPark2Layer => "oshpark_2layer",
            Preset::OshPark4Layer => "oshpark_4layer",
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
    /// assert_eq!(presets.len(), 8);
    /// ```
    pub fn all() -> &'static [Preset] {
        &[
            Preset::Jlcpcb2Layer,
            Preset::Jlcpcb4Layer,
            Preset::JlcpcbAdvanced2Layer,
            Preset::JlcpcbAdvanced4Layer,
            Preset::OshPark2Layer,
            Preset::OshPark4Layer,
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
        assert_eq!(rules.min_clearance, Nm::from_mm(0.127));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.127));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.3));
        assert_eq!(rules.min_via_drill, Nm::from_mm(0.3));
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
        assert_eq!(
            Preset::from_name("jlcpcb_2layer"),
            Some(Preset::Jlcpcb2Layer)
        );
        assert_eq!(
            Preset::from_name("jlcpcb_4layer"),
            Some(Preset::Jlcpcb4Layer)
        );
        assert_eq!(
            Preset::from_name("jlcpcb_advanced"),
            Some(Preset::JlcpcbAdvanced2Layer)
        );
        assert_eq!(
            Preset::from_name("jlcpcb_advanced_2layer"),
            Some(Preset::JlcpcbAdvanced2Layer)
        );
        assert_eq!(
            Preset::from_name("jlcpcb_advanced_4layer"),
            Some(Preset::JlcpcbAdvanced4Layer)
        );
        assert_eq!(Preset::from_name("oshpark"), Some(Preset::OshPark2Layer));
        assert_eq!(
            Preset::from_name("oshpark_2layer"),
            Some(Preset::OshPark2Layer)
        );
        assert_eq!(
            Preset::from_name("oshpark_4layer"),
            Some(Preset::OshPark4Layer)
        );
        assert_eq!(Preset::from_name("pcbway"), Some(Preset::PcbwayStandard));
        assert_eq!(
            Preset::from_name("pcbway_standard"),
            Some(Preset::PcbwayStandard)
        );
        assert_eq!(Preset::from_name("prototype"), Some(Preset::Prototype));
        assert_eq!(Preset::from_name("unknown"), None);
    }

    #[test]
    fn test_preset_from_name_case_insensitive() {
        assert_eq!(Preset::from_name("JLCPCB"), Some(Preset::Jlcpcb2Layer));
        assert_eq!(Preset::from_name("OshPark"), Some(Preset::OshPark2Layer));
        assert_eq!(
            Preset::from_name("OSHPARK_4LAYER"),
            Some(Preset::OshPark4Layer)
        );
    }

    #[test]
    fn test_preset_from_name_hyphen_alias() {
        assert_eq!(
            Preset::from_name("oshpark-2layer"),
            Some(Preset::OshPark2Layer)
        );
        assert_eq!(
            Preset::from_name("jlcpcb-advanced"),
            Some(Preset::JlcpcbAdvanced2Layer)
        );
        assert_eq!(
            Preset::from_name("jlcpcb-advanced-4layer"),
            Some(Preset::JlcpcbAdvanced4Layer)
        );
    }

    #[test]
    fn drc_presets_do_not_diverge_from_routing_constraints() {
        use cypcb_rules::presets::RulesPreset;

        let pairs = [
            (Preset::Jlcpcb2Layer, RulesPreset::JlcpcbStandard2Layer),
            (Preset::Jlcpcb4Layer, RulesPreset::JlcpcbStandard4Layer),
            (
                Preset::JlcpcbAdvanced2Layer,
                RulesPreset::JlcpcbAdvanced2Layer,
            ),
            (
                Preset::JlcpcbAdvanced4Layer,
                RulesPreset::JlcpcbAdvanced4Layer,
            ),
            (Preset::OshPark2Layer, RulesPreset::OshPark2Layer),
            (Preset::OshPark4Layer, RulesPreset::OshPark4Layer),
            (Preset::PcbwayStandard, RulesPreset::PcbWayStandard),
        ];

        for (drc_preset, routing_preset) in pairs {
            let rules = drc_preset.rules();
            let constraints = routing_preset.constraints();
            assert_eq!(
                rules.min_clearance,
                constraints.min_clearance,
                "{} clearance must match what the router routes against",
                drc_preset.name()
            );
            assert_eq!(rules.min_trace_width, constraints.min_trace_width);
            assert_eq!(rules.min_drill_size, constraints.min_drill_size);
            assert_eq!(rules.min_via_drill, constraints.min_via_drill);
            assert_eq!(rules.min_annular_ring, constraints.min_annular_ring);
            assert_eq!(rules.min_edge_clearance, constraints.min_edge_clearance);
        }
    }

    #[test]
    fn test_preset_rules_accessor() {
        let rules = Preset::Jlcpcb2Layer.rules();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.127));

        let rules = Preset::PcbwayStandard.rules();
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.2));
        assert_eq!(rules.min_silk_width, Nm::from_mm(0.22));
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
        assert_eq!(
            Preset::JlcpcbAdvanced2Layer.name(),
            "jlcpcb_advanced_2layer"
        );
        assert_eq!(
            Preset::JlcpcbAdvanced4Layer.name(),
            "jlcpcb_advanced_4layer"
        );
        assert_eq!(Preset::OshPark2Layer.name(), "oshpark_2layer");
        assert_eq!(Preset::OshPark4Layer.name(), "oshpark_4layer");
        assert_eq!(Preset::PcbwayStandard.name(), "pcbway_standard");
        assert_eq!(Preset::Prototype.name(), "prototype");
    }

    #[test]
    fn test_preset_display() {
        assert_eq!(format!("{}", Preset::Jlcpcb2Layer), "jlcpcb_2layer");
        assert_eq!(format!("{}", Preset::PcbwayStandard), "pcbway_standard");
        assert_eq!(format!("{}", Preset::OshPark2Layer), "oshpark_2layer");
    }

    #[test]
    fn test_preset_all() {
        let all = Preset::all();
        assert_eq!(all.len(), 8);
        assert!(all.contains(&Preset::Jlcpcb2Layer));
        assert!(all.contains(&Preset::Jlcpcb4Layer));
        assert!(all.contains(&Preset::JlcpcbAdvanced2Layer));
        assert!(all.contains(&Preset::JlcpcbAdvanced4Layer));
        assert!(all.contains(&Preset::OshPark2Layer));
        assert!(all.contains(&Preset::OshPark4Layer));
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
