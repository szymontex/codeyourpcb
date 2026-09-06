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
    /// | Min clearance | 0.1524mm (6 mil) | Published |
    /// | Min trace width | 0.1524mm (6 mil) | Published |
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
    /// assert_eq!(rules.min_clearance, Nm::from_mil(6.0));
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
    /// | Min annular ring | 0.1016mm (4 mil) | Tighter than 2L |
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::Nm;

    #[test]
    fn test_oshpark_2layer_values() {
        let rules = DesignRules::oshpark_2layer();
        // 6mil, as OSH Park publishes it. These read 0.15mm until the table
        // was checked against its own source: 6mil is 0.1524mm, and the
        // comment beside the value had said "6 mil" all along.
        assert_eq!(rules.min_clearance, Nm::from_mm(0.1524));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.1524));
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
        // 4mil is 0.1016mm. This read 0.1mm, which is looser than the figure
        // its own comment named.
        assert_eq!(rules.min_annular_ring, Nm::from_mm(0.1016));
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
}
