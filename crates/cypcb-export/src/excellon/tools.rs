//! Tool table management for Excellon drill files.
//!
//! Maps drill diameters to tool numbers and generates tool definition headers.

use std::collections::HashMap;

use cypcb_core::Nm;

use crate::coords::{CoordinateFormat, nm_to_gerber};

/// Common drill sizes as constants.

/// Default via drill diameter (0.3mm).
pub const VIA_DRILL_DEFAULT: Nm = Nm(300_000);

/// Small through-hole drill (0.8mm).
pub const THT_DRILL_SMALL: Nm = Nm(800_000);

/// Large through-hole drill (1.0mm).
pub const THT_DRILL_LARGE: Nm = Nm(1_000_000);

/// Tool table mapping drill diameters to tool numbers.
///
/// The tool table manages the assignment of tool numbers (T1, T2, etc.)
/// to unique drill sizes. Tool numbers start at 1 and are assigned
/// sequentially as new drill sizes are encountered.
///
/// # Example
///
/// ```
/// use cypcb_export::excellon::ToolTable;
/// use cypcb_export::coords::CoordinateFormat;
/// use cypcb_core::Nm;
///
/// let mut table = ToolTable::new();
///
/// // First drill size gets T1
/// let t1 = table.get_or_create(Nm::from_mm(0.3));
/// assert_eq!(t1, 1);
///
/// // Same size reuses T1
/// let t1_again = table.get_or_create(Nm::from_mm(0.3));
/// assert_eq!(t1_again, 1);
///
/// // Different size gets T2
/// let t2 = table.get_or_create(Nm::from_mm(0.8));
/// assert_eq!(t2, 2);
///
/// assert_eq!(table.tool_count(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct ToolTable {
    /// Map from drill diameter to tool number.
    tools: HashMap<Nm, u8>,
    /// Next available tool number (starts at 1).
    next_tool: u8,
}

impl ToolTable {
    /// Create a new empty tool table.
    pub fn new() -> Self {
        ToolTable {
            tools: HashMap::new(),
            next_tool: 1,
        }
    }

    /// Get or create a tool number for a drill diameter.
    ///
    /// Returns the tool number for the given drill size. If the drill size
    /// hasn't been seen before, assigns a new tool number.
    ///
    /// # Arguments
    ///
    /// * `drill_diameter` - The drill diameter in nanometers
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_export::excellon::ToolTable;
    /// use cypcb_core::Nm;
    ///
    /// let mut table = ToolTable::new();
    ///
    /// let t1 = table.get_or_create(Nm::from_mm(0.3));
    /// assert_eq!(t1, 1);
    ///
    /// let t2 = table.get_or_create(Nm::from_mm(0.8));
    /// assert_eq!(t2, 2);
    ///
    /// // Reusing drill size returns same tool
    /// let t1_again = table.get_or_create(Nm::from_mm(0.3));
    /// assert_eq!(t1_again, 1);
    /// ```
    pub fn get_or_create(&mut self, drill_diameter: Nm) -> u8 {
        if let Some(&tool) = self.tools.get(&drill_diameter) {
            tool
        } else {
            let tool = self.next_tool;
            self.tools.insert(drill_diameter, tool);
            self.next_tool += 1;
            tool
        }
    }

    /// Get the number of tools in the table.
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Generate Excellon tool definition header lines.
    ///
    /// Returns tool definition lines sorted by tool number.
    /// Format: T{n}C{diameter}
    ///
    /// # Arguments
    ///
    /// * `format` - Coordinate format for drill diameter output
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_export::excellon::ToolTable;
    /// use cypcb_export::coords::CoordinateFormat;
    /// use cypcb_core::Nm;
    ///
    /// let mut table = ToolTable::new();
    /// table.get_or_create(Nm::from_mm(0.3));
    /// table.get_or_create(Nm::from_mm(0.8));
    ///
    /// let header = table.to_header(&CoordinateFormat::FORMAT_MM_2_6);
    /// assert!(header.contains("T1C0.300000"));
    /// assert!(header.contains("T2C0.800000"));
    /// ```
    pub fn to_header(&self, format: &CoordinateFormat) -> String {
        // Collect and sort tools by tool number
        let mut tools: Vec<_> = self.tools.iter().collect();
        tools.sort_by_key(|(_, &tool_num)| tool_num);

        let mut lines = Vec::new();
        for (drill_diameter, &tool_num) in tools {
            let diameter_str = nm_to_gerber(drill_diameter.0, format);
            lines.push(format!("T{}C{}", tool_num, diameter_str));
        }

        lines.join("\n")
    }
}

impl Default for ToolTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_table_creation() {
        let table = ToolTable::new();
        assert_eq!(table.tool_count(), 0);
    }

    #[test]
    fn test_tool_assignment() {
        let mut table = ToolTable::new();

        let t1 = table.get_or_create(Nm::from_mm(0.3));
        assert_eq!(t1, 1);

        let t2 = table.get_or_create(Nm::from_mm(0.8));
        assert_eq!(t2, 2);

        assert_eq!(table.tool_count(), 2);
    }

    #[test]
    fn test_tool_reuse() {
        let mut table = ToolTable::new();

        let t1 = table.get_or_create(Nm::from_mm(0.3));
        let t1_again = table.get_or_create(Nm::from_mm(0.3));

        assert_eq!(t1, t1_again);
        assert_eq!(table.tool_count(), 1);
    }

    #[test]
    fn test_header_generation_metric() {
        let mut table = ToolTable::new();
        table.get_or_create(Nm::from_mm(0.3));
        table.get_or_create(Nm::from_mm(0.8));

        let format = CoordinateFormat::FORMAT_MM_2_6;
        let header = table.to_header(&format);

        assert!(header.contains("T1C0.300000"));
        assert!(header.contains("T2C0.800000"));
    }

    #[test]
    fn test_header_generation_precision() {
        let mut table = ToolTable::new();
        table.get_or_create(Nm::from_mm(0.3));

        let format = CoordinateFormat::FORMAT_MM_2_6;
        let header = table.to_header(&format);

        // Should have 6 decimal places
        assert!(header.contains("0.300000"));
    }

    #[test]
    fn test_header_generation_sorting() {
        let mut table = ToolTable::new();
        table.get_or_create(Nm::from_mm(1.0));
        table.get_or_create(Nm::from_mm(0.3));
        table.get_or_create(Nm::from_mm(0.8));

        let format = CoordinateFormat::FORMAT_MM_2_6;
        let header = table.to_header(&format);

        let lines: Vec<&str> = header.lines().collect();
        assert_eq!(lines.len(), 3);

        // Lines should be sorted by tool number
        assert!(lines[0].starts_with("T1"));
        assert!(lines[1].starts_with("T2"));
        assert!(lines[2].starts_with("T3"));
    }

    #[test]
    fn test_empty_table_header() {
        let table = ToolTable::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;
        let header = table.to_header(&format);

        assert_eq!(header, "");
    }

    #[test]
    fn test_via_drill_constant() {
        assert_eq!(VIA_DRILL_DEFAULT, Nm::from_mm(0.3));
    }

    #[test]
    fn test_tht_drill_constants() {
        assert_eq!(THT_DRILL_SMALL, Nm::from_mm(0.8));
        assert_eq!(THT_DRILL_LARGE, Nm::from_mm(1.0));
    }
}
