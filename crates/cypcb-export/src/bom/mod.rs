//! Bill of Materials (BOM) export functionality.
//!
//! Provides BOM generation in multiple formats (CSV, JSON) with component
//! grouping and consolidation. Used for ordering parts and assembly planning.

pub mod csv;
pub mod json;

pub use csv::export_bom_csv;
pub use json::export_bom_json;

use serde::{Deserialize, Serialize};
use cypcb_world::BoardWorld;
use cypcb_world::components::{RefDes, Value, FootprintRef};
use std::collections::HashMap;

/// A single line in the Bill of Materials.
///
/// Represents a group of identical components (same value and footprint).
/// Designators are collected into a list for traceability.
///
/// # Examples
///
/// ```
/// use cypcb_export::bom::BomEntry;
///
/// let entry = BomEntry {
///     designators: vec!["R1".to_string(), "R2".to_string(), "R3".to_string()],
///     footprint: "0402".to_string(),
///     value: "10k".to_string(),
///     quantity: 3,
///     comment: None,
/// };
///
/// assert_eq!(entry.quantity, 3);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BomEntry {
    /// List of component designators (e.g., ["R1", "R2", "R3"]).
    pub designators: Vec<String>,
    /// Footprint name (e.g., "0402", "SOIC-8").
    pub footprint: String,
    /// Component value (e.g., "10k", "100nF", "ATmega328P").
    pub value: String,
    /// Total quantity of this component type.
    pub quantity: u32,
    /// Optional comment or notes.
    pub comment: Option<String>,
}

/// Group components by value and footprint to create BOM entries.
///
/// Queries all components in the board world and consolidates identical
/// components (same value and footprint) into single BOM entries with
/// multiple designators.
///
/// # Arguments
///
/// * `world` - The board world containing all components
///
/// # Returns
///
/// A vector of BOM entries, sorted by footprint and value for consistency.
///
/// # Examples
///
/// ```
/// use cypcb_export::bom::group_components;
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
/// let bom = group_components(&mut world);
/// assert_eq!(bom.len(), 1);
/// assert_eq!(bom[0].quantity, 2);
/// assert_eq!(bom[0].designators, vec!["R1", "R2"]);
/// ```
pub fn group_components(world: &mut BoardWorld) -> Vec<BomEntry> {
    // Map: (value, footprint) -> Vec<designator>
    let mut groups: HashMap<(String, String), Vec<String>> = HashMap::new();

    // Query all components
    let mut query = world.ecs_mut().query::<(&RefDes, &Value, &FootprintRef)>();

    for (refdes, value, footprint) in query.iter(world.ecs()) {
        let key = (value.0.clone(), footprint.0.clone());
        groups.entry(key).or_insert_with(Vec::new).push(refdes.0.clone());
    }

    // Convert groups to BOM entries
    let mut bom: Vec<BomEntry> = groups
        .into_iter()
        .map(|((value, footprint), mut designators)| {
            // Sort designators for consistent output
            designators.sort_by(|a, b| natural_sort_key(a).cmp(&natural_sort_key(b)));

            let quantity = designators.len() as u32;

            BomEntry {
                designators,
                footprint: footprint.clone(),
                value: value.clone(),
                quantity,
                comment: None,
            }
        })
        .collect();

    // Sort by footprint, then value for consistent output
    bom.sort_by(|a, b| {
        a.footprint.cmp(&b.footprint)
            .then_with(|| a.value.cmp(&b.value))
    });

    bom
}

/// Natural sort key for component designators.
///
/// Extracts prefix and number separately so that "R1", "R2", "R10" sorts
/// correctly (not as "R1", "R10", "R2" which lexical sort would give).
///
/// Returns (prefix, number) tuple for comparison.
fn natural_sort_key(refdes: &str) -> (String, u32) {
    let end = refdes.find(|c: char| c.is_ascii_digit()).unwrap_or(refdes.len());
    let prefix = refdes[..end].to_string();
    let number = refdes[end..].parse::<u32>().unwrap_or(0);
    (prefix, number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_world::{Position, Rotation, NetConnections};

    #[test]
    fn test_natural_sort_key() {
        assert_eq!(natural_sort_key("R1"), ("R".to_string(), 1));
        assert_eq!(natural_sort_key("R10"), ("R".to_string(), 10));
        assert_eq!(natural_sort_key("LED2"), ("LED".to_string(), 2));
        assert_eq!(natural_sort_key("U100"), ("U".to_string(), 100));
    }

    #[test]
    fn test_natural_sort_order() {
        let mut designators = vec!["R10", "R2", "R1", "R100"];
        designators.sort_by(|a, b| natural_sort_key(a).cmp(&natural_sort_key(b)));
        assert_eq!(designators, vec!["R1", "R2", "R10", "R100"]);
    }

    #[test]
    fn test_group_components_empty() {
        let mut world = BoardWorld::new();
        let bom = group_components(&mut world);
        assert_eq!(bom.len(), 0);
    }

    #[test]
    fn test_group_components_single() {
        let mut world = BoardWorld::new();
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let bom = group_components(&mut world);
        assert_eq!(bom.len(), 1);
        assert_eq!(bom[0].quantity, 1);
        assert_eq!(bom[0].designators, vec!["R1"]);
        assert_eq!(bom[0].value, "10k");
        assert_eq!(bom[0].footprint, "0402");
    }

    #[test]
    fn test_group_components_identical() {
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
            RefDes::new("R10"),
            Value::new("10k"),
            Position::from_mm(30.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let bom = group_components(&mut world);
        assert_eq!(bom.len(), 1);
        assert_eq!(bom[0].quantity, 3);
        assert_eq!(bom[0].designators, vec!["R1", "R2", "R10"]); // Natural sort
    }

    #[test]
    fn test_group_components_different() {
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

        let bom = group_components(&mut world);
        assert_eq!(bom.len(), 3);
        // Each component is unique
        for entry in &bom {
            assert_eq!(entry.quantity, 1);
        }
    }

    #[test]
    fn test_group_components_sorting() {
        let mut world = BoardWorld::new();
        // Add components in random order
        world.spawn_component(
            RefDes::new("U1"),
            Value::new("ATmega328P"),
            Position::from_mm(10.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("SOIC-8"),
            NetConnections::new(),
        );
        world.spawn_component(
            RefDes::new("R1"),
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

        let bom = group_components(&mut world);
        // Should be sorted by footprint, then value
        assert_eq!(bom[0].footprint, "0402");
        assert_eq!(bom[1].footprint, "0402");
        assert_eq!(bom[2].footprint, "SOIC-8");
    }
}
