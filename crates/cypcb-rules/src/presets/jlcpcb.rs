//! JLCPCB manufacturer presets.
//!
//! Source: <https://jlcpcb.com/capabilities/pcb-capabilities>
//! Additional: <https://www.schemalyzer.com/en/blog/manufacturing/jlcpcb/jlcpcb-design-rules>
//!
//! JLCPCB offers standard and advanced processes. Standard process supports
//! 5mil (0.127mm) trace/space on 2-layer boards — note this is tighter than
//! the commonly cited 6mil minimum.
//!
//! Capabilities verified 2026-03-13.

use cypcb_core::Nm;

use crate::constraints::DesignConstraints;
use crate::stackup::{LayerStackEntry, Stackup};

/// JLCPCB standard 2-layer process.
///
/// Source: <https://jlcpcb.com/capabilities/pcb-capabilities>
/// - Min trace/space: 5mil (0.127mm) — standard pricing
/// - Min drill: 0.3mm (via), 0.15mm (smallest mechanical)
/// - 1oz copper, 1.6mm FR-4, HASL finish
/// - No blind/buried vias
pub fn standard_2layer() -> DesignConstraints {
    DesignConstraints {
        // Basic geometry
        min_clearance: Nm::from_mm(0.127),   // 5 mil
        min_trace_width: Nm::from_mm(0.127), // 5 mil
        min_drill_size: Nm::from_mm(0.3),    // 0.3mm min mechanical drill
        min_via_drill: Nm::from_mm(0.3),     // 0.3mm min via drill
        // JLCPCB publishes, for a 2-layer board at 1oz, a recommended PTH
        // annular ring of 0.25mm and an absolute minimum of 0.18mm. This read
        // 0.15mm, which is under the figure the page calls absolute - the
        // checker passing a ring the house refuses.
        min_annular_ring: Nm::from_mm(0.18), // 0.18mm, published absolute minimum
        min_silk_width: Nm::from_mm(0.15),   // 0.15mm
        min_edge_clearance: Nm::from_mm(0.3), // 0.3mm

        // Advanced geometry
        min_via_annular_ring: Nm::from_mm(0.127), // 5 mil
        max_drill_aspect_ratio: 800,              // 8:1
        min_solder_mask_bridge: Nm::from_mm(0.1), // 0.1mm
        min_paste_clearance: Nm::from_mm(0.127),  // 5 mil
        solder_mask_expansion: Nm::from_mm(0.05), // 0.05mm typical
        min_pad_size: Some(Nm::from_mm(0.5)),     // 0.5mm min pad
        min_slot_clearance: Nm::from_mm(0.3),     // 0.3mm

        // Signal integrity
        default_impedance_ohms_x100: 5000, // 50.00 Ω (no controlled impedance)
        diff_pair_gap: Nm::from_mm(0.127), // 5 mil
        diff_pair_tolerance: Nm::from_mm(0.025), // 25µm
        max_stub_length: Nm::from_mm(1.0), // 1mm (not impedance controlled)
        length_match_tolerance: Nm::from_mm(0.5), // 0.5mm
        max_vias_per_high_speed_net: 4,

        // Thermal
        max_current_per_width_x100: 100_000, // 1000 mA/mm (1oz, outer, 10°C rise)
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

        // The three assembly-side rules a routing table has no use for. None
        // means this fab does not state one and the checker derives it.
        min_via_diameter: None,
        min_silk_clearance: None,
        min_courtyard_clearance: None,
    }
}

/// JLCPCB standard 2-layer stackup.
pub fn standard_2layer_stackup() -> Stackup {
    let cu = Nm::from_mm(0.035); // 1oz ≈ 35µm
    let core = Nm::from_mm(1.5); // FR-4 core
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
        name: "JLCPCB Standard 2-Layer 1oz".into(),
        layers,
        total_thickness: total,
    }
}

/// JLCPCB standard 4-layer process.
///
/// Source: <https://jlcpcb.com/capabilities/pcb-capabilities>
/// - Min trace/space: 4mil (0.1mm) on standard 4-layer
/// - Min via drill: 0.2mm
/// - 1oz outer / 0.5oz inner, 1.6mm FR-4
/// - Standard JLC7628 stackup
pub fn standard_4layer() -> DesignConstraints {
    DesignConstraints {
        // Basic geometry — tighter than 2-layer
        min_clearance: Nm::from_mm(0.1),   // 4 mil
        min_trace_width: Nm::from_mm(0.1), // 4 mil
        min_drill_size: Nm::from_mm(0.2),  // 0.2mm
        min_via_drill: Nm::from_mm(0.2),   // 0.2mm
        // Multilayer at 1oz: the page publishes PTH annular ring >= 0.20mm.
        // This read 0.125mm under a "~5 mil" comment, which is neither the
        // published figure nor 5 mil.
        min_annular_ring: Nm::from_mm(0.2),    // 0.20mm, published
        min_silk_width: Nm::from_mm(0.15),     // 0.15mm
        min_edge_clearance: Nm::from_mm(0.25), // 0.25mm

        // Advanced geometry
        min_via_annular_ring: Nm::from_mm(0.1), // 4 mil
        max_drill_aspect_ratio: 1000,           // 10:1 for thicker boards
        // 0.10mm, published as the minimum pad spacing a mask dam needs.
        min_solder_mask_bridge: Nm::from_mm(0.1),
        min_paste_clearance: Nm::from_mm(0.1),    // 4 mil
        solder_mask_expansion: Nm::from_mm(0.05), // 0.05mm
        min_pad_size: Some(Nm::from_mm(0.45)),    // 0.45mm
        min_slot_clearance: Nm::from_mm(0.25),    // 0.25mm

        // Signal integrity — impedance control available
        default_impedance_ohms_x100: 5000, // 50 Ω (controlled impedance available)
        diff_pair_gap: Nm::from_mm(0.1),   // 4 mil
        diff_pair_tolerance: Nm::from_mm(0.025), // 25µm
        max_stub_length: Nm::from_mm(0.5), // 0.5mm
        length_match_tolerance: Nm::from_mm(0.25), // 0.25mm
        max_vias_per_high_speed_net: 3,

        // Thermal
        max_current_per_width_x100: 100_000,    // 1000 mA/mm
        thermal_relief_gap: Nm::from_mm(0.254), // 10 mil
        thermal_relief_spoke_width: Nm::from_mm(0.254), // 10 mil
        min_copper_pour_clearance: Nm::from_mm(0.2), // 0.2mm
        thermal_relief_spokes: 4,

        // Manufacturing
        copper_weight_oz_x10: 10,            // 1.0 oz outer
        board_thickness: Nm::from_mm(1.6),   // 1.6mm
        min_hole_to_hole: Nm::from_mm(0.45), // 0.45mm
        min_hole_to_edge: Nm::from_mm(0.25), // 0.25mm
        blind_vias_allowed: false,           // not on standard
        buried_vias_allowed: false,
        min_acid_trap: Nm::from_mm(0.1), // 4 mil
        max_copper_layers: 4,
        castellated_holes_allowed: false,

        // The three assembly-side rules a routing table has no use for. None
        // means this fab does not state one and the checker derives it.
        min_via_diameter: None,
        min_silk_clearance: None,
        min_courtyard_clearance: None,
    }
}

/// JLCPCB standard 4-layer stackup (JLC7628).
///
/// Source: <https://jlcpcb.com/capabilities/pcb-capabilities>
/// Signal / GND / Power / Signal — 1oz outer, 0.5oz inner
pub fn standard_4layer_stackup() -> Stackup {
    let outer_cu = Nm::from_mm(0.035); // 1oz
    let inner_cu = Nm::from_mm(0.018); // 0.5oz
    let prepreg = Nm::from_mm(0.2); // 7628 prepreg
    let core = Nm::from_mm(1.065); // FR-4 core (adjusted for 1.6mm total)
    let mask = Nm::from_mm(0.01);
    let silk = Nm::from_mm(0.005);

    let layers = vec![
        LayerStackEntry::silkscreen("Top Silkscreen", silk),
        LayerStackEntry::solder_mask("Top Solder Mask", mask),
        LayerStackEntry::signal("Top Copper (L1)", outer_cu, 10),
        LayerStackEntry::dielectric("Prepreg 1 (7628)", prepreg, "FR-4 Prepreg 7628", 4600),
        LayerStackEntry::plane("GND Plane (L2)", inner_cu, 5),
        LayerStackEntry::dielectric("Core", core, "FR-4", 4500),
        LayerStackEntry::plane("Power Plane (L3)", inner_cu, 5),
        LayerStackEntry::dielectric("Prepreg 2 (7628)", prepreg, "FR-4 Prepreg 7628", 4600),
        LayerStackEntry::signal("Bottom Copper (L4)", outer_cu, 10),
        LayerStackEntry::solder_mask("Bottom Solder Mask", mask),
        LayerStackEntry::silkscreen("Bottom Silkscreen", silk),
    ];

    let total = Nm(layers.iter().map(|l| l.thickness.raw()).sum());

    Stackup {
        name: "JLCPCB Standard 4-Layer".into(),
        layers,
        total_thickness: total,
    }
}

/// JLCPCB advanced 2-layer process.
///
/// Source: <https://jlcpcb.com/capabilities/pcb-capabilities>
/// - Min trace/space: 3.5mil (0.09mm) — advanced pricing tier
/// - Min drill: 0.15mm
/// - Supports controlled impedance
/// - Higher cost, longer lead time
pub fn advanced_2layer() -> DesignConstraints {
    DesignConstraints {
        // Basic geometry — tightest 2-layer capabilities
        // 0.10mm, published. The page gives 3.5mil for **multilayer** at 1oz
        // and 4mil for one and two layers; this table carried the multilayer
        // figure on a two-layer board, which passes a trace JLCPCB does not
        // make at this layer count.
        min_clearance: Nm::from_mm(0.1),
        min_trace_width: Nm::from_mm(0.1),
        min_drill_size: Nm::from_mm(0.15), // 0.15mm
        min_via_drill: Nm::from_mm(0.15),  // 0.15mm micro via
        // Same page, same 0.20mm: there is no published process tier under it.
        min_annular_ring: Nm::from_mm(0.2),   // 0.20mm, published
        min_silk_width: Nm::from_mm(0.15),    // 0.15mm, published
        min_edge_clearance: Nm::from_mm(0.2), // 0.2mm

        // Advanced geometry
        min_via_annular_ring: Nm::from_mm(0.1),   // 4 mil
        max_drill_aspect_ratio: 1000,             // 10:1
        min_solder_mask_bridge: Nm::from_mm(0.1), // 0.10mm, published
        min_paste_clearance: Nm::from_mm(0.09),   // 3.5 mil
        solder_mask_expansion: Nm::from_mm(0.04), // 0.04mm
        min_pad_size: Some(Nm::from_mm(0.35)),    // 0.35mm
        min_slot_clearance: Nm::from_mm(0.2),     // 0.2mm

        // Signal integrity — controlled impedance standard
        default_impedance_ohms_x100: 5000,       // 50 Ω controlled
        diff_pair_gap: Nm::from_mm(0.09),        // 3.5 mil
        diff_pair_tolerance: Nm::from_mm(0.015), // 15µm
        max_stub_length: Nm::from_mm(0.3),       // 0.3mm
        length_match_tolerance: Nm::from_mm(0.127), // 5 mil
        max_vias_per_high_speed_net: 3,

        // Thermal
        max_current_per_width_x100: 100_000,
        thermal_relief_gap: Nm::from_mm(0.2),         // 8 mil
        thermal_relief_spoke_width: Nm::from_mm(0.2), // 8 mil
        min_copper_pour_clearance: Nm::from_mm(0.2),  // 0.2mm
        thermal_relief_spokes: 4,

        // Manufacturing
        copper_weight_oz_x10: 10, // 1.0 oz
        board_thickness: Nm::from_mm(1.6),
        min_hole_to_hole: Nm::from_mm(0.4),
        min_hole_to_edge: Nm::from_mm(0.2),
        blind_vias_allowed: false,
        buried_vias_allowed: false,
        min_acid_trap: Nm::from_mm(0.09), // 3.5 mil
        max_copper_layers: 2,
        castellated_holes_allowed: true, // available on advanced

        // The three assembly-side rules a routing table has no use for. None
        // means this fab does not state one and the checker derives it.
        min_via_diameter: None,
        min_silk_clearance: None,
        min_courtyard_clearance: None,
    }
}

/// JLCPCB advanced 2-layer stackup.
pub fn advanced_2layer_stackup() -> Stackup {
    // Same physical stackup as standard, just tighter tolerances
    let cu = Nm::from_mm(0.035);
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
        name: "JLCPCB Advanced 2-Layer 1oz".into(),
        layers,
        total_thickness: total,
    }
}

/// JLCPCB advanced 4-layer process.
///
/// Source: <https://jlcpcb.com/capabilities/pcb-capabilities>
/// - Min trace/space: 3.5mil (0.09mm)
/// - Min via drill: 0.15mm
/// - Controlled impedance, ENIG finish
/// - Via-in-pad available
pub fn advanced_4layer() -> DesignConstraints {
    DesignConstraints {
        // Basic geometry
        min_clearance: Nm::from_mm(0.09),   // 3.5 mil
        min_trace_width: Nm::from_mm(0.09), // 3.5 mil
        min_drill_size: Nm::from_mm(0.15),  // 0.15mm
        min_via_drill: Nm::from_mm(0.15),   // 0.15mm
        // Same page, same 0.20mm: there is no published process tier under it.
        min_annular_ring: Nm::from_mm(0.2),   // 0.20mm, published
        min_silk_width: Nm::from_mm(0.15),    // 0.15mm, published
        min_edge_clearance: Nm::from_mm(0.2), // 0.2mm

        // Advanced geometry
        min_via_annular_ring: Nm::from_mm(0.1),   // 4 mil
        max_drill_aspect_ratio: 1200,             // 12:1
        min_solder_mask_bridge: Nm::from_mm(0.1), // 0.10mm, published
        min_paste_clearance: Nm::from_mm(0.09),   // 3.5 mil
        solder_mask_expansion: Nm::from_mm(0.04), // 0.04mm
        min_pad_size: Some(Nm::from_mm(0.35)),    // 0.35mm
        min_slot_clearance: Nm::from_mm(0.2),     // 0.2mm

        // Signal integrity
        default_impedance_ohms_x100: 5000,
        diff_pair_gap: Nm::from_mm(0.09),           // 3.5 mil
        diff_pair_tolerance: Nm::from_mm(0.015),    // 15µm
        max_stub_length: Nm::from_mm(0.25),         // 0.25mm
        length_match_tolerance: Nm::from_mm(0.127), // 5 mil
        max_vias_per_high_speed_net: 2,

        // Thermal
        max_current_per_width_x100: 100_000,
        thermal_relief_gap: Nm::from_mm(0.2),
        thermal_relief_spoke_width: Nm::from_mm(0.2),
        min_copper_pour_clearance: Nm::from_mm(0.2),
        thermal_relief_spokes: 4,

        // Manufacturing
        copper_weight_oz_x10: 10, // 1.0 oz outer
        board_thickness: Nm::from_mm(1.6),
        min_hole_to_hole: Nm::from_mm(0.4),
        min_hole_to_edge: Nm::from_mm(0.2),
        blind_vias_allowed: true, // available on advanced 4L
        buried_vias_allowed: false,
        min_acid_trap: Nm::from_mm(0.09),
        max_copper_layers: 4,
        castellated_holes_allowed: true,

        // The three assembly-side rules a routing table has no use for. None
        // means this fab does not state one and the checker derives it.
        min_via_diameter: None,
        min_silk_clearance: None,
        min_courtyard_clearance: None,
    }
}

/// JLCPCB advanced 4-layer stackup.
pub fn advanced_4layer_stackup() -> Stackup {
    let outer_cu = Nm::from_mm(0.035); // 1oz
    let inner_cu = Nm::from_mm(0.018); // 0.5oz
    let prepreg = Nm::from_mm(0.2); // 7628 prepreg
    let core = Nm::from_mm(1.065);
    let mask = Nm::from_mm(0.01);
    let silk = Nm::from_mm(0.005);

    let layers = vec![
        LayerStackEntry::silkscreen("Top Silkscreen", silk),
        LayerStackEntry::solder_mask("Top Solder Mask", mask),
        LayerStackEntry::signal("Top Copper (L1)", outer_cu, 10),
        LayerStackEntry::dielectric("Prepreg 1 (7628)", prepreg, "FR-4 Prepreg 7628", 4600),
        LayerStackEntry::plane("GND Plane (L2)", inner_cu, 5),
        LayerStackEntry::dielectric("Core", core, "FR-4", 4500),
        LayerStackEntry::plane("Power Plane (L3)", inner_cu, 5),
        LayerStackEntry::dielectric("Prepreg 2 (7628)", prepreg, "FR-4 Prepreg 7628", 4600),
        LayerStackEntry::signal("Bottom Copper (L4)", outer_cu, 10),
        LayerStackEntry::solder_mask("Bottom Solder Mask", mask),
        LayerStackEntry::silkscreen("Bottom Silkscreen", silk),
    ];

    let total = Nm(layers.iter().map(|l| l.thickness.raw()).sum());

    Stackup {
        name: "JLCPCB Advanced 4-Layer".into(),
        layers,
        total_thickness: total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_2layer_5mil_trace() {
        let dc = standard_2layer();
        // JLCPCB 2-layer standard supports 5mil, NOT 6mil
        assert_eq!(dc.min_trace_width, Nm::from_mm(0.127));
        assert_eq!(dc.min_clearance, Nm::from_mm(0.127));
    }

    #[test]
    fn test_standard_2layer_manufacturing() {
        let dc = standard_2layer();
        assert_eq!(dc.copper_weight_oz_x10, 10); // 1oz
        assert_eq!(dc.board_thickness, Nm::from_mm(1.6));
        assert_eq!(dc.max_copper_layers, 2);
        assert!(!dc.blind_vias_allowed);
        assert!(!dc.buried_vias_allowed);
    }

    #[test]
    fn test_standard_4layer_tighter_than_2layer() {
        let dc2 = standard_2layer();
        let dc4 = standard_4layer();
        assert!(dc4.min_trace_width.raw() < dc2.min_trace_width.raw());
        assert!(dc4.min_clearance.raw() < dc2.min_clearance.raw());
        assert_eq!(dc4.max_copper_layers, 4);
    }

    #[test]
    fn test_advanced_2layer_tighter_than_standard() {
        let std = standard_2layer();
        let adv = advanced_2layer();
        assert!(adv.min_trace_width.raw() < std.min_trace_width.raw());
        assert!(adv.min_clearance.raw() < std.min_clearance.raw());
        assert!(adv.min_drill_size.raw() < std.min_drill_size.raw());
    }

    #[test]
    fn test_advanced_4layer_tightest() {
        let adv4 = advanced_4layer();
        assert_eq!(adv4.min_trace_width, Nm::from_mm(0.09)); // 3.5 mil
        assert!(adv4.blind_vias_allowed);
        assert!(adv4.castellated_holes_allowed);
    }

    #[test]
    fn test_stackup_2layer() {
        let s = standard_2layer_stackup();
        assert_eq!(s.copper_layer_count(), 2);
        let mm = s.total_thickness.to_mm();
        assert!(mm > 1.4 && mm < 1.8, "2L thickness {mm}mm out of range");
    }

    #[test]
    fn test_stackup_4layer() {
        let s = standard_4layer_stackup();
        assert_eq!(s.copper_layer_count(), 4);
        let mm = s.total_thickness.to_mm();
        assert!(mm > 1.3 && mm < 1.9, "4L thickness {mm}mm out of range");
    }

    #[test]
    fn test_advanced_stackups_match_layer_count() {
        assert_eq!(advanced_2layer_stackup().copper_layer_count(), 2);
        assert_eq!(advanced_4layer_stackup().copper_layer_count(), 4);
    }
}
