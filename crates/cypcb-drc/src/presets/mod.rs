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
//! use cypcb_drc::presets::{DesignRules, Preset, PresetRules};
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
/// - `max_diff_pair_skew`: How far apart the halves of a differential pair may end up
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
///     max_diff_pair_skew: Nm::from_mm(0.5),
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
    /// How far apart the two halves of a differential pair may end up.
    ///
    /// The fab's number, from the same table the router is priced with. A pair
    /// is two nets carrying one signal, and the receiver reads the difference
    /// between them: copper one half runs and the other does not is skew, and
    /// past this it is a signal problem the board cannot be talked out of.
    pub max_diff_pair_skew: Nm,
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
    /// Three assembly-side rules are the fab's when it states one and derived
    /// when it does not: the via diameter comes from the drill plus two
    /// annular rings, silk clearance follows the silk width, and courtyard
    /// clearance takes a conservative IPC-style value. `prototype` states all
    /// three, because a via pad big enough to solder by hand is the whole
    /// point of that preset.
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
            min_via_diameter: c
                .min_via_diameter
                .unwrap_or(Nm(c.min_via_drill.0 + 2 * c.min_via_annular_ring.0)),
            min_annular_ring: c.min_annular_ring,
            min_silk_width: c.min_silk_width,
            min_edge_clearance: c.min_edge_clearance,
            min_hole_to_hole: c.min_hole_to_hole,
            min_solder_mask_bridge: c.min_solder_mask_bridge,
            solder_mask_expansion: c.solder_mask_expansion,
            min_silk_clearance: c.min_silk_clearance.unwrap_or(c.min_silk_width),
            min_courtyard_clearance: c.min_courtyard_clearance.unwrap_or(Nm::from_mm(0.25)),
            max_diff_pair_skew: c.length_match_tolerance,
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

/// Manufacturer presets, and the design rules each one implies.
///
/// There used to be two of these. `cypcb_drc::Preset` listed eight fabs and
/// `cypcb_rules::RulesPreset` ten, they spelled the same fab differently
/// (`jlcpcb_2layer` against `jlcpcb_standard_2layer`), and each was missing
/// what the other had: the checker had `prototype`, the router had IPC classes
/// `ipc1`, `ipc2` and `ipc3` that every CLI command refused. The numbers were
/// already shared - the checker's presets read `RulesPreset::constraints()` -
/// so what the second enum bought was a way for the two to drift.
///
/// One list now. Every name either table accepted still resolves.
pub use cypcb_rules::presets::RulesPreset as Preset;

/// The design rules a preset implies.
///
/// An extension trait rather than a method, because the presets live in
/// `cypcb-rules`, which knows how to describe a fab and nothing about
/// checking a board.
pub trait PresetRules {
    /// The rules the checker enforces for this preset.
    fn rules(self) -> DesignRules;
}

impl PresetRules for Preset {
    fn rules(self) -> DesignRules {
        DesignRules::from_constraints(&self.constraints())
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
        assert_eq!(
            Preset::from_name("jlcpcb"),
            Some(Preset::JlcpcbStandard2Layer)
        );
        assert_eq!(
            Preset::from_name("jlcpcb_2layer"),
            Some(Preset::JlcpcbStandard2Layer)
        );
        assert_eq!(
            Preset::from_name("jlcpcb_4layer"),
            Some(Preset::JlcpcbStandard4Layer)
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
        assert_eq!(Preset::from_name("pcbway"), Some(Preset::PcbWayStandard));
        assert_eq!(
            Preset::from_name("pcbway_standard"),
            Some(Preset::PcbWayStandard)
        );
        assert_eq!(Preset::from_name("prototype"), Some(Preset::Prototype));
        assert_eq!(Preset::from_name("unknown"), None);
    }

    #[test]
    fn test_preset_from_name_case_insensitive() {
        assert_eq!(
            Preset::from_name("JLCPCB"),
            Some(Preset::JlcpcbStandard2Layer)
        );
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
            (
                Preset::JlcpcbStandard2Layer,
                RulesPreset::JlcpcbStandard2Layer,
            ),
            (
                Preset::JlcpcbStandard4Layer,
                RulesPreset::JlcpcbStandard4Layer,
            ),
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
            (Preset::PcbWayStandard, RulesPreset::PcbWayStandard),
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
        let rules = Preset::JlcpcbStandard2Layer.rules();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.127));

        let rules = Preset::PcbWayStandard.rules();
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
        // The canonical name is the rules crate's, which spells the process
        // out; `jlcpcb_2layer` is still accepted as an alias, and the test
        // below is what proves it.
        assert_eq!(
            Preset::JlcpcbStandard2Layer.name(),
            "jlcpcb_standard_2layer"
        );
        assert_eq!(
            Preset::JlcpcbStandard4Layer.name(),
            "jlcpcb_standard_4layer"
        );
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
        assert_eq!(Preset::PcbWayStandard.name(), "pcbway_standard");
        assert_eq!(Preset::Prototype.name(), "prototype");
    }

    #[test]
    fn test_preset_display() {
        assert_eq!(
            format!("{}", Preset::JlcpcbStandard2Layer),
            "jlcpcb_standard_2layer"
        );
        assert_eq!(format!("{}", Preset::PcbWayStandard), "pcbway_standard");
        assert_eq!(format!("{}", Preset::OshPark2Layer), "oshpark_2layer");
    }

    #[test]
    fn test_preset_all() {
        let all = Preset::all();
        assert_eq!(all.len(), Preset::ALL.len());
        assert!(all.contains(&Preset::JlcpcbStandard2Layer));
        assert!(all.contains(&Preset::JlcpcbStandard4Layer));
        assert!(all.contains(&Preset::JlcpcbAdvanced2Layer));
        assert!(all.contains(&Preset::JlcpcbAdvanced4Layer));
        assert!(all.contains(&Preset::OshPark2Layer));
        assert!(all.contains(&Preset::OshPark4Layer));
        assert!(all.contains(&Preset::PcbWayStandard));
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
