//! BOM CSV export in JLCPCB format.
//!
//! Exports Bill of Materials as CSV files compatible with JLCPCB and
//! other PCB assembly services. Follows standard column naming conventions.

use crate::bom::{group_components, BomEntry};
use cypcb_world::BoardWorld;
use serde::Serialize;

/// CSV row format matching JLCPCB BOM requirements.
///
/// JLCPCB expects:
/// - "Designator": Comma-separated list of component references (e.g., "R1,R2,R3")
/// - "Footprint": Footprint name (e.g., "0402")
/// - "Quantity": Number of components
/// - "Comment": Component value goes here per JLCPCB convention
#[derive(Debug, Serialize)]
struct BomCsvRow {
    #[serde(rename = "Designator")]
    designator: String,

    #[serde(rename = "Footprint")]
    footprint: String,

    #[serde(rename = "Quantity")]
    quantity: u32,

    #[serde(rename = "Comment")]
    comment: String,
}

impl From<&BomEntry> for BomCsvRow {
    fn from(entry: &BomEntry) -> Self {
        BomCsvRow {
            designator: entry.designators.join(","),
            footprint: entry.footprint.clone(),
            quantity: entry.quantity,
            comment: entry.comment.clone().unwrap_or_else(|| entry.value.clone()),
        }
    }
}

/// Export BOM as CSV string in JLCPCB format.
///
/// Generates a CSV file with headers: Designator, Footprint, Quantity, Comment.
/// Components are grouped by value and footprint, with designators comma-separated.
///
/// # Arguments
///
/// * `world` - The board world containing all components
///
/// # Returns
///
/// A CSV string ready to be written to a file, or an error if export fails.
///
/// # Examples
///
/// ```
/// use cypcb_export::bom::csv::export_bom_csv;
/// use cypcb_world::{BoardWorld, RefDes, Value, Position, Rotation, FootprintRef, NetConnections};
/// use cypcb_core::Nm;
///
/// let mut world = BoardWorld::new();
/// world.spawn_component(
///     RefDes::new("R1"),
///     Value::new("10k"),
///     Position::from_mm(10.0, 10.0),
///     Rotation::ZERO,
///     FootprintRef::new("0402"),
///     NetConnections::new(),
/// );
/// world.spawn_component(
///     RefDes::new("R2"),
///     Value::new("10k"),
///     Position::from_mm(20.0, 10.0),
///     Rotation::ZERO,
///     FootprintRef::new("0402"),
///     NetConnections::new(),
/// );
///
/// let csv = export_bom_csv(&mut world).unwrap();
/// assert!(csv.contains("Designator,Footprint,Quantity,Comment"));
/// assert!(csv.contains("R1,R2"));
/// ```
pub fn export_bom_csv(world: &mut BoardWorld) -> Result<String, Box<dyn std::error::Error>> {
    let bom = group_components(world);

    let mut wtr = csv::Writer::from_writer(vec![]);

    // Write all rows
    for entry in &bom {
        let row = BomCsvRow::from(entry);
        wtr.serialize(row)?;
    }

    wtr.flush()?;

    let data = wtr.into_inner()?;
    let csv_string = String::from_utf8(data)?;

    Ok(csv_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_world::{RefDes, Value, Position, Rotation, FootprintRef, NetConnections};

    #[test]
    fn test_export_bom_csv_empty() {
        let mut world = BoardWorld::new();
        let csv = export_bom_csv(&mut world).unwrap();

        // Empty BOM produces empty CSV (csv crate doesn't write headers without data)
        assert_eq!(csv.trim(), "");
    }

    #[test]
    fn test_export_bom_csv_single() {
        let mut world = BoardWorld::new();
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let csv = export_bom_csv(&mut world).unwrap();
        assert!(csv.contains("Designator,Footprint,Quantity,Comment"));
        assert!(csv.contains("R1,0402,1,10k"));
    }

    #[test]
    fn test_export_bom_csv_grouped() {
        let mut world = BoardWorld::new();
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );
        world.spawn_component(
            RefDes::new("R2"),
            Value::new("10k"),
            Position::from_mm(20.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );
        world.spawn_component(
            RefDes::new("R3"),
            Value::new("10k"),
            Position::from_mm(30.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let csv = export_bom_csv(&mut world).unwrap();

        // Should have single row with comma-separated designators
        assert!(csv.contains("R1,R2,R3"));
        assert!(csv.contains("0402"));
        assert!(csv.contains("3")); // Quantity
        assert!(csv.contains("10k")); // Comment
    }

    #[test]
    fn test_export_bom_csv_multiple_groups() {
        let mut world = BoardWorld::new();
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );
        world.spawn_component(
            RefDes::new("R2"),
            Value::new("100k"),
            Position::from_mm(20.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );
        world.spawn_component(
            RefDes::new("C1"),
            Value::new("100nF"),
            Position::from_mm(30.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let csv = export_bom_csv(&mut world).unwrap();

        // Should have 3 data rows (+ 1 header)
        let lines: Vec<&str> = csv.trim().lines().collect();
        assert_eq!(lines.len(), 4);

        assert!(csv.contains("R1,0402,1,10k"));
        assert!(csv.contains("R2,0402,1,100k"));
        assert!(csv.contains("C1,0402,1,100nF"));
    }

    #[test]
    fn test_csv_column_headers() {
        let mut world = BoardWorld::new();
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let csv = export_bom_csv(&mut world).unwrap();
        let lines: Vec<&str> = csv.lines().collect();

        // First line should be header
        assert_eq!(lines[0], "Designator,Footprint,Quantity,Comment");
    }
}
