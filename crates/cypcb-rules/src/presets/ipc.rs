//! IPC reliability class presets.
//!
//! Source: IPC-2221B "Generic Standard on Printed Board Design"
//! Reference: <https://www.ipc.org/ipc-2221>
//!
//! IPC defines three product classes with increasing reliability requirements:
//! - **Class 1** — Consumer electronics (relaxed)
//! - **Class 2** — Dedicated service equipment (standard)
//! - **Class 3** — High reliability / military / medical (tight)
//!
//! These presets use generic 2-layer stackups. For specific layer counts,
//! combine with a manufacturer preset or custom stackup.
//!
//! Values based on IPC-2221B Tables 6-1, 6-2, and IPC-2222 requirements.

use cypcb_core::Nm;

use crate::constraints::DesignConstraints;
use crate::stackup::{LayerStackEntry, Stackup};

/// IPC Class 1 — Consumer electronics.
///
/// Relaxed tolerances suitable for non-critical consumer products.
/// Examples: toys, LED lighting, simple power supplies, hobby boards.
///
/// Source: IPC-2221B Class 1 requirements
pub fn class1() -> DesignConstraints {
    DesignConstraints {
        // Basic geometry — relaxed
        min_clearance: Nm::from_mm(0.2),       // 8 mil
        min_trace_width: Nm::from_mm(0.2),     // 8 mil
        min_drill_size: Nm::from_mm(0.3),      // 0.3mm
        min_via_drill: Nm::from_mm(0.3),       // 0.3mm
        min_annular_ring: Nm::from_mm(0.15),   // 6 mil (IPC Class 1 minimum)
        min_silk_width: Nm::from_mm(0.2),      // 8 mil
        min_edge_clearance: Nm::from_mm(0.25), // 10 mil

        // Advanced geometry
        min_via_annular_ring: Nm::from_mm(0.127),  // 5 mil
        max_drill_aspect_ratio: 800,               // 8:1
        min_solder_mask_bridge: Nm::from_mm(0.1),  // 4 mil
        min_paste_clearance: Nm::from_mm(0.15),    // 6 mil
        solder_mask_expansion: Nm::from_mm(0.075), // 0.075mm
        min_pad_size: Nm::from_mm(0.6),            // 24 mil
        min_slot_clearance: Nm::from_mm(0.3),      // 12 mil

        // Signal integrity — not critical for Class 1
        default_impedance_ohms_x100: 5000,
        diff_pair_gap: Nm::from_mm(0.2),          // 8 mil
        diff_pair_tolerance: Nm::from_mm(0.1),    // 100µm (relaxed)
        max_stub_length: Nm::from_mm(5.0),        // effectively no limit
        length_match_tolerance: Nm::from_mm(5.0), // effectively no limit
        max_vias_per_high_speed_net: 10,          // relaxed

        // Thermal
        max_current_per_width_x100: 100_000,
        thermal_relief_gap: Nm::from_mm(0.3),         // 12 mil
        thermal_relief_spoke_width: Nm::from_mm(0.3), // 12 mil
        min_copper_pour_clearance: Nm::from_mm(0.3),  // 12 mil
        thermal_relief_spokes: 4,

        // Manufacturing
        copper_weight_oz_x10: 10, // 1.0 oz
        board_thickness: Nm::from_mm(1.6),
        min_hole_to_hole: Nm::from_mm(0.6), // relaxed
        min_hole_to_edge: Nm::from_mm(0.4),
        blind_vias_allowed: false,
        buried_vias_allowed: false,
        min_acid_trap: Nm::from_mm(0.2), // 8 mil
        max_copper_layers: 6,
        castellated_holes_allowed: false,

        // The three assembly-side rules a routing table has no use for. None
        // means this fab does not state one and the checker derives it.
        min_via_diameter: None,
        min_silk_clearance: None,
        min_courtyard_clearance: None,
    }
}

/// IPC Class 1 generic 2-layer stackup.
pub fn class1_stackup() -> Stackup {
    generic_2layer_stackup("IPC Class 1 2-Layer")
}

/// IPC Class 2 — Dedicated service equipment.
///
/// Standard tolerances for equipment requiring extended life and reliable
/// performance. Examples: industrial controls, telecommunications,
/// commercial computing, automotive non-safety.
///
/// Source: IPC-2221B Class 2 requirements
pub fn class2() -> DesignConstraints {
    DesignConstraints {
        // Basic geometry — moderate
        min_clearance: Nm::from_mm(0.15),      // 6 mil
        min_trace_width: Nm::from_mm(0.15),    // 6 mil
        min_drill_size: Nm::from_mm(0.25),     // 0.25mm
        min_via_drill: Nm::from_mm(0.25),      // 0.25mm
        min_annular_ring: Nm::from_mm(0.127),  // 5 mil (IPC Class 2 minimum)
        min_silk_width: Nm::from_mm(0.15),     // 6 mil
        min_edge_clearance: Nm::from_mm(0.25), // 10 mil

        // Advanced geometry
        min_via_annular_ring: Nm::from_mm(0.127), // 5 mil
        max_drill_aspect_ratio: 1000,             // 10:1
        min_solder_mask_bridge: Nm::from_mm(0.1), // 4 mil
        min_paste_clearance: Nm::from_mm(0.127),  // 5 mil
        solder_mask_expansion: Nm::from_mm(0.05), // 0.05mm
        min_pad_size: Nm::from_mm(0.5),           // 20 mil
        min_slot_clearance: Nm::from_mm(0.25),    // 10 mil

        // Signal integrity
        default_impedance_ohms_x100: 5000,
        diff_pair_gap: Nm::from_mm(0.15),         // 6 mil
        diff_pair_tolerance: Nm::from_mm(0.05),   // 50µm
        max_stub_length: Nm::from_mm(1.0),        // 1mm
        length_match_tolerance: Nm::from_mm(1.0), // 1mm
        max_vias_per_high_speed_net: 6,

        // Thermal
        max_current_per_width_x100: 100_000,
        thermal_relief_gap: Nm::from_mm(0.254), // 10 mil
        thermal_relief_spoke_width: Nm::from_mm(0.254), // 10 mil
        min_copper_pour_clearance: Nm::from_mm(0.254), // 10 mil
        thermal_relief_spokes: 4,

        // Manufacturing
        copper_weight_oz_x10: 10,
        board_thickness: Nm::from_mm(1.6),
        min_hole_to_hole: Nm::from_mm(0.5),
        min_hole_to_edge: Nm::from_mm(0.3),
        blind_vias_allowed: false,
        buried_vias_allowed: false,
        min_acid_trap: Nm::from_mm(0.15), // 6 mil
        max_copper_layers: 8,
        castellated_holes_allowed: false,

        // The three assembly-side rules a routing table has no use for. None
        // means this fab does not state one and the checker derives it.
        min_via_diameter: None,
        min_silk_clearance: None,
        min_courtyard_clearance: None,
    }
}

/// IPC Class 2 generic 2-layer stackup.
pub fn class2_stackup() -> Stackup {
    generic_2layer_stackup("IPC Class 2 2-Layer")
}

/// IPC Class 3 — High reliability.
///
/// Tight tolerances for life-critical and mission-critical equipment.
/// Examples: medical devices, military/aerospace, safety-critical automotive,
/// life-support equipment, flight controls.
///
/// Source: IPC-2221B Class 3 requirements
pub fn class3() -> DesignConstraints {
    DesignConstraints {
        // Basic geometry — tight
        min_clearance: Nm::from_mm(0.1),      // 4 mil
        min_trace_width: Nm::from_mm(0.1),    // 4 mil
        min_drill_size: Nm::from_mm(0.2),     // 0.2mm
        min_via_drill: Nm::from_mm(0.2),      // 0.2mm
        min_annular_ring: Nm::from_mm(0.127), // 5 mil (IPC Class 3 stricter)
        min_silk_width: Nm::from_mm(0.127),   // 5 mil
        min_edge_clearance: Nm::from_mm(0.5), // 20 mil (wider for reliability)

        // Advanced geometry
        min_via_annular_ring: Nm::from_mm(0.127),   // 5 mil
        max_drill_aspect_ratio: 1200,               // 12:1 (more reliable plating)
        min_solder_mask_bridge: Nm::from_mm(0.075), // 3 mil
        min_paste_clearance: Nm::from_mm(0.1),      // 4 mil
        solder_mask_expansion: Nm::from_mm(0.05),   // 0.05mm
        min_pad_size: Nm::from_mm(0.45),            // 18 mil
        min_slot_clearance: Nm::from_mm(0.25),      // 10 mil

        // Signal integrity — controlled
        default_impedance_ohms_x100: 5000,
        diff_pair_gap: Nm::from_mm(0.1),            // 4 mil
        diff_pair_tolerance: Nm::from_mm(0.02),     // 20µm (tight)
        max_stub_length: Nm::from_mm(0.25),         // 0.25mm (strict)
        length_match_tolerance: Nm::from_mm(0.127), // 5 mil (strict)
        max_vias_per_high_speed_net: 2,             // minimize vias

        // Thermal
        max_current_per_width_x100: 100_000,
        thermal_relief_gap: Nm::from_mm(0.2),         // 8 mil
        thermal_relief_spoke_width: Nm::from_mm(0.3), // 12 mil (wider for reliability)
        min_copper_pour_clearance: Nm::from_mm(0.2),  // 8 mil
        thermal_relief_spokes: 4,

        // Manufacturing — strict
        copper_weight_oz_x10: 10,
        board_thickness: Nm::from_mm(1.6),
        min_hole_to_hole: Nm::from_mm(0.5),
        min_hole_to_edge: Nm::from_mm(0.5), // wider for reliability
        blind_vias_allowed: false,          // through-hole only for reliability
        buried_vias_allowed: false,
        min_acid_trap: Nm::from_mm(0.1), // 4 mil
        max_copper_layers: 12,
        castellated_holes_allowed: false,

        // The three assembly-side rules a routing table has no use for. None
        // means this fab does not state one and the checker derives it.
        min_via_diameter: None,
        min_silk_clearance: None,
        min_courtyard_clearance: None,
    }
}

/// IPC Class 3 generic 2-layer stackup.
pub fn class3_stackup() -> Stackup {
    generic_2layer_stackup("IPC Class 3 2-Layer")
}

/// Generic 2-layer 1oz FR-4 stackup used for IPC presets.
fn generic_2layer_stackup(name: &str) -> Stackup {
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
        name: name.into(),
        layers,
        total_thickness: total,
    }
}

/// Prototyping - bigger than any fab requires, on purpose.
///
/// Not a manufacturer: larger minimums for hand-soldering, beginner designs
/// and cheap fabrication, where yield matters more than density. It lived in
/// the checker's own preset table as thirteen hand-written numbers while the
/// router's table had no such preset at all; those thirteen are kept here
/// exactly, so a board checked against `prototype` is measured by the same
/// rules it always was.
///
/// Everything a checker does not read - signal integrity, thermal, stackup -
/// is IPC Class 1, because a prototype is a consumer-class board and inventing
/// numbers for it would be worse than saying where they came from.
pub fn prototype() -> DesignConstraints {
    DesignConstraints {
        // The thirteen the checker reads, from the table this preset had.
        min_clearance: Nm::from_mm(0.2),    // 8 mil
        min_trace_width: Nm::from_mm(0.25), // 10 mil
        min_drill_size: Nm::from_mm(0.4),
        min_via_drill: Nm::from_mm(0.3),
        min_annular_ring: Nm::from_mm(0.2),
        min_silk_width: Nm::from_mm(0.2),
        min_edge_clearance: Nm::from_mm(0.5),
        min_hole_to_hole: Nm::from_mm(0.6),
        min_solder_mask_bridge: Nm::from_mm(0.15),
        solder_mask_expansion: Nm::from_mm(0.075),
        // Stated rather than derived: a via pad big enough to solder by hand
        // is the point of this preset, and courtyard spacing to match.
        min_via_diameter: Some(Nm::from_mm(0.8)),
        min_silk_clearance: Some(Nm::from_mm(0.2)),
        min_courtyard_clearance: Some(Nm::from_mm(0.5)),

        // The rest is IPC Class 1.
        ..class1()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_class1_relaxed() {
        let dc = class1();
        assert_eq!(dc.min_trace_width, Nm::from_mm(0.2)); // 8 mil
        assert_eq!(dc.min_clearance, Nm::from_mm(0.2));
    }

    #[test]
    fn test_class2_moderate() {
        let dc = class2();
        assert_eq!(dc.min_trace_width, Nm::from_mm(0.15)); // 6 mil
        assert_eq!(dc.min_clearance, Nm::from_mm(0.15));
    }

    #[test]
    fn test_class3_tight() {
        let dc = class3();
        assert_eq!(dc.min_trace_width, Nm::from_mm(0.1)); // 4 mil
        assert_eq!(dc.min_clearance, Nm::from_mm(0.1));
    }

    #[test]
    fn test_classes_ordered_strictness() {
        let c1 = class1();
        let c2 = class2();
        let c3 = class3();

        // Trace width: Class 1 > Class 2 > Class 3
        assert!(c1.min_trace_width.raw() > c2.min_trace_width.raw());
        assert!(c2.min_trace_width.raw() > c3.min_trace_width.raw());

        // Length match tolerance: Class 1 > Class 2 > Class 3
        assert!(c1.length_match_tolerance.raw() > c2.length_match_tolerance.raw());
        assert!(c2.length_match_tolerance.raw() > c3.length_match_tolerance.raw());
    }

    #[test]
    fn test_class3_wider_edge_clearance() {
        let c1 = class1();
        let c3 = class3();
        // Class 3 has wider edge clearance for reliability
        assert!(c3.min_edge_clearance.raw() > c1.min_edge_clearance.raw());
    }

    #[test]
    fn test_ipc_stackups() {
        for (name, stackup) in [
            ("Class 1", class1_stackup()),
            ("Class 2", class2_stackup()),
            ("Class 3", class3_stackup()),
        ] {
            assert_eq!(stackup.copper_layer_count(), 2, "{name} should be 2-layer");
            let mm = stackup.total_thickness.to_mm();
            assert!(mm > 1.4 && mm < 1.8, "{name} thickness {mm}mm out of range");
        }
    }
}
