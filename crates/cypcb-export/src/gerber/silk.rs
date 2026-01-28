//! Silkscreen layer Gerber export.
//!
//! Exports silkscreen layers (top/bottom) with component designator markers
//! and courtyard outlines. Uses the Legend file function per X2 spec.
//!
//! # MVP Limitations
//!
//! - Designators rendered as simple crosshair marks (not full text)
//! - Component outlines use courtyard bounds
//!
//! Full text rendering requires vector stroke fonts or bitmap fonts rendered
//! as polylines, which is deferred to future implementation.

use cypcb_world::{BoardWorld, Layer};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_core::Nm;
use crate::coords::{CoordinateFormat, nm_to_gerber};
use crate::apertures::{ApertureManager, ApertureShape};
use crate::gerber::header::{write_header, GerberFileFunction, Side};
use cypcb_world::components::{Position, FootprintRef, Rotation};

/// Silkscreen export error types.
#[derive(Debug, thiserror::Error)]
pub enum SilkError {
    #[error("Footprint not found in library: {0}")]
    FootprintNotFound(String),
}

/// Silkscreen configuration.
#[derive(Debug, Clone)]
pub struct SilkConfig {
    /// Line width for silkscreen features (default 0.15mm).
    pub line_width: Nm,
    /// Show component courtyard outlines.
    pub show_courtyards: bool,
    /// Show designator position marks (crosshairs).
    pub show_designator_marks: bool,
}

impl Default for SilkConfig {
    fn default() -> Self {
        SilkConfig {
            line_width: Nm::from_mm(0.15),
            show_courtyards: true,
            show_designator_marks: true,
        }
    }
}

/// Export silkscreen layer to Gerber format.
///
/// Generates a complete Gerber file for the specified silkscreen layer,
/// including component courtyard outlines and designator position markers.
///
/// # MVP Implementation
///
/// This MVP implementation renders:
/// - Component courtyard rectangles
/// - Designator position marks (crosshairs at component centers)
///
/// Full text rendering is deferred to future implementation.
///
/// # Arguments
///
/// * `world` - The board world containing all entities
/// * `library` - Footprint library for courtyard definitions
/// * `side` - Which side to export (Top or Bottom)
/// * `format` - Coordinate format specification
/// * `config` - Silkscreen rendering configuration
///
/// # Returns
///
/// A complete Gerber file as a string, or an error if export fails.
///
/// # Examples
///
/// ```
/// use cypcb_export::gerber::silk::{export_silkscreen, SilkConfig};
/// use cypcb_export::gerber::header::Side;
/// use cypcb_world::{BoardWorld, Layer};
/// use cypcb_world::footprint::FootprintLibrary;
/// use cypcb_export::coords::CoordinateFormat;
/// use cypcb_core::Nm;
///
/// let mut world = BoardWorld::new();
/// world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(80.0)), 2);
/// let library = FootprintLibrary::new();
/// let format = CoordinateFormat::FORMAT_MM_2_6;
/// let config = SilkConfig::default();
///
/// let gerber = export_silkscreen(&mut world, &library, Side::Top, &format, &config).unwrap();
/// assert!(gerber.contains("TF.FileFunction,Legend,Top"));
/// assert!(gerber.contains("M02*")); // End of file
/// ```
pub fn export_silkscreen(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    side: Side,
    format: &CoordinateFormat,
    config: &SilkConfig,
) -> Result<String, SilkError> {
    let mut output = String::new();
    let mut apertures = ApertureManager::new();

    // Get board info for header
    let board_name = world.board_name().unwrap_or("board");
    let total_layers = world.board_info()
        .map(|(_, ls)| ls.count)
        .unwrap_or(2);

    // Write header with Legend file function
    output.push_str(&write_header(
        &GerberFileFunction::Silkscreen(side),
        board_name,
        format,
        total_layers,
    ));

    // Collect drawing commands
    let mut drawing_commands = String::new();

    // Create aperture for silkscreen line width
    let aperture_shape = ApertureShape::Circle {
        diameter: config.line_width.0,
    };
    let dcode = apertures.get_or_create(aperture_shape);

    // Select aperture
    drawing_commands.push_str(&format!("D{}*\n", dcode));

    // Determine which layer we're exporting for
    let target_layer = match side {
        Side::Top => Layer::TopCopper,
        Side::Bottom => Layer::BottomCopper,
    };

    // Query all components with position and footprint
    let mut query = world.ecs_mut().query::<(&Position, &FootprintRef, &Rotation)>();

    for (position, footprint_ref, rotation) in query.iter(world.ecs()) {
        // Look up footprint in library
        let footprint = library.get(&footprint_ref.0)
            .ok_or_else(|| SilkError::FootprintNotFound(footprint_ref.0.clone()))?;

        // Check if component has pads on target layer
        let on_target_side = footprint.pads.iter()
            .any(|pad| pad.layers.contains(&target_layer));

        if !on_target_side {
            continue;
        }

        // Draw designator mark (crosshair at component center)
        if config.show_designator_marks {
            draw_crosshair(
                position.0,
                config.line_width,
                &mut drawing_commands,
                format,
            );
        }

        // Draw courtyard outline
        if config.show_courtyards {
            draw_courtyard(
                position.0,
                &footprint.courtyard,
                rotation.0,
                &mut drawing_commands,
                format,
            );
        }
    }

    // Emit aperture definitions
    output.push_str(&apertures.to_definitions(format));

    // Emit drawing commands
    output.push_str(&drawing_commands);

    // End of file
    output.push_str("M02*\n");

    Ok(output)
}

/// Draw a crosshair mark at the given position.
///
/// Draws a simple + mark to indicate component location without full text rendering.
fn draw_crosshair(
    position: cypcb_core::Point,
    line_width: Nm,
    output: &mut String,
    format: &CoordinateFormat,
) {
    // Crosshair size: 2x line width for visibility
    let half_size = line_width.0 * 2;

    // Horizontal line: left to right
    let x_left = nm_to_gerber(position.x.0 - half_size, format);
    let y = nm_to_gerber(position.y.0, format);
    output.push_str(&format!("X{}Y{}D02*\n", x_left, y)); // Move to left

    let x_right = nm_to_gerber(position.x.0 + half_size, format);
    output.push_str(&format!("X{}Y{}D01*\n", x_right, y)); // Draw to right

    // Vertical line: bottom to top
    let x = nm_to_gerber(position.x.0, format);
    let y_bottom = nm_to_gerber(position.y.0 - half_size, format);
    output.push_str(&format!("X{}Y{}D02*\n", x, y_bottom)); // Move to bottom

    let y_top = nm_to_gerber(position.y.0 + half_size, format);
    output.push_str(&format!("X{}Y{}D01*\n", x, y_top)); // Draw to top
}

/// Draw a courtyard outline rectangle.
///
/// Draws the footprint courtyard bounds as a rectangle outline.
fn draw_courtyard(
    position: cypcb_core::Point,
    courtyard: &cypcb_core::Rect,
    rotation_millideg: i32,
    output: &mut String,
    format: &CoordinateFormat,
) {
    // For MVP, draw axis-aligned rectangle at component position
    // TODO: Handle rotation by rotating courtyard corners

    // Calculate absolute courtyard corners
    let min_x = position.x.0 + courtyard.min.x.0;
    let min_y = position.y.0 + courtyard.min.y.0;
    let max_x = position.x.0 + courtyard.max.x.0;
    let max_y = position.y.0 + courtyard.max.y.0;

    // Draw rectangle: bottom-left -> bottom-right -> top-right -> top-left -> close
    let corners = [
        (min_x, min_y), // Bottom-left
        (max_x, min_y), // Bottom-right
        (max_x, max_y), // Top-right
        (min_x, max_y), // Top-left
        (min_x, min_y), // Back to bottom-left (close)
    ];

    // Move to first corner
    let x = nm_to_gerber(corners[0].0, format);
    let y = nm_to_gerber(corners[0].1, format);
    output.push_str(&format!("X{}Y{}D02*\n", x, y));

    // Draw to remaining corners
    for i in 1..corners.len() {
        let x = nm_to_gerber(corners[i].0, format);
        let y = nm_to_gerber(corners[i].1, format);
        output.push_str(&format!("X{}Y{}D01*\n", x, y));
    }

    // Suppress unused rotation warning - will be used when rotation is implemented
    let _ = rotation_millideg;
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_world::{BoardWorld, RefDes, Value, NetConnections, PadShape};
    use cypcb_world::footprint::{Footprint, PadDef};
    use cypcb_core::{Point, Rect, Nm};

    fn create_test_footprint() -> Footprint {
        // Simple 2-pad footprint
        let pad1 = PadDef {
            number: "1".to_string(),
            position: Point::from_mm(-1.0, 0.0),
            shape: PadShape::Circle,
            size: (Nm::from_mm(1.0), Nm::from_mm(1.0)),
            layers: vec![Layer::TopCopper],
            drill: None,
        };

        let pad2 = PadDef {
            number: "2".to_string(),
            position: Point::from_mm(1.0, 0.0),
            shape: PadShape::Circle,
            size: (Nm::from_mm(1.0), Nm::from_mm(1.0)),
            layers: vec![Layer::TopCopper],
            drill: None,
        };

        Footprint {
            name: "TEST_0402".to_string(),
            description: "Test footprint".to_string(),
            pads: vec![pad1, pad2],
            bounds: Rect::new(
                Point::from_mm(-1.5, -0.75),
                Point::from_mm(1.5, 0.75),
            ),
            courtyard: Rect::new(
                Point::from_mm(-2.0, -1.0),
                Point::from_mm(2.0, 1.0),
            ),
        }
    }

    #[test]
    fn test_export_silkscreen_top() {
        let mut world = BoardWorld::new();
        world.set_board("test_board".into(), (Nm::from_mm(100.0), Nm::from_mm(80.0)), 2);

        // Add a component on top side
        let mut library = FootprintLibrary::new();
        library.register(create_test_footprint());

        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(50.0, 40.0),
            Rotation::ZERO,
            FootprintRef::new("TEST_0402"),
            NetConnections::new(),
        );

        let format = CoordinateFormat::FORMAT_MM_2_6;
        let config = SilkConfig::default();
        let gerber = export_silkscreen(&mut world, &library, Side::Top, &format, &config).unwrap();

        // Check header
        assert!(gerber.contains("TF.FileFunction,Legend,Top"));
        assert!(gerber.contains("CodeYourPCB"));

        // Check end of file
        assert!(gerber.contains("M02*"));
    }

    #[test]
    fn test_export_silkscreen_bottom() {
        let mut world = BoardWorld::new();
        world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(80.0)), 2);

        let library = FootprintLibrary::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;
        let config = SilkConfig::default();

        let gerber = export_silkscreen(&mut world, &library, Side::Bottom, &format, &config).unwrap();

        // Check header for bottom side
        assert!(gerber.contains("TF.FileFunction,Legend,Bot"));
    }

    #[test]
    fn test_export_silkscreen_with_component() {
        let mut world = BoardWorld::new();
        world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(80.0)), 2);

        let mut library = FootprintLibrary::new();
        library.register(create_test_footprint());

        world.spawn_component(
            RefDes::new("C1"),
            Value::new("100nF"),
            Position::from_mm(25.0, 25.0),
            Rotation::ZERO,
            FootprintRef::new("TEST_0402"),
            NetConnections::new(),
        );

        let format = CoordinateFormat::FORMAT_MM_2_6;
        let config = SilkConfig::default();
        let gerber = export_silkscreen(&mut world, &library, Side::Top, &format, &config).unwrap();

        // Should contain drawing commands for crosshair and courtyard
        assert!(gerber.contains("D01*")); // Draw commands present
    }

    #[test]
    fn test_silkscreen_config_default() {
        let config = SilkConfig::default();
        assert_eq!(config.line_width.to_mm(), 0.15);
        assert!(config.show_courtyards);
        assert!(config.show_designator_marks);
    }

    #[test]
    fn test_silkscreen_aperture_defined() {
        let mut world = BoardWorld::new();
        world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(100.0)), 2);

        let library = FootprintLibrary::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;
        let config = SilkConfig::default();

        let gerber = export_silkscreen(&mut world, &library, Side::Top, &format, &config).unwrap();

        // Should define aperture for line width
        assert!(gerber.contains("%ADD10C,0.150000*%")); // 0.15mm circular aperture
        assert!(gerber.contains("D10*")); // Aperture selection
    }

    #[test]
    fn test_silkscreen_side_filtering() {
        let mut world = BoardWorld::new();
        world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(80.0)), 2);

        // Create footprint with bottom-side pads
        let pad = PadDef {
            number: "1".to_string(),
            position: Point::ORIGIN,
            shape: PadShape::Circle,
            size: (Nm::from_mm(1.0), Nm::from_mm(1.0)),
            layers: vec![Layer::BottomCopper], // Bottom side only
            drill: None,
        };

        let footprint = Footprint {
            name: "BOTTOM_COMP".to_string(),
            description: "Bottom component".to_string(),
            pads: vec![pad],
            bounds: Rect::new(Point::from_mm(-0.75, -0.75), Point::from_mm(0.75, 0.75)),
            courtyard: Rect::new(Point::from_mm(-1.0, -1.0), Point::from_mm(1.0, 1.0)),
        };

        let mut library = FootprintLibrary::new();
        library.register(footprint);

        world.spawn_component(
            RefDes::new("U1"),
            Value::new("IC"),
            Position::from_mm(50.0, 40.0),
            Rotation::ZERO,
            FootprintRef::new("BOTTOM_COMP"),
            NetConnections::new(),
        );

        let format = CoordinateFormat::FORMAT_MM_2_6;
        let config = SilkConfig::default();

        // Export top silkscreen - should NOT include bottom-only component
        let gerber_top = export_silkscreen(&mut world, &library, Side::Top, &format, &config).unwrap();
        // Check that there are no drawing commands (only header and end)
        let draw_count_top = gerber_top.matches("D01*").count();
        assert_eq!(draw_count_top, 0); // No draw commands for bottom component on top silk

        // Export bottom silkscreen - should include bottom component
        let gerber_bottom = export_silkscreen(&mut world, &library, Side::Bottom, &format, &config).unwrap();
        let draw_count_bottom = gerber_bottom.matches("D01*").count();
        assert!(draw_count_bottom > 0); // Has draw commands for bottom component
    }
}
