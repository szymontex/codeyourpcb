//! OSHPark manufacturer presets.
//!
//! Source: <https://docs.oshpark.com/design-tools/>
//! Additional: <https://docs.oshpark.com/services/>
//!
//! OSHPark is a US-based PCB service known for purple solder mask
//! ("After Dark" boards), ENIG finish, and shared-panel pricing.
//! More conservative design rules than budget Asian fabs.
//!
//! Capabilities verified 2026-03-13.

use cypcb_core::Nm;

use crate::constraints::DesignConstraints;
use crate::stackup::{LayerStackEntry, Stackup};

/// OSHPark 2-layer service.
///
/// Source: <https://docs.oshpark.com/design-tools/>
/// - Min trace/space: 6mil (0.15mm)
/// - Min drill: 10mil (0.254mm)
/// - 1oz copper both sides, 1.6mm FR-4
/// - ENIG finish, purple solder mask
/// - No blind/buried vias
pub fn two_layer() -> DesignConstraints {
    DesignConstraints {
        // Basic geometry
        // 6mil is 0.1524mm. These carried 0.15mm under a "6 mil" comment,
        // which is the value disagreeing with the source beside it.
        min_clearance: Nm::from_mm(0.1524),     // 6 mil, published
        min_trace_width: Nm::from_mm(0.1524),   // 6 mil, published
        min_drill_size: Nm::from_mm(0.254),     // 10 mil
        min_via_drill: Nm::from_mm(0.254),      // 10 mil
        min_annular_ring: Nm::from_mm(0.127),   // 5 mil
        min_silk_width: Nm::from_mm(0.127),     // 5 mil, published
        min_edge_clearance: Nm::from_mm(0.381), // 15 mil, published

        // Advanced geometry
        min_via_annular_ring: Nm::from_mm(0.127), // 5 mil
        max_drill_aspect_ratio: 800,              // 8:1
        // 4 mil, published as the minimum soldermask web. 4mil is
        // 0.1016mm; the 0.1 that stood here was the mil figure rounded
        // down, which is looser than the page by 1.6 microns.
        min_solder_mask_bridge: Nm::from_mm(0.1016),
        // UNSOURCED. Read 2026-08-21: the service page publishes no
        // stencil aperture and no mask expansion. What it does publish is
        // a 3mil soldermask alignment that "covers retraction, expansion,
        // and shift" - a tolerance on where the mask lands, not a figure
        // to enlarge an opening by, so it is not this field.
        min_paste_clearance: Nm::from_mm(0.127),
        solder_mask_expansion: Nm::from_mm(0.05),
        // OSH Park publishes an annular ring and no pad diameter. Derived.
        min_pad_size: None,
        min_slot_clearance: Nm::from_mm(0.381), // 15 mil

        // Signal integrity — basic (no controlled impedance on 2L)
        default_impedance_ohms_x100: 5000,
        diff_pair_gap: Nm::from_mm(0.15),         // 6 mil
        diff_pair_tolerance: Nm::from_mm(0.05),   // 50µm
        max_stub_length: Nm::from_mm(2.0),        // relaxed
        length_match_tolerance: Nm::from_mm(1.0), // relaxed
        max_vias_per_high_speed_net: 6,

        // Thermal
        max_current_per_width_x100: 100_000,
        thermal_relief_gap: Nm::from_mm(0.254), // 10 mil
        thermal_relief_spoke_width: Nm::from_mm(0.254), // 10 mil
        min_copper_pour_clearance: Nm::from_mm(0.254), // 10 mil
        thermal_relief_spokes: 4,

        // Manufacturing
        copper_weight_oz_x10: 10,          // 1.0 oz
        board_thickness: Nm::from_mm(1.6), // 63 mil (1.6mm)
        // The service page states "Board Thickness: 63mil (1.6mm) nominal"
        // and publishes no tolerance for it, so none is carried.
        board_thickness_tolerance_percent: None,
        board_thickness_tolerance_thin: None,
        // "Drill Size tolerance: Max: +/- 2.5mil (0.0635mm), Typical: +/-
        // 1.0mil (0.0254)". The maximum is what a board is guaranteed, so it
        // is the figure carried; the typical one is not a promise.
        // <https://docs.oshpark.com/services/two-layer/>
        hole_tolerance_plus: Some(Nm::from_mm(0.0635)),
        hole_tolerance_minus: Some(Nm::from_mm(0.0635)),
        min_hole_to_hole: Nm::from_mm(0.635), // 25 mil
        min_hole_to_edge: Nm::from_mm(0.381), // 15 mil
        blind_vias_allowed: false,
        buried_vias_allowed: false,
        min_acid_trap: Nm::from_mm(0.15), // 6 mil
        max_copper_layers: 2,
        castellated_holes_allowed: false,

        // The three assembly-side rules a routing table has no use for. None
        // means this fab does not state one and the checker derives it.
        min_via_diameter: None,
        min_silk_clearance: None,
        min_courtyard_clearance: None,

        // No published flex stiffener list has been read for this house, so
        // the checker holds a design to none.
        stiffener_thickness_um: Vec::new(),
    }
}

/// OSHPark 2-layer stackup.
///
/// Source: <https://docs.oshpark.com/services/>
/// 1oz copper, FR-408 dielectric (lower loss than standard FR-4).
pub fn two_layer_stackup() -> Stackup {
    let cu = Nm::from_mm(0.035); // 1oz
    let core = Nm::from_mm(1.5); // FR-408 core
    let mask = Nm::from_mm(0.01);
    let silk = Nm::from_mm(0.005);

    let layers = vec![
        LayerStackEntry::silkscreen("Top Silkscreen", silk),
        LayerStackEntry::solder_mask("Top Solder Mask (Purple)", mask),
        LayerStackEntry::signal("Top Copper", cu, 10),
        LayerStackEntry::dielectric("Core", core, "FR-408", 3700), // FR-408 εr ≈ 3.7
        LayerStackEntry::signal("Bottom Copper", cu, 10),
        LayerStackEntry::solder_mask("Bottom Solder Mask (Purple)", mask),
        LayerStackEntry::silkscreen("Bottom Silkscreen", silk),
    ];

    let total = Nm(layers.iter().map(|l| l.thickness.raw()).sum());

    Stackup {
        name: "OSHPark 2-Layer".into(),
        layers,
        total_thickness: total,
    }
}

/// OSHPark 4-layer service.
///
/// Source: <https://docs.oshpark.com/services/>
/// - Min trace/space: 5mil (0.127mm)
/// - Min drill: 10mil (0.254mm)
/// - 1oz outer / 0.5oz inner
/// - ENIG finish, controlled impedance available
/// - No blind/buried vias
pub fn four_layer() -> DesignConstraints {
    DesignConstraints {
        // Basic geometry — tighter than 2L
        min_clearance: Nm::from_mm(0.127),   // 5 mil
        min_trace_width: Nm::from_mm(0.127), // 5 mil
        min_drill_size: Nm::from_mm(0.254),  // 10 mil
        min_via_drill: Nm::from_mm(0.254),   // 10 mil
        // 4mil is 0.1016mm. This carried 0.1mm, which is looser than the
        // figure its own comment names.
        min_annular_ring: Nm::from_mm(0.1016), // 4 mil, published
        min_silk_width: Nm::from_mm(0.127),    // 5 mil, published
        min_edge_clearance: Nm::from_mm(0.381), // 15 mil, published

        // Advanced geometry
        min_via_annular_ring: Nm::from_mm(0.1016), // 4 mil, published
        max_drill_aspect_ratio: 1000,              // 10:1
        min_solder_mask_bridge: Nm::from_mm(0.1016), // 4 mil, published
        // UNSOURCED, the same two as the two-layer table above.
        min_paste_clearance: Nm::from_mm(0.127),
        solder_mask_expansion: Nm::from_mm(0.05),
        // OSH Park publishes an annular ring and no pad diameter. Derived.
        min_pad_size: None,
        min_slot_clearance: Nm::from_mm(0.381), // 15 mil

        // Signal integrity — controlled impedance available
        default_impedance_ohms_x100: 5000,
        diff_pair_gap: Nm::from_mm(0.127),       // 5 mil
        diff_pair_tolerance: Nm::from_mm(0.025), // 25µm
        max_stub_length: Nm::from_mm(0.5),
        length_match_tolerance: Nm::from_mm(0.5),
        max_vias_per_high_speed_net: 4,

        // Thermal
        max_current_per_width_x100: 100_000,
        thermal_relief_gap: Nm::from_mm(0.254),
        thermal_relief_spoke_width: Nm::from_mm(0.254),
        min_copper_pour_clearance: Nm::from_mm(0.254),
        thermal_relief_spokes: 4,

        // Manufacturing
        copper_weight_oz_x10: 10, // 1.0 oz outer
        board_thickness: Nm::from_mm(1.6),
        // The service page states "Board Thickness: 63mil (1.6mm) nominal"
        // and publishes no tolerance for it, so none is carried.
        board_thickness_tolerance_percent: None,
        board_thickness_tolerance_thin: None,
        // "Drill Size tolerance: Max: +/- 2.5mil (0.0635mm), Typical: +/-
        // 1.0mil (0.0254)". The maximum is what a board is guaranteed, so it
        // is the figure carried; the typical one is not a promise.
        // <https://docs.oshpark.com/services/two-layer/>
        hole_tolerance_plus: Some(Nm::from_mm(0.0635)),
        hole_tolerance_minus: Some(Nm::from_mm(0.0635)),
        min_hole_to_hole: Nm::from_mm(0.508), // 20 mil
        min_hole_to_edge: Nm::from_mm(0.381), // 15 mil
        blind_vias_allowed: false,
        buried_vias_allowed: false,
        min_acid_trap: Nm::from_mm(0.127), // 5 mil
        max_copper_layers: 4,
        castellated_holes_allowed: false,

        // The three assembly-side rules a routing table has no use for. None
        // means this fab does not state one and the checker derives it.
        min_via_diameter: None,
        min_silk_clearance: None,
        min_courtyard_clearance: None,

        // No published flex stiffener list has been read for this house, so
        // the checker holds a design to none.
        stiffener_thickness_um: Vec::new(),
    }
}

/// OSHPark 4-layer stackup.
///
/// Source: <https://docs.oshpark.com/services/>
/// Signal / GND / Power / Signal — 1oz outer, 0.5oz inner, FR-408 dielectric.
pub fn four_layer_stackup() -> Stackup {
    let outer_cu = Nm::from_mm(0.035); // 1oz
    let inner_cu = Nm::from_mm(0.018); // 0.5oz
    let prepreg = Nm::from_mm(0.2);
    let core = Nm::from_mm(1.065);
    let mask = Nm::from_mm(0.01);
    let silk = Nm::from_mm(0.005);

    let layers = vec![
        LayerStackEntry::silkscreen("Top Silkscreen", silk),
        LayerStackEntry::solder_mask("Top Solder Mask (Purple)", mask),
        LayerStackEntry::signal("Top Copper (L1)", outer_cu, 10),
        LayerStackEntry::dielectric("Prepreg 1", prepreg, "FR-408 Prepreg", 3700),
        LayerStackEntry::plane("GND Plane (L2)", inner_cu, 5),
        LayerStackEntry::dielectric("Core", core, "FR-408", 3700),
        LayerStackEntry::plane("Power Plane (L3)", inner_cu, 5),
        LayerStackEntry::dielectric("Prepreg 2", prepreg, "FR-408 Prepreg", 3700),
        LayerStackEntry::signal("Bottom Copper (L4)", outer_cu, 10),
        LayerStackEntry::solder_mask("Bottom Solder Mask (Purple)", mask),
        LayerStackEntry::silkscreen("Bottom Silkscreen", silk),
    ];

    let total = Nm(layers.iter().map(|l| l.thickness.raw()).sum());

    Stackup {
        name: "OSHPark 4-Layer".into(),
        layers,
        total_thickness: total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oshpark_2l_6mil() {
        // The test is named for 6mil and asserted 0.15mm, which is not 6mil.
        let dc = two_layer();
        assert_eq!(dc.min_trace_width, Nm::from_mil(6.0));
        assert_eq!(dc.min_clearance, Nm::from_mil(6.0));
    }

    #[test]
    fn test_oshpark_2l_10mil_drill() {
        let dc = two_layer();
        assert_eq!(dc.min_drill_size, Nm::from_mm(0.254));
        assert_eq!(dc.min_via_drill, Nm::from_mm(0.254));
    }

    #[test]
    fn test_oshpark_2l_no_advanced_vias() {
        let dc = two_layer();
        assert!(!dc.blind_vias_allowed);
        assert!(!dc.buried_vias_allowed);
    }

    #[test]
    fn test_oshpark_4l_5mil() {
        let dc = four_layer();
        assert_eq!(dc.min_trace_width, Nm::from_mm(0.127));
        assert_eq!(dc.min_clearance, Nm::from_mm(0.127));
        assert_eq!(dc.max_copper_layers, 4);
    }

    #[test]
    fn test_oshpark_stackup_2l() {
        let s = two_layer_stackup();
        assert_eq!(s.copper_layer_count(), 2);
        // OSHPark uses FR-408
        let core = s.layers.iter().find(|l| l.name.contains("Core")).unwrap();
        assert!(core.material.contains("FR-408"));
    }

    #[test]
    fn test_oshpark_stackup_4l() {
        let s = four_layer_stackup();
        assert_eq!(s.copper_layer_count(), 4);
    }

    #[test]
    fn test_oshpark_wider_edge_clearance() {
        // OSHPark has 15mil edge clearance — more conservative than JLCPCB
        let dc = two_layer();
        assert_eq!(dc.min_edge_clearance, Nm::from_mm(0.381));
    }
}
