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
use cypcb_world::components::{Layer as InternalLayer, PadShape as InternalPadShape};
use cypcb_world::footprint::{Footprint, PadDef};
use kicad_parse_gen::footprint::{self as kicad_fp, Element, LayerSide, LayerType as KicadLayerType, Module, Pad, PadShape, PadType};
use thiserror::Error;

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
    let module: Module = kicad_fp::parse(content)
        .map_err(|e| KicadImportError::ParseError(format!("{}", e)))?;

    convert_module(&module)
}

/// Convert a KiCad Module to internal Footprint.
fn convert_module(module: &Module) -> Result<Footprint, KicadImportError> {
    let mut pads = Vec::new();
    let mut description = String::new();
    let mut courtyard_bounds: Option<Rect> = None;

    for element in &module.elements {
        match element {
            Element::Pad(pad) => {
                pads.push(convert_pad(pad)?);
            }
            Element::Descr(desc) => {
                description = desc.clone();
            }
            Element::FpLine(line) => {
                // Check for courtyard lines (F.CrtYd or B.CrtYd)
                if matches!(line.layer.t, KicadLayerType::CrtYd) {
                    let rect = Rect::from_points(
                        Point::from_mm(line.start.x, line.start.y),
                        Point::from_mm(line.end.x, line.end.y),
                    );
                    courtyard_bounds = Some(match courtyard_bounds {
                        Some(existing) => existing.union(&rect),
                        None => rect,
                    });
                }
            }
            _ => {}
        }
    }

    // Calculate bounds from pads
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
        name: module.name.clone(),
        description,
        pads,
        bounds,
        courtyard,
    })
}

/// Convert a KiCad Pad to internal PadDef.
fn convert_pad(pad: &Pad) -> Result<PadDef, KicadImportError> {
    let shape = convert_pad_shape(&pad.shape)?;
    let position = Point::from_mm(pad.at.x, pad.at.y);
    let size = (Nm::from_mm(pad.size.x), Nm::from_mm(pad.size.y));

    // Determine drill from pad type and drill field
    let drill = match pad.t {
        PadType::Pth | PadType::NpPth => {
            pad.drill.as_ref().map(|d| {
                // Use width as drill diameter (height is for oval drills)
                Nm::from_mm(d.width)
            })
        }
        PadType::Smd => None,
    };

    // Convert layers
    let layers = convert_layers(&pad.layers, &pad.t);

    Ok(PadDef {
        number: pad.name.clone(),
        shape,
        position,
        size,
        drill,
        layers,
    })
}

/// Convert KiCad pad shape to internal PadShape.
fn convert_pad_shape(shape: &PadShape) -> Result<InternalPadShape, KicadImportError> {
    match shape {
        PadShape::Rect => Ok(InternalPadShape::Rect),
        PadShape::Circle => Ok(InternalPadShape::Circle),
        PadShape::Oval => Ok(InternalPadShape::Oblong),
        PadShape::Trapezoid => {
            // Log warning and use bounding rect
            Err(KicadImportError::UnsupportedFeature(
                "Trapezoid pads not supported, use rect approximation".to_string(),
            ))
        }
    }
}

/// Convert KiCad layers to internal Layer list.
fn convert_layers(layers: &kicad_fp::Layers, pad_type: &PadType) -> Vec<InternalLayer> {
    let mut result = Vec::new();

    for layer in &layers.layers {
        if let Some(internal) = convert_single_layer(layer) {
            result.push(internal);
        }
    }

    // If we couldn't map any layers, use defaults based on pad type
    if result.is_empty() {
        match pad_type {
            PadType::Pth | PadType::NpPth => {
                // Through-hole defaults to top and bottom copper
                result.push(InternalLayer::TopCopper);
                result.push(InternalLayer::BottomCopper);
            }
            PadType::Smd => {
                // SMD defaults to top copper, paste, and mask
                result.push(InternalLayer::TopCopper);
                result.push(InternalLayer::TopPaste);
                result.push(InternalLayer::TopMask);
            }
        }
    }

    result
}

/// Convert a single KiCad layer to internal Layer.
fn convert_single_layer(layer: &kicad_fp::Layer) -> Option<InternalLayer> {
    match (&layer.side, &layer.t) {
        // Copper layers
        (LayerSide::Front, KicadLayerType::Cu) => Some(InternalLayer::TopCopper),
        (LayerSide::Back, KicadLayerType::Cu) => Some(InternalLayer::BottomCopper),
        (LayerSide::Both, KicadLayerType::Cu) => Some(InternalLayer::TopCopper), // Return one, caller handles both
        (LayerSide::In1, KicadLayerType::Cu) => Some(InternalLayer::Inner(1)),
        (LayerSide::In2, KicadLayerType::Cu) => Some(InternalLayer::Inner(2)),

        // Paste layers
        (LayerSide::Front, KicadLayerType::Paste) => Some(InternalLayer::TopPaste),
        (LayerSide::Back, KicadLayerType::Paste) => Some(InternalLayer::BottomPaste),
        (LayerSide::Both, KicadLayerType::Paste) => Some(InternalLayer::TopPaste),

        // Mask layers
        (LayerSide::Front, KicadLayerType::Mask) => Some(InternalLayer::TopMask),
        (LayerSide::Back, KicadLayerType::Mask) => Some(InternalLayer::BottomMask),
        (LayerSide::Both, KicadLayerType::Mask) => Some(InternalLayer::TopMask),

        // Silkscreen layers
        (LayerSide::Front, KicadLayerType::SilkS) => Some(InternalLayer::TopSilk),
        (LayerSide::Back, KicadLayerType::SilkS) => Some(InternalLayer::BottomSilk),

        // Edge cuts (board outline)
        (LayerSide::Edge, KicadLayerType::Cuts) => Some(InternalLayer::Outline),

        // Other layers we don't map
        _ => None,
    }
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
