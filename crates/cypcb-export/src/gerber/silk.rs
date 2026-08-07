//! Silkscreen layer Gerber export.
//!
//! Exports silkscreen layers (top/bottom) with part designators, footprint
//! artwork and courtyard outlines. Uses the Legend file function per X2 spec.
//!
//! Designators print as strokes from `cypcb_world::silk_text`, which is also
//! what the silkscreen clearance rule measures - the checker and the file
//! cannot disagree about what is on the board. A part whose name this font
//! cannot spell falls back to a position crosshair.
//!
//! A footprint that carries its own artwork prints that; one that does not
//! prints its courtyard outline.

use crate::apertures::{ApertureManager, ApertureShape};
use crate::coords::{nm_to_gerber, CoordinateFormat};
use crate::gerber::header::{write_header, GerberFileFunction, Side};
use cypcb_core::Nm;
use cypcb_world::components::{FootprintRef, Position, Rotation};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{BoardWorld, Layer};

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
    /// How tall a printed designator is.
    ///
    /// One millimetre is what a fabricator's own silkscreen minimum allows on
    /// a 0.15mm stroke, and what a person can read without a loupe.
    pub text_height: Nm,
    /// How far the ink must stay from solderable copper.
    ///
    /// The legend is clipped to this, so it should be the clearance the
    /// fabricator this board is for asks for - `DesignRules::min_clearance`
    /// of the preset the user named. The default is JLCPCB's, which is what
    /// every other default in this project is measured against.
    pub clearance: Nm,
}

impl Default for SilkConfig {
    fn default() -> Self {
        SilkConfig {
            line_width: Nm::from_mm(0.15),
            show_courtyards: true,
            show_designator_marks: true,
            text_height: Nm::from_mm(1.0),
            clearance: Nm::from_mm(0.13),
        }
    }
}

/// A part whose name the legend could not print in full.
///
/// The name crosses so much copper that clipping took most of it, so nobody
/// holding the board can read which part it labels. Reported rather than
/// silently drawn, because a half-eaten designator looks like a legend on
/// screen and like nothing on the board.
#[derive(Debug, Clone)]
pub struct SilkWarning {
    /// The part whose name was eaten.
    pub refdes: String,
    /// How many strokes survived the clipping.
    pub strokes_drawn: usize,
    /// How many the name is made of.
    pub strokes_wanted: usize,
}

/// Export silkscreen layer to Gerber format.
///
/// Generates a complete Gerber file for the specified silkscreen layer: each
/// part's designator, its own artwork when the footprint carries any, and its
/// courtyard outline when it does not.
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
    export_silkscreen_reporting(world, library, side, format, config).map(|(gerber, _)| gerber)
}

/// The same export, plus the names it could not print in full.
///
/// Every caller that shows a user what was written should use this one: a
/// designator eaten by clipping is a part nobody can identify on the board,
/// and it is invisible in the file itself.
pub fn export_silkscreen_reporting(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    side: Side,
    format: &CoordinateFormat,
    config: &SilkConfig,
) -> Result<(String, Vec<SilkWarning>), SilkError> {
    let mut output = String::new();
    let mut apertures = ApertureManager::new();

    // Get board info for header
    let board_name = world.board_name().unwrap_or("board");
    let total_layers = world.board_info().map(|(_, ls)| ls.count).unwrap_or(2);

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

    // The copper this legend must not print on.
    //
    // A board house clips silkscreen off solderable copper before it prints,
    // so a file that needs clipping is a file whose legend nobody has seen.
    // Clipping here means the Gerber that leaves is the Gerber that gets made,
    // and it is measured against the clearance the named fabricator asks for
    // rather than a number this crate picked.
    let keepouts = cypcb_world::silk_text::pad_keepouts(
        world,
        library,
        target_layer,
        Nm(config.clearance.0 + config.line_width.0 / 2),
    );

    // Everything this legend draws, in board coordinates, before clipping.
    let mut shapes: Vec<cypcb_world::footprint::SilkShape> = Vec::new();
    let mut unprintable_names = Vec::new();

    // Query all components with position and footprint
    let mut query = world.ecs_mut().query::<(
        &Position,
        &FootprintRef,
        &Rotation,
        Option<&cypcb_world::components::Side>,
        Option<&cypcb_world::components::RefDes>,
    )>();

    for (position, footprint_ref, rotation, part_side, refdes) in query.iter(world.ecs()) {
        // Look up footprint in library
        let footprint = library
            .get(&footprint_ref.0)
            .ok_or_else(|| SilkError::FootprintNotFound(footprint_ref.0.clone()))?;

        // Which legend this part belongs on.
        //
        // The `Side` component is the answer when the design states one. Asking
        // the footprint's pads instead draws a part assembled underneath onto
        // the top legend whenever it shares a footprint with top-side parts,
        // and the assembler is left with a legend that does not match the board
        // in front of them.
        let on_target_side = match part_side {
            Some(side) => side.copper() == target_layer,
            None => footprint
                .pads
                .iter()
                .any(|pad| pad.layers.contains(&target_layer)),
        };

        if !on_target_side {
            continue;
        }

        // The part's name, printed where an assembler can read it.
        //
        // A board with no `R1` beside R1 cannot be assembled by eye - the
        // person holding the reel has to read the design file instead. Gerber
        // has no text a fabricator is obliged to honour, so the letters are
        // strokes like everything else on this layer. A part with no refdes,
        // or a name this font cannot spell, falls back to the crosshair that
        // used to be all there was.
        if config.show_designator_marks {
            let name = refdes.map(|r| r.as_str()).filter(|name| !name.is_empty());
            let letters = name
                .map(|name| {
                    cypcb_world::silk_text::designator_strokes(
                        name,
                        position.0,
                        config.text_height,
                        config.line_width,
                        cypcb_world::silk_text::artwork_rise(footprint, rotation.to_degrees()),
                    )
                })
                .unwrap_or_default();

            if letters.is_empty() {
                // No name, or a name this font cannot spell: the position mark
                // that used to be all there was.
                shapes.extend(crosshair(position.0, config.line_width));
            } else {
                // How much of the name survives the clipping decides whether
                // an assembler can still read it, which the caller is told
                // about rather than left to discover on the board.
                let kept = cypcb_world::silk_text::clip_strokes(letters.clone(), &keepouts);
                if kept.len() * 2 < letters.len() {
                    unprintable_names.push(SilkWarning {
                        refdes: name.unwrap_or("").to_string(),
                        strokes_drawn: kept.len(),
                        strokes_wanted: letters.len(),
                    });
                }
                shapes.extend(letters);
            }
        }

        // A footprint that carries its own artwork prints that. Drawing the
        // courtyard as well would put a box on the board that the footprint
        // never had.
        if !footprint.silk.is_empty() {
            shapes.extend(place_artwork(
                position.0,
                &footprint.silk,
                rotation.to_degrees(),
                config.line_width,
            ));
        } else if config.show_courtyards {
            shapes.extend(courtyard_outline(
                position.0,
                &footprint.courtyard,
                rotation.to_degrees(),
                config.line_width,
            ));
        }
    }

    let mut pen = None;
    for shape in cypcb_world::silk_text::clip_strokes(shapes, &keepouts) {
        emit(&shape, &mut pen, &mut drawing_commands, format);
    }

    // Emit aperture definitions
    output.push_str(&apertures.to_definitions(format));

    // Emit drawing commands
    output.push_str(&drawing_commands);

    // End of file
    output.push_str("M02*\n");

    Ok((output, unprintable_names))
}

/// Write one shape as Gerber, lifting the pen only where strokes stop joining.
///
/// `pen` is where the last command left it. Clipping cuts polylines into
/// pieces, and emitting a pen-up before every piece would write two commands
/// where the artwork needs one - the legend for a small board grew by half
/// before this was threaded through.
fn emit(
    shape: &cypcb_world::footprint::SilkShape,
    pen: &mut Option<cypcb_core::Point>,
    output: &mut String,
    format: &CoordinateFormat,
) {
    use cypcb_world::footprint::SilkShape;

    match shape {
        SilkShape::Segment { start, end, .. } => {
            if *pen != Some(*start) {
                output.push_str(&format!(
                    "X{}Y{}D02*\n",
                    nm_to_gerber(start.x.0, format),
                    nm_to_gerber(start.y.0, format)
                ));
            }
            output.push_str(&format!(
                "X{}Y{}D01*\n",
                nm_to_gerber(end.x.0, format),
                nm_to_gerber(end.y.0, format)
            ));
            *pen = Some(*end);
        }
        SilkShape::Circle { centre, radius, .. } => {
            const STEPS: usize = 32;
            let radius = radius.0 as f64;
            let mut last = (0, 0);
            for step in 0..=STEPS {
                let angle = step as f64 / STEPS as f64 * std::f64::consts::TAU;
                let x = centre.x.0 + (radius * angle.cos()).round() as i64;
                let y = centre.y.0 + (radius * angle.sin()).round() as i64;
                let command = if step == 0 { "D02" } else { "D01" };
                output.push_str(&format!(
                    "X{}Y{}{}*\n",
                    nm_to_gerber(x, format),
                    nm_to_gerber(y, format),
                    command
                ));
                last = (x, y);
            }
            *pen = Some(cypcb_core::Point::new(Nm(last.0), Nm(last.1)));
        }
    }
}

/// A position mark, for a part whose name cannot be printed.
fn crosshair(position: cypcb_core::Point, line_width: Nm) -> Vec<cypcb_world::footprint::SilkShape> {
    use cypcb_world::footprint::SilkShape;

    let half = Nm(line_width.0 * 2);
    vec![
        SilkShape::Segment {
            start: cypcb_core::Point::new(Nm(position.x.0 - half.0), position.y),
            end: cypcb_core::Point::new(Nm(position.x.0 + half.0), position.y),
            width: line_width,
        },
        SilkShape::Segment {
            start: cypcb_core::Point::new(position.x, Nm(position.y.0 - half.0)),
            end: cypcb_core::Point::new(position.x, Nm(position.y.0 + half.0)),
            width: line_width,
        },
    ]
}

/// Turn a footprint's own artwork about its origin and put it on the board.
fn place_artwork(
    position: cypcb_core::Point,
    shapes: &[cypcb_world::footprint::SilkShape],
    rotation_deg: f64,
    line_width: Nm,
) -> Vec<cypcb_world::footprint::SilkShape> {
    use cypcb_world::footprint::SilkShape;

    let (sin, cos) = rotation_deg.to_radians().sin_cos();
    let place = |p: cypcb_core::Point| -> cypcb_core::Point {
        let x = p.x.0 as f64;
        let y = p.y.0 as f64;
        cypcb_core::Point::new(
            Nm(position.x.0 + (x * cos - y * sin).round() as i64),
            Nm(position.y.0 + (x * sin + y * cos).round() as i64),
        )
    };

    shapes
        .iter()
        .map(|shape| match shape {
            SilkShape::Segment { start, end, width } => SilkShape::Segment {
                start: place(*start),
                end: place(*end),
                width: if width.0 > 0 { *width } else { line_width },
            },
            SilkShape::Circle {
                centre,
                radius,
                width,
            } => SilkShape::Circle {
                centre: place(*centre),
                radius: *radius,
                width: if width.0 > 0 { *width } else { line_width },
            },
        })
        .collect()
}

/// The courtyard outline a part prints when its footprint carries no artwork.
///
/// Rotated with the part. It was drawn axis-aligned whatever the rotation
/// until 2026-08-07, while `silk-clearance` rotated the same corners before
/// measuring them - so on any turned part the checker and the file disagreed
/// about where the outline was.
fn courtyard_outline(
    position: cypcb_core::Point,
    courtyard: &cypcb_core::Rect,
    rotation_deg: f64,
    line_width: Nm,
) -> Vec<cypcb_world::footprint::SilkShape> {
    use cypcb_world::footprint::SilkShape;

    let (sin, cos) = rotation_deg.to_radians().sin_cos();
    let place = |x: Nm, y: Nm| -> cypcb_core::Point {
        let (x, y) = (x.0 as f64, y.0 as f64);
        cypcb_core::Point::new(
            Nm(position.x.0 + (x * cos - y * sin).round() as i64),
            Nm(position.y.0 + (x * sin + y * cos).round() as i64),
        )
    };

    let corners = [
        place(courtyard.min.x, courtyard.min.y),
        place(courtyard.max.x, courtyard.min.y),
        place(courtyard.max.x, courtyard.max.y),
        place(courtyard.min.x, courtyard.max.y),
    ];

    (0..4)
        .map(|index| SilkShape::Segment {
            start: corners[index],
            end: corners[(index + 1) % 4],
            width: line_width,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::{Nm, Point, Rect};
    use cypcb_world::footprint::{Footprint, PadDef};
    use cypcb_world::{BoardWorld, NetConnections, PadShape, RefDes, Value};

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
            bounds: Rect::new(Point::from_mm(-1.5, -0.75), Point::from_mm(1.5, 0.75)),
            courtyard: Rect::new(Point::from_mm(-2.0, -1.0), Point::from_mm(2.0, 1.0)),
            silk: Vec::new(),
        }
    }

    #[test]
    fn test_export_silkscreen_top() {
        let mut world = BoardWorld::new();
        world.set_board(
            "test_board".into(),
            (Nm::from_mm(100.0), Nm::from_mm(80.0)),
            2,
        );

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

        let gerber =
            export_silkscreen(&mut world, &library, Side::Bottom, &format, &config).unwrap();

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
            silk: Vec::new(),
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
        let gerber_top =
            export_silkscreen(&mut world, &library, Side::Top, &format, &config).unwrap();
        // Check that there are no drawing commands (only header and end)
        let draw_count_top = gerber_top.matches("D01*").count();
        assert_eq!(draw_count_top, 0); // No draw commands for bottom component on top silk

        // Export bottom silkscreen - should include bottom component
        let gerber_bottom =
            export_silkscreen(&mut world, &library, Side::Bottom, &format, &config).unwrap();
        let draw_count_bottom = gerber_bottom.matches("D01*").count();
        assert!(draw_count_bottom > 0); // Has draw commands for bottom component
    }
}
