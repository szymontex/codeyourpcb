//! Through-hole (THT) footprint generators.
//!
//! This module provides functions that generate standard through-hole footprints
//! for components with leads that pass through the PCB.
//!
//! # Supported Packages
//!
//! - **AXIAL-300**: 300mil lead spacing for axial resistors, diodes
//! - **DIP-8**: 8-pin dual in-line package (300mil row spacing)
//! - **PIN-HDR-1x2**: 2-pin header (100mil pitch)
//!
//! # Example
//!
//! ```
//! use cypcb_world::footprint::FootprintLibrary;
//!
//! let lib = FootprintLibrary::new();
//! let dip = lib.get("DIP-8").unwrap();
//!
//! // DIP-8 has 8 pins
//! assert_eq!(dip.pads.len(), 8);
//!
//! // All pads are through-hole
//! for pad in &dip.pads {
//!     assert!(pad.drill.is_some());
//! }
//! ```

use cypcb_core::{Nm, Point, Rect};

use super::library::{Footprint, PadDef};
use crate::components::{Layer, PadShape};

/// Standard through-hole pad layers (top and bottom copper).
fn tht_layers() -> Vec<Layer> {
    vec![Layer::TopCopper, Layer::BottomCopper]
}

/// Axial 300mil (7.62mm) lead spacing footprint.
///
/// Suitable for:
/// - 1/4W through-hole resistors
/// - Small signal diodes
/// - Glass-body components
///
/// Dimensions:
/// - Lead spacing: 300mil (7.62mm)
/// - Drill: 0.8mm (for standard 0.6mm leads)
/// - Pad: 1.6mm circular
pub fn axial_300mil() -> Footprint {
    let lead_spacing = Nm::from_mil(300.0);
    let half_spacing = Nm(lead_spacing.0 / 2);
    let drill = Nm::from_mm(0.8);
    let pad_size = Nm::from_mm(1.6);

    Footprint {
        name: "AXIAL-300".into(),
        description: "Axial component 300mil (7.62mm) lead spacing".into(),
        pads: vec![
            PadDef {
                number: "1".into(),
                shape: PadShape::Circle,
                position: Point::new(-half_spacing, Nm::ZERO),
                size: (pad_size, pad_size),
                drill: Some(drill),
                layers: tht_layers(),
            },
            PadDef {
                number: "2".into(),
                shape: PadShape::Circle,
                position: Point::new(half_spacing, Nm::ZERO),
                size: (pad_size, pad_size),
                drill: Some(drill),
                layers: tht_layers(),
            },
        ],
        bounds: Rect::from_center_size(
            Point::ORIGIN,
            (lead_spacing + pad_size, Nm::from_mm(2.5)),
        ),
        courtyard: Rect::from_center_size(
            Point::ORIGIN,
            (lead_spacing + pad_size + Nm::from_mm(0.5), Nm::from_mm(3.5)),
        ),
    }
}

/// DIP-8 (Dual In-line Package, 8 pins) footprint.
///
/// Standard 300mil row spacing DIP package with 100mil pin pitch.
/// Commonly used for op-amps (LM358, TL072), timers (555), etc.
///
/// Dimensions:
/// - Row spacing: 300mil (7.62mm)
/// - Pin pitch: 100mil (2.54mm)
/// - Drill: 0.8mm
/// - Pad: 1.6mm x 1.6mm oblong
///
/// Pin numbering follows standard convention:
/// ```text
///        Pin 1  Pin 2  Pin 3  Pin 4
///          |      |      |      |
///        +---------------------------+
///        |  U (notch)                |
///        +---------------------------+
///          |      |      |      |
///        Pin 8  Pin 7  Pin 6  Pin 5
/// ```
pub fn dip8() -> Footprint {
    let row_spacing = Nm::from_mil(300.0);
    let _pin_pitch = Nm::from_mil(100.0); // Used for documentation, actual offsets are hard-coded
    let half_row = Nm(row_spacing.0 / 2);
    let drill = Nm::from_mm(0.8);
    let pad_width = Nm::from_mm(1.6);
    let pad_height = Nm::from_mm(1.6);

    // 4 pins per side, centered around origin
    // Pins 1-4 on top row (positive Y), pins 5-8 on bottom row (negative Y)
    // Actually, standard DIP has pins 1-4 on one side, 5-8 on opposite side
    // Pin 1 is at top-left, numbering goes counter-clockwise

    let mut pads = Vec::with_capacity(8);

    // Pins 1-4 on left side (negative X), from top to bottom
    for i in 0..4 {
        let pin_num = i + 1;
        let y_offset = Nm::from_mil(150.0 - (i as f64 * 100.0)); // 150, 50, -50, -150 mils
        pads.push(PadDef {
            number: pin_num.to_string(),
            shape: PadShape::Oblong,
            position: Point::new(-half_row, y_offset),
            size: (pad_width, pad_height),
            drill: Some(drill),
            layers: tht_layers(),
        });
    }

    // Pins 5-8 on right side (positive X), from bottom to top
    for i in 0..4 {
        let pin_num = i + 5;
        let y_offset = Nm::from_mil(-150.0 + (i as f64 * 100.0)); // -150, -50, 50, 150 mils
        pads.push(PadDef {
            number: pin_num.to_string(),
            shape: PadShape::Oblong,
            position: Point::new(half_row, y_offset),
            size: (pad_width, pad_height),
            drill: Some(drill),
            layers: tht_layers(),
        });
    }

    let body_width = row_spacing + Nm::from_mm(1.5); // Pins + body overhang
    let body_height = Nm::from_mil(400.0); // 4 pins * 100mil pitch

    Footprint {
        name: "DIP-8".into(),
        description: "8-pin DIP, 300mil row spacing".into(),
        pads,
        bounds: Rect::from_center_size(Point::ORIGIN, (body_width, body_height)),
        courtyard: Rect::from_center_size(
            Point::ORIGIN,
            (body_width + Nm::from_mm(0.5), body_height + Nm::from_mm(0.5)),
        ),
    }
}

/// 1x2 pin header footprint.
///
/// Standard 100mil (2.54mm) pitch header connector.
///
/// Dimensions:
/// - Pin pitch: 100mil (2.54mm)
/// - Drill: 1.0mm (for standard 0.64mm square pins)
/// - Pad: 1.7mm circular
pub fn pin_header_1x2() -> Footprint {
    let pin_pitch = Nm::from_mil(100.0);
    let half_pitch = Nm(pin_pitch.0 / 2);
    let drill = Nm::from_mm(1.0);
    let pad_size = Nm::from_mm(1.7);

    Footprint {
        name: "PIN-HDR-1x2".into(),
        description: "1x2 pin header, 100mil (2.54mm) pitch".into(),
        pads: vec![
            PadDef {
                number: "1".into(),
                shape: PadShape::Rect, // Square pad for pin 1
                position: Point::new(-half_pitch, Nm::ZERO),
                size: (pad_size, pad_size),
                drill: Some(drill),
                layers: tht_layers(),
            },
            PadDef {
                number: "2".into(),
                shape: PadShape::Circle,
                position: Point::new(half_pitch, Nm::ZERO),
                size: (pad_size, pad_size),
                drill: Some(drill),
                layers: tht_layers(),
            },
        ],
        bounds: Rect::from_center_size(
            Point::ORIGIN,
            (pin_pitch + pad_size, Nm::from_mm(2.54)),
        ),
        courtyard: Rect::from_center_size(
            Point::ORIGIN,
            (pin_pitch + pad_size + Nm::from_mm(0.5), Nm::from_mm(3.5)),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_axial_300mil_dimensions() {
        let fp = axial_300mil();
        assert_eq!(fp.name, "AXIAL-300");
        assert_eq!(fp.pads.len(), 2);

        let pad1 = fp.get_pad("1").unwrap();
        let pad2 = fp.get_pad("2").unwrap();

        // Check drill exists (through-hole)
        assert!(pad1.drill.is_some());
        assert_eq!(pad1.drill.unwrap(), Nm::from_mm(0.8));

        // Check spacing is 300mil = 7.62mm
        let spacing = (pad2.position.x.0 - pad1.position.x.0).abs();
        assert_eq!(spacing, Nm::from_mil(300.0).0);
    }

    #[test]
    fn test_dip8_has_8_pins() {
        let fp = dip8();
        assert_eq!(fp.name, "DIP-8");
        assert_eq!(fp.pads.len(), 8);

        // All pins have drill holes
        for pad in &fp.pads {
            assert!(pad.drill.is_some());
        }

        // Pin numbering 1-8
        for i in 1..=8 {
            assert!(fp.get_pad(&i.to_string()).is_some());
        }
    }

    #[test]
    fn test_dip8_row_spacing() {
        let fp = dip8();

        // Pin 1 and Pin 8 should be on opposite sides
        let pin1 = fp.get_pad("1").unwrap();
        let pin8 = fp.get_pad("8").unwrap();

        // They should have opposite X positions
        assert!(pin1.position.x.0 < 0);
        assert!(pin8.position.x.0 > 0);

        // Row spacing should be 300mil
        let row_spacing = (pin8.position.x.0 - pin1.position.x.0).abs();
        assert_eq!(row_spacing, Nm::from_mil(300.0).0);
    }

    #[test]
    fn test_pin_header_1x2() {
        let fp = pin_header_1x2();
        assert_eq!(fp.name, "PIN-HDR-1x2");
        assert_eq!(fp.pads.len(), 2);

        let pad1 = fp.get_pad("1").unwrap();
        let pad2 = fp.get_pad("2").unwrap();

        // Pin 1 is square (rect), pin 2 is round
        assert_eq!(pad1.shape, PadShape::Rect);
        assert_eq!(pad2.shape, PadShape::Circle);

        // Drill for 0.64mm square pins
        assert_eq!(pad1.drill.unwrap(), Nm::from_mm(1.0));

        // Pitch is 100mil = 2.54mm
        let pitch = (pad2.position.x.0 - pad1.position.x.0).abs();
        assert_eq!(pitch, Nm::from_mil(100.0).0);
    }

    #[test]
    fn test_tht_pads_have_correct_layers() {
        let fp = axial_300mil();
        let pad = fp.get_pad("1").unwrap();

        // Through-hole pads should be on top and bottom copper
        assert!(pad.layers.contains(&Layer::TopCopper));
        assert!(pad.layers.contains(&Layer::BottomCopper));
        assert_eq!(pad.layers.len(), 2);
    }
}
