//! Specctra DSN export.
//!
//! Exports board data to Specctra DSN format for FreeRouting autorouter.
//!
//! # DSN Format Overview
//!
//! The Specctra DSN format is a text-based format with S-expression syntax:
//!
//! ```text
//! (pcb "board_name"
//!   (parser ...)
//!   (resolution mil 10)
//!   (unit mil)
//!   (structure ...)      ; Board boundary, layers, rules
//!   (placement ...)      ; Component positions
//!   (library ...)        ; Footprints and padstacks
//!   (network ...)        ; Nets and connections
//!   (wiring ...)         ; Existing traces (locked)
//! )
//! ```

use std::collections::HashSet;
use std::io::Write;

use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

use thiserror::Error;

/// Errors that can occur during DSN export.
#[derive(Debug, Error)]
pub enum DsnExportError {
    /// IO error writing DSN output.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// No board defined in the world.
    #[error("No board defined - use set_board() before export")]
    MissingBoard,

    /// Net has no connections (would cause invalid DSN).
    #[error("Net '{0}' has no pin connections")]
    EmptyNet(String),
}

/// Convert nanometers to mils (thousandths of an inch).
///
/// 1 mil = 25,400 nanometers
#[inline]
fn nm_to_mil(nm: i64) -> f64 {
    nm as f64 / 25_400.0
}

/// Quote a string for DSN format.
///
/// Wraps the string in quotes and escapes any internal quotes.
fn quote_dsn(s: &str) -> String {
    // DSN uses double quotes, escape any internal quotes
    let escaped = s.replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

/// Convert a Layer to DSN layer name.
fn layer_to_dsn_name(layer: cypcb_world::Layer) -> &'static str {
    use cypcb_world::Layer;
    match layer {
        Layer::TopCopper => "F.Cu",
        Layer::BottomCopper => "B.Cu",
        Layer::Inner(n) => match n {
            0 => "In1.Cu",
            1 => "In2.Cu",
            2 => "In3.Cu",
            3 => "In4.Cu",
            _ => "In1.Cu", // Fallback
        },
        _ => "F.Cu", // Non-copper layers default to top
    }
}

/// Export a BoardWorld to Specctra DSN format.
///
/// Writes the complete board definition including:
/// - Board boundary and layers
/// - Component placements
/// - Footprint library (padstacks)
/// - Network (nets and pin connections)
/// - Locked traces as fixed wiring
///
/// # Arguments
///
/// * `world` - The board world to export (mutable for ECS queries)
/// * `library` - Footprint library for pad definitions
/// * `output` - Writer to output DSN data
///
/// # Errors
///
/// Returns `DsnExportError` if:
/// - No board is defined in the world
/// - IO errors occur while writing
///
/// # Example
///
/// ```rust,ignore
/// use cypcb_router::export_dsn;
/// use std::fs::File;
///
/// let mut file = File::create("board.dsn")?;
/// export_dsn(&mut world, &library, &mut file)?;
/// ```
pub fn export_dsn(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    output: &mut impl Write,
) -> Result<(), DsnExportError> {
    // Verify board exists
    let (board_size, layer_stack) = world
        .board_info()
        .ok_or(DsnExportError::MissingBoard)?;

    let board_name = world.board_name().unwrap_or("board");

    // Write header
    writeln!(output, "(pcb {}", quote_dsn(board_name))?;

    // Parser settings
    writeln!(output, "  (parser")?;
    writeln!(output, "    (string_quote \")")?;
    writeln!(output, "    (space_in_quoted_tokens on)")?;
    writeln!(output, "    (host_cad \"CodeYourPCB\")")?;
    writeln!(output, "    (host_version \"0.1.0\")")?;
    writeln!(output, "  )")?;

    // Resolution and units
    writeln!(output, "  (resolution mil 10)")?;
    writeln!(output, "  (unit mil)")?;

    // Structure section (boundary, layers, rules)
    write_structure(world, &board_size, &layer_stack, output)?;

    // Placement section (components)
    write_placement(world, library, output)?;

    // Library section (padstacks)
    write_library(world, library, output)?;

    // Network section (nets and connections)
    write_network(world, output)?;

    // Wiring section (locked traces)
    write_wiring(world, output)?;

    // Close PCB
    writeln!(output, ")")?;

    Ok(())
}

/// Write the structure section (boundary, layers, design rules).
fn write_structure(
    _world: &BoardWorld,
    board_size: &cypcb_world::BoardSize,
    layer_stack: &cypcb_world::LayerStack,
    output: &mut impl Write,
) -> Result<(), DsnExportError> {
    writeln!(output, "  (structure")?;

    // Board boundary
    let width_mil = nm_to_mil(board_size.width.0);
    let height_mil = nm_to_mil(board_size.height.0);

    writeln!(output, "    (boundary")?;
    writeln!(output, "      (rect pcb 0 0 {:.4} {:.4})", width_mil, height_mil)?;
    writeln!(output, "    )")?;

    // Layer definitions
    for i in 0..layer_stack.count {
        let layer_name = if i == 0 {
            "F.Cu"
        } else if i == layer_stack.count - 1 {
            "B.Cu"
        } else {
            match i {
                1 => "In1.Cu",
                2 => "In2.Cu",
                3 => "In3.Cu",
                _ => "In4.Cu",
            }
        };

        writeln!(output, "    (layer {}", layer_name)?;
        writeln!(output, "      (type signal)")?;
        writeln!(output, "    )")?;
    }

    // Default design rules
    writeln!(output, "    (rule")?;
    writeln!(output, "      (width 8)")?;     // 8 mil (0.2mm) default trace
    writeln!(output, "      (clearance 6)")?; // 6 mil (0.15mm) default clearance
    writeln!(output, "    )")?;

    // Via rule (required for FreeRouting to route multi-layer boards)
    writeln!(output, "    (via via_default)")?;

    writeln!(output, "  )")?;
    Ok(())
}

/// Write the placement section (component positions).
fn write_placement(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    output: &mut impl Write,
) -> Result<(), DsnExportError> {
    writeln!(output, "  (placement")?;

    // Group components by footprint
    let mut components_by_footprint: std::collections::HashMap<
        String,
        Vec<(String, cypcb_world::Position, cypcb_world::Rotation)>,
    > = std::collections::HashMap::new();

    // Query components using ECS
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<(
        &cypcb_world::RefDes,
        &cypcb_world::Position,
        &cypcb_world::Rotation,
        &cypcb_world::FootprintRef,
    )>();

    for (refdes, position, rotation, footprint) in query.iter(ecs) {
        components_by_footprint
            .entry(footprint.as_str().to_string())
            .or_default()
            .push((refdes.as_str().to_string(), *position, *rotation));
    }

    // Write each footprint group
    for (footprint_name, components) in &components_by_footprint {
        // Get footprint to determine if components are on top or bottom
        let _footprint = library.get(footprint_name);

        writeln!(output, "    (component {}", quote_dsn(footprint_name))?;

        for (refdes, position, rotation) in components {
            let x_mil = nm_to_mil(position.0.x.0);
            let y_mil = nm_to_mil(position.0.y.0);
            let angle = rotation.to_degrees();

            // Determine side based on position (simple heuristic - top by default)
            let side = "front";

            writeln!(
                output,
                "      (place {} {:.4} {:.4} {} {:.1})",
                quote_dsn(refdes),
                x_mil,
                y_mil,
                side,
                angle
            )?;
        }

        writeln!(output, "    )")?;
    }

    writeln!(output, "  )")?;
    Ok(())
}

/// Write the library section (padstacks).
fn write_library(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    output: &mut impl Write,
) -> Result<(), DsnExportError> {
    writeln!(output, "  (library")?;

    // Collect unique footprints used in the design
    let mut used_footprints: HashSet<String> = HashSet::new();

    let ecs = world.ecs_mut();
    let mut query = ecs.query::<&cypcb_world::FootprintRef>();

    for footprint_ref in query.iter(ecs) {
        used_footprints.insert(footprint_ref.as_str().to_string());
    }

    // Track padstacks we've already written
    let mut written_padstacks: HashSet<String> = HashSet::new();

    // Write footprint images
    for footprint_name in &used_footprints {
        if let Some(footprint) = library.get(footprint_name) {
            writeln!(output, "    (image {}", quote_dsn(footprint_name))?;

            for pad in &footprint.pads {
                let padstack_name = format_padstack_name(pad);

                // Record padstack for later definition
                written_padstacks.insert(padstack_name.clone());

                let x_mil = nm_to_mil(pad.position.x.0);
                let y_mil = nm_to_mil(pad.position.y.0);

                writeln!(
                    output,
                    "      (pin {} {} {:.4} {:.4})",
                    padstack_name,
                    quote_dsn(&pad.number),
                    x_mil,
                    y_mil
                )?;
            }

            writeln!(output, "    )")?;
        }
    }

    // Write padstack definitions
    for footprint_name in &used_footprints {
        if let Some(footprint) = library.get(footprint_name) {
            for pad in &footprint.pads {
                let padstack_name = format_padstack_name(pad);

                // Only write each padstack once
                if written_padstacks.contains(&padstack_name) {
                    written_padstacks.remove(&padstack_name);
                    write_padstack(output, pad)?;
                }
            }
        }
    }

    // Write default via padstack (required for FreeRouting)
    // Standard via: 0.8mm pad, 0.4mm drill
    writeln!(output, "    (padstack via_default")?;
    writeln!(output, "      (shape (circle F.Cu 31.4961))")?; // 0.8mm = 31.5 mil
    writeln!(output, "      (shape (circle B.Cu 31.4961))")?;
    writeln!(output, "      (attach off)")?;
    writeln!(output, "      (hole round 15.7480)")?; // 0.4mm drill = 15.75 mil
    writeln!(output, "    )")?;

    writeln!(output, "  )")?;
    Ok(())
}

/// Format a padstack name based on pad properties.
fn format_padstack_name(pad: &cypcb_world::footprint::PadDef) -> String {
    let shape_str = match pad.shape {
        cypcb_world::PadShape::Circle => "round",
        cypcb_world::PadShape::Rect => "rect",
        cypcb_world::PadShape::RoundRect { .. } => "roundrect",
        cypcb_world::PadShape::Oblong => "oval",
    };

    let width_mil = nm_to_mil(pad.size.0.0) as i32;
    let height_mil = nm_to_mil(pad.size.1.0) as i32;

    if let Some(drill) = pad.drill {
        let drill_mil = nm_to_mil(drill.0) as i32;
        format!("{}_{}_{}_{}", shape_str, width_mil, height_mil, drill_mil)
    } else {
        format!("{}_{}_{}", shape_str, width_mil, height_mil)
    }
}

/// Write a single padstack definition.
fn write_padstack(
    output: &mut impl Write,
    pad: &cypcb_world::footprint::PadDef,
) -> Result<(), DsnExportError> {
    let padstack_name = format_padstack_name(pad);
    let width_mil = nm_to_mil(pad.size.0.0);
    let height_mil = nm_to_mil(pad.size.1.0);

    writeln!(output, "    (padstack {}", padstack_name)?;

    // Write shape for each layer the pad appears on
    for layer in &pad.layers {
        if layer.is_copper() {
            let layer_name = layer_to_dsn_name(*layer);

            match pad.shape {
                cypcb_world::PadShape::Circle => {
                    writeln!(
                        output,
                        "      (shape (circle {} {:.4}))",
                        layer_name, width_mil
                    )?;
                }
                cypcb_world::PadShape::Rect => {
                    let hw = width_mil / 2.0;
                    let hh = height_mil / 2.0;
                    writeln!(
                        output,
                        "      (shape (rect {} {:.4} {:.4} {:.4} {:.4}))",
                        layer_name, -hw, -hh, hw, hh
                    )?;
                }
                cypcb_world::PadShape::RoundRect { .. } | cypcb_world::PadShape::Oblong => {
                    // Approximate as rect for DSN
                    let hw = width_mil / 2.0;
                    let hh = height_mil / 2.0;
                    writeln!(
                        output,
                        "      (shape (rect {} {:.4} {:.4} {:.4} {:.4}))",
                        layer_name, -hw, -hh, hw, hh
                    )?;
                }
            }
        }
    }

    // Add drill hole if through-hole
    if let Some(drill) = pad.drill {
        let drill_mil = nm_to_mil(drill.0);
        writeln!(output, "      (attach off)")?;
        writeln!(output, "      (hole round {:.4})", drill_mil)?;
    } else {
        writeln!(output, "      (attach off)")?;
    }

    writeln!(output, "    )")?;
    Ok(())
}

/// Write the network section (nets and pin connections).
fn write_network(world: &mut BoardWorld, output: &mut impl Write) -> Result<(), DsnExportError> {
    writeln!(output, "  (network")?;

    // Build a map of net_id -> [(refdes, pin)]
    let mut net_pins: std::collections::HashMap<cypcb_world::NetId, Vec<(String, String)>> =
        std::collections::HashMap::new();

    let ecs = world.ecs_mut();
    let mut query = ecs.query::<(&cypcb_world::RefDes, &cypcb_world::NetConnections)>();

    for (refdes, net_conns) in query.iter(ecs) {
        for conn in net_conns.iter() {
            net_pins
                .entry(conn.net)
                .or_default()
                .push((refdes.as_str().to_string(), conn.pin.clone()));
        }
    }

    // Write each net
    for (net_id, net_name) in world.nets() {
        if let Some(pins) = net_pins.get(&net_id) {
            if !pins.is_empty() {
                writeln!(output, "    (net {}", quote_dsn(net_name))?;
                write!(output, "      (pins")?;

                for (refdes, pin) in pins {
                    write!(output, " {}-{}", refdes, pin)?;
                }

                writeln!(output, ")")?;
                writeln!(output, "    )")?;
            }
        }
    }

    // Write default net class
    writeln!(output, "    (class default")?;
    for (_, net_name) in world.nets() {
        write!(output, " {}", quote_dsn(net_name))?;
    }
    writeln!(output)?;
    writeln!(output, "      (rule")?;
    writeln!(output, "        (width 8)")?;     // 8 mil default
    writeln!(output, "        (clearance 6)")?; // 6 mil default
    writeln!(output, "      )")?;
    writeln!(output, "    )")?;

    writeln!(output, "  )")?;
    Ok(())
}

/// Write the wiring section (locked traces).
fn write_wiring(world: &mut BoardWorld, output: &mut impl Write) -> Result<(), DsnExportError> {
    writeln!(output, "  (wiring")?;

    // Collect locked traces first to avoid borrow issues
    let locked_traces: Vec<_> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&cypcb_world::components::trace::Trace>();
        query
            .iter(ecs)
            .filter(|t| t.locked && !t.segments.is_empty())
            .cloned()
            .collect()
    };

    // Now process the collected traces
    for trace in locked_traces {
        let net_name = world.net_name(trace.net_id).unwrap_or("unknown");
        let layer_name = layer_to_dsn_name(trace.layer);
        let width_mil = nm_to_mil(trace.width.0);

        writeln!(output, "    (wire")?;

        // Write path
        write!(output, "      (path {} {:.4}", layer_name, width_mil)?;

        for segment in &trace.segments {
            let x1 = nm_to_mil(segment.start.x.0);
            let y1 = nm_to_mil(segment.start.y.0);
            write!(output, " {:.4} {:.4}", x1, y1)?;
        }

        // Add the last endpoint
        if let Some(last) = trace.segments.last() {
            let x2 = nm_to_mil(last.end.x.0);
            let y2 = nm_to_mil(last.end.y.0);
            write!(output, " {:.4} {:.4}", x2, y2)?;
        }

        writeln!(output, ")")?;
        writeln!(output, "      (net {})", quote_dsn(net_name))?;
        writeln!(output, "      (type fix)")?; // Fixed, don't reroute
        writeln!(output, "    )")?;
    }

    writeln!(output, "  )")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::{Nm, Point};

    #[test]
    fn test_nm_to_mil() {
        // 1 mil = 25400 nm
        assert!((nm_to_mil(25_400) - 1.0).abs() < 0.0001);
        assert!((nm_to_mil(254_000) - 10.0).abs() < 0.0001);

        // 1mm = 1000000 nm = ~39.37 mil
        let one_mm = 1_000_000;
        let expected = one_mm as f64 / 25_400.0;
        assert!((nm_to_mil(one_mm) - expected).abs() < 0.0001);
    }

    #[test]
    fn test_quote_dsn() {
        assert_eq!(quote_dsn("hello"), "\"hello\"");
        assert_eq!(quote_dsn("test board"), "\"test board\"");
        assert_eq!(quote_dsn("with\"quote"), "\"with\\\"quote\"");
    }

    #[test]
    fn test_layer_to_dsn_name() {
        use cypcb_world::Layer;

        assert_eq!(layer_to_dsn_name(Layer::TopCopper), "F.Cu");
        assert_eq!(layer_to_dsn_name(Layer::BottomCopper), "B.Cu");
        assert_eq!(layer_to_dsn_name(Layer::Inner(0)), "In1.Cu");
        assert_eq!(layer_to_dsn_name(Layer::Inner(1)), "In2.Cu");
    }

    #[test]
    fn test_export_empty_board() {
        let mut world = BoardWorld::new();
        world.set_board("TestBoard".to_string(), (Nm::from_mm(50.0), Nm::from_mm(30.0)), 2);

        let library = FootprintLibrary::new();
        let mut output = Vec::new();

        let result = export_dsn(&mut world, &library, &mut output);
        assert!(result.is_ok());

        let dsn = String::from_utf8(output).unwrap();

        // Verify structure
        assert!(dsn.contains("(pcb \"TestBoard\""));
        assert!(dsn.contains("(parser"));
        assert!(dsn.contains("(resolution mil 10)"));
        assert!(dsn.contains("(structure"));
        assert!(dsn.contains("(boundary"));
        assert!(dsn.contains("(placement"));
        assert!(dsn.contains("(library"));
        assert!(dsn.contains("(network"));
        assert!(dsn.contains("(wiring"));
    }

    #[test]
    fn test_export_missing_board() {
        let mut world = BoardWorld::new();
        let library = FootprintLibrary::new();
        let mut output = Vec::new();

        let result = export_dsn(&mut world, &library, &mut output);
        assert!(matches!(result, Err(DsnExportError::MissingBoard)));
    }

    #[test]
    fn test_export_with_component() {
        use cypcb_world::*;

        let mut world = BoardWorld::new();
        world.set_board("TestBoard".to_string(), (Nm::from_mm(50.0), Nm::from_mm(30.0)), 2);

        // Add a component
        let vcc = world.intern_net("VCC");
        let gnd = world.intern_net("GND");

        let mut nets = NetConnections::new();
        nets.add(PinConnection::new("1", vcc));
        nets.add(PinConnection::new("2", gnd));

        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 15.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            nets,
        );

        let library = FootprintLibrary::new();
        let mut output = Vec::new();

        let result = export_dsn(&mut world, &library, &mut output);
        assert!(result.is_ok());

        let dsn = String::from_utf8(output).unwrap();

        // Verify component placement
        assert!(dsn.contains("(component \"0402\""));
        assert!(dsn.contains("(place \"R1\""));

        // Verify nets
        assert!(dsn.contains("(net \"VCC\""));
        assert!(dsn.contains("(net \"GND\""));
        assert!(dsn.contains("R1-1"));
        assert!(dsn.contains("R1-2"));
    }

    #[test]
    fn test_export_with_locked_trace() {
        use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
        use cypcb_world::*;

        let mut world = BoardWorld::new();
        world.set_board("TestBoard".to_string(), (Nm::from_mm(50.0), Nm::from_mm(30.0)), 2);

        let vcc = world.intern_net("VCC");

        // Add a locked trace
        let trace = Trace {
            segments: vec![
                TraceSegment::new(Point::from_mm(0.0, 0.0), Point::from_mm(10.0, 0.0)),
                TraceSegment::new(Point::from_mm(10.0, 0.0), Point::from_mm(10.0, 10.0)),
            ],
            width: Nm::from_mm(0.3),
            layer: Layer::TopCopper,
            net_id: vcc,
            locked: true,
            source: TraceSource::Manual,
        };

        world.spawn_entity(trace);

        let library = FootprintLibrary::new();
        let mut output = Vec::new();

        let result = export_dsn(&mut world, &library, &mut output);
        assert!(result.is_ok());

        let dsn = String::from_utf8(output).unwrap();

        // Verify wiring section contains the trace
        assert!(dsn.contains("(wire"));
        assert!(dsn.contains("(path F.Cu"));
        assert!(dsn.contains("(type fix)")); // Locked trace
    }

    #[test]
    fn test_export_unlocked_trace_not_included() {
        use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
        use cypcb_world::*;

        let mut world = BoardWorld::new();
        world.set_board("TestBoard".to_string(), (Nm::from_mm(50.0), Nm::from_mm(30.0)), 2);

        let vcc = world.intern_net("VCC");

        // Add an unlocked trace (should NOT be exported)
        let trace = Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 0.0),
            )],
            width: Nm::from_mm(0.3),
            layer: Layer::TopCopper,
            net_id: vcc,
            locked: false, // Not locked
            source: TraceSource::Autorouted,
        };

        world.spawn_entity(trace);

        let library = FootprintLibrary::new();
        let mut output = Vec::new();

        let result = export_dsn(&mut world, &library, &mut output);
        assert!(result.is_ok());

        let dsn = String::from_utf8(output).unwrap();

        // Wiring section should be empty (no "(wire" within it)
        // The section exists but has no wire elements
        let wiring_start = dsn.find("(wiring").unwrap();
        let wiring_section = &dsn[wiring_start..];
        let next_close = wiring_section.find("\n  )").unwrap();
        let wiring_content = &wiring_section[..next_close];

        // Should only have the opening "(wiring" and nothing else significant
        assert!(!wiring_content.contains("(wire"));
    }
}
