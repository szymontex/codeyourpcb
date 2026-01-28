//! Board outline/profile Gerber export.
//!
//! Exports the board outline as a closed polygon defining the physical board
//! dimensions for routing/cutting. Uses the Profile file function per X2 spec.

use cypcb_world::BoardWorld;
use cypcb_core::Nm;
use crate::coords::{CoordinateFormat, nm_to_gerber};
use crate::apertures::{ApertureManager, ApertureShape};
use crate::gerber::header::{write_header, GerberFileFunction};

/// Board outline export error types.
#[derive(Debug, thiserror::Error)]
pub enum OutlineError {
    #[error("Board size not defined")]
    NoBoardSize,
}

/// Default outline width (router bit kerf).
pub const OUTLINE_WIDTH: Nm = Nm(100_000); // 0.1mm

/// Export board outline/profile to Gerber format.
///
/// Generates a complete Gerber file defining the board outline as a closed
/// polygon. The outline is used by manufacturers for routing/cutting the board.
///
/// For rectangular boards, exports a simple closed rectangle path. Future
/// versions may support complex outlines from Zone entities with ZoneKind::BoardOutline.
///
/// # Arguments
///
/// * `world` - The board world containing board dimensions
/// * `format` - Coordinate format specification
///
/// # Returns
///
/// A complete Gerber file as a string, or an error if board size is not defined.
///
/// # Examples
///
/// ```
/// use cypcb_export::gerber::outline::export_outline;
/// use cypcb_world::BoardWorld;
/// use cypcb_export::coords::CoordinateFormat;
/// use cypcb_core::Nm;
///
/// let mut world = BoardWorld::new();
/// world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(80.0)), 2);
/// let format = CoordinateFormat::FORMAT_MM_2_6;
///
/// let gerber = export_outline(&world, &format).unwrap();
/// assert!(gerber.contains("TF.FileFunction,Profile,NP"));
/// assert!(gerber.contains("M02*")); // End of file
/// ```
pub fn export_outline(
    world: &BoardWorld,
    format: &CoordinateFormat,
) -> Result<String, OutlineError> {
    let mut output = String::new();
    let mut apertures = ApertureManager::new();

    // Get board dimensions
    let (board_size, layer_stack) = world.board_info()
        .ok_or(OutlineError::NoBoardSize)?;

    let board_name = world.board_name().unwrap_or("board");

    // Write header with Profile file function
    output.push_str(&write_header(
        &GerberFileFunction::Profile,
        board_name,
        format,
        layer_stack.count,
    ));

    // Collect drawing commands
    let mut drawing_commands = String::new();

    // Create thin line aperture for outline
    let aperture_shape = ApertureShape::Circle {
        diameter: OUTLINE_WIDTH.0,
    };
    let dcode = apertures.get_or_create(aperture_shape);

    // Select aperture
    drawing_commands.push_str(&format!("D{}*\n", dcode));

    // Draw closed polygon
    // Corner coordinates (origin at bottom-left)
    let corners = [
        (Nm::ZERO, Nm::ZERO),           // Bottom-left (0, 0)
        (board_size.width, Nm::ZERO),   // Bottom-right (w, 0)
        (board_size.width, board_size.height), // Top-right (w, h)
        (Nm::ZERO, board_size.height),  // Top-left (0, h)
        (Nm::ZERO, Nm::ZERO),           // Back to bottom-left (close path)
    ];

    // Move to first corner (D02)
    let x = nm_to_gerber(corners[0].0.0, format);
    let y = nm_to_gerber(corners[0].1.0, format);
    drawing_commands.push_str(&format!("X{}Y{}D02*\n", x, y));

    // Draw to remaining corners (D01)
    for i in 1..corners.len() {
        let x = nm_to_gerber(corners[i].0.0, format);
        let y = nm_to_gerber(corners[i].1.0, format);
        drawing_commands.push_str(&format!("X{}Y{}D01*\n", x, y));
    }

    // Emit aperture definitions
    output.push_str(&apertures.to_definitions(format));

    // Emit drawing commands
    output.push_str(&drawing_commands);

    // End of file
    output.push_str("M02*\n");

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_world::BoardWorld;
    use cypcb_core::Nm;

    #[test]
    fn test_export_outline_rectangular() {
        let mut world = BoardWorld::new();
        world.set_board(
            "test_board".into(),
            (Nm::from_mm(100.0), Nm::from_mm(80.0)),
            2,
        );

        let format = CoordinateFormat::FORMAT_MM_2_6;
        let gerber = export_outline(&world, &format).unwrap();

        // Check header
        assert!(gerber.contains("TF.FileFunction,Profile,NP"));
        assert!(gerber.contains("CodeYourPCB"));
        assert!(gerber.contains("Board: test_board"));

        // Check end of file
        assert!(gerber.contains("M02*"));
    }

    #[test]
    fn test_export_outline_closed_polygon() {
        let mut world = BoardWorld::new();
        world.set_board(
            "test".into(),
            (Nm::from_mm(50.0), Nm::from_mm(40.0)),
            2,
        );

        let format = CoordinateFormat::FORMAT_MM_2_6;
        let gerber = export_outline(&world, &format).unwrap();

        // Should contain 5 coordinate pairs (4 corners + close path)
        // Move to (0,0), draw to (50,0), draw to (50,40), draw to (0,40), draw to (0,0)
        assert!(gerber.contains("X0.000000Y0.000000D02*")); // Move to origin
        assert!(gerber.contains("X50.000000Y0.000000D01*")); // Draw to bottom-right
        assert!(gerber.contains("X50.000000Y40.000000D01*")); // Draw to top-right
        assert!(gerber.contains("X0.000000Y40.000000D01*")); // Draw to top-left
        assert!(gerber.contains("X0.000000Y0.000000D01*")); // Close path
    }

    #[test]
    fn test_export_outline_aperture_defined() {
        let mut world = BoardWorld::new();
        world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(100.0)), 2);

        let format = CoordinateFormat::FORMAT_MM_2_6;
        let gerber = export_outline(&world, &format).unwrap();

        // Should define a circular aperture (D10) for outline width
        assert!(gerber.contains("%ADD10C,0.100000*%")); // 0.1mm circular aperture
        assert!(gerber.contains("D10*")); // Aperture selection
    }

    #[test]
    fn test_export_outline_no_board_size() {
        let world = BoardWorld::new(); // No board set
        let format = CoordinateFormat::FORMAT_MM_2_6;

        let result = export_outline(&world, &format);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), OutlineError::NoBoardSize));
    }

    #[test]
    fn test_export_outline_coordinate_format() {
        let mut world = BoardWorld::new();
        world.set_board("test".into(), (Nm::from_mm(25.4), Nm::from_mm(25.4)), 2);

        let format = CoordinateFormat::FORMAT_MM_2_6;
        let gerber = export_outline(&world, &format).unwrap();

        // Check format declaration
        assert!(gerber.contains("%FSLAX26Y26*%"));
        assert!(gerber.contains("%MOMM*%"));
    }

    #[test]
    fn test_outline_width_constant() {
        // Verify outline width is 0.1mm (100,000 nanometers)
        assert_eq!(OUTLINE_WIDTH.0, 100_000);
        assert_eq!(OUTLINE_WIDTH.to_mm(), 0.1);
    }
}
