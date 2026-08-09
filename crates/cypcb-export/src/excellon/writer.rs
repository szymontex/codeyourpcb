//! Excellon drill file writer.
//!
//! Generates complete Excellon drill files with header, tool definitions, and drill hits.

use std::collections::HashMap;

use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::Via;
use cypcb_world::components::{FootprintRef, Layer, Position, Rotation};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

use crate::coords::{nm_to_decimal, CoordinateFormat};
use crate::gerber::copper::{place_pad_millideg, ExportError};

use super::tools::ToolTable;

/// Type of drill hole (plated or non-plated).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrillType {
    /// Plated through-hole (component pads, vias).
    Plated,
    /// Non-plated through-hole (mounting holes).
    NonPlated,
}

/// A single drill hit location.
#[derive(Debug, Clone)]
struct DrillHit {
    position: Point,
    drill_diameter: Nm,
    /// The far end of a milled slot, when the hole is one.
    ///
    /// A slot is not drilled: a bit the width of its narrow dimension is put
    /// in one end and driven to the other. Excellon says that with `G85`
    /// between the two end centres, and a file that writes only the first
    /// point orders a round hole - which is a connector that does not fit and
    /// a board that is scrap.
    slot_end: Option<Point>,
    drill_type: DrillType,
    /// The layers this hole joins.
    ///
    /// A drill file with no stated pair means "through the whole board" to
    /// every fabricator. A blind or buried via belongs in a file of its own,
    /// named for the pair it joins - putting it in the through file has the
    /// board drilled from the outside, which is a board nobody can make.
    span: (Layer, Layer),
}

/// Export Excellon drill file with optional drill type filtering.
///
/// Generates a complete Excellon drill file with header, tool definitions,
/// and drill hit coordinates. Can filter to only plated (PTH) or non-plated (NPTH) holes.
///
/// # Arguments
///
/// * `world` - Board world with components and vias
/// * `library` - Footprint library for pad definitions
/// * `format` - Coordinate format (typically FORMAT_MM_2_6)
/// * `drill_type_filter` - Optional filter (None = all drills, Some(Plated) = PTH only, Some(NonPlated) = NPTH only)
///
/// # Returns
///
/// Returns the Excellon drill file content as a string.
///
/// # Errors
///
/// Returns `ExportError::FootprintNotFound` if a component references an unknown footprint.
///
/// # Examples
///
/// ```no_run
/// use cypcb_export::excellon::{export_excellon, DrillType};
/// use cypcb_export::coords::CoordinateFormat;
/// use cypcb_world::BoardWorld;
/// use cypcb_world::footprint::FootprintLibrary;
///
/// let mut world = BoardWorld::new();
/// let library = FootprintLibrary::new();
/// let format = CoordinateFormat::FORMAT_MM_2_6;
///
/// // Export all drills
/// let all = export_excellon(&mut world, &library, &format, None).unwrap();
///
/// // Export only plated holes (PTH)
/// let pth = export_excellon(&mut world, &library, &format, Some(DrillType::Plated)).unwrap();
///
/// // Export only non-plated holes (NPTH)
/// let npth = export_excellon(&mut world, &library, &format, Some(DrillType::NonPlated)).unwrap();
/// ```
pub fn export_excellon(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    format: &CoordinateFormat,
    drill_type_filter: Option<DrillType>,
) -> Result<String, ExportError> {
    export_excellon_span(
        world,
        library,
        format,
        drill_type_filter,
        (Layer::TopCopper, Layer::BottomCopper),
    )
}

/// The holes that join one pair of layers, as their own Excellon file.
///
/// The through pair is what `export_excellon` writes. A blind or buried via
/// joins some other pair and a fabricator expects it in a separate file: put
/// it in the through file and the board is drilled from the outside.
pub fn export_excellon_span(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    format: &CoordinateFormat,
    drill_type_filter: Option<DrillType>,
    span: (Layer, Layer),
) -> Result<String, ExportError> {
    // How many copper layers the board has, which is what a drill span is
    // stated against.
    let total_layers = world
        .board_info()
        .map(|(_, stack)| stack.count)
        .unwrap_or(2);

    // Collect all drill hits
    let all_hits: Vec<DrillHit> = collect_drill_hits(world, library)?
        .into_iter()
        .filter(|hit| same_span(hit.span, span))
        .collect();

    // Filter by drill type if requested
    let hits: Vec<&DrillHit> = if let Some(filter_type) = drill_type_filter {
        all_hits
            .iter()
            .filter(|h| h.drill_type == filter_type)
            .collect()
    } else {
        all_hits.iter().collect()
    };

    // Early return if no drills after filtering
    if hits.is_empty() {
        return Ok(generate_empty_excellon(
            drill_type_filter,
            span,
            total_layers,
        ));
    }

    let mut output = String::new();
    let mut tool_table = ToolTable::new();

    // Assign tool numbers to all unique drill sizes
    for hit in &hits {
        tool_table.get_or_create(hit.drill_diameter);
    }

    // Write header
    output.push_str(&drill_header(drill_type_filter, span, total_layers));

    // Add drill type to header comment
    match drill_type_filter {
        Some(DrillType::Plated) => output.push_str("; Plated Through Holes\n"),
        Some(DrillType::NonPlated) => output.push_str("; Non-Plated Through Holes\n"),
        None => output.push_str("; All drill holes\n"),
    }

    // The digit counts here have to be the ones actually written, and they
    // were not: this line said `2:4` no matter what `format` held, while the
    // body was written with six decimals. A drill file whose header describes
    // a different format than its data is the same defect the Gerber writer
    // had, one file over.
    output.push_str(&format!(
        "; FORMAT={{{}:{}/ absolute / metric / decimal}}\n",
        format.integer_places, format.decimal_places
    ));
    output.push_str("METRIC,TZ\n"); // Metric units, leading zeros suppressed

    // Write tool definitions
    output.push_str(&tool_table.to_header(format));
    output.push('\n');

    output.push_str("%\n"); // End of header

    // Group hits by drill size (tool number)
    let grouped_hits = group_hits_by_tool_refs(&hits, &mut tool_table);

    // Output drill hits grouped by tool
    for (tool_num, tool_hits) in grouped_hits {
        output.push_str(&format!("T{}\n", tool_num));
        for hit in tool_hits {
            let x = nm_to_decimal(hit.position.x.0, format);
            let y = nm_to_decimal(hit.position.y.0, format);
            match hit.slot_end {
                Some(end) => {
                    let end_x = nm_to_decimal(end.x.0, format);
                    let end_y = nm_to_decimal(end.y.0, format);
                    output.push_str(&format!("X{}Y{}G85X{}Y{}\n", x, y, end_x, end_y));
                }
                None => output.push_str(&format!("X{}Y{}\n", x, y)),
            }
        }
    }

    // End of file
    output.push_str("M30\n");

    Ok(output)
}

/// What a drill file states it is, in the Gerber file function vocabulary.
///
/// `Plated,1,4,PTH` is a plated hole through a four-layer board; a via that
/// stops short is `Blind` when it reaches an outer layer and `Buried` when it
/// does not. The layer numbers come from the same function the copper files
/// use, so a drill file and the layers it joins cannot disagree.
fn drill_file_function(
    drill_type: Option<DrillType>,
    span: (Layer, Layer),
    total_layers: u8,
) -> String {
    let from = crate::gerber::header::copper_layer_number(span.0, total_layers);
    let to = crate::gerber::header::copper_layer_number(span.1, total_layers);
    let through = from == 1 && to == total_layers;
    let touches_outside = from == 1 || to == total_layers;

    let plating = match drill_type {
        Some(DrillType::NonPlated) => "NonPlated",
        // A file holding both is what the job format calls `XPlated`, and this
        // exporter never writes one: plated and non-plated holes go to
        // separate files, which is what the Gerber layer states.
        _ => "Plated",
    };
    let kind = match (through, touches_outside, drill_type) {
        (true, _, Some(DrillType::NonPlated)) => "NPTH",
        (true, _, _) => "PTH",
        (false, true, _) => "Blind",
        (false, false, _) => "Buried",
    };

    format!("{plating},{from},{to},{kind}")
}

/// The lines every drill file this exporter writes begins with.
///
/// The X2 attributes are the point: a drill file used to say nothing about
/// itself, so the job file beside it could not name it without recomputing
/// what it was - a second source of truth for the one thing a fabricator must
/// not get wrong. Written as `; #@!` comments, which is where NC formats carry
/// Gerber attributes and what every CAM tool that reads them expects.
fn drill_header(drill_type: Option<DrillType>, span: (Layer, Layer), total_layers: u8) -> String {
    let mut header = String::new();
    header.push_str("M48\n");
    header.push_str("; DRILL file generated by CodeYourPCB\n");
    header.push_str(&format!(
        "; #@! TF.GenerationSoftware,CodeYourPCB,cypcb,{}\n",
        env!("CARGO_PKG_VERSION")
    ));
    header.push_str(&format!(
        "; #@! TF.CreationDate,{}\n",
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%z")
    ));
    header.push_str(&format!(
        "; #@! TF.FileFunction,{}\n",
        drill_file_function(drill_type, span, total_layers)
    ));
    header
}

/// Generate empty Excellon file (no drill hits).
fn generate_empty_excellon(
    drill_type_filter: Option<DrillType>,
    span: (Layer, Layer),
    total_layers: u8,
) -> String {
    let mut output = String::new();
    output.push_str(&drill_header(drill_type_filter, span, total_layers));

    match drill_type_filter {
        Some(DrillType::Plated) => output.push_str("; No plated drill holes\n"),
        Some(DrillType::NonPlated) => output.push_str("; No non-plated drill holes\n"),
        None => output.push_str("; No drill holes\n"),
    }

    output.push_str("METRIC,TZ\n");
    output.push_str("%\n");
    output.push_str("M30\n");
    output
}

/// Collect all drill hits from components and vias.
fn collect_drill_hits(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
) -> Result<Vec<DrillHit>, ExportError> {
    let mut hits = Vec::new();

    // Collect from component pads (through-hole)
    let mut query = world
        .ecs_mut()
        .query::<(&Position, &FootprintRef, &Rotation)>();

    for (position, footprint_ref, rotation) in query.iter(world.ecs()) {
        // Look up footprint in library
        let footprint = library
            .get(&footprint_ref.0)
            .ok_or_else(|| ExportError::FootprintNotFound(footprint_ref.0.clone()))?;

        // Iterate over pads with drill holes
        for pad in &footprint.pads {
            if let Some(drill_diameter) = pad.drill {
                // Calculate absolute position (component position + rotated pad offset)
                let abs_pos = place_pad_millideg(position.0, pad.position, rotation.0);

                // A slot's two ends are two pad offsets, so the same rotation
                // that places the pad places both of them.
                let slot_end = pad.slot_half_travel().map(|half| {
                    let start = Point::new(
                        Nm(pad.position.x.0 - half.x.0),
                        Nm(pad.position.y.0 - half.y.0),
                    );
                    let end = Point::new(
                        Nm(pad.position.x.0 + half.x.0),
                        Nm(pad.position.y.0 + half.y.0),
                    );
                    (
                        place_pad_millideg(position.0, start, rotation.0),
                        place_pad_millideg(position.0, end, rotation.0),
                    )
                });

                hits.push(DrillHit {
                    span: (Layer::TopCopper, Layer::BottomCopper),
                    position: slot_end.map_or(abs_pos, |(start, _)| start),
                    slot_end: slot_end.map(|(_, end)| end),
                    drill_diameter,
                    // Not "component pads are always plated", which is what
                    // stood here and put every mounting hole in the plated
                    // file. A pad with copper is plated; a hole without is a
                    // mounting hole and the fabricator must leave it bare.
                    drill_type: if pad.is_non_plated() {
                        DrillType::NonPlated
                    } else {
                        DrillType::Plated
                    },
                });
            }
        }
    }

    // Collect from vias
    let mut via_query = world.ecs_mut().query::<&Via>();
    for via in via_query.iter(world.ecs()) {
        hits.push(DrillHit {
            position: via.position,
            slot_end: None, // A via is drilled, never milled.
            drill_diameter: via.drill,
            drill_type: DrillType::Plated, // Vias are always plated
            span: (via.start_layer, via.end_layer),
        });
    }

    Ok(hits)
}

/// Whether two layer pairs name the same hole, in either order.
fn same_span(a: (Layer, Layer), b: (Layer, Layer)) -> bool {
    (a.0 == b.0 && a.1 == b.1) || (a.0 == b.1 && a.1 == b.0)
}

/// Every layer pair the board's holes join, the through pair excluded.
///
/// Ordered so the file set comes out the same on every run.
pub fn non_through_spans(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
) -> Result<Vec<(Layer, Layer)>, ExportError> {
    let through = (Layer::TopCopper, Layer::BottomCopper);
    let mut spans: Vec<(Layer, Layer)> = Vec::new();

    for hit in collect_drill_hits(world, library)? {
        if same_span(hit.span, through) {
            continue;
        }
        if !spans.iter().any(|known| same_span(*known, hit.span)) {
            spans.push(hit.span);
        }
    }

    spans.sort_by_key(|(start, end)| (format!("{start:?}"), format!("{end:?}")));
    Ok(spans)
}

/// Group drill hits by tool number.
///
/// Returns a Vec of (tool_number, hits) sorted by tool number.
#[allow(dead_code)] // Needed once multi-tool Excellon output is implemented
fn group_hits_by_tool<'a>(
    hits: &'a [DrillHit],
    tool_table: &mut ToolTable,
) -> Vec<(u8, Vec<&'a DrillHit>)> {
    let mut grouped: HashMap<u8, Vec<&'a DrillHit>> = HashMap::new();

    for hit in hits {
        let tool_num = tool_table.get_or_create(hit.drill_diameter);
        grouped.entry(tool_num).or_default().push(hit);
    }

    // Sort by tool number
    let mut result: Vec<_> = grouped.into_iter().collect();
    result.sort_by_key(|(tool_num, _)| *tool_num);

    result
}

/// Group drill hit references by tool number.
///
/// This variant takes references to DrillHits (used when filtering).
fn group_hits_by_tool_refs<'a>(
    hits: &[&'a DrillHit],
    tool_table: &mut ToolTable,
) -> Vec<(u8, Vec<&'a DrillHit>)> {
    let mut grouped: HashMap<u8, Vec<&'a DrillHit>> = HashMap::new();

    for hit in hits {
        let tool_num = tool_table.get_or_create(hit.drill_diameter);
        grouped.entry(tool_num).or_default().push(hit);
    }

    // Sort by tool number
    let mut result: Vec<_> = grouped.into_iter().collect();
    result.sort_by_key(|(tool_num, _)| *tool_num);

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_world::footprint::FootprintLibrary;
    use cypcb_world::BoardWorld;

    #[test]
    fn test_export_empty_board() {
        let mut world = BoardWorld::new();
        let library = FootprintLibrary::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;

        let result = export_excellon(&mut world, &library, &format, None).unwrap();

        assert!(result.contains("M48"));
        assert!(result.contains("No drill holes"));
        assert!(result.contains("M30"));
    }

    #[test]
    fn test_export_header_format() {
        let mut world = BoardWorld::new();
        let library = FootprintLibrary::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;

        let result = export_excellon(&mut world, &library, &format, None).unwrap();

        assert!(result.contains("M48")); // Start of header
        assert!(result.contains("METRIC,TZ")); // Metric units
        assert!(result.contains("%")); // End of header
        assert!(result.contains("M30")); // End of file
    }

    #[test]
    fn test_place_pad_millideg_no_rotation() {
        let comp_pos = Point::from_mm(10.0, 20.0);
        let pad_offset = Point::from_mm(1.0, 2.0);

        let result = place_pad_millideg(comp_pos, pad_offset, 0);

        assert_eq!(result, Point::from_mm(11.0, 22.0));
    }

    #[test]
    fn test_place_pad_millideg_with_rotation() {
        let comp_pos = Point::from_mm(10.0, 10.0);
        let pad_offset = Point::from_mm(1.0, 0.0);

        // 90 degree rotation (90,000 millidegrees)
        let result = place_pad_millideg(comp_pos, pad_offset, 90_000);

        // Pad at (1, 0) rotated 90 degrees becomes (0, 1)
        // Component at (10, 10), so result should be (10, 11)
        assert_eq!(result.x, Nm::from_mm(10.0));
        // Allow small floating-point error
        assert!((result.y.0 - Nm::from_mm(11.0).0).abs() < 1000); // Within 1µm
    }

    #[test]
    fn test_collect_drill_hits_empty() {
        let mut world = BoardWorld::new();
        let library = FootprintLibrary::new();

        let hits = collect_drill_hits(&mut world, &library).unwrap();

        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn test_group_hits_by_tool_empty() {
        let hits = vec![];
        let mut tool_table = ToolTable::new();

        let grouped = group_hits_by_tool(&hits, &mut tool_table);

        assert_eq!(grouped.len(), 0);
    }

    #[test]
    fn test_group_hits_by_tool_single_size() {
        let hits = vec![
            DrillHit {
                position: Point::from_mm(0.0, 0.0),
                drill_diameter: Nm::from_mm(0.3),
                drill_type: DrillType::Plated,
                slot_end: None,
                span: (Layer::TopCopper, Layer::BottomCopper),
            },
            DrillHit {
                position: Point::from_mm(1.0, 1.0),
                drill_diameter: Nm::from_mm(0.3),
                drill_type: DrillType::Plated,
                slot_end: None,
                span: (Layer::TopCopper, Layer::BottomCopper),
            },
        ];
        let mut tool_table = ToolTable::new();

        let grouped = group_hits_by_tool(&hits, &mut tool_table);

        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped[0].0, 1); // Tool T1
        assert_eq!(grouped[0].1.len(), 2); // Two hits
    }

    #[test]
    fn test_group_hits_by_tool_multiple_sizes() {
        let hits = vec![
            DrillHit {
                position: Point::from_mm(0.0, 0.0),
                drill_diameter: Nm::from_mm(0.3),
                drill_type: DrillType::Plated,
                slot_end: None,
                span: (Layer::TopCopper, Layer::BottomCopper),
            },
            DrillHit {
                position: Point::from_mm(1.0, 1.0),
                drill_diameter: Nm::from_mm(0.8),
                drill_type: DrillType::Plated,
                slot_end: None,
                span: (Layer::TopCopper, Layer::BottomCopper),
            },
            DrillHit {
                position: Point::from_mm(2.0, 2.0),
                drill_diameter: Nm::from_mm(0.3),
                drill_type: DrillType::Plated,
                slot_end: None,
                span: (Layer::TopCopper, Layer::BottomCopper),
            },
        ];
        let mut tool_table = ToolTable::new();

        let grouped = group_hits_by_tool(&hits, &mut tool_table);

        assert_eq!(grouped.len(), 2);

        // Tool 1 should have 2 hits (0.3mm)
        let tool1 = grouped.iter().find(|(t, _)| *t == 1).unwrap();
        assert_eq!(tool1.1.len(), 2);

        // Tool 2 should have 1 hit (0.8mm)
        let tool2 = grouped.iter().find(|(t, _)| *t == 2).unwrap();
        assert_eq!(tool2.1.len(), 1);
    }

    #[test]
    fn test_export_filter_pth_only() {
        let mut world = BoardWorld::new();
        let library = FootprintLibrary::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;

        let result =
            export_excellon(&mut world, &library, &format, Some(DrillType::Plated)).unwrap();

        assert!(result.contains("M48"));
        // Empty case says "No plated drill holes"
        assert!(result.contains("plated drill holes"));
        assert!(result.contains("M30"));
    }

    #[test]
    fn test_export_filter_npth_only() {
        let mut world = BoardWorld::new();
        let library = FootprintLibrary::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;

        let result =
            export_excellon(&mut world, &library, &format, Some(DrillType::NonPlated)).unwrap();

        assert!(result.contains("M48"));
        // Empty case says "No non-plated drill holes"
        assert!(result.contains("non-plated drill holes"));
        assert!(result.contains("M30"));
    }

    #[test]
    fn test_export_all_drills() {
        let mut world = BoardWorld::new();
        let library = FootprintLibrary::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;

        let result = export_excellon(&mut world, &library, &format, None).unwrap();

        assert!(result.contains("M48"));
        // Empty case says "No drill holes"
        assert!(result.contains("drill holes"));
        assert!(result.contains("M30"));
    }
}
