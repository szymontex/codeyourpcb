//! PCBWay manufacturer preset.
//!
//! Source: <https://www.pcbway.com/capabilities.html>
//!
//! PCBWay standard process — general-purpose 2-layer fabrication.
//! 6mil trace/space recommended, 3mil achievable at extra cost.
//! Supports blind/buried vias (extra cost).
//!
//! Capabilities verified 2026-03-13.

use cypcb_core::Nm;

use crate::constraints::DesignConstraints;
use crate::stackup::{LayerStackEntry, Stackup};

/// PCBWay standard process.
///
/// Source: <https://www.pcbway.com/capabilities.html>
/// - Min trace/space: 6mil (0.15mm) standard, 3mil advanced
/// - Min drill: 0.2mm
/// - 1oz copper, 1.6mm FR-4
/// - Blind/buried vias available (extra cost)
/// - HASL/ENIG finish options
pub fn standard() -> DesignConstraints {
    DesignConstraints {
        // Basic geometry
        min_clearance: Nm::from_mm(0.15),     // 6 mil (recommended)
        min_trace_width: Nm::from_mm(0.15),   // 6 mil
        min_drill_size: Nm::from_mm(0.2),     // 0.2mm
        min_via_drill: Nm::from_mm(0.2),      // 0.2mm
        min_annular_ring: Nm::from_mm(0.15),  // 6 mil
        min_silk_width: Nm::from_mm(0.22),    // 0.22mm (wider than JLCPCB)
        min_edge_clearance: Nm::from_mm(0.3), // 0.3mm

        // Advanced geometry
        min_via_annular_ring: Nm::from_mm(0.127), // 5 mil
        max_drill_aspect_ratio: 1000,             // 10:1
        min_solder_mask_bridge: Nm::from_mm(0.1), // 0.1mm
        min_paste_clearance: Nm::from_mm(0.127),  // 5 mil
        solder_mask_expansion: Nm::from_mm(0.05), // 0.05mm
        min_pad_size: Nm::from_mm(0.5),           // 0.5mm
        min_slot_clearance: Nm::from_mm(0.3),     // 0.3mm

        // Signal integrity
        default_impedance_ohms_x100: 5000,        // 50 Ω
        diff_pair_gap: Nm::from_mm(0.15),         // 6 mil
        diff_pair_tolerance: Nm::from_mm(0.025),  // 25µm
        max_stub_length: Nm::from_mm(1.0),        // 1mm
        length_match_tolerance: Nm::from_mm(0.5), // 0.5mm
        max_vias_per_high_speed_net: 4,

        // Thermal
        max_current_per_width_x100: 100_000,
        thermal_relief_gap: Nm::from_mm(0.254), // 10 mil
        thermal_relief_spoke_width: Nm::from_mm(0.254), // 10 mil
        min_copper_pour_clearance: Nm::from_mm(0.254), // 10 mil
        thermal_relief_spokes: 4,

        // Manufacturing
        copper_weight_oz_x10: 10, // 1.0 oz
        board_thickness: Nm::from_mm(1.6),
        min_hole_to_hole: Nm::from_mm(0.5),
        min_hole_to_edge: Nm::from_mm(0.3),
        blind_vias_allowed: true,         // available at extra cost
        buried_vias_allowed: true,        // available at extra cost
        min_acid_trap: Nm::from_mm(0.15), // 6 mil
        max_copper_layers: 14,            // PCBWay supports up to 14 layers
        castellated_holes_allowed: true,
    }
}

/// PCBWay standard 2-layer stackup.
pub fn standard_stackup() -> Stackup {
    let cu = Nm::from_mm(0.035); // 1oz
    let core = Nm::from_mm(1.5);
    let mask = Nm::from_mm(0.01);
    let silk = Nm::from_mm(0.005);

    let layers = vec![
        LayerStackEntry::silkscreen("Top Silkscreen", silk),
        LayerStackEntry::solder_mask("Top Solder Mask", mask),
        LayerStackEntry::signal("Top Copper", cu, 10),
        LayerStackEntry::dielectric("Core", core, "FR-4", 4500),
        LayerStackEntry::signal("Bottom Copper", cu, 10),
        LayerStackEntry::solder_mask("Bottom Solder Mask", mask),
        LayerStackEntry::silkscreen("Bottom Silkscreen", silk),
    ];

    let total = Nm(layers.iter().map(|l| l.thickness.raw()).sum());

    Stackup {
        name: "PCBWay Standard 2-Layer".into(),
        layers,
        total_thickness: total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pcbway_6mil_traces() {
        let dc = standard();
        assert_eq!(dc.min_trace_width, Nm::from_mm(0.15)); // 6 mil
        assert_eq!(dc.min_clearance, Nm::from_mm(0.15)); // 6 mil
    }

    #[test]
    fn test_pcbway_allows_blind_buried() {
        let dc = standard();
        assert!(dc.blind_vias_allowed);
        assert!(dc.buried_vias_allowed);
    }

    #[test]
    fn test_pcbway_high_layer_count() {
        let dc = standard();
        assert_eq!(dc.max_copper_layers, 14);
    }

    #[test]
    fn test_pcbway_stackup() {
        let s = standard_stackup();
        assert_eq!(s.copper_layer_count(), 2);
        let mm = s.total_thickness.to_mm();
        assert!(
            mm > 1.4 && mm < 1.8,
            "PCBWay 2L thickness {mm}mm out of range"
        );
    }
}
