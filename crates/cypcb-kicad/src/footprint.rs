//! KiCad footprint (.kicad_mod) import.
//!
//! Converts KiCad footprint files to internal [`Footprint`](cypcb_world::footprint::Footprint) type.
//!
//! # Supported Features
//!
//! - All standard pad shapes: rect, circle, oval
//! - SMD and through-hole pads
//! - Drill holes with size
//! - Layer mapping from KiCad layers to internal layers
//! - Courtyard extraction from F.CrtYd/B.CrtYd layers
//!
//! # Example
//!
//! ```rust,ignore
//! use cypcb_kicad::import_footprint;
//! use std::path::Path;
//!
//! let fp = import_footprint(Path::new("Resistors_SMD.pretty/R_0402.kicad_mod"))?;
//! println!("Imported: {} with {} pads", fp.name, fp.pads.len());
//! ```

use std::fs;
use std::path::Path;

use cypcb_core::{Nm, Point, Rect};
use cypcb_world::footprint::{Footprint, PadDef};
use symbolic_expressions::Sexp;
use thiserror::Error;

use crate::pcb_parser::{find_xy_child, get_string, list_name, parse_pad, NetIndex};

/// Errors that can occur during KiCad footprint import.
#[derive(Error, Debug)]
pub enum KicadImportError {
    /// File I/O error.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Failed to parse the KiCad file format.
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Feature not supported by the importer.
    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),

    /// Required data missing from footprint.
    #[error("Missing data: {0}")]
    MissingData(String),
}

/// Import a KiCad .kicad_mod footprint file.
///
/// # Arguments
///
/// * `path` - Path to the .kicad_mod file
///
/// # Returns
///
/// The imported footprint converted to internal representation.
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed.
///
/// # Example
///
/// ```rust,ignore
/// use cypcb_kicad::import_footprint;
/// use std::path::Path;
///
/// let fp = import_footprint(Path::new("Package_SO.pretty/SOIC-8.kicad_mod"))?;
/// assert_eq!(fp.pads.len(), 8);
/// ```
pub fn import_footprint(path: &Path) -> Result<Footprint, KicadImportError> {
    // Read and parse the file
    let content = fs::read_to_string(path)?;
    import_footprint_from_str(&content)
}

/// Import a KiCad footprint from a string.
///
/// This is useful for testing or when the content is already in memory.
pub fn import_footprint_from_str(content: &str) -> Result<Footprint, KicadImportError> {
    let sexp = symbolic_expressions::parser::parse_str(content)
        .map_err(|e| KicadImportError::ParseError(format!("{e}")))?;
    footprint_from_sexp(&sexp)
}

/// Read a footprint out of the file's S-expression tree.
///
/// This crate has two readers for KiCad's format: `kicad_parse_gen`, which
/// this function used, and the one in `pcb_parser.rs`, which reads the boards
/// KiCad writes today. The first knows the KiCad 5 spelling and nothing since:
/// the list's head was renamed from `module` to `footprint` in 6.0, the format
/// version and the writing program's name were put at the top of it, the
/// reference and value became `property` lists in 7.0, and `roundrect` became
/// the shape almost every generated pad uses. Any one of those is
/// `unknown element in module: <name>` and the whole file is refused.
///
/// Measured on the six footprints in this repository that KiCad 6 or later
/// wrote - `viewer/kicad-tools` and `viewer/faebryk` carry them as their own
/// fixtures - **five were refused**, the sixth being a hand-written file with
/// none of those fields. The same six through the reader that reads boards:
/// six read.
///
/// So the footprint path now uses that reader too, and this crate has one
/// answer to what a KiCad file says rather than two. What is read here is what
/// a footprint file holds and a board file does not repeat: the name, the
/// description, the pads, and the courtyard drawn on `F.CrtYd` or `B.CrtYd`.
fn footprint_from_sexp(sexp: &Sexp) -> Result<Footprint, KicadImportError> {
    let list = sexp
        .list()
        .map_err(|e| KicadImportError::ParseError(format!("{e}")))?;

    let head = list.first().and_then(get_string).unwrap_or_default();
    if head != "footprint" && head != "module" {
        return Err(KicadImportError::ParseError(format!(
            "a footprint file opens with `footprint` or `module`, this one opens with `{head}`"
        )));
    }

    let name = list.get(1).and_then(get_string).unwrap_or_default();
    let mut pads = Vec::new();
    let mut description = String::new();
    let mut courtyard_bounds: Option<Rect> = None;

    for child in list.iter().skip(2) {
        match list_name(child).as_deref() {
            Some("descr") => {
                if let Ok(items) = child.list() {
                    description = items.get(1).and_then(get_string).unwrap_or_default();
                }
            }
            Some("pad") => {
                if let Some(pad) = read_pad(child)? {
                    pads.push(pad);
                }
            }
            Some("fp_line") => {
                if let Some(rect) = courtyard_line(child) {
                    courtyard_bounds = Some(match courtyard_bounds {
                        Some(existing) => existing.union(&rect),
                        None => rect,
                    });
                }
            }
            _ => {}
        }
    }

    let bounds = calculate_pad_bounds(&pads);

    // Use courtyard if found, otherwise add IPC-7351B margin (0.5mm)
    let courtyard = courtyard_bounds.unwrap_or_else(|| {
        let margin = Nm::from_mm(0.5);
        Rect::from_points(
            Point::new(bounds.min.x - margin, bounds.min.y - margin),
            Point::new(bounds.max.x + margin, bounds.max.y + margin),
        )
    });

    Ok(Footprint {
        name,
        description,
        pads,
        bounds,
        courtyard,
        silk: Vec::new(),
    })
}

/// The pad shapes this project has a shape for.
///
/// The board reader falls back to a rectangle for anything else, which is the
/// right answer for a board - one pad of an unknown shape should not cost a
/// person the other nine hundred. A footprint file is one part, so an unknown
/// shape is refused by name instead: `custom` is a polygon somebody drew and a
/// rectangle is not a conservative reading of it.
const KNOWN_SHAPES: [&str; 4] = ["rect", "circle", "oval", "roundrect"];

/// Read one `(pad ...)`, through the reader boards are read with.
fn read_pad(pad: &Sexp) -> Result<Option<PadDef>, KicadImportError> {
    let items = pad
        .list()
        .map_err(|e| KicadImportError::ParseError(format!("{e}")))?;

    let shape = items.get(3).and_then(get_string).unwrap_or_default();
    if !KNOWN_SHAPES.contains(&shape.as_str()) {
        return Err(KicadImportError::UnsupportedFeature(format!(
            "pad shape `{shape}`"
        )));
    }

    let Some(parsed) = parse_pad(&items[1..], &NetIndex::default())
        .map_err(|e| KicadImportError::ParseError(format!("{e}")))?
    else {
        return Ok(None);
    };

    // A hole the file does not state is a hole this crate does not invent. The
    // board reader gives a through-hole pad a 0.8mm drill when the file names
    // none, which keeps a board routable; a footprint read for a library is
    // read to be measured, and a made-up hole is a number nobody wrote.
    let states_drill = items
        .iter()
        .skip(1)
        .any(|item| list_name(item).as_deref() == Some("drill"));

    Ok(Some(PadDef {
        number: parsed.number,
        shape: parsed.shape,
        position: parsed.local_position,
        size: parsed.size,
        drill: if states_drill { parsed.drill } else { None },
        slot: parsed.slot,
        layers: parsed.layers,
    }))
}

/// The rectangle an `(fp_line ...)` on a courtyard layer spans, if it is one.
fn courtyard_line(line: &Sexp) -> Option<Rect> {
    let items = line.list().ok()?;
    let on_courtyard = items.iter().any(|item| {
        list_name(item).as_deref() == Some("layer")
            && item
                .list()
                .ok()
                .and_then(|layer| layer.get(1).and_then(get_string))
                .is_some_and(|name| name.ends_with("CrtYd"))
    });
    if !on_courtyard {
        return None;
    }

    let (start_x, start_y) = find_xy_child(line, "start")?;
    let (end_x, end_y) = find_xy_child(line, "end")?;
    Some(Rect::from_points(
        Point::from_mm(start_x, start_y),
        Point::from_mm(end_x, end_y),
    ))
}

/// Calculate bounding box from pad definitions.
fn calculate_pad_bounds(pads: &[PadDef]) -> Rect {
    if pads.is_empty() {
        return Rect::default();
    }

    let mut min_x = i64::MAX;
    let mut min_y = i64::MAX;
    let mut max_x = i64::MIN;
    let mut max_y = i64::MIN;

    for pad in pads {
        let half_w = pad.size.0 .0 / 2;
        let half_h = pad.size.1 .0 / 2;

        min_x = min_x.min(pad.position.x.0 - half_w);
        min_y = min_y.min(pad.position.y.0 - half_h);
        max_x = max_x.max(pad.position.x.0 + half_w);
        max_y = max_y.max(pad.position.y.0 + half_h);
    }

    Rect::from_points(
        Point::new(Nm(min_x), Nm(min_y)),
        Point::new(Nm(max_x), Nm(max_y)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_world::components::{Layer as InternalLayer, PadShape as InternalPadShape};

    /// Minimal 0402 footprint for testing
    const MINIMAL_0402: &str = r#"(module R_0402 (layer F.Cu)
  (descr "Chip resistor 0402")
  (pad 1 smd rect (at -0.5 0) (size 0.6 0.5) (layers F.Cu F.Paste F.Mask))
  (pad 2 smd rect (at 0.5 0) (size 0.6 0.5) (layers F.Cu F.Paste F.Mask))
)"#;

    /// Through-hole footprint
    const THT_RESISTOR: &str = r#"(module R_Axial (layer F.Cu)
  (descr "Through-hole resistor")
  (pad 1 thru_hole circle (at -3.81 0) (size 1.8 1.8) (drill 1.0) (layers *.Cu))
  (pad 2 thru_hole circle (at 3.81 0) (size 1.8 1.8) (drill 1.0) (layers *.Cu))
)"#;

    /// SOIC-8 for IC testing
    const SOIC8: &str = r#"(module SOIC-8 (layer F.Cu)
  (descr "SOIC-8 package")
  (pad 1 smd rect (at -2.7 -1.905) (size 1.55 0.6) (layers F.Cu F.Paste F.Mask))
  (pad 2 smd rect (at -2.7 -0.635) (size 1.55 0.6) (layers F.Cu F.Paste F.Mask))
  (pad 3 smd rect (at -2.7 0.635) (size 1.55 0.6) (layers F.Cu F.Paste F.Mask))
  (pad 4 smd rect (at -2.7 1.905) (size 1.55 0.6) (layers F.Cu F.Paste F.Mask))
  (pad 5 smd rect (at 2.7 1.905) (size 1.55 0.6) (layers F.Cu F.Paste F.Mask))
  (pad 6 smd rect (at 2.7 0.635) (size 1.55 0.6) (layers F.Cu F.Paste F.Mask))
  (pad 7 smd rect (at 2.7 -0.635) (size 1.55 0.6) (layers F.Cu F.Paste F.Mask))
  (pad 8 smd rect (at 2.7 -1.905) (size 1.55 0.6) (layers F.Cu F.Paste F.Mask))
)"#;

    /// Footprint with oval pads
    const OVAL_PADS: &str = r#"(module Connector (layer F.Cu)
  (descr "Connector with oval pads")
  (pad 1 smd oval (at 0 -1.27) (size 2.0 1.0) (layers F.Cu F.Paste F.Mask))
  (pad 2 smd oval (at 0 1.27) (size 2.0 1.0) (layers F.Cu F.Paste F.Mask))
)"#;

    /// Footprint with courtyard
    const WITH_COURTYARD: &str = r#"(module R_0603 (layer F.Cu)
  (descr "0603 with courtyard")
  (fp_line (start -1.1 -0.8) (end 1.1 -0.8) (layer F.CrtYd) (width 0.05))
  (fp_line (start 1.1 -0.8) (end 1.1 0.8) (layer F.CrtYd) (width 0.05))
  (fp_line (start 1.1 0.8) (end -1.1 0.8) (layer F.CrtYd) (width 0.05))
  (fp_line (start -1.1 0.8) (end -1.1 -0.8) (layer F.CrtYd) (width 0.05))
  (pad 1 smd rect (at -0.8 0) (size 0.9 0.95) (layers F.Cu F.Paste F.Mask))
  (pad 2 smd rect (at 0.8 0) (size 0.9 0.95) (layers F.Cu F.Paste F.Mask))
)"#;

    #[test]
    fn test_import_minimal_0402() {
        let fp = import_footprint_from_str(MINIMAL_0402).unwrap();

        assert_eq!(fp.name, "R_0402");
        assert_eq!(fp.description, "Chip resistor 0402");
        assert_eq!(fp.pads.len(), 2);

        let pad1 = fp.pads.iter().find(|p| p.number == "1").unwrap();
        assert!(matches!(pad1.shape, InternalPadShape::Rect));
        assert_eq!(pad1.position.x, Nm::from_mm(-0.5));
        assert_eq!(pad1.position.y, Nm::from_mm(0.0));
        assert_eq!(pad1.size.0, Nm::from_mm(0.6));
        assert_eq!(pad1.size.1, Nm::from_mm(0.5));
        assert!(pad1.drill.is_none()); // SMD pad, no drill
    }

    #[test]
    fn test_import_tht_resistor() {
        let fp = import_footprint_from_str(THT_RESISTOR).unwrap();

        assert_eq!(fp.name, "R_Axial");
        assert_eq!(fp.pads.len(), 2);

        let pad1 = fp.pads.iter().find(|p| p.number == "1").unwrap();
        assert!(matches!(pad1.shape, InternalPadShape::Circle));
        assert_eq!(pad1.size.0, Nm::from_mm(1.8));
        assert!(pad1.drill.is_some());
        assert_eq!(pad1.drill.unwrap(), Nm::from_mm(1.0));

        // THT pads should have copper on both sides
        assert!(pad1.layers.contains(&InternalLayer::TopCopper));
        // Note: *.Cu maps to TopCopper, defaults add BottomCopper
    }

    #[test]
    fn test_import_soic8() {
        let fp = import_footprint_from_str(SOIC8).unwrap();

        assert_eq!(fp.name, "SOIC-8");
        assert_eq!(fp.pads.len(), 8);

        // Check pin numbering
        for i in 1..=8 {
            let pad = fp.pads.iter().find(|p| p.number == i.to_string());
            assert!(pad.is_some(), "Should have pad {}", i);
        }

        // All pads should be SMD (no drill)
        for pad in &fp.pads {
            assert!(pad.drill.is_none(), "SOIC pads should be SMD");
        }
    }

    #[test]
    fn test_import_oval_pads() {
        let fp = import_footprint_from_str(OVAL_PADS).unwrap();

        assert_eq!(fp.pads.len(), 2);

        let pad = fp.pads.iter().find(|p| p.number == "1").unwrap();
        assert!(matches!(pad.shape, InternalPadShape::Oblong));
    }

    #[test]
    fn test_import_with_courtyard() {
        let fp = import_footprint_from_str(WITH_COURTYARD).unwrap();

        // Courtyard should be extracted from fp_line on F.CrtYd
        // Lines define: (-1.1, -0.8) to (1.1, 0.8)
        assert!(fp.courtyard.min.x <= Nm::from_mm(-1.1));
        assert!(fp.courtyard.max.x >= Nm::from_mm(1.1));
        assert!(fp.courtyard.min.y <= Nm::from_mm(-0.8));
        assert!(fp.courtyard.max.y >= Nm::from_mm(0.8));
    }

    #[test]
    fn test_smd_pad_has_correct_layers() {
        let fp = import_footprint_from_str(MINIMAL_0402).unwrap();
        let pad = fp.pads.iter().find(|p| p.number == "1").unwrap();

        assert!(pad.layers.contains(&InternalLayer::TopCopper));
        assert!(pad.layers.contains(&InternalLayer::TopPaste));
        assert!(pad.layers.contains(&InternalLayer::TopMask));
        assert!(!pad.layers.contains(&InternalLayer::BottomCopper));
    }

    #[test]
    fn test_pad_positions_are_symmetric() {
        let fp = import_footprint_from_str(MINIMAL_0402).unwrap();
        let pad1 = fp.pads.iter().find(|p| p.number == "1").unwrap();
        let pad2 = fp.pads.iter().find(|p| p.number == "2").unwrap();

        // Pads should be symmetric about Y axis
        assert_eq!(pad1.position.x.0, -pad2.position.x.0);
        assert_eq!(pad1.position.y, pad2.position.y);
    }

    #[test]
    fn test_bounds_calculated_correctly() {
        let fp = import_footprint_from_str(MINIMAL_0402).unwrap();

        // Pad 1: at (-0.5, 0), size (0.6, 0.5) -> extends from -0.8 to -0.2 in X
        // Pad 2: at (0.5, 0), size (0.6, 0.5) -> extends from 0.2 to 0.8 in X
        // Y extends from -0.25 to 0.25

        assert!(fp.bounds.min.x <= Nm::from_mm(-0.8));
        assert!(fp.bounds.max.x >= Nm::from_mm(0.8));
    }

    #[test]
    fn test_courtyard_fallback_adds_margin() {
        let fp = import_footprint_from_str(MINIMAL_0402).unwrap();

        // No explicit courtyard, should add 0.5mm margin to bounds
        let margin = Nm::from_mm(0.5);
        assert!(fp.courtyard.min.x < fp.bounds.min.x);
        assert!(fp.courtyard.max.x > fp.bounds.max.x);
        assert_eq!(fp.courtyard.min.x, fp.bounds.min.x - margin);
        assert_eq!(fp.courtyard.max.x, fp.bounds.max.x + margin);
    }

    #[test]
    fn test_negative_pad_positions() {
        // SOIC-8 has pads at negative X and Y positions
        let fp = import_footprint_from_str(SOIC8).unwrap();

        let pad1 = fp.pads.iter().find(|p| p.number == "1").unwrap();
        assert!(pad1.position.x.0 < 0, "Pad 1 should have negative X");
        assert!(pad1.position.y.0 < 0, "Pad 1 should have negative Y");
    }
}
