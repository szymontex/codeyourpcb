//! Soldermask and solderpaste layer export.
//!
//! Soldermask layers define where solder mask is ABSENT (openings for pads).
//! Solderpaste layers define where solder paste should be applied (SMD pads only).

use cypcb_world::{BoardWorld, Layer};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_core::{Nm, Point};
use crate::coords::{CoordinateFormat, nm_to_gerber};
use crate::apertures::{ApertureManager, ApertureShape, aperture_for_pad};
use crate::gerber::header::{write_header, GerberFileFunction, Side};
use cypcb_world::components::{Position, FootprintRef, Rotation};
use crate::gerber::copper::{ExportError, calculate_pad_position};

/// Configuration for mask and paste layer export.
///
/// Controls dimensional adjustments for mask openings and paste stencils.
#[derive(Debug, Clone, Copy)]
pub struct MaskPasteConfig {
    /// Mask expansion: how much larger the mask opening is than the pad.
    /// Typical value: 50,000 nm (0.05mm) for standard manufacturing.
    pub mask_expansion: Nm,

    /// Paste reduction: percentage reduction of paste aperture vs pad size.
    /// Typical value: 0.0 (no reduction) to 0.1 (10% reduction).
    /// 0.0 means paste aperture = pad size.
    pub paste_reduction: f64,
}

impl Default for MaskPasteConfig {
    fn default() -> Self {
        Self {
            mask_expansion: Nm(50_000),  // 0.05mm standard expansion
            paste_reduction: 0.0,         // No reduction by default
        }
    }
}

impl MaskPasteConfig {
    /// Create a new configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set mask expansion.
    pub fn with_mask_expansion(mut self, expansion: Nm) -> Self {
        self.mask_expansion = expansion;
        self
    }

    /// Set paste reduction percentage (0.0 to 1.0).
    pub fn with_paste_reduction(mut self, reduction: f64) -> Self {
        self.paste_reduction = reduction;
        self
    }
}

/// Export a soldermask layer to Gerber format.
///
/// Soldermask is the protective coating on the PCB. This export defines
/// the OPENINGS where mask is absent (where pads are exposed for soldering).
///
/// # Arguments
///
/// * `world` - The board world containing all entities
/// * `library` - Footprint library for pad definitions
/// * `side` - Which side to export (Top or Bottom)
/// * `format` - Coordinate format specification
/// * `config` - Mask/paste configuration (for expansion)
///
/// # Returns
///
/// A complete Gerber file as a string, or an error if export fails.
///
/// # Examples
///
/// ```
/// use cypcb_export::gerber::mask::{export_soldermask, MaskPasteConfig};
/// use cypcb_export::gerber::header::Side;
/// use cypcb_world::{BoardWorld, FootprintLibrary};
/// use cypcb_export::coords::CoordinateFormat;
/// use cypcb_core::Nm;
///
/// let mut world = BoardWorld::new();
/// world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(100.0)), 2);
/// let library = FootprintLibrary::new();
/// let format = CoordinateFormat::FORMAT_MM_2_6;
/// let config = MaskPasteConfig::default();
///
/// let gerber = export_soldermask(&mut world, &library, Side::Top, &format, &config).unwrap();
/// assert!(gerber.contains("TF.FileFunction,Soldermask,Top"));
/// assert!(gerber.contains("M02*")); // End of file
/// ```
pub fn export_soldermask(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    side: Side,
    format: &CoordinateFormat,
    config: &MaskPasteConfig,
) -> Result<String, ExportError> {
    let mut output = String::new();
    let mut apertures = ApertureManager::new();

    // Determine layer based on side
    let layer = match side {
        Side::Top => Layer::TopCopper,
        Side::Bottom => Layer::BottomCopper,
    };

    let function = GerberFileFunction::Soldermask(side);
    let board_name = world.board_info()
        .and_then(|(_size, _)| Some("board"))
        .unwrap_or("board");
    let total_layers = world.board_info()
        .map(|(_, ls)| ls.count)
        .unwrap_or(2);

    // Write header
    output.push_str(&write_header(&function, board_name, format, total_layers));

    // Collect drawing commands
    let mut drawing_commands = String::new();

    // Set polarity to dark (exposed areas are positive)
    drawing_commands.push_str("%LPD*%\n");

    // Export mask openings (pads with expansion)
    export_mask_openings(world, library, layer, &mut apertures, &mut drawing_commands, format, config)?;

    // Emit aperture definitions
    output.push_str(&apertures.to_definitions(format));

    // Emit drawing commands
    output.push_str(&drawing_commands);

    // End of file
    output.push_str("M02*\n");

    Ok(output)
}

/// Export a solderpaste layer to Gerber format.
///
/// Solderpaste (solder stencil) defines where solder paste is applied.
/// Only SMD pads receive paste (THT pads are excluded).
///
/// # Arguments
///
/// * `world` - The board world containing all entities
/// * `library` - Footprint library for pad definitions
/// * `side` - Which side to export (Top or Bottom)
/// * `format` - Coordinate format specification
/// * `config` - Mask/paste configuration (for paste reduction)
///
/// # Returns
///
/// A complete Gerber file as a string, or an error if export fails.
///
/// # Examples
///
/// ```
/// use cypcb_export::gerber::mask::{export_solderpaste, MaskPasteConfig};
/// use cypcb_export::gerber::header::Side;
/// use cypcb_world::{BoardWorld, FootprintLibrary};
/// use cypcb_export::coords::CoordinateFormat;
/// use cypcb_core::Nm;
///
/// let mut world = BoardWorld::new();
/// world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(100.0)), 2);
/// let library = FootprintLibrary::new();
/// let format = CoordinateFormat::FORMAT_MM_2_6;
/// let config = MaskPasteConfig::default();
///
/// let gerber = export_solderpaste(&mut world, &library, Side::Top, &format, &config).unwrap();
/// assert!(gerber.contains("TF.FileFunction,Paste,Top"));
/// assert!(gerber.contains("M02*")); // End of file
/// ```
pub fn export_solderpaste(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    side: Side,
    format: &CoordinateFormat,
    config: &MaskPasteConfig,
) -> Result<String, ExportError> {
    let mut output = String::new();
    let mut apertures = ApertureManager::new();

    // Determine layer based on side
    let layer = match side {
        Side::Top => Layer::TopCopper,
        Side::Bottom => Layer::BottomCopper,
    };

    let function = GerberFileFunction::Solderpaste(side);
    let board_name = world.board_info()
        .and_then(|(_size, _)| Some("board"))
        .unwrap_or("board");
    let total_layers = world.board_info()
        .map(|(_, ls)| ls.count)
        .unwrap_or(2);

    // Write header
    output.push_str(&write_header(&function, board_name, format, total_layers));

    // Collect drawing commands
    let mut drawing_commands = String::new();

    // Set polarity to dark (paste areas are positive)
    drawing_commands.push_str("%LPD*%\n");

    // Export paste stencil openings (SMD pads only, with reduction)
    export_paste_openings(world, library, layer, &mut apertures, &mut drawing_commands, format, config)?;

    // Emit aperture definitions
    output.push_str(&apertures.to_definitions(format));

    // Emit drawing commands
    output.push_str(&drawing_commands);

    // End of file
    output.push_str("M02*\n");

    Ok(output)
}

/// Export mask openings (pads with expansion).
fn export_mask_openings(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    layer: Layer,
    apertures: &mut ApertureManager,
    output: &mut String,
    format: &CoordinateFormat,
    config: &MaskPasteConfig,
) -> Result<(), ExportError> {
    // Query all components with position and footprint
    let mut query = world.ecs_mut().query::<(&Position, &FootprintRef, &Rotation)>();

    for (position, footprint_ref, rotation) in query.iter(world.ecs()) {
        // Look up footprint in library
        let footprint = library.get(&footprint_ref.0)
            .ok_or_else(|| ExportError::FootprintNotFound(footprint_ref.0.clone()))?;

        // Iterate over pads
        for pad in &footprint.pads {
            // Check if pad is on this layer
            if !pad.layers.contains(&layer) {
                continue;
            }

            // Calculate absolute position (component position + rotated pad offset)
            let abs_pos = calculate_pad_position(position.0, pad.position, rotation.0);

            // Get base aperture shape for this pad
            let base_shape = aperture_for_pad(pad);

            // Apply mask expansion
            let expanded_shape = apply_expansion(base_shape, config.mask_expansion);

            let dcode = apertures.get_or_create(expanded_shape);

            // Select aperture
            output.push_str(&format!("D{}*\n", dcode));

            // Flash opening at position
            let x = nm_to_gerber(abs_pos.x.0, format);
            let y = nm_to_gerber(abs_pos.y.0, format);
            output.push_str(&format!("X{}Y{}D03*\n", x, y));
        }
    }

    Ok(())
}

/// Export paste stencil openings (SMD pads only, with reduction).
fn export_paste_openings(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    layer: Layer,
    apertures: &mut ApertureManager,
    output: &mut String,
    format: &CoordinateFormat,
    config: &MaskPasteConfig,
) -> Result<(), ExportError> {
    // Query all components with position and footprint
    let mut query = world.ecs_mut().query::<(&Position, &FootprintRef, &Rotation)>();

    for (position, footprint_ref, rotation) in query.iter(world.ecs()) {
        // Look up footprint in library
        let footprint = library.get(&footprint_ref.0)
            .ok_or_else(|| ExportError::FootprintNotFound(footprint_ref.0.clone()))?;

        // Iterate over pads
        for pad in &footprint.pads {
            // Check if pad is on this layer
            if !pad.layers.contains(&layer) {
                continue;
            }

            // Only SMD pads get paste (check for no drill)
            if pad.drill.is_some() {
                continue; // THT pad, skip
            }

            // Calculate absolute position (component position + rotated pad offset)
            let abs_pos = calculate_pad_position(position.0, pad.position, rotation.0);

            // Get base aperture shape for this pad
            let base_shape = aperture_for_pad(pad);

            // Apply paste reduction
            let reduced_shape = apply_reduction(base_shape, config.paste_reduction);

            let dcode = apertures.get_or_create(reduced_shape);

            // Select aperture
            output.push_str(&format!("D{}*\n", dcode));

            // Flash paste opening at position
            let x = nm_to_gerber(abs_pos.x.0, format);
            let y = nm_to_gerber(abs_pos.y.0, format);
            output.push_str(&format!("X{}Y{}D03*\n", x, y));
        }
    }

    Ok(())
}

/// Apply expansion to an aperture shape (for mask openings).
fn apply_expansion(shape: ApertureShape, expansion: Nm) -> ApertureShape {
    match shape {
        ApertureShape::Circle { diameter } => ApertureShape::Circle {
            diameter: diameter + (expansion.0 * 2), // Add to diameter (both sides)
        },
        ApertureShape::Rectangle { width, height } => ApertureShape::Rectangle {
            width: width + (expansion.0 * 2),
            height: height + (expansion.0 * 2),
        },
        ApertureShape::Oblong { width, height } => ApertureShape::Oblong {
            width: width + (expansion.0 * 2),
            height: height + (expansion.0 * 2),
        },
        ApertureShape::RoundRect { width, height, corner_ratio } => ApertureShape::RoundRect {
            width: width + (expansion.0 * 2),
            height: height + (expansion.0 * 2),
            corner_ratio,
        },
    }
}

/// Apply reduction to an aperture shape (for paste stencils).
fn apply_reduction(shape: ApertureShape, reduction: f64) -> ApertureShape {
    if reduction <= 0.0 {
        // No reduction, return original shape
        return shape;
    }

    let factor = 1.0 - reduction; // e.g., 0.1 reduction = 0.9 factor

    match shape {
        ApertureShape::Circle { diameter } => ApertureShape::Circle {
            diameter: ((diameter as f64) * factor) as i64,
        },
        ApertureShape::Rectangle { width, height } => ApertureShape::Rectangle {
            width: ((width as f64) * factor) as i64,
            height: ((height as f64) * factor) as i64,
        },
        ApertureShape::Oblong { width, height } => ApertureShape::Oblong {
            width: ((width as f64) * factor) as i64,
            height: ((height as f64) * factor) as i64,
        },
        ApertureShape::RoundRect { width, height, corner_ratio } => ApertureShape::RoundRect {
            width: ((width as f64) * factor) as i64,
            height: ((height as f64) * factor) as i64,
            corner_ratio,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_world::{RefDes, Value, NetConnections};
    use cypcb_world::footprint::{Footprint, PadDef};
    use cypcb_world::components::PadShape;
    use cypcb_core::Rect;

    #[test]
    fn test_export_empty_soldermask() {
        let mut world = BoardWorld::new();
        world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(80.0)), 2);
        let library = FootprintLibrary::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;
        let config = MaskPasteConfig::default();

        let result = export_soldermask(&mut world, &library, Side::Top, &format, &config);
        assert!(result.is_ok());

        let gerber = result.unwrap();
        assert!(gerber.contains("TF.FileFunction,Soldermask,Top"));
        assert!(gerber.contains("%LPD*%")); // Dark polarity
        assert!(gerber.contains("M02*"));
    }

    #[test]
    fn test_export_empty_solderpaste() {
        let mut world = BoardWorld::new();
        world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(80.0)), 2);
        let library = FootprintLibrary::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;
        let config = MaskPasteConfig::default();

        let result = export_solderpaste(&mut world, &library, Side::Top, &format, &config);
        assert!(result.is_ok());

        let gerber = result.unwrap();
        assert!(gerber.contains("TF.FileFunction,Paste,Top"));
        assert!(gerber.contains("M02*"));
    }

    #[test]
    fn test_export_soldermask_with_pad() {
        let mut world = BoardWorld::new();
        world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(80.0)), 2);

        // Create a footprint with one SMD pad
        let mut library = FootprintLibrary::new();
        let footprint = Footprint {
            name: "test_fp".into(),
            description: "Test footprint".into(),
            bounds: Rect::new(Point::ORIGIN, Point::ORIGIN),
            courtyard: Rect::new(Point::ORIGIN, Point::ORIGIN),
            pads: vec![PadDef {
                number: "1".into(),
                shape: PadShape::Circle,
                position: Point::ORIGIN,
                size: (Nm::from_mm(1.0), Nm::from_mm(1.0)),
                drill: None, // SMD pad
                layers: vec![Layer::TopCopper],
            }],
        };
        library.register(footprint);

        // Spawn component
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 20.0),
            Rotation::ZERO,
            FootprintRef::new("test_fp"),
            NetConnections::new(),
        );

        let format = CoordinateFormat::FORMAT_MM_2_6;
        let config = MaskPasteConfig::default(); // 0.05mm expansion

        let result = export_soldermask(&mut world, &library, Side::Top, &format, &config);
        assert!(result.is_ok());

        let gerber = result.unwrap();
        // Should have aperture (expanded from 1.0mm to 1.1mm)
        assert!(gerber.contains("%ADD"));
        assert!(gerber.contains("D03")); // Flash command
        assert!(gerber.contains("1.100000")); // Expanded diameter
    }

    #[test]
    fn test_export_solderpaste_excludes_tht() {
        let mut world = BoardWorld::new();
        world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(80.0)), 2);

        // Create a footprint with THT pad
        let mut library = FootprintLibrary::new();
        let footprint = Footprint {
            name: "test_fp".into(),
            description: "Test footprint".into(),
            bounds: Rect::new(Point::ORIGIN, Point::ORIGIN),
            courtyard: Rect::new(Point::ORIGIN, Point::ORIGIN),
            pads: vec![PadDef {
                number: "1".into(),
                shape: PadShape::Circle,
                position: Point::ORIGIN,
                size: (Nm::from_mm(1.0), Nm::from_mm(1.0)),
                drill: Some(Nm::from_mm(0.6)), // THT pad with drill
                layers: vec![Layer::TopCopper],
            }],
        };
        library.register(footprint);

        // Spawn component
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 20.0),
            Rotation::ZERO,
            FootprintRef::new("test_fp"),
            NetConnections::new(),
        );

        let format = CoordinateFormat::FORMAT_MM_2_6;
        let config = MaskPasteConfig::default();

        let result = export_solderpaste(&mut world, &library, Side::Top, &format, &config);
        assert!(result.is_ok());

        let gerber = result.unwrap();
        // Should have header but NO apertures (THT pad excluded)
        assert!(!gerber.contains("%ADD"));
        assert!(!gerber.contains("D03"));
    }

    #[test]
    fn test_mask_paste_config_defaults() {
        let config = MaskPasteConfig::default();
        assert_eq!(config.mask_expansion, Nm(50_000)); // 0.05mm
        assert_eq!(config.paste_reduction, 0.0);
    }

    #[test]
    fn test_mask_paste_config_builders() {
        let config = MaskPasteConfig::new()
            .with_mask_expansion(Nm(100_000))
            .with_paste_reduction(0.1);

        assert_eq!(config.mask_expansion, Nm(100_000));
        assert_eq!(config.paste_reduction, 0.1);
    }

    #[test]
    fn test_apply_expansion() {
        let shape = ApertureShape::Circle {
            diameter: 1_000_000, // 1mm
        };
        let expanded = apply_expansion(shape, Nm(50_000)); // +0.05mm on each side

        assert_eq!(expanded, ApertureShape::Circle {
            diameter: 1_100_000, // 1.1mm
        });
    }

    #[test]
    fn test_apply_reduction() {
        let shape = ApertureShape::Rectangle {
            width: 1_000_000,  // 1mm
            height: 500_000,   // 0.5mm
        };
        let reduced = apply_reduction(shape, 0.1); // 10% reduction = 90% of original

        assert_eq!(reduced, ApertureShape::Rectangle {
            width: 900_000,  // 0.9mm
            height: 450_000, // 0.45mm
        });
    }

    #[test]
    fn test_apply_reduction_zero() {
        let shape = ApertureShape::Circle {
            diameter: 1_000_000,
        };
        let reduced = apply_reduction(shape.clone(), 0.0);

        assert_eq!(reduced, shape); // No change
    }
}
