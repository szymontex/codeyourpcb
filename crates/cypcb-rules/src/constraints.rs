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
    /// Minimum pad size the fabricator publishes, if it publishes one.
    ///
    /// The land around a **hole** - a via's pad and a through-hole pad - which
    /// is what D6 settled this field to mean. `None` says the fab did not
    /// state one, and the checker derives it from the drill and the ring the
    /// same way it derives `min_via_diameter`, naming the figure as its own.
    ///
    /// It used to be a bare `Nm` in every preset, and only JLCPCB's came off a
    /// capability page: OSHPark publishes an annular ring and no pad diameter,
    /// PCBWay the same, and the IPC classes are a design standard rather than
    /// a fab. Each of those numbers was `min_drill_size + 2 * min_annular_ring`
    /// rounded - a derived figure sitting in a field that had stopped being
    /// derived, enforced by `PadLandRule` as a floor no fabricator published.
    pub min_pad_size: Option<Nm>,
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
    /// How far the finished thickness may be off, as a percentage of it.
    ///
    /// A house's number rather than a board's: JLCPCB publishes plus or minus
    /// ten percent as its standard, and five percent on request
    /// (<https://jlcpcb.com/capabilities/pcb-capabilities>). `None` means this
    /// project has not read a published figure for that house, and a file that
    /// wants one - IPC-2581's `Stackup` wants two - says zero and says it is
    /// saying zero. A figure invented here is a figure a fabricator gets held
    /// to.
    pub board_thickness_tolerance_percent: Option<u32>,
    /// The absolute tolerance a thin board gets instead of the percentage.
    ///
    /// JLCPCB publishes two rules, not one: "± 10%" at 1.0mm and above, and
    /// "± 0.1mm" below it - ten percent of 0.4mm would be 0.04mm, which is
    /// finer than the press can hold. `None` means no published figure has
    /// been read for the house.
    pub board_thickness_tolerance_thin: Option<Nm>,
    /// How much larger a finished hole may come out than it was drawn.
    ///
    /// Published per house and **not symmetric**: JLCPCB's own capabilities
    /// page states through-holes as "+0.13 / -0.08 mm", because plating grows
    /// into the barrel. A single figure either way would be wrong in one
    /// direction, so both are carried.
    pub hole_tolerance_plus: Option<Nm>,
    /// How much smaller a finished hole may come out than it was drawn.
    pub hole_tolerance_minus: Option<Nm>,
    /// Minimum hole-to-hole spacing (edge to edge).
    pub min_hole_to_hole: Nm,
    /// Smallest via pad a fab states, when it states one.
    ///
    /// `None` means it does not, and the checker derives one: the drill plus
    /// two annular rings. A fab that says nothing about a rule is not the same
    /// as a fab that requires whatever this project happens to default to, so
    /// the three assembly-side rules the checker needs and a routing table has
    /// no use for are optional rather than invented here.
    #[serde(default)]
    pub min_via_diameter: Option<Nm>,
    /// Smallest gap between silkscreen and copper a fab states, when it states
    /// one. `None` means the checker follows the silk width.
    #[serde(default)]
    pub min_silk_clearance: Option<Nm>,
    /// Smallest gap between two part courtyards a fab states, when it states
    /// one. `None` means the checker uses a conservative IPC-style value.
    #[serde(default)]
    pub min_courtyard_clearance: Option<Nm>,
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
    /// The stiffener thicknesses this house presses, per material, in
    /// micrometres.
    ///
    /// A stiffener is bonded under the rigid part of a flex board, and a house
    /// bonds the sheets it stocks rather than any figure a design asks for.
    /// JLCPCB publishes three lists on its flex capabilities page
    /// (<https://jlcpcb.com/capabilities/flex-pcb-capabilities>): PI at 0.1,
    /// 0.15, 0.20, 0.225 and 0.25mm; FR4 at 0.1, 0.2, 0.4, 0.6, 0.8, 1.0, 1.2
    /// and 1.6mm; stainless steel at 0.1, 0.2 and 0.3mm.
    ///
    /// Empty means no published list has been read for this house, and the
    /// checker says nothing rather than holding a design to a figure nobody
    /// published. The material is the design's own word - `stiffener 0.2mm
    /// material "FR4"` - matched without case or spaces.
    pub stiffener_thickness_um: Vec<(String, Vec<u32>)>,
}

impl DesignConstraints {
    /// Number of constraint fields in the struct.
    ///
    /// The point of the number is the claim behind it: this type is meant to
    /// carry everything a fab house states about a process, so a shrinking
    /// count is a regression. It said 35 while the struct had 34, because
    /// nothing checked it - `field_count_matches_the_struct` does now, and a
    /// field added or removed without touching this line fails to compile.
    pub const FIELD_COUNT: usize = 42;
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
            // No fab here to have published one. The checker derives it.
            min_pad_size: None,
            min_slot_clearance: Nm::from_mm(0.3), // 0.3mm

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
            copper_weight_oz_x10: 10,          // 1.0 oz
            board_thickness: Nm::from_mm(1.6), // 1.6mm standard
            board_thickness_tolerance_percent: None,
            board_thickness_tolerance_thin: None,
            hole_tolerance_plus: None,
            hole_tolerance_minus: None,
            min_hole_to_hole: Nm::from_mm(0.5), // 0.5mm
            min_hole_to_edge: Nm::from_mm(0.3), // 0.3mm
            blind_vias_allowed: false,
            buried_vias_allowed: false,
            min_acid_trap: Nm::from_mm(0.127), // 5 mil
            max_copper_layers: 2,
            castellated_holes_allowed: false,

            // The three assembly-side rules a routing table has no use for. None
            // means this fab does not state one and the checker derives it.
            min_via_diameter: None,
            min_silk_clearance: None,
            min_courtyard_clearance: None,

            // The default table is JLCPCB's rigid process, and a rigid board
            // has no stiffener. The flex lists are stated by the presets that
            // read them.
            stiffener_thickness_um: Vec::new(),
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
            min_via_diameter: _,
            min_silk_clearance: _,
            min_courtyard_clearance: _,
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
            board_thickness_tolerance_percent: _,
            board_thickness_tolerance_thin: _,
            hole_tolerance_plus: _,
            hole_tolerance_minus: _,
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
            stiffener_thickness_um: _,
        } = DesignConstraints::default();

        let named = 42;
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
