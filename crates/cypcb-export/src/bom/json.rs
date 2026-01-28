//! BOM JSON export with metadata.
//!
//! Exports Bill of Materials as structured JSON with additional metadata
//! like board name, export date, and total component counts.

use crate::bom::{group_components, BomEntry};
use cypcb_world::BoardWorld;
use serde::{Deserialize, Serialize};
use chrono::Utc;

/// Complete BOM document with metadata.
///
/// Wraps the BOM entries with contextual information about the board
/// and export operation.
#[derive(Debug, Serialize, Deserialize)]
pub struct BomDocument {
    /// Metadata about the board and export.
    pub metadata: BomMetadata,
    /// List of BOM entries (grouped components).
    pub components: Vec<BomEntry>,
}

/// Metadata for BOM export.
#[derive(Debug, Serialize, Deserialize)]
pub struct BomMetadata {
    /// Name of the board.
    pub board_name: String,
    /// ISO 8601 timestamp of export.
    pub export_date: String,
    /// Total number of unique component types.
    pub unique_components: u32,
    /// Total number of individual components.
    pub total_components: u32,
}

/// Export BOM as JSON string with metadata.
///
/// Generates a structured JSON document containing the BOM entries plus
/// metadata about the board and export operation.
///
/// # Arguments
///
/// * `world` - The board world containing all components
/// * `board_name` - Optional board name (defaults to "board")
///
/// # Returns
///
/// A pretty-printed JSON string, or an error if serialization fails.
///
/// # Examples
///
/// ```
/// use cypcb_export::bom::json::export_bom_json;
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
///
/// let json = export_bom_json(&mut world, Some("TestBoard")).unwrap();
/// assert!(json.contains("TestBoard"));
/// assert!(json.contains("\"unique_components\": 1"));
/// assert!(json.contains("\"total_components\": 1"));
/// ```
pub fn export_bom_json(
    world: &mut BoardWorld,
    board_name: Option<&str>,
) -> Result<String, serde_json::Error> {
    let bom = group_components(world);

    let total_components: u32 = bom.iter().map(|e| e.quantity).sum();
    let unique_components = bom.len() as u32;

    let doc = BomDocument {
        metadata: BomMetadata {
            board_name: board_name.unwrap_or("board").to_string(),
            export_date: Utc::now().to_rfc3339(),
            unique_components,
            total_components,
        },
        components: bom,
    };

    serde_json::to_string_pretty(&doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_world::{RefDes, Value, Position, Rotation, FootprintRef, NetConnections};

    #[test]
    fn test_export_bom_json_empty() {
        let mut world = BoardWorld::new();
        let json = export_bom_json(&mut world, Some("TestBoard")).unwrap();

        // Parse to verify structure
        let doc: BomDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(doc.metadata.board_name, "TestBoard");
        assert_eq!(doc.metadata.unique_components, 0);
        assert_eq!(doc.metadata.total_components, 0);
        assert_eq!(doc.components.len(), 0);
    }

    #[test]
    fn test_export_bom_json_single() {
        let mut world = BoardWorld::new();
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let json = export_bom_json(&mut world, Some("TestBoard")).unwrap();

        let doc: BomDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(doc.metadata.unique_components, 1);
        assert_eq!(doc.metadata.total_components, 1);
        assert_eq!(doc.components.len(), 1);
        assert_eq!(doc.components[0].designators, vec!["R1"]);
        assert_eq!(doc.components[0].value, "10k");
    }

    #[test]
    fn test_export_bom_json_grouped() {
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

        let json = export_bom_json(&mut world, Some("TestBoard")).unwrap();

        let doc: BomDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(doc.metadata.unique_components, 1);
        assert_eq!(doc.metadata.total_components, 2);
        assert_eq!(doc.components[0].quantity, 2);
        assert_eq!(doc.components[0].designators, vec!["R1", "R2"]);
    }

    #[test]
    fn test_export_bom_json_multiple_types() {
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
            RefDes::new("C1"),
            Value::new("100nF"),
            Position::from_mm(30.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let json = export_bom_json(&mut world, Some("TestBoard")).unwrap();

        let doc: BomDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(doc.metadata.unique_components, 2);
        assert_eq!(doc.metadata.total_components, 3);
        assert_eq!(doc.components.len(), 2);
    }

    #[test]
    fn test_json_contains_timestamp() {
        let mut world = BoardWorld::new();
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let json = export_bom_json(&mut world, None).unwrap();

        // Should contain ISO 8601 timestamp
        assert!(json.contains("export_date"));
        assert!(json.contains("T")); // ISO format separator
        // RFC3339 can use either "Z" or "+00:00" for UTC
        assert!(json.contains("Z") || json.contains("+00:00"));
    }

    #[test]
    fn test_json_default_board_name() {
        let mut world = BoardWorld::new();
        let json = export_bom_json(&mut world, None).unwrap();

        let doc: BomDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(doc.metadata.board_name, "board");
    }
}
