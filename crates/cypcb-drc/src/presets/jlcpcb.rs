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
use cypcb_core::Nm;

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
    /// | Min clearance | 0.15mm (6 mil) | Standard tolerance |
    /// | Min trace width | 0.15mm (6 mil) | Standard tolerance |
    /// | Min drill | 0.3mm | Mechanical drilling |
    /// | Min via drill | 0.2mm | Via holes |
    /// | Min annular ring | 0.15mm (6 mil) | Copper around drill |
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
    /// assert_eq!(rules.min_clearance, Nm::from_mm(0.15));
    /// assert_eq!(rules.min_drill_size, Nm::from_mm(0.3));
    /// ```
    pub fn jlcpcb_2layer() -> Self {
        DesignRules {
            min_clearance: Nm::from_mm(0.15),       // 6 mil
            min_trace_width: Nm::from_mm(0.15),     // 6 mil
            min_drill_size: Nm::from_mm(0.3),       // 0.3mm mechanical
            min_via_drill: Nm::from_mm(0.2),        // 0.2mm via
            min_annular_ring: Nm::from_mm(0.15),    // 6 mil
            min_silk_width: Nm::from_mm(0.15),      // 6 mil
            min_edge_clearance: Nm::from_mm(0.3),   // 0.3mm
        }
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
    /// | Min annular ring | 0.125mm (5 mil) | Tighter tolerance |
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
        DesignRules {
            min_clearance: Nm::from_mm(0.1),        // 4 mil
            min_trace_width: Nm::from_mm(0.1),      // 4 mil
            min_drill_size: Nm::from_mm(0.2),       // 0.2mm
            min_via_drill: Nm::from_mm(0.2),
            min_annular_ring: Nm::from_mm(0.125),   // 5 mil
            min_silk_width: Nm::from_mm(0.15),
            min_edge_clearance: Nm::from_mm(0.25),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jlcpcb_2layer_clearance() {
        let rules = DesignRules::jlcpcb_2layer();
        // 6 mil = 0.1524mm, rounded to 0.15mm
        assert_eq!(rules.min_clearance, Nm::from_mm(0.15));
    }

    #[test]
    fn test_jlcpcb_2layer_drill() {
        let rules = DesignRules::jlcpcb_2layer();
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.3));
        assert_eq!(rules.min_via_drill, Nm::from_mm(0.2));
    }

    #[test]
    fn test_jlcpcb_4layer_tighter() {
        let two = DesignRules::jlcpcb_2layer();
        let four = DesignRules::jlcpcb_4layer();

        // 4-layer should have tighter (smaller) minimums
        assert!(four.min_clearance < two.min_clearance);
        assert!(four.min_trace_width < two.min_trace_width);
        assert!(four.min_drill_size < two.min_drill_size);
        assert!(four.min_annular_ring < two.min_annular_ring);
        assert!(four.min_edge_clearance < two.min_edge_clearance);
    }

    #[test]
    fn test_jlcpcb_2layer_all_fields() {
        let rules = DesignRules::jlcpcb_2layer();
        assert_eq!(rules.min_clearance, Nm::from_mm(0.15));
        assert_eq!(rules.min_trace_width, Nm::from_mm(0.15));
        assert_eq!(rules.min_drill_size, Nm::from_mm(0.3));
        assert_eq!(rules.min_via_drill, Nm::from_mm(0.2));
        assert_eq!(rules.min_annular_ring, Nm::from_mm(0.15));
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
        assert_eq!(rules.min_annular_ring, Nm::from_mm(0.125));
        assert_eq!(rules.min_silk_width, Nm::from_mm(0.15));
        assert_eq!(rules.min_edge_clearance, Nm::from_mm(0.25));
    }
}
