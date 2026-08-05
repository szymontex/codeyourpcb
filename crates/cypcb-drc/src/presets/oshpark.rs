//! OSHPark manufacturer design rules.
//!
//! OSHPark is a US-based PCB fabrication service known for purple solder mask,
//! ENIG finish, and shared-panel pricing. More conservative design rules than
//! budget Asian fabs, but high quality and excellent for hobby/prototype boards.
//!
//! # Sources
//!
//! - <https://docs.oshpark.com/design-tools/>
//! - <https://docs.oshpark.com/services/>

use super::DesignRules;
use cypcb_rules::presets::RulesPreset;

impl DesignRules {
    /// OSHPark 2-layer board rules.
    ///
    /// # Specifications
    ///
    /// | Parameter | Value | Notes |
    /// |-----------|-------|-------|
    /// | Min clearance | 0.15mm (6 mil) | Standard |
    /// | Min trace width | 0.15mm (6 mil) | Standard |
    /// | Min drill | 0.254mm (10 mil) | Larger than JLCPCB |
    /// | Min via drill | 0.254mm (10 mil) | Same as mechanical drill |
    /// | Min annular ring | 0.127mm (5 mil) | |
    /// | Min silk width | 0.127mm (5 mil) | |
    /// | Min edge clearance | 0.381mm (15 mil) | More conservative |
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_drc::DesignRules;
    /// use cypcb_core::Nm;
    ///
    /// let rules = DesignRules::oshpark_2layer();
    /// assert_eq!(rules.min_clearance, Nm::from_mm(0.15));
    /// assert_eq!(rules.min_drill_size, Nm::from_mm(0.254));
    /// ```
    pub fn oshpark_2layer() -> Self {
        Self::from_constraints(&RulesPreset::OshPark2Layer.constraints())
    }

    /// OSHPark 4-layer board rules.
    ///
    /// Tighter trace/space than 2-layer. Controlled impedance available.
    ///
    /// # Specifications
    ///
    /// | Parameter | Value | Notes |
    /// |-----------|-------|-------|
    /// | Min clearance | 0.127mm (5 mil) | Tighter than 2L |
    /// | Min trace width | 0.127mm (5 mil) | Tighter than 2L |
    /// | Min drill | 0.254mm (10 mil) | Same as 2L |
    /// | Min via drill | 0.254mm (10 mil) | Same as 2L |
    /// | Min annular ring | 0.1mm (4 mil) | Tighter than 2L |
    /// | Min silk width | 0.127mm (5 mil) | Same as 2L |
    /// | Min edge clearance | 0.381mm (15 mil) | Same as 2L |
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_drc::DesignRules;
    /// use cypcb_core::Nm;
    ///
    /// let rules = DesignRules::oshpark_4layer();
    /// assert_eq!(rules.min_clearance, Nm::from_mm(0.127));
    /// assert_eq!(rules.min_drill_size, Nm::from_mm(0.254));
    /// ```
    pub fn oshpark_4layer() -> Self {
        Self::from_constraints(&RulesPreset::OshPark4Layer.constraints())
    }

    /// JLCPCB advanced 2-layer board rules.
    ///
    /// Advanced process with tighter tolerances. Higher cost, longer lead time.
    ///
    /// # Specifications
    ///
    /// | Parameter | Value | Notes |
    /// |-----------|-------|-------|
    /// | Min clearance | 0.09mm (3.5 mil) | Advanced process |
    /// | Min trace width | 0.09mm (3.5 mil) | Advanced process |
    /// | Min drill | 0.15mm | Micro-drill capable |
    /// | Min via drill | 0.15mm | Micro-via |
    /// | Min annular ring | 0.1mm (4 mil) | |
    /// | Min silk width | 0.1mm | |
    /// | Min edge clearance | 0.2mm | |
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_drc::DesignRules;
    /// use cypcb_core::Nm;
    ///
    /// let rules = DesignRules::jlcpcb_advanced_2layer();
    /// assert_eq!(rules.min_clearance, Nm::from_mm(0.09));
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
    /// | Min annular ring | 0.1mm (4 mil) | |
    /// | Min silk width | 0.1mm | |
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
    fn test_oshpark_2layer_values() {
        let rules = DesignRules::oshpark_2layer();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.15));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.15));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.254));
        assert_eq!(rules.min_via_drill, Nm::from_mm(0.254));
        assert_eq!(rules.min_annular_ring, Nm::from_mm(0.127));
        assert_eq!(rules.min_silk_width, Nm::from_mm(0.127));
        assert_eq!(rules.min_edge_clearance, Nm::from_mm(0.381));
    }

    #[test]
    fn test_oshpark_4layer_values() {
        let rules = DesignRules::oshpark_4layer();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.127));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.127));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.254));
        assert_eq!(rules.min_annular_ring, Nm::from_mm(0.1));
    }

    #[test]
    fn test_oshpark_4layer_tighter_than_2layer() {
        let two = DesignRules::oshpark_2layer();
        let four = DesignRules::oshpark_4layer();
        assert!(four.min_clearance < two.min_clearance);
        assert!(four.min_trace_width < two.min_trace_width);
        assert!(four.min_annular_ring < two.min_annular_ring);
    }

    #[test]
    fn test_oshpark_vs_jlcpcb_drills() {
        let oshpark = DesignRules::oshpark_2layer();
        let jlcpcb = DesignRules::jlcpcb_2layer();
        // OSHPark 10mil (0.254mm) is smaller than JLCPCB's 0.3mm mechanical drill
        // but both are valid manufacturer minimums
        assert_eq!(oshpark.min_drill_size, Nm::from_mm(0.254));
        assert_eq!(jlcpcb.min_drill_size, Nm::from_mm(0.3));
    }

    #[test]
    fn test_oshpark_wider_edge_clearance() {
        let oshpark = DesignRules::oshpark_2layer();
        let jlcpcb = DesignRules::jlcpcb_2layer();
        assert!(oshpark.min_edge_clearance > jlcpcb.min_edge_clearance);
    }

    #[test]
    fn test_jlcpcb_advanced_2layer_values() {
        let rules = DesignRules::jlcpcb_advanced_2layer();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.09));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.09));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.15));
        assert_eq!(rules.min_via_drill, Nm::from_mm(0.15));
        assert_eq!(rules.min_annular_ring, Nm::from_mm(0.1));
        assert_eq!(rules.min_silk_width, Nm::from_mm(0.1));
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
        assert!(adv.min_annular_ring < std.min_annular_ring);
        assert!(adv.min_edge_clearance < std.min_edge_clearance);
    }
}
