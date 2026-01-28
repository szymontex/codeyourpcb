//! Copper layer Gerber export.
//!
//! Exports copper layers (Top, Bottom, Inner) with pads, traces, and vias.
//! Uses flash commands (D03) for pads and vias, draw commands (D01) for traces.

use cypcb_world::{BoardWorld, Layer};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_core::{Nm, Point, Rect};
use crate::coords::{CoordinateFormat, nm_to_gerber};
use crate::apertures::{ApertureManager, ApertureShape, aperture_for_pad};
use crate::gerber::header::{write_header, GerberFileFunction, CopperSide};
use cypcb_world::components::{Position, FootprintRef, Rotation};
use cypcb_world::components::trace::{Trace, Via};

/// Export error types.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("Footprint not found in library: {0}")]
    FootprintNotFound(String),
}

/// Export a copper layer to Gerber format.
///
/// Generates a complete Gerber file for the specified copper layer,
/// including pads, traces, and vias.
///
/// # Arguments
///
/// * `world` - The board world containing all entities
/// * `library` - Footprint library for pad definitions
/// * `layer` - Which copper layer to export (TopCopper, BottomCopper, Inner)
/// * `format` - Coordinate format specification
///
/// # Returns
///
/// A complete Gerber file as a string, or an error if export fails.
///
/// # Examples
///
/// ```
/// use cypcb_export::gerber::copper::export_copper_layer;
/// use cypcb_world::{BoardWorld, Layer};
/// use cypcb_world::footprint::FootprintLibrary;
/// use cypcb_export::coords::CoordinateFormat;
/// use cypcb_core::Nm;
///
/// let mut world = BoardWorld::new();
/// world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(100.0)), 2);
/// let library = FootprintLibrary::new();
/// let format = CoordinateFormat::FORMAT_MM_2_6;
///
/// let gerber = export_copper_layer(&mut world, &library, Layer::TopCopper, &format).unwrap();
/// assert!(gerber.contains("TF.FileFunction,Copper,L1,Top"));
/// assert!(gerber.contains("M02*")); // End of file
/// ```
pub fn export_copper_layer(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    layer: Layer,
    format: &CoordinateFormat,
) -> Result<String, ExportError> {
    // Only copper layers are supported
    assert!(layer.is_copper(), "Layer must be a copper layer");

    let mut output = String::new();
    let mut apertures = ApertureManager::new();

    // Determine file function based on layer
    let (copper_side, layer_num) = match layer {
        Layer::TopCopper => (CopperSide::Top, Some(1)),
        Layer::BottomCopper => {
            // Get total layers from board
            let total_layers = world.board_info()
                .map(|(_, ls)| ls.count)
                .unwrap_or(2);
            (CopperSide::Bottom, Some(total_layers))
        }
        Layer::Inner(n) => (CopperSide::Inner(n), Some(n + 1)), // L2 for first inner
        _ => unreachable!("Non-copper layer passed to export_copper_layer"),
    };

    let function = GerberFileFunction::Copper(copper_side, layer_num);
    let board_name = world.board_info()
        .and_then(|(_size, _)| Some("board"))
        .unwrap_or("board");
    let total_layers = world.board_info()
        .map(|(_, ls)| ls.count)
        .unwrap_or(2);

    // Write header
    output.push_str(&write_header(&function, board_name, format, total_layers));

    // Collect all drawing commands (will be emitted after aperture definitions)
    let mut drawing_commands = String::new();

    // Export pads
    export_pads(world, library, layer, &mut apertures, &mut drawing_commands, format)?;

    // Export traces
    export_traces(world, layer, &mut apertures, &mut drawing_commands, format);

    // Export vias (if they span this layer)
    export_vias(world, layer, &mut apertures, &mut drawing_commands, format);

    // Emit aperture definitions
    output.push_str(&apertures.to_definitions(format));

    // Emit drawing commands
    output.push_str(&drawing_commands);

    // End of file
    output.push_str("M02*\n");

    Ok(output)
}

/// Export pads on the specified layer.
fn export_pads(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    layer: Layer,
    apertures: &mut ApertureManager,
    output: &mut String,
    format: &CoordinateFormat,
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

            // Get or create aperture for this pad
            let aperture_shape = aperture_for_pad(pad);
            let dcode = apertures.get_or_create(aperture_shape);

            // Select aperture
            output.push_str(&format!("D{}*\n", dcode));

            // Flash pad at position
            let x = nm_to_gerber(abs_pos.x.0, format);
            let y = nm_to_gerber(abs_pos.y.0, format);
            output.push_str(&format!("X{}Y{}D03*\n", x, y));
        }
    }

    Ok(())
}

/// Export traces on the specified layer.
fn export_traces(
    world: &mut BoardWorld,
    layer: Layer,
    apertures: &mut ApertureManager,
    output: &mut String,
    format: &CoordinateFormat,
) {
    // Query all traces on this layer
    let mut query = world.ecs_mut().query::<&Trace>();

    for trace in query.iter(world.ecs()) {
        // Skip traces on other layers
        if trace.layer != layer {
            continue;
        }

        // Skip empty traces
        if trace.segments.is_empty() {
            continue;
        }

        // Get or create circular aperture for trace width
        let aperture_shape = ApertureShape::Circle {
            diameter: trace.width.0,
        };
        let dcode = apertures.get_or_create(aperture_shape);

        // Select aperture
        output.push_str(&format!("D{}*\n", dcode));

        // Move to start of first segment
        let start = trace.segments[0].start;
        let x = nm_to_gerber(start.x.0, format);
        let y = nm_to_gerber(start.y.0, format);
        output.push_str(&format!("X{}Y{}D02*\n", x, y)); // D02 = move

        // Draw all segments
        for segment in &trace.segments {
            let end = segment.end;
            let x = nm_to_gerber(end.x.0, format);
            let y = nm_to_gerber(end.y.0, format);
            output.push_str(&format!("X{}Y{}D01*\n", x, y)); // D01 = draw
        }
    }
}

/// Export vias that span the specified layer.
fn export_vias(
    world: &mut BoardWorld,
    layer: Layer,
    apertures: &mut ApertureManager,
    output: &mut String,
    format: &CoordinateFormat,
) {
    // Query all vias
    let mut query = world.ecs_mut().query::<&Via>();

    for via in query.iter(world.ecs()) {
        // Check if via spans this layer
        if !via_spans_layer(via, layer) {
            continue;
        }

        // Get or create circular aperture for via outer diameter
        let aperture_shape = ApertureShape::Circle {
            diameter: via.outer_diameter.0,
        };
        let dcode = apertures.get_or_create(aperture_shape);

        // Select aperture
        output.push_str(&format!("D{}*\n", dcode));

        // Flash via at position
        let x = nm_to_gerber(via.position.x.0, format);
        let y = nm_to_gerber(via.position.y.0, format);
        output.push_str(&format!("X{}Y{}D03*\n", x, y));
    }
}

/// Calculate absolute pad position accounting for component rotation.
///
/// Rotates the pad offset around the component origin, then adds component position.
fn calculate_pad_position(component_pos: Point, pad_offset: Point, rotation_millideg: i32) -> Point {
    if rotation_millideg == 0 {
        // No rotation, simple addition
        return Point::new(
            Nm(component_pos.x.0 + pad_offset.x.0),
            Nm(component_pos.y.0 + pad_offset.y.0),
        );
    }

    // Convert millidegrees to radians
    let angle_rad = (rotation_millideg as f64) / 1000.0 * std::f64::consts::PI / 180.0;
    let cos_theta = angle_rad.cos();
    let sin_theta = angle_rad.sin();

    // Rotate pad offset around origin
    let rotated_x = (pad_offset.x.0 as f64) * cos_theta - (pad_offset.y.0 as f64) * sin_theta;
    let rotated_y = (pad_offset.x.0 as f64) * sin_theta + (pad_offset.y.0 as f64) * cos_theta;

    // Add component position
    Point::new(
        Nm(component_pos.x.0 + (rotated_x as i64)),
        Nm(component_pos.y.0 + (rotated_y as i64)),
    )
}

/// Check if a via spans the specified copper layer.
fn via_spans_layer(via: &Via, layer: Layer) -> bool {
    // For simplicity, assume through-hole vias span all layers
    // Blind/buried vias would need more complex logic
    if via.is_through_hole() {
        return layer.is_copper();
    }

    // For blind/buried vias, check if layer is between start and end
    // This is a simplified check - real implementation would need layer ordering
    match (via.start_layer, via.end_layer, layer) {
        (Layer::TopCopper, Layer::Inner(_n), Layer::TopCopper) => true,
        (Layer::TopCopper, Layer::Inner(n), Layer::Inner(m)) if m <= n => true,
        (Layer::Inner(_n), Layer::BottomCopper, Layer::BottomCopper) => true,
        (Layer::Inner(n), Layer::BottomCopper, Layer::Inner(m)) if m >= n => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_world::{RefDes, Value, NetConnections};
    use cypcb_world::footprint::{Footprint, PadDef};
    use cypcb_world::components::PadShape;
    use cypcb_world::components::trace::TraceSegment;
    use cypcb_world::NetId;

    #[test]
    fn test_export_empty_layer() {
        let mut world = BoardWorld::new();
        world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(80.0)), 2);
        let library = FootprintLibrary::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;

        let result = export_copper_layer(&mut world, &library, Layer::TopCopper, &format);
        assert!(result.is_ok());

        let gerber = result.unwrap();
        assert!(gerber.contains("TF.FileFunction,Copper,L1,Top"));
        assert!(gerber.contains("M02*"));
    }

    #[test]
    fn test_export_with_simple_pad() {
        let mut world = BoardWorld::new();
        world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(80.0)), 2);

        // Create a simple footprint with one pad
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
            drill: None,
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
        let result = export_copper_layer(&mut world, &library, Layer::TopCopper, &format);
        assert!(result.is_ok());

        let gerber = result.unwrap();
        // Should have aperture definition
        assert!(gerber.contains("%ADD"));
        // Should have flash command (D03)
        assert!(gerber.contains("D03"));
        // Should have coordinate at 10mm, 20mm
        assert!(gerber.contains("10.000000"));
        assert!(gerber.contains("20.000000"));
    }

    #[test]
    fn test_export_with_trace() {
        let mut world = BoardWorld::new();
        world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(80.0)), 2);

        // Create a trace
        let net_id = world.intern_net("VCC");
        let mut trace = Trace::new(net_id);
        trace.layer = Layer::TopCopper;
        trace.width = Nm::from_mm(0.2);
        trace.add_segment(TraceSegment::new(
            Point::from_mm(5.0, 5.0),
            Point::from_mm(10.0, 5.0),
        ));
        trace.add_segment(TraceSegment::new(
            Point::from_mm(10.0, 5.0),
            Point::from_mm(10.0, 10.0),
        ));
        world.ecs_mut().spawn(trace);

        let library = FootprintLibrary::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;
        let result = export_copper_layer(&mut world, &library, Layer::TopCopper, &format);
        assert!(result.is_ok());

        let gerber = result.unwrap();
        // Should have move command (D02)
        assert!(gerber.contains("D02"));
        // Should have draw commands (D01)
        assert!(gerber.contains("D01"));
    }

    #[test]
    fn test_export_with_via() {
        let mut world = BoardWorld::new();
        world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(80.0)), 2);

        // Create a via
        let net_id = world.intern_net("GND");
        let via = Via::new(Point::from_mm(15.0, 25.0), net_id);
        world.ecs_mut().spawn(via);

        let library = FootprintLibrary::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;
        let result = export_copper_layer(&mut world, &library, Layer::TopCopper, &format);
        assert!(result.is_ok());

        let gerber = result.unwrap();
        // Should flash via (D03)
        assert!(gerber.contains("D03"));
        // Should have via position
        assert!(gerber.contains("15.000000"));
        assert!(gerber.contains("25.000000"));
    }

    #[test]
    fn test_calculate_pad_position_no_rotation() {
        let comp_pos = Point::from_mm(10.0, 20.0);
        let pad_offset = Point::from_mm(1.0, 2.0);
        let result = calculate_pad_position(comp_pos, pad_offset, 0);
        assert_eq!(result, Point::from_mm(11.0, 22.0));
    }

    #[test]
    fn test_calculate_pad_position_with_rotation() {
        let comp_pos = Point::from_mm(10.0, 20.0);
        let pad_offset = Point::from_mm(5.0, 0.0);
        // 90 degrees = 90,000 millidegrees
        let result = calculate_pad_position(comp_pos, pad_offset, 90_000);

        // After 90° rotation: (5, 0) -> (0, 5)
        // Then add component position: (10, 20) + (0, 5) = (10, 25)
        // Allow small floating-point error
        let expected = Point::from_mm(10.0, 25.0);
        let dx = (result.x.0 - expected.x.0).abs();
        let dy = (result.y.0 - expected.y.0).abs();
        assert!(dx < 100); // Less than 0.1 micron error
        assert!(dy < 100);
    }

    #[test]
    fn test_via_spans_layer_through_hole() {
        let net_id = NetId::new(0);
        let via = Via::new(Point::ORIGIN, net_id);
        assert!(via.is_through_hole());
        assert!(via_spans_layer(&via, Layer::TopCopper));
        assert!(via_spans_layer(&via, Layer::BottomCopper));
        assert!(via_spans_layer(&via, Layer::Inner(0)));
    }

    #[test]
    fn test_export_bottom_layer_number() {
        let mut world = BoardWorld::new();
        world.set_board("test".into(), (Nm::from_mm(100.0), Nm::from_mm(80.0)), 4);
        let library = FootprintLibrary::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;

        let result = export_copper_layer(&mut world, &library, Layer::BottomCopper, &format);
        assert!(result.is_ok());

        let gerber = result.unwrap();
        // Bottom layer of 4-layer board should be L4
        assert!(gerber.contains("TF.FileFunction,Copper,L4,Bot"));
    }
}
