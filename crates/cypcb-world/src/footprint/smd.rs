//! SMD (Surface Mount Device) footprint generators.
//!
//! This module provides functions that generate standard SMD footprints
//! following IPC-7351B guidelines for land pattern dimensions.
//!
//! # Supported Packages
//!
//! - **0402** (1005 metric): Chip resistors/capacitors 1.0mm x 0.5mm
//! - **0603** (1608 metric): Chip resistors/capacitors 1.6mm x 0.8mm
//! - **0805** (2012 metric): Chip resistors/capacitors 2.0mm x 1.25mm
//! - **1206** (3216 metric): Chip resistors/capacitors 3.2mm x 1.6mm
//! - **2512** (6332 metric): High-power resistors 6.3mm x 3.2mm
//!
//! # Example
//!
//! ```
//! use cypcb_world::footprint::FootprintLibrary;
//!
//! let lib = FootprintLibrary::new();
//! let fp = lib.get("0603").unwrap();
//!
//! // 0603 has 2 pads
//! assert_eq!(fp.pads.len(), 2);
//!
//! // Pads are symmetric about origin
//! let pad1 = fp.get_pad("1").unwrap();
//! let pad2 = fp.get_pad("2").unwrap();
//! assert_eq!(pad1.position.x.0, -pad2.position.x.0);
//! ```

use cypcb_core::{Nm, Point, Rect};

use super::library::{Footprint, PadDef};
use crate::components::{Layer, PadShape};

/// Generate a standard 2-pad chip footprint.
///
/// This helper creates symmetric footprints for chip resistors, capacitors,
/// inductors, and similar 2-terminal SMD components.
///
/// # Parameters
///
/// - `name`: Footprint identifier (e.g., "0402")
/// - `description`: Human-readable description
/// - `pad_width`: Width of each pad (X dimension)
/// - `pad_height`: Height of each pad (Y dimension)
/// - `pad_span`: Center-to-center distance between pads (X direction)
/// - `body_width`: Component body width (for bounds)
/// - `body_height`: Component body height (for bounds)
fn chip_footprint(
    name: &str,
    description: &str,
    pad_width: Nm,
    pad_height: Nm,
    pad_span: Nm,
    body_width: Nm,
    body_height: Nm,
) -> Footprint {
    let half_span = Nm(pad_span.0 / 2);

    // Standard SMD pad layers
    let smd_layers = vec![Layer::TopCopper, Layer::TopPaste, Layer::TopMask];

    Footprint {
        name: name.into(),
        description: description.into(),
        pads: vec![
            PadDef {
                number: "1".into(),
                shape: PadShape::Rect,
                position: Point::new(-half_span, Nm::ZERO),
                size: (pad_width, pad_height),
                drill: None,
                layers: smd_layers.clone(),
            },
            PadDef {
                number: "2".into(),
                shape: PadShape::Rect,
                position: Point::new(half_span, Nm::ZERO),
                size: (pad_width, pad_height),
                drill: None,
                layers: smd_layers,
            },
        ],
        bounds: Rect::from_center_size(Point::ORIGIN, (body_width, body_height)),
        // Courtyard: body + 0.25mm clearance per IPC-7351B
        courtyard: Rect::from_center_size(
            Point::ORIGIN,
            (body_width + Nm::from_mm(0.5), body_height + Nm::from_mm(0.5)),
        ),
    }
}

/// 0402 (1005 metric) chip resistor/capacitor footprint.
///
/// Dimensions per IPC-7351B nominal density:
/// - Body: 1.0mm x 0.5mm
/// - Pad: 0.6mm x 0.5mm
/// - Pad span: 1.0mm center-to-center
///
/// # Example
///
/// ```
/// use cypcb_world::footprint::FootprintLibrary;
/// use cypcb_core::Nm;
///
/// let lib = FootprintLibrary::new();
/// let fp = lib.get("0402").unwrap();
///
/// let pad = fp.get_pad("1").unwrap();
/// assert_eq!(pad.size.0, Nm::from_mm(0.6)); // width
/// assert_eq!(pad.size.1, Nm::from_mm(0.5)); // height
/// ```
pub fn chip_0402() -> Footprint {
    chip_footprint(
        "0402",
        "Chip 0402 (1005 metric)",
        Nm::from_mm(0.6),  // pad width
        Nm::from_mm(0.5),  // pad height
        Nm::from_mm(1.0),  // pad span (center-to-center)
        Nm::from_mm(1.0),  // body width
        Nm::from_mm(0.5),  // body height
    )
}

/// 0603 (1608 metric) chip resistor/capacitor footprint.
///
/// Dimensions per IPC-7351B nominal density:
/// - Body: 1.6mm x 0.8mm
/// - Pad: 0.9mm x 0.95mm
/// - Pad span: 1.6mm center-to-center
pub fn chip_0603() -> Footprint {
    chip_footprint(
        "0603",
        "Chip 0603 (1608 metric)",
        Nm::from_mm(0.9),  // pad width
        Nm::from_mm(0.95), // pad height
        Nm::from_mm(1.6),  // pad span
        Nm::from_mm(1.6),  // body width
        Nm::from_mm(0.8),  // body height
    )
}

/// 0805 (2012 metric) chip resistor/capacitor footprint.
///
/// Dimensions per IPC-7351B nominal density:
/// - Body: 2.0mm x 1.25mm
/// - Pad: 1.0mm x 1.45mm
/// - Pad span: 1.9mm center-to-center
pub fn chip_0805() -> Footprint {
    chip_footprint(
        "0805",
        "Chip 0805 (2012 metric)",
        Nm::from_mm(1.0),  // pad width
        Nm::from_mm(1.45), // pad height
        Nm::from_mm(1.9),  // pad span
        Nm::from_mm(2.0),  // body width
        Nm::from_mm(1.25), // body height
    )
}

/// 1206 (3216 metric) chip resistor/capacitor footprint.
///
/// Dimensions per IPC-7351B nominal density:
/// - Body: 3.2mm x 1.6mm
/// - Pad: 1.15mm x 1.8mm
/// - Pad span: 3.4mm center-to-center
pub fn chip_1206() -> Footprint {
    chip_footprint(
        "1206",
        "Chip 1206 (3216 metric)",
        Nm::from_mm(1.15), // pad width
        Nm::from_mm(1.8),  // pad height
        Nm::from_mm(3.4),  // pad span
        Nm::from_mm(3.2),  // body width
        Nm::from_mm(1.6),  // body height
    )
}

/// 2512 (6332 metric) chip resistor footprint.
///
/// Dimensions per IPC-7351B nominal density:
/// - Body: 6.3mm x 3.2mm
/// - Pad: 1.4mm x 3.4mm
/// - Pad span: 6.5mm center-to-center
///
/// Commonly used for high-power resistors (1W+).
pub fn chip_2512() -> Footprint {
    chip_footprint(
        "2512",
        "Chip 2512 (6332 metric)",
        Nm::from_mm(1.4),  // pad width
        Nm::from_mm(3.4),  // pad height
        Nm::from_mm(6.5),  // pad span
        Nm::from_mm(6.3),  // body width
        Nm::from_mm(3.2),  // body height
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chip_0402_dimensions() {
        let fp = chip_0402();
        assert_eq!(fp.name, "0402");
        assert_eq!(fp.pads.len(), 2);

        let pad1 = fp.get_pad("1").unwrap();
        assert_eq!(pad1.size.0, Nm::from_mm(0.6));
        assert_eq!(pad1.size.1, Nm::from_mm(0.5));
        assert!(pad1.drill.is_none()); // SMD - no drill
    }

    #[test]
    fn test_smd_pads_symmetric() {
        let fp = chip_0603();
        let pad1 = fp.get_pad("1").unwrap();
        let pad2 = fp.get_pad("2").unwrap();

        // Pads should be symmetric about Y axis
        assert_eq!(pad1.position.x.0, -pad2.position.x.0);
        assert_eq!(pad1.position.y, pad2.position.y);
        assert_eq!(pad1.position.y, Nm::ZERO);
    }

    #[test]
    fn test_smd_pads_have_correct_layers() {
        let fp = chip_0805();
        let pad = fp.get_pad("1").unwrap();

        assert!(pad.layers.contains(&Layer::TopCopper));
        assert!(pad.layers.contains(&Layer::TopPaste));
        assert!(pad.layers.contains(&Layer::TopMask));
        assert!(!pad.layers.contains(&Layer::BottomCopper));
    }

    #[test]
    fn test_all_smd_footprints_have_two_pads() {
        assert_eq!(chip_0402().pads.len(), 2);
        assert_eq!(chip_0603().pads.len(), 2);
        assert_eq!(chip_0805().pads.len(), 2);
        assert_eq!(chip_1206().pads.len(), 2);
        assert_eq!(chip_2512().pads.len(), 2);
    }

    #[test]
    fn test_pad_span_increases_with_package_size() {
        let fp0402 = chip_0402();
        let fp0603 = chip_0603();
        let fp0805 = chip_0805();
        let fp1206 = chip_1206();
        let fp2512 = chip_2512();

        // Get pad 2 x position (positive side) for each
        let span_0402 = fp0402.get_pad("2").unwrap().position.x.0;
        let span_0603 = fp0603.get_pad("2").unwrap().position.x.0;
        let span_0805 = fp0805.get_pad("2").unwrap().position.x.0;
        let span_1206 = fp1206.get_pad("2").unwrap().position.x.0;
        let span_2512 = fp2512.get_pad("2").unwrap().position.x.0;

        assert!(span_0402 < span_0603);
        assert!(span_0603 < span_0805);
        assert!(span_0805 < span_1206);
        assert!(span_1206 < span_2512);
    }
}
