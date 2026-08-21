//! PCBWay manufacturer design rules.
//!
//! PCBWay is a Chinese PCB manufacturer with similar capabilities to JLCPCB.
//! They offer competitive pricing and good quality for both hobbyist and
//! professional projects.
//!
//! # Sources
//!
//! - <https://www.pcbway.com/capabilities.html>

use super::DesignRules;
use cypcb_rules::presets::RulesPreset;

impl DesignRules {
    /// PCBWay standard rules.
    ///
    /// Taken from PCBWay's published capabilities page, read 2026-08-13.
    ///
    /// The paragraph that stood here said the table was PCBWay's recommended
    /// minimums, that 3mil was achievable in some cases and that 6mil was the
    /// figure to design to. None of that is on the page, and the table below
    /// it disagreed with it: the page publishes 0.1mm/4mil as the minimum
    /// trace and the minimum spacing. A capability table says what a house can
    /// make; a margin on top of it is the designer's to choose, and belongs in
    /// a `netclass` rather than in here.
    ///
    /// # Specifications
    ///
    /// | Parameter | Value | Notes |
    /// |-----------|-------|-------|
    /// | Min clearance | 0.1mm (4 mil) | Published |
    /// | Min trace width | 0.1mm (4 mil) | Published |
    /// | Min drill | 0.2mm | Advanced normal process |
    /// | Min via drill | 0.2mm | Advanced normal process |
    /// | Min annular ring | 0.15mm (6 mil) | Published, vias and pads alike |
    /// | Min silk width | 0.22mm (8.66 mil) | Wider than JLCPCB |
    /// | Min edge clearance | 0.3mm | Copper to board edge |
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_drc::DesignRules;
    /// use cypcb_core::Nm;
    ///
    /// let rules = DesignRules::pcbway_standard();
    /// assert_eq!(rules.min_clearance, Nm::from_mm(0.1));
    /// assert_eq!(rules.min_drill_size, Nm::from_mm(0.2));
    /// ```
    pub fn pcbway_standard() -> Self {
        Self::from_constraints(&RulesPreset::PcbWayStandard.constraints())
    }

    /// Relaxed rules for prototyping.
    ///
    /// These rules provide larger margins for hand-soldering, beginner designs,
    /// or when using lower-quality fabrication services. Using larger minimums
    /// increases yield and reduces manufacturing issues.
    ///
    /// # Specifications
    ///
    /// | Parameter | Value | Notes |
    /// |-----------|-------|-------|
    /// | Min clearance | 0.2mm (8 mil) | Conservative |
    /// | Min trace width | 0.25mm (10 mil) | Easier soldering |
    /// | Min drill | 0.4mm | Larger holes |
    /// | Min via drill | 0.3mm | Larger vias |
    /// | Min annular ring | 0.2mm (8 mil) | More copper |
    /// | Min silk width | 0.2mm (8 mil) | Readable text |
    /// | Min edge clearance | 0.5mm | Safe margin |
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_drc::DesignRules;
    /// use cypcb_core::Nm;
    ///
    /// let rules = DesignRules::prototype();
    /// assert_eq!(rules.min_clearance, Nm::from_mm(0.2));
    /// assert_eq!(rules.min_trace_width, Nm::from_mm(0.25));
    /// ```
    pub fn prototype() -> Self {
        // The last hand-written copy of a preset's numbers, and it is gone:
        // `prototype` moved into the shared table when the two preset
        // registries were merged, and `prototype_kept_every_number_it_had`
        // proves the move changed nothing.
        Self::from_constraints(&RulesPreset::Prototype.constraints())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::Nm;

    #[test]
    fn test_pcbway_standard_values() {
        let rules = DesignRules::pcbway_standard();
        // 0.1mm/4mil, which is what PCBWay publishes. These read 0.15mm and
        // were commented "recommended" - an unsourced margin sitting in a
        // table that says what a house can make.
        assert_eq!(rules.min_clearance, Nm::from_mm(0.1));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.1));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.2));
        assert_eq!(rules.min_via_drill, Nm::from_mm(0.2));
        assert_eq!(rules.min_annular_ring, Nm::from_mm(0.15));
        // 0.15mm and 0.25mm, likewise published. These read 0.22mm and
        // 0.3mm, both margins on top of the page rather than the page.
        assert_eq!(rules.min_silk_width, Nm::from_mm(0.15));
        assert_eq!(rules.min_edge_clearance, Nm::from_mm(0.25));
    }

    #[test]
    fn test_pcbway_and_jlcpcb_publish_the_same_silk() {
        // This test used to assert that PCBWay needs a wider legend than
        // JLCPCB, which was true only while this table carried 0.22mm - a
        // margin nobody sourced. Both pages publish 0.15mm, read 2026-08-21,
        // so the difference was the table's and not the houses'.
        let jlcpcb = DesignRules::jlcpcb_2layer();
        let pcbway = DesignRules::pcbway_standard();
        assert_eq!(pcbway.min_silk_width, Nm::from_mm(0.15));
        assert_eq!(pcbway.min_silk_width, jlcpcb.min_silk_width);
    }

    #[test]
    fn test_pcbway_smaller_drill() {
        // PCBWay allows smaller drills than JLCPCB 2-layer
        let jlcpcb = DesignRules::jlcpcb_2layer();
        let pcbway = DesignRules::pcbway_standard();
        assert!(pcbway.min_drill_size < jlcpcb.min_drill_size);
    }

    #[test]
    fn test_prototype_relaxed() {
        let proto = DesignRules::prototype();
        let jlcpcb = DesignRules::jlcpcb_2layer();

        // Prototype should have larger (more relaxed) minimums. Via drill is the
        // one exception: JLCPCB's standard process also wants 0.3mm, so it can
        // only be no tighter, not strictly larger.
        assert!(proto.min_clearance > jlcpcb.min_clearance);
        assert!(proto.min_trace_width > jlcpcb.min_trace_width);
        assert!(proto.min_drill_size > jlcpcb.min_drill_size);
        assert!(proto.min_via_drill >= jlcpcb.min_via_drill);
        assert!(proto.min_annular_ring > jlcpcb.min_annular_ring);
        assert!(proto.min_edge_clearance > jlcpcb.min_edge_clearance);
    }

    #[test]
    fn test_prototype_all_fields() {
        let rules = DesignRules::prototype();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.2));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.25));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.4));
        assert_eq!(rules.min_via_drill, Nm::from_mm(0.3));
        assert_eq!(rules.min_annular_ring, Nm::from_mm(0.2));
        assert_eq!(rules.min_silk_width, Nm::from_mm(0.2));
        assert_eq!(rules.min_edge_clearance, Nm::from_mm(0.5));
    }
}
