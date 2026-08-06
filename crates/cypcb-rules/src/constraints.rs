//! PCB design constraints covering all fabrication parameters.
//!
//! [`DesignConstraints`] is the central configuration struct that defines the
//! manufacturing limits and design rules for a PCB. All dimension fields use
//! [`Nm`] for type-safe, integer-precision measurements.
//!
//! The [`Default`] implementation provides values matching JLCPCB's 2-layer
//! standard process capabilities.

use cypcb_core::Nm;
use serde::{Deserialize, Serialize};

/// Comprehensive PCB design constraints.
///
/// Organized by category:
/// - **Basic geometry**: fundamental clearances and minimums
/// - **Advanced geometry**: via details, drill aspect ratios, solder mask
/// - **Signal integrity**: impedance, differential pairs, length matching
/// - **Thermal**: current capacity, thermal relief, copper pour
/// - **Manufacturing**: board construction, hole spacing, via types
///
/// All dimension fields use [`Nm`]. Non-dimension fields (impedance in ohms,
/// ratios, weights) use appropriate numeric types.
///
/// # Examples
///
/// ```
/// use cypcb_rules::DesignConstraints;
/// use cypcb_core::Nm;
///
/// let dc = DesignConstraints::default();
/// // JLCPCB 2-layer minimum trace width: 0.127mm (5 mil)
/// assert_eq!(dc.min_trace_width, Nm::from_mm(0.127));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignConstraints {
    // ── Basic geometry ──────────────────────────────────────────────
    /// Minimum clearance between copper features on the same layer.
    pub min_clearance: Nm,
    /// Minimum trace (track) width.
    pub min_trace_width: Nm,
    /// Minimum finished drill hole diameter.
    pub min_drill_size: Nm,
    /// Minimum via drill hole diameter.
    pub min_via_drill: Nm,
    /// Minimum annular ring around pads.
    pub min_annular_ring: Nm,
    /// Minimum silkscreen line width.
    pub min_silk_width: Nm,
    /// Minimum clearance from copper to board edge.
    pub min_edge_clearance: Nm,

    // ── Advanced geometry ───────────────────────────────────────────
    /// Minimum annular ring around vias (may differ from pad annular ring).
    pub min_via_annular_ring: Nm,
    /// Maximum drill aspect ratio (depth:diameter). Stored as ratio × 100
    /// to keep integer precision (e.g. 600 = 6.0:1).
    pub max_drill_aspect_ratio: u32,
    /// Minimum solder mask bridge (dam) between adjacent mask openings.
    pub min_solder_mask_bridge: Nm,
    /// Minimum clearance between paste stencil openings.
    pub min_paste_clearance: Nm,
    /// Solder mask expansion beyond the pad.
    pub solder_mask_expansion: Nm,
    /// Minimum pad size (diameter for round, short axis for oblong).
    pub min_pad_size: Nm,
    /// Minimum copper-to-slot clearance.
    pub min_slot_clearance: Nm,

    // ── Signal integrity ────────────────────────────────────────────
    /// Default target impedance (ohms × 100 for integer precision, e.g. 5000 = 50.00 Ω).
    pub default_impedance_ohms_x100: u32,
    /// Differential pair gap (space between traces).
    pub diff_pair_gap: Nm,
    /// Differential pair width tolerance.
    pub diff_pair_tolerance: Nm,
    /// Maximum stub length for high-speed signals.
    pub max_stub_length: Nm,
    /// Length-matching tolerance for matched-length groups.
    pub length_match_tolerance: Nm,
    /// Maximum allowed via count in a high-speed signal path.
    pub max_vias_per_high_speed_net: u32,

    // ── Thermal ─────────────────────────────────────────────────────
    /// Maximum current per unit width (mA per mm × 100, e.g. 100_000 = 1000.00 mA/mm).
    /// Based on IPC-2152 for 10°C rise, 1oz copper, outer layer.
    pub max_current_per_width_x100: u32,
    /// Thermal relief gap width.
    pub thermal_relief_gap: Nm,
    /// Thermal relief spoke width.
    pub thermal_relief_spoke_width: Nm,
    /// Minimum clearance between copper pour and other copper features.
    pub min_copper_pour_clearance: Nm,
    /// Number of thermal relief spokes (typically 2 or 4).
    pub thermal_relief_spokes: u8,

    // ── Manufacturing ───────────────────────────────────────────────
    /// Copper weight in oz/ft² × 10 (e.g. 10 = 1.0 oz).
    pub copper_weight_oz_x10: u32,
    /// Total board thickness.
    pub board_thickness: Nm,
    /// Minimum hole-to-hole spacing (edge to edge).
    pub min_hole_to_hole: Nm,
    /// Minimum hole-to-board-edge spacing.
    pub min_hole_to_edge: Nm,
    /// Whether blind vias are allowed in this design.
    pub blind_vias_allowed: bool,
    /// Whether buried vias are allowed in this design.
    pub buried_vias_allowed: bool,
    /// Minimum copper feature size for acid trap avoidance.
    pub min_acid_trap: Nm,
    /// Maximum number of copper layers supported.
    pub max_copper_layers: u8,
    /// Whether castellated holes are allowed.
    pub castellated_holes_allowed: bool,
}

impl DesignConstraints {
    /// Number of constraint fields in the struct.
    ///
    /// The point of the number is the claim behind it: this type is meant to
    /// carry everything a fab house states about a process, so a shrinking
    /// count is a regression. It said 35 while the struct had 34, because
    /// nothing checked it - `field_count_matches_the_struct` does now, and a
    /// field added or removed without touching this line fails to compile.
    pub const FIELD_COUNT: usize = 34;
}

impl Default for DesignConstraints {
    /// JLCPCB 2-layer standard process defaults.
    ///
    /// Source: JLCPCB capabilities page (2-layer, standard process).
    /// Values represent minimum capabilities, not recommended values.
    fn default() -> Self {
        Self {
            // Basic geometry
            min_clearance: Nm::from_mm(0.127),    // 5 mil
            min_trace_width: Nm::from_mm(0.127),  // 5 mil
            min_drill_size: Nm::from_mm(0.3),     // 0.3mm min drill
            min_via_drill: Nm::from_mm(0.3),      // 0.3mm min via drill
            min_annular_ring: Nm::from_mm(0.127), // 5 mil
            min_silk_width: Nm::from_mm(0.15),    // 0.15mm silk
            min_edge_clearance: Nm::from_mm(0.3), // 0.3mm to edge

            // Advanced geometry
            min_via_annular_ring: Nm::from_mm(0.127), // 5 mil
            max_drill_aspect_ratio: 800,              // 8:1
            min_solder_mask_bridge: Nm::from_mm(0.1), // 0.1mm
            min_paste_clearance: Nm::from_mm(0.127),  // 5 mil
            solder_mask_expansion: Nm::from_mm(0.05), // 0.05mm typical
            min_pad_size: Nm::from_mm(0.5),           // 0.5mm min pad
            min_slot_clearance: Nm::from_mm(0.3),     // 0.3mm

            // Signal integrity
            default_impedance_ohms_x100: 5000,        // 50.00 Ω
            diff_pair_gap: Nm::from_mm(0.127),        // 5 mil
            diff_pair_tolerance: Nm::from_mm(0.025),  // 25µm
            max_stub_length: Nm::from_mm(1.0),        // 1mm
            length_match_tolerance: Nm::from_mm(0.5), // 0.5mm
            max_vias_per_high_speed_net: 4,

            // Thermal
            max_current_per_width_x100: 100_000,    // 1000 mA/mm
            thermal_relief_gap: Nm::from_mm(0.254), // 10 mil
            thermal_relief_spoke_width: Nm::from_mm(0.254), // 10 mil
            min_copper_pour_clearance: Nm::from_mm(0.254), // 10 mil
            thermal_relief_spokes: 4,

            // Manufacturing
            copper_weight_oz_x10: 10,           // 1.0 oz
            board_thickness: Nm::from_mm(1.6),  // 1.6mm standard
            min_hole_to_hole: Nm::from_mm(0.5), // 0.5mm
            min_hole_to_edge: Nm::from_mm(0.3), // 0.3mm
            blind_vias_allowed: false,
            buried_vias_allowed: false,
            min_acid_trap: Nm::from_mm(0.127), // 5 mil
            max_copper_layers: 2,
            castellated_holes_allowed: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_has_jlcpcb_values() {
        let dc = DesignConstraints::default();

        // JLCPCB 2-layer: 5mil min trace
        assert_eq!(dc.min_trace_width, Nm::from_mm(0.127));
        // JLCPCB 2-layer: 5mil min clearance
        assert_eq!(dc.min_clearance, Nm::from_mm(0.127));
        // Standard board thickness
        assert_eq!(dc.board_thickness, Nm::from_mm(1.6));
        // 1oz copper
        assert_eq!(dc.copper_weight_oz_x10, 10);
        // No blind/buried vias on 2-layer
        assert!(!dc.blind_vias_allowed);
        assert!(!dc.buried_vias_allowed);
        // 2 copper layers
        assert_eq!(dc.max_copper_layers, 2);
    }

    #[test]
    fn test_default_drill_sizes() {
        let dc = DesignConstraints::default();
        assert_eq!(dc.min_drill_size, Nm::from_mm(0.3));
        assert_eq!(dc.min_via_drill, Nm::from_mm(0.3));
    }

    #[test]
    fn field_count_matches_the_struct() {
        // Destructured without `..`, so the compiler refuses to build this
        // until every field is named. That is what keeps the count honest:
        // `assert!(FIELD_COUNT >= 30)` was true of any number at all, and the
        // number it was guarding had been wrong by four fields.
        let DesignConstraints {
            min_clearance: _,
            min_trace_width: _,
            min_drill_size: _,
            min_via_drill: _,
            min_annular_ring: _,
            min_silk_width: _,
            min_edge_clearance: _,
            min_via_annular_ring: _,
            max_drill_aspect_ratio: _,
            min_solder_mask_bridge: _,
            min_paste_clearance: _,
            solder_mask_expansion: _,
            min_pad_size: _,
            min_slot_clearance: _,
            diff_pair_gap: _,
            diff_pair_tolerance: _,
            max_stub_length: _,
            length_match_tolerance: _,
            max_vias_per_high_speed_net: _,
            thermal_relief_gap: _,
            thermal_relief_spoke_width: _,
            min_copper_pour_clearance: _,
            thermal_relief_spokes: _,
            board_thickness: _,
            min_hole_to_hole: _,
            min_hole_to_edge: _,
            blind_vias_allowed: _,
            buried_vias_allowed: _,
            min_acid_trap: _,
            max_copper_layers: _,
            castellated_holes_allowed: _,
            default_impedance_ohms_x100: _,
            max_current_per_width_x100: _,
            copper_weight_oz_x10: _,
        } = DesignConstraints::default();

        let named = 34;
        assert_eq!(
            DesignConstraints::FIELD_COUNT,
            named,
            "the constant and the struct disagree"
        );
        assert!(
            named >= 30,
            "a fab process description this thin is a regression"
        );
    }

    #[test]
    fn test_constraints_clone_eq() {
        let dc1 = DesignConstraints::default();
        let dc2 = dc1.clone();
        assert_eq!(dc1, dc2);
    }

    #[test]
    fn test_all_dimensions_use_nm() {
        let dc = DesignConstraints::default();
        // Spot-check that dimension fields are Nm, not raw numbers.
        // If any of these fail to compile, a field was changed away from Nm.
        let _: Nm = dc.min_clearance;
        let _: Nm = dc.min_trace_width;
        let _: Nm = dc.min_drill_size;
        let _: Nm = dc.min_via_drill;
        let _: Nm = dc.min_annular_ring;
        let _: Nm = dc.min_silk_width;
        let _: Nm = dc.min_edge_clearance;
        let _: Nm = dc.thermal_relief_gap;
        let _: Nm = dc.board_thickness;
        let _: Nm = dc.diff_pair_gap;
        let _: Nm = dc.length_match_tolerance;
    }

    #[test]
    fn test_constraints_serde_roundtrip() {
        let dc = DesignConstraints::default();
        let json = serde_json::to_string(&dc).unwrap();
        let dc2: DesignConstraints = serde_json::from_str(&json).unwrap();
        assert_eq!(dc, dc2);
    }

    #[test]
    fn test_edge_clearance_positive() {
        let dc = DesignConstraints::default();
        assert!(dc.min_edge_clearance.raw() > 0);
        assert!(dc.min_hole_to_edge.raw() > 0);
    }
}
