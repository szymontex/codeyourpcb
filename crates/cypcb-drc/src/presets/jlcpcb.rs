//! JLCPCB manufacturer design rules.
//!
//! JLCPCB is a popular Chinese PCB manufacturer known for competitive pricing
//! and fast turnaround. Their capabilities are well-documented and suitable
//! for most hobbyist and professional projects.
//!
//! # Sources
//!
//! - <https://jlcpcb.com/capabilities/pcb-capabilities>
//! - <https://www.schemalyzer.com/en/blog/manufacturing/jlcpcb/jlcpcb-design-rules>

use super::DesignRules;
use cypcb_rules::presets::RulesPreset;

impl DesignRules {
    /// JLCPCB standard 2-layer board rules.
    ///
    /// These are the most common rules for hobbyist projects. The 6mil (0.15mm)
    /// minimum is achievable at standard pricing.
    ///
    /// # Specifications
    ///
    /// | Parameter | Value | Notes |
    /// |-----------|-------|-------|
    /// | Min clearance | 0.127mm (5 mil) | Published |
    /// | Min trace width | 0.127mm (5 mil) | Published |
    /// | Min drill | 0.3mm | Mechanical drilling |
    /// | Min via drill | 0.3mm | Same as the mechanical drill |
    /// | Min annular ring | 0.18mm | Published absolute minimum |
    /// | Min silk width | 0.15mm (6 mil) | Silkscreen lines |
    /// | Min edge clearance | 0.3mm | Copper to board edge |
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_drc::DesignRules;
    /// use cypcb_core::Nm;
    ///
    /// let rules = DesignRules::jlcpcb_2layer();
    /// assert_eq!(rules.min_clearance, Nm::from_mm(0.127));
    /// assert_eq!(rules.min_drill_size, Nm::from_mm(0.3));
    /// ```
    pub fn jlcpcb_2layer() -> Self {
        Self::from_constraints(&RulesPreset::JlcpcbStandard2Layer.constraints())
    }

    /// JLCPCB 4-layer board rules with tighter tolerances.
    ///
    /// 4-layer boards have access to tighter tolerances due to better
    /// manufacturing control. The 4mil (0.1mm) minimum is available
    /// at slightly higher cost.
    ///
    /// # Specifications
    ///
    /// | Parameter | Value | Notes |
    /// |-----------|-------|-------|
    /// | Min clearance | 0.1mm (4 mil) | Tighter tolerance |
    /// | Min trace width | 0.1mm (4 mil) | Tighter tolerance |
    /// | Min drill | 0.2mm | Smaller drills available |
    /// | Min via drill | 0.2mm | Via holes |
    /// | Min annular ring | 0.2mm | Published for multilayer, and larger than 2-layer |
    /// | Min silk width | 0.15mm (6 mil) | Same as 2-layer |
    /// | Min edge clearance | 0.25mm | Tighter than 2-layer |
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_drc::DesignRules;
    /// use cypcb_core::Nm;
    ///
    /// let rules = DesignRules::jlcpcb_4layer();
    /// assert_eq!(rules.min_clearance, Nm::from_mm(0.1));
    /// assert_eq!(rules.min_drill_size, Nm::from_mm(0.2));
    /// ```
    pub fn jlcpcb_4layer() -> Self {
        Self::from_constraints(&RulesPreset::JlcpcbStandard4Layer.constraints())
    }

    /// JLCPCB advanced 2-layer board rules.
    ///
    /// Advanced process with tighter tolerances. Higher cost, longer lead time.
    ///
    /// # Specifications
    ///
    /// | Parameter | Value | Notes |
    /// |-----------|-------|-------|
    /// | Min clearance | 0.10mm (4 mil) | Published for 1 and 2 layers |
    /// | Min trace width | 0.10mm (4 mil) | 3.5 mil is the multilayer figure |
    /// | Min drill | 0.15mm | |
    /// | Min via drill | 0.15mm | |
    /// | Min annular ring | 0.20mm | One published figure, no tier under it |
    /// | Min silk width | 0.15mm | Likewise |
    /// | Min edge clearance | 0.2mm | |
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_drc::DesignRules;
    /// use cypcb_core::Nm;
    ///
    /// let rules = DesignRules::jlcpcb_advanced_2layer();
    /// assert_eq!(rules.min_clearance, Nm::from_mm(0.1));
    /// assert_eq!(rules.min_drill_size, Nm::from_mm(0.15));
    /// ```
    pub fn jlcpcb_advanced_2layer() -> Self {
        Self::from_constraints(&RulesPreset::JlcpcbAdvanced2Layer.constraints())
    }

    /// JLCPCB advanced 4-layer board rules.
    ///
    /// Advanced 4-layer process with blind/buried vias available.
    ///
    /// # Specifications
    ///
    /// | Parameter | Value | Notes |
    /// |-----------|-------|-------|
    /// | Min clearance | 0.09mm (3.5 mil) | Advanced process |
    /// | Min trace width | 0.09mm (3.5 mil) | Advanced process |
    /// | Min drill | 0.15mm | Micro-drill |
    /// | Min via drill | 0.15mm | Micro-via |
    /// | Min annular ring | 0.20mm | One published figure, no tier under it |
    /// | Min silk width | 0.15mm | Likewise |
    /// | Min edge clearance | 0.2mm | |
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_drc::DesignRules;
    /// use cypcb_core::Nm;
    ///
    /// let rules = DesignRules::jlcpcb_advanced_4layer();
    /// assert_eq!(rules.min_clearance, Nm::from_mm(0.09));
    /// ```
    pub fn jlcpcb_advanced_4layer() -> Self {
        Self::from_constraints(&RulesPreset::JlcpcbAdvanced4Layer.constraints())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::Nm;

    #[test]
    fn test_jlcpcb_2layer_clearance() {
        let rules = DesignRules::jlcpcb_2layer();
        // 5 mil standard process, same number the autorouter routes against
        assert_eq!(rules.min_clearance, Nm::from_mm(0.127));
    }

    #[test]
    fn test_jlcpcb_2layer_drill() {
        let rules = DesignRules::jlcpcb_2layer();
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.3));
        assert_eq!(rules.min_via_drill, Nm::from_mm(0.3));
    }

    #[test]
    fn test_jlcpcb_4layer_tighter() {
        let two = DesignRules::jlcpcb_2layer();
        let four = DesignRules::jlcpcb_4layer();

        // 4-layer is tighter in what it can image and drill.
        assert!(four.min_clearance < two.min_clearance);
        assert!(four.min_trace_width < two.min_trace_width);
        assert!(four.min_drill_size < two.min_drill_size);
        assert!(four.min_edge_clearance < two.min_edge_clearance);

        // The ring goes the other way, and this asserted the opposite until the
        // page was read. JLCPCB publishes a PTH annular ring of 0.20mm for
        // multilayer at 1oz and an absolute minimum of 0.18mm for 2-layer, so
        // more layers ask for *more* copper around a hole, not less. A finer
        // process does not make a ring safer to shrink.
        assert!(four.min_annular_ring > two.min_annular_ring);
    }

    #[test]
    fn test_jlcpcb_2layer_all_fields() {
        let rules = DesignRules::jlcpcb_2layer();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.127));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.127));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.3));
        assert_eq!(rules.min_via_drill, Nm::from_mm(0.3));
        assert_eq!(rules.min_annular_ring, Nm::from_mm(0.18));
        assert_eq!(rules.min_silk_width, Nm::from_mm(0.15));
        assert_eq!(rules.min_edge_clearance, Nm::from_mm(0.3));
    }

    #[test]
    fn test_jlcpcb_4layer_all_fields() {
        let rules = DesignRules::jlcpcb_4layer();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.1));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.1));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.2));
        assert_eq!(rules.min_via_drill, Nm::from_mm(0.2));
        assert_eq!(rules.min_annular_ring, Nm::from_mm(0.2));
        assert_eq!(rules.min_silk_width, Nm::from_mm(0.15));
        assert_eq!(rules.min_edge_clearance, Nm::from_mm(0.25));
    }

    #[test]
    fn test_jlcpcb_advanced_2layer_values() {
        let rules = DesignRules::jlcpcb_advanced_2layer();
        // 0.10mm, not 0.09mm: the page publishes 3.5mil for multilayer and
        // 4mil for one and two layers, and this is a two-layer table.
        assert_eq!(rules.min_clearance, Nm::from_mm(0.1));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.1));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.15));
        assert_eq!(rules.min_via_drill, Nm::from_mm(0.15));
        assert_eq!(rules.min_annular_ring, Nm::from_mm(0.2));
        // 0.15mm is what the page publishes for silkscreen line width, on
        // every tier, because there is only one.
        assert_eq!(rules.min_silk_width, Nm::from_mm(0.15));
        assert_eq!(rules.min_edge_clearance, Nm::from_mm(0.2));
    }

    #[test]
    fn test_jlcpcb_advanced_4layer_values() {
        let rules = DesignRules::jlcpcb_advanced_4layer();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.09));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.09));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.15));
    }

    #[test]
    fn test_jlcpcb_advanced_tighter_than_standard() {
        let std = DesignRules::jlcpcb_2layer();
        let adv = DesignRules::jlcpcb_advanced_2layer();
        assert!(adv.min_clearance < std.min_clearance);
        assert!(adv.min_trace_width < std.min_trace_width);
        assert!(adv.min_drill_size < std.min_drill_size);
        assert!(adv.min_edge_clearance < std.min_edge_clearance);
        // Not the silkscreen either, and for the same reason as the ring: one
        // published figure, no tier under it. Both tables carry 0.15mm.
        assert_eq!(adv.min_silk_width, std.min_silk_width);

        // Not the ring. JLCPCB publishes one PTH annular ring per layer count
        // and copper weight, and no process tier under it - so the advanced
        // table carries the same 0.20mm the multilayer standard does, which is
        // larger than 2-layer's 0.18mm rather than smaller.
        assert!(adv.min_annular_ring > std.min_annular_ring);
    }
}
