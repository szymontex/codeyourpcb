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
        // Read against the published capabilities page, 2026-08-13. The two
        // figures below were 0.15mm each and commented "6 mil (recommended)".
        // PCBWay publishes 0.1mm/4mil as its minimum trace and minimum
        // spacing; the 0.15mm was somebody's comfort margin baked into a table
        // that says what a house can make. A margin is the designer's to
        // choose and belongs in a `netclass`, not in a fab's capabilities.
        min_clearance: Nm::from_mm(0.1),   // 0.1mm/4mil, published
        min_trace_width: Nm::from_mm(0.1), // 0.1mm/4mil, published
        // The standard section publishes 0.15mm as the smallest hole drilled;
        // the advanced section puts the normal process at 0.20mm and notes
        // that holes under 0.2mm need special consideration. This table is the
        // normal process, so 0.2mm - written down here so the next reader does
        // not "correct" it to 0.15 without the row that pairs with it.
        min_drill_size: Nm::from_mm(0.2), // 0.2mm, advanced normal process
        min_via_drill: Nm::from_mm(0.2),  // 0.2mm, advanced normal process
        min_annular_ring: Nm::from_mm(0.15), // 0.15mm/6mil, published
        // 0.15mm, published as the minimum legend width. 0.22 was a
        // margin on top of it, and a capability table says what a house
        // can make - the same correction the trace and space in this
        // table already took.
        min_silk_width: Nm::from_mm(0.15),
        // 0.25mm, published for the standard CNC-milled process. The page
        // puts 0.20mm under "medium difficulty" and says anything below
        // that needs special consideration, so 0.25 is the normal one -
        // the same standard-versus-advanced pairing this table's drill
        // figure already follows.
        min_edge_clearance: Nm::from_mm(0.25),

        // Advanced geometry
        // PCBWay publishes one annular-ring minimum, 0.15mm/6mil, and does not
        // distinguish a via's ring from a pad's. This was 0.127mm, which came
        // from nowhere the page states and was **looser** than the published
        // figure - so a board with a 0.13mm via ring passed `cypcb check
        // --preset pcbway` and PCBWay's own page refuses it.
        min_via_annular_ring: Nm::from_mm(0.15), // 0.15mm/6mil, published
        max_drill_aspect_ratio: 1000,            // 10:1
        // 4 mil, published for copper under 2oz. 4mil is 0.1016mm.
        min_solder_mask_bridge: Nm::from_mm(0.1016),
        // UNSOURCED. Read 2026-08-21: the capabilities page publishes no
        // stencil aperture at all.
        min_paste_clearance: Nm::from_mm(0.127),
        // 2 mil, published as the standard mask opening enlargement. 2mil
        // is 0.0508mm.
        solder_mask_expansion: Nm::from_mm(0.0508),
        // PCBWay publishes an annular ring and no pad diameter. Derived.
        min_pad_size: None,
        min_slot_clearance: Nm::from_mm(0.3), // 0.3mm

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

        // The three assembly-side rules a routing table has no use for. None
        // means this fab does not state one and the checker derives it.
        min_via_diameter: None,
        min_silk_clearance: None,
        min_courtyard_clearance: None,
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
    fn test_pcbway_4mil_traces() {
        // Named for 6mil until the page was read: PCBWay publishes 0.1mm/4mil
        // as its minimum trace and its minimum spacing. The 0.15mm this
        // asserted was an unsourced margin, and 0.15mm is not 6mil either.
        let dc = standard();
        assert_eq!(dc.min_trace_width, Nm::from_mm(0.1));
        assert_eq!(dc.min_clearance, Nm::from_mm(0.1));
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
