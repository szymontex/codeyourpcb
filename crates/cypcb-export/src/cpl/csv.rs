//! CPL CSV export in JLCPCB format.
//!
//! Exports Component Placement List as CSV files compatible with JLCPCB and
//! other pick-and-place services. Follows standard column naming conventions.

use crate::cpl::{CplConfig, CplEntry};
use cypcb_world::BoardWorld;
use cypcb_world::components::{RefDes, Position, Rotation, FootprintRef};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::Layer;
use cypcb_core::Nm;
use serde::Serialize;

/// CSV row format matching JLCPCB CPL requirements.
///
/// JLCPCB expects:
/// - "Designator": Component reference (e.g., "U1")
/// - "Mid X": X coordinate with "mm" suffix (e.g., "50.800mm")
/// - "Mid Y": Y coordinate with "mm" suffix (e.g., "30.480mm")
/// - "Layer": "Top" or "Bottom"
/// - "Rotation": Rotation angle in degrees (e.g., "90")
#[derive(Debug, Serialize)]
struct CplCsvRow {
    #[serde(rename = "Designator")]
    designator: String,

    #[serde(rename = "Mid X")]
    mid_x: String,

    #[serde(rename = "Mid Y")]
    mid_y: String,

    #[serde(rename = "Layer")]
    layer: String,

    #[serde(rename = "Rotation")]
    rotation: String,
}

impl From<&CplEntry> for CplCsvRow {
    fn from(entry: &CplEntry) -> Self {
        CplCsvRow {
            designator: entry.designator.clone(),
            mid_x: format!("{:.3}mm", entry.x_mm),
            mid_y: format!("{:.3}mm", entry.y_mm),
            layer: entry.layer.clone(),
            rotation: format!("{:.0}", entry.rotation),
        }
    }
}

/// Export CPL as CSV string in JLCPCB format.
///
/// Generates a CSV file with headers: Designator, Mid X, Mid Y, Layer, Rotation.
/// Coordinates are component centers in millimeters. Rotation is in degrees.
///
/// # Arguments
///
/// * `world` - The board world containing all components
/// * `library` - Footprint library for layer detection
/// * `config` - Optional configuration for rotation offset and Y-flip
///
/// # Returns
///
/// A CSV string ready to be written to a file, or an error if export fails.
///
/// # Examples
///
/// ```
/// use cypcb_export::cpl::csv::export_cpl;
/// use cypcb_world::{BoardWorld, RefDes, Value, Position, Rotation, FootprintRef, NetConnections};
/// use cypcb_world::footprint::FootprintLibrary;
/// use cypcb_core::Nm;
///
/// let mut world = BoardWorld::new();
/// world.spawn_component(
///     RefDes::new("U1"),
///     Value::new("ATmega328P"),
///     Position::from_mm(50.0, 30.0),
///     Rotation::from_degrees(90.0),
///     FootprintRef::new("SOIC-8"),
///     NetConnections::new(),
/// );
///
/// let library = FootprintLibrary::new();
/// let csv = export_cpl(&mut world, &library, None).unwrap();
/// assert!(csv.contains("Designator,Mid X,Mid Y,Layer,Rotation"));
/// assert!(csv.contains("U1"));
/// assert!(csv.contains("50.000mm"));
/// ```
pub fn export_cpl(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    config: Option<&CplConfig>,
) -> Result<String, Box<dyn std::error::Error>> {
    let default_config = CplConfig::default();
    let config = config.unwrap_or(&default_config);
    let mut entries = Vec::new();

    // Query all components
    let mut query = world.ecs_mut().query::<(&RefDes, &Position, &Rotation, &FootprintRef)>();

    for (refdes, position, rotation, footprint_ref) in query.iter(world.ecs()) {
        // Get footprint to determine layer
        let footprint = library.get(&footprint_ref.0)
            .ok_or_else(|| format!("Footprint not found: {}", footprint_ref.0))?;

        // Determine layer from first pad
        let layer = if let Some(first_pad) = footprint.pads.first() {
            if first_pad.layers.contains(&Layer::TopCopper) {
                "Top"
            } else if first_pad.layers.contains(&Layer::BottomCopper) {
                "Bottom"
            } else {
                "Top" // Default to top if no copper layer found
            }
        } else {
            "Top" // Default to top if no pads
        };

        // Convert coordinates from nanometers to millimeters
        let x_mm = position.0.x.0 as f64 / 1_000_000.0;
        let y_mm_raw = position.0.y.0 as f64 / 1_000_000.0;

        // Apply Y-flip if configured
        let y_mm = if config.flip_y {
            // Would need board height to flip correctly, for now just negate
            -y_mm_raw
        } else {
            y_mm_raw
        };

        // Convert rotation from millidegrees to degrees and apply offset
        let rotation_deg = {
            let rot = rotation.0 as f64 / 1000.0;
            (rot + config.rotation_offset).rem_euclid(360.0)
        };

        entries.push(CplEntry {
            designator: refdes.0.clone(),
            x_mm,
            y_mm,
            layer: layer.to_string(),
            rotation: rotation_deg,
        });
    }

    // Sort by designator for consistent output
    entries.sort_by(|a, b| {
        let a_key = natural_sort_key(&a.designator);
        let b_key = natural_sort_key(&b.designator);
        a_key.cmp(&b_key)
    });

    // Write to CSV
    let mut wtr = csv::Writer::from_writer(vec![]);

    for entry in &entries {
        let row = CplCsvRow::from(entry);
        wtr.serialize(row)?;
    }

    wtr.flush()?;

    let data = wtr.into_inner()?;
    let csv_string = String::from_utf8(data)?;

    Ok(csv_string)
}

/// Natural sort key for designators.
///
/// Extracts prefix and number separately so that "R1", "R2", "R10" sorts
/// correctly (not as "R1", "R10", "R2" which lexical sort would give).
fn natural_sort_key(refdes: &str) -> (String, u32) {
    let end = refdes.find(|c: char| c.is_ascii_digit()).unwrap_or(refdes.len());
    let prefix = refdes[..end].to_string();
    let number = refdes[end..].parse::<u32>().unwrap_or(0);
    (prefix, number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_world::{Value, NetConnections};

    #[test]
    fn test_export_cpl_empty() {
        let mut world = BoardWorld::new();
        let library = FootprintLibrary::new();

        let csv = export_cpl(&mut world, &library, None).unwrap();

        // Empty CPL produces empty CSV
        assert_eq!(csv.trim(), "");
    }

    #[test]
    fn test_export_cpl_single() {
        let mut world = BoardWorld::new();
        let library = FootprintLibrary::new();

        world.spawn_component(
            RefDes::new("U1"),
            Value::new("ATmega328P"),
            Position::from_mm(50.0, 30.0),
            Rotation::from_degrees(90.0),
            FootprintRef::new("SOIC-8"),
            NetConnections::new(),
        );

        let csv = export_cpl(&mut world, &library, None).unwrap();

        assert!(csv.contains("Designator,Mid X,Mid Y,Layer,Rotation"));
        assert!(csv.contains("U1"));
        assert!(csv.contains("50.000mm"));
        assert!(csv.contains("30.000mm"));
        assert!(csv.contains("90")); // Rotation
        assert!(csv.contains("Top"));
    }

    #[test]
    fn test_export_cpl_coordinates() {
        let mut world = BoardWorld::new();
        let library = FootprintLibrary::new();

        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(12.345, 67.890),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let csv = export_cpl(&mut world, &library, None).unwrap();

        assert!(csv.contains("12.345mm"));
        assert!(csv.contains("67.890mm"));
    }

    #[test]
    fn test_export_cpl_rotation() {
        let mut world = BoardWorld::new();
        let library = FootprintLibrary::new();

        world.spawn_component(
            RefDes::new("U1"),
            Value::new("IC"),
            Position::from_mm(10.0, 10.0),
            Rotation::from_degrees(45.5),
            FootprintRef::new("SOIC-8"),
            NetConnections::new(),
        );

        let csv = export_cpl(&mut world, &library, None).unwrap();

        assert!(csv.contains("46")); // Rounded to nearest degree
    }

    #[test]
    fn test_export_cpl_rotation_offset() {
        let mut world = BoardWorld::new();
        let library = FootprintLibrary::new();

        world.spawn_component(
            RefDes::new("U1"),
            Value::new("IC"),
            Position::from_mm(10.0, 10.0),
            Rotation::from_degrees(45.0),
            FootprintRef::new("SOIC-8"),
            NetConnections::new(),
        );

        let config = CplConfig::with_rotation_offset(90.0);
        let csv = export_cpl(&mut world, &library, Some(&config)).unwrap();

        assert!(csv.contains("135")); // 45 + 90
    }

    #[test]
    fn test_export_cpl_multiple_sorted() {
        let mut world = BoardWorld::new();
        let library = FootprintLibrary::new();

        // Add in random order
        world.spawn_component(
            RefDes::new("R10"),
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
            RefDes::new("R1"),
            Value::new("1k"),
            Position::from_mm(30.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let csv = export_cpl(&mut world, &library, None).unwrap();

        // Find positions of each designator in CSV
        let r1_pos = csv.find("R1,").unwrap();
        let r2_pos = csv.find("R2,").unwrap();
        let r10_pos = csv.find("R10,").unwrap();

        // Should be sorted naturally: R1, R2, R10
        assert!(r1_pos < r2_pos);
        assert!(r2_pos < r10_pos);
    }

    #[test]
    fn test_csv_column_headers() {
        let mut world = BoardWorld::new();
        let library = FootprintLibrary::new();

        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let csv = export_cpl(&mut world, &library, None).unwrap();
        let lines: Vec<&str> = csv.lines().collect();

        // First line should be header
        assert_eq!(lines[0], "Designator,Mid X,Mid Y,Layer,Rotation");
    }

    #[test]
    fn test_natural_sort_key() {
        assert_eq!(natural_sort_key("R1"), ("R".to_string(), 1));
        assert_eq!(natural_sort_key("R10"), ("R".to_string(), 10));
        assert_eq!(natural_sort_key("LED2"), ("LED".to_string(), 2));
        assert_eq!(natural_sort_key("U100"), ("U".to_string(), 100));
    }
}
