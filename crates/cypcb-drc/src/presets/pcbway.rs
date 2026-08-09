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
    /// These rules represent PCBWay's recommended minimums for standard
    /// pricing. While they can achieve 3mil in some cases, 6mil is recommended
    /// for reliable results.
    ///
    /// # Specifications
    ///
    /// | Parameter | Value | Notes |
    /// |-----------|-------|-------|
    /// | Min clearance | 0.15mm (6 mil) | Recommended |
    /// | Min trace width | 0.15mm (6 mil) | Recommended |
    /// | Min drill | 0.2mm | Mechanical drilling |
    /// | Min via drill | 0.2mm | Via holes |
    /// | Min annular ring | 0.15mm (6 mil) | Copper around drill |
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
    /// assert_eq!(rules.min_clearance, Nm::from_mm(0.15));
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
        assert_eq!(rules.min_clearance, Nm::from_mm(0.15));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.15));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.2));
        assert_eq!(rules.min_via_drill, Nm::from_mm(0.2));
        assert_eq!(rules.min_annular_ring, Nm::from_mm(0.15));
        assert_eq!(rules.min_silk_width, Nm::from_mm(0.22));
        assert_eq!(rules.min_edge_clearance, Nm::from_mm(0.3));
    }

    #[test]
    fn test_pcbway_wider_silk() {
        // PCBWay requires wider silkscreen than JLCPCB
        let jlcpcb = DesignRules::jlcpcb_2layer();
        let pcbway = DesignRules::pcbway_standard();
        assert!(pcbway.min_silk_width > jlcpcb.min_silk_width);
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
