//! Gull-wing IC footprint generators (SOIC, SOT, QFP).
//!
//! This module provides functions that generate standard gull-wing footprints
//! following IPC-7351B guidelines for land pattern dimensions.
//!
//! # Supported Packages
//!
//! - **SOIC-8**: 8-pin Small Outline IC, 1.27mm pitch
//! - **SOIC-14**: 14-pin Small Outline IC, 1.27mm pitch
//! - **SOT-23**: 3-pin Small Outline Transistor
//! - **SOT-23-5**: 5-pin Small Outline Transistor
//! - **TQFP-32**: 32-pin Thin Quad Flat Package, 0.8mm pitch
//!
//! # Pin Numbering
//!
//! All IC packages use counter-clockwise pin numbering starting from pin 1
//! at the bottom-left (standard IC convention).
//!
//! # Example
//!
//! ```
//! use cypcb_world::footprint::FootprintLibrary;
//!
//! let lib = FootprintLibrary::new();
//! let soic = lib.get("SOIC-8").unwrap();
//!
//! // SOIC-8 has 8 pins
//! assert_eq!(soic.pads.len(), 8);
//!
//! // All pads are SMD (no drill)
//! for pad in &soic.pads {
//!     assert!(pad.drill.is_none());
//! }
//! ```

use cypcb_core::{Nm, Point, Rect};

use super::library::{Footprint, PadDef};
use crate::components::{Layer, PadShape};

/// Standard SMD pad layers for gull-wing packages.
fn smd_layers() -> Vec<Layer> {
    vec![Layer::TopCopper, Layer::TopPaste, Layer::TopMask]
}

/// Generate a dual-row gull-wing IC footprint (SOIC, SSOP, etc).
///
/// This helper creates symmetric footprints for dual-row gull-wing ICs
/// where pins are arranged on two opposite sides.
///
/// # Arguments
///
/// * `name` - Footprint identifier (e.g., "SOIC-8")
/// * `description` - Human-readable description
/// * `pin_count` - Total number of pins (must be even)
/// * `pitch` - Pin pitch (center-to-center between adjacent pins)
/// * `pad_width` - Width of each pad (perpendicular to pin row)
/// * `pad_length` - Length of each pad (along pin row direction)
/// * `row_span` - Distance between pad row centers (X direction)
/// * `body_size` - Component body dimensions (width, height)
///
/// # Pin Numbering
///
/// Pins are numbered counter-clockwise starting from bottom-left:
/// - Left side: pins 1 to n/2 (bottom to top)
/// - Right side: pins n to n/2+1 (top to bottom)
///
/// ```text
///     Pin n/2  ...  Pin 2  Pin 1
///         |          |      |
///       +-------------------+
///       |                   |
///       +-------------------+
///         |          |      |
///     Pin n/2+1 ... Pin n-1 Pin n
/// ```
///
/// # Panics
///
/// Panics if `pin_count` is not even.
pub fn gullwing_footprint(
    name: &str,
    description: &str,
    pin_count: usize,
    pitch: Nm,
    pad_width: Nm,
    pad_length: Nm,
    row_span: Nm,
    body_size: (Nm, Nm),
) -> Footprint {
    assert!(
        pin_count % 2 == 0,
        "Pin count must be even for dual-row package"
    );

    let pins_per_side = pin_count / 2;
    let half_span = Nm(row_span.0 / 2);
    let layers = smd_layers();

    let mut pads = Vec::with_capacity(pin_count);

    // Calculate vertical offset for centering
    let total_height = Nm(pitch.0 * (pins_per_side - 1) as i64);
    let y_offset = Nm(total_height.0 / 2);

    // Left side (pins 1 to pins_per_side), bottom to top
    for i in 0..pins_per_side {
        let pin_num = i + 1;
        let y = Nm(i as i64 * pitch.0) - y_offset;

        pads.push(PadDef {
            number: pin_num.to_string(),
            shape: PadShape::Rect,
            position: Point::new(-half_span, y),
            size: (pad_length, pad_width), // Horizontal pad orientation
            drill: None,
            layers: layers.clone(),
        });
    }

    // Right side (pins pin_count down to pins_per_side+1), top to bottom
    for i in 0..pins_per_side {
        let pin_num = pin_count - i;
        let y = Nm((pins_per_side - 1 - i) as i64 * pitch.0) - y_offset;

        pads.push(PadDef {
            number: pin_num.to_string(),
            shape: PadShape::Rect,
            position: Point::new(half_span, y),
            size: (pad_length, pad_width),
            drill: None,
            layers: layers.clone(),
        });
    }

    // Courtyard: body + 0.5mm margin (0.25mm each side per IPC-7351B)
    let courtyard_margin = Nm::from_mm(0.5);

    Footprint {
        name: name.into(),
        description: description.into(),
        pads,
        bounds: Rect::from_center_size(Point::ORIGIN, body_size),
        courtyard: Rect::from_center_size(
            Point::ORIGIN,
            (
                body_size.0 + courtyard_margin,
                body_size.1 + courtyard_margin,
            ),
        ),
    }
}

/// SOIC-8 (150mil body width, 1.27mm pitch) footprint.
///
/// Standard Small Outline IC with 8 pins, commonly used for:
/// - Op-amps (LM358, NE5532)
/// - Timers (NE555)
/// - Voltage regulators
/// - EEPROM (24C02)
///
/// Dimensions per IPC-7351B:
/// - Pitch: 1.27mm
/// - Pad: 1.5mm x 0.6mm
/// - Row span: 5.4mm (center-to-center)
/// - Body: 5.0mm x 4.0mm
///
/// # Example
///
/// ```
/// use cypcb_world::footprint::gullwing::soic8;
///
/// let fp = soic8();
/// assert_eq!(fp.pads.len(), 8);
/// assert!(fp.get_pad("1").is_some());
/// assert!(fp.get_pad("8").is_some());
/// ```
pub fn soic8() -> Footprint {
    gullwing_footprint(
        "SOIC-8",
        "Small Outline IC, 8 pins, 1.27mm pitch",
        8,
        Nm::from_mm(1.27),                    // pitch
        Nm::from_mm(0.6),                     // pad width
        Nm::from_mm(1.5),                     // pad length
        Nm::from_mm(5.4),                     // row span
        (Nm::from_mm(5.0), Nm::from_mm(4.0)), // body
    )
}

/// SOIC-14 (150mil body width, 1.27mm pitch) footprint.
///
/// Standard Small Outline IC with 14 pins, commonly used for:
/// - Quad op-amps (LM324, TL074)
/// - Logic ICs (74HC series)
/// - Motor drivers
///
/// Dimensions per IPC-7351B:
/// - Pitch: 1.27mm
/// - Pad: 1.5mm x 0.6mm
/// - Row span: 5.4mm (center-to-center)
/// - Body: 5.0mm x 8.7mm
///
/// # Example
///
/// ```
/// use cypcb_world::footprint::gullwing::soic14;
///
/// let fp = soic14();
/// assert_eq!(fp.pads.len(), 14);
/// ```
pub fn soic14() -> Footprint {
    gullwing_footprint(
        "SOIC-14",
        "Small Outline IC, 14 pins, 1.27mm pitch",
        14,
        Nm::from_mm(1.27),
        Nm::from_mm(0.6),
        Nm::from_mm(1.5),
        Nm::from_mm(5.4),
        (Nm::from_mm(5.0), Nm::from_mm(8.7)),
    )
}

/// SOT-23 (3-pin Small Outline Transistor) footprint.
///
/// Asymmetric 3-pin package commonly used for:
/// - Small signal transistors (2N7002, MMBT3904)
/// - Voltage references
/// - Single-gate logic
///
/// Pin layout (asymmetric):
/// ```text
///         Pin 3
///           |
///     +----------+
///     |          |
///     +----------+
///       |      |
///     Pin 1  Pin 2
/// ```
///
/// Dimensions:
/// - Pin 1: left side at (-0.95mm, -1.0mm)
/// - Pin 2: right side at (0.95mm, -1.0mm)
/// - Pin 3: top center at (0mm, 1.0mm)
/// - Pad size: 0.6mm x 1.0mm
/// - Body: 3.0mm x 2.5mm
///
/// # Example
///
/// ```
/// use cypcb_world::footprint::gullwing::sot23;
///
/// let fp = sot23();
/// assert_eq!(fp.pads.len(), 3);
/// ```
pub fn sot23() -> Footprint {
    let layers = smd_layers();

    Footprint {
        name: "SOT-23".into(),
        description: "Small Outline Transistor, 3 pins".into(),
        pads: vec![
            // Pin 1: left side bottom
            PadDef {
                number: "1".into(),
                shape: PadShape::Rect,
                position: Point::new(Nm::from_mm(-0.95), Nm::from_mm(-1.0)),
                size: (Nm::from_mm(0.6), Nm::from_mm(1.0)),
                drill: None,
                layers: layers.clone(),
            },
            // Pin 2: right side bottom
            PadDef {
                number: "2".into(),
                shape: PadShape::Rect,
                position: Point::new(Nm::from_mm(0.95), Nm::from_mm(-1.0)),
                size: (Nm::from_mm(0.6), Nm::from_mm(1.0)),
                drill: None,
                layers: layers.clone(),
            },
            // Pin 3: top center
            PadDef {
                number: "3".into(),
                shape: PadShape::Rect,
                position: Point::new(Nm::ZERO, Nm::from_mm(1.0)),
                size: (Nm::from_mm(0.6), Nm::from_mm(1.0)),
                drill: None,
                layers,
            },
        ],
        bounds: Rect::from_center_size(
            Point::ORIGIN,
            (Nm::from_mm(3.0), Nm::from_mm(2.5)),
        ),
        courtyard: Rect::from_center_size(
            Point::ORIGIN,
            (Nm::from_mm(3.5), Nm::from_mm(3.0)),
        ),
    }
}

/// SOT-23-5 (5-pin Small Outline Transistor) footprint.
///
/// 5-pin variant commonly used for:
/// - Voltage regulators (MCP1700, AP2112)
/// - Op-amps (MCP6001)
/// - Analog switches
///
/// Pin layout:
/// ```text
///     Pin 3  Pin 4
///       |      |
///     +----------+
///     |          |
///     +----------+
///       |    |
///     Pin 1  |  Pin 5
///          Pin 2
/// ```
///
/// Pins 1-3 on left side (bottom to top), pins 4-5 on right side (top to bottom).
///
/// Dimensions:
/// - Pitch: 0.95mm
/// - Pad size: 1.0mm x 0.6mm (horizontal)
/// - Row span: 2.4mm (center-to-center)
/// - Body: 3.0mm x 3.0mm
///
/// # Example
///
/// ```
/// use cypcb_world::footprint::gullwing::sot23_5;
///
/// let fp = sot23_5();
/// assert_eq!(fp.pads.len(), 5);
/// ```
pub fn sot23_5() -> Footprint {
    let layers = smd_layers();
    let pitch = Nm::from_mm(0.95);
    let half_span = Nm::from_mm(1.2); // row_span / 2 = 2.4 / 2

    Footprint {
        name: "SOT-23-5".into(),
        description: "Small Outline Transistor, 5 pins".into(),
        pads: vec![
            // Pins 1-3: left side, bottom to top
            PadDef {
                number: "1".into(),
                shape: PadShape::Rect,
                position: Point::new(-half_span, -pitch),
                size: (Nm::from_mm(1.0), Nm::from_mm(0.6)),
                drill: None,
                layers: layers.clone(),
            },
            PadDef {
                number: "2".into(),
                shape: PadShape::Rect,
                position: Point::new(-half_span, Nm::ZERO),
                size: (Nm::from_mm(1.0), Nm::from_mm(0.6)),
                drill: None,
                layers: layers.clone(),
            },
            PadDef {
                number: "3".into(),
                shape: PadShape::Rect,
                position: Point::new(-half_span, pitch),
                size: (Nm::from_mm(1.0), Nm::from_mm(0.6)),
                drill: None,
                layers: layers.clone(),
            },
            // Pins 4-5: right side, top to bottom
            PadDef {
                number: "4".into(),
                shape: PadShape::Rect,
                position: Point::new(half_span, pitch),
                size: (Nm::from_mm(1.0), Nm::from_mm(0.6)),
                drill: None,
                layers: layers.clone(),
            },
            PadDef {
                number: "5".into(),
                shape: PadShape::Rect,
                position: Point::new(half_span, -pitch),
                size: (Nm::from_mm(1.0), Nm::from_mm(0.6)),
                drill: None,
                layers,
            },
        ],
        bounds: Rect::from_center_size(
            Point::ORIGIN,
            (Nm::from_mm(3.0), Nm::from_mm(3.0)),
        ),
        courtyard: Rect::from_center_size(
            Point::ORIGIN,
            (Nm::from_mm(3.5), Nm::from_mm(3.5)),
        ),
    }
}

/// TQFP-32 (7x7mm body, 0.8mm pitch) footprint.
///
/// Thin Quad Flat Package with 32 pins (8 per side), commonly used for:
/// - Microcontrollers (ATmega328, STM32)
/// - Interface ICs
///
/// Pin numbering (counter-clockwise from bottom-left):
/// ```text
///            17 18 19 20 21 22 23 24
///             |  |  |  |  |  |  |  |
///         +---------------------------+
///      16-|                           |-9
///      15-|                           |-10
///      14-|                           |-11
///      13-|                           |-12
///         +---------------------------+
///             |  |  |  |  |  |  |  |
///             1  2  3  4  5  6  7  8
/// ```
///
/// Dimensions:
/// - Pitch: 0.8mm
/// - Pad: 0.45mm x 1.5mm
/// - Row span: 9.0mm (center-to-center)
/// - Body: 7.0mm x 7.0mm
///
/// # Example
///
/// ```
/// use cypcb_world::footprint::gullwing::tqfp32;
///
/// let fp = tqfp32();
/// assert_eq!(fp.pads.len(), 32);
/// ```
pub fn tqfp32() -> Footprint {
    let pin_count = 32;
    let pins_per_side = pin_count / 4;
    let pitch = Nm::from_mm(0.8);
    let pad_width = Nm::from_mm(0.45);
    let pad_length = Nm::from_mm(1.5);
    let body_size = Nm::from_mm(7.0);
    let row_span = Nm::from_mm(9.0);

    let half_span = Nm(row_span.0 / 2);
    let layers = smd_layers();

    let mut pads = Vec::with_capacity(pin_count);

    // Calculate offset for centering pins on each side
    let side_length = Nm(pitch.0 * (pins_per_side - 1) as i64);
    let offset = Nm(side_length.0 / 2);

    let mut pin_num = 1;

    // Bottom side (left to right): pins 1-8
    for i in 0..pins_per_side {
        let x = Nm(i as i64 * pitch.0) - offset;
        pads.push(PadDef {
            number: pin_num.to_string(),
            shape: PadShape::Rect,
            position: Point::new(x, -half_span),
            size: (pad_width, pad_length), // Vertical pad
            drill: None,
            layers: layers.clone(),
        });
        pin_num += 1;
    }

    // Right side (bottom to top): pins 9-16
    for i in 0..pins_per_side {
        let y = Nm(i as i64 * pitch.0) - offset;
        pads.push(PadDef {
            number: pin_num.to_string(),
            shape: PadShape::Rect,
            position: Point::new(half_span, y),
            size: (pad_length, pad_width), // Horizontal pad
            drill: None,
            layers: layers.clone(),
        });
        pin_num += 1;
    }

    // Top side (right to left): pins 17-24
    for i in 0..pins_per_side {
        let x = offset - Nm(i as i64 * pitch.0);
        pads.push(PadDef {
            number: pin_num.to_string(),
            shape: PadShape::Rect,
            position: Point::new(x, half_span),
            size: (pad_width, pad_length), // Vertical pad
            drill: None,
            layers: layers.clone(),
        });
        pin_num += 1;
    }

    // Left side (top to bottom): pins 25-32
    for i in 0..pins_per_side {
        let y = offset - Nm(i as i64 * pitch.0);
        pads.push(PadDef {
            number: pin_num.to_string(),
            shape: PadShape::Rect,
            position: Point::new(-half_span, y),
            size: (pad_length, pad_width), // Horizontal pad
            drill: None,
            layers: layers.clone(),
        });
        pin_num += 1;
    }

    let courtyard_margin = Nm::from_mm(0.5);

    Footprint {
        name: "TQFP-32".into(),
        description: "Thin Quad Flat Package, 32 pins, 0.8mm pitch".into(),
        pads,
        bounds: Rect::from_center_size(Point::ORIGIN, (body_size, body_size)),
        courtyard: Rect::from_center_size(
            Point::ORIGIN,
            (body_size + courtyard_margin, body_size + courtyard_margin),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soic8_dimensions() {
        let fp = soic8();
        assert_eq!(fp.name, "SOIC-8");
        assert_eq!(fp.pads.len(), 8);

        // Check pad dimensions
        let pad1 = fp.get_pad("1").unwrap();
        assert_eq!(pad1.size.0, Nm::from_mm(1.5)); // length
        assert_eq!(pad1.size.1, Nm::from_mm(0.6)); // width
        assert!(pad1.drill.is_none()); // SMD - no drill
    }

    #[test]
    fn test_soic8_pin_positions() {
        let fp = soic8();

        // Pin 1 should be on left side (negative X)
        let pin1 = fp.get_pad("1").unwrap();
        assert!(pin1.position.x.0 < 0);

        // Pin 8 should be on right side (positive X)
        let pin8 = fp.get_pad("8").unwrap();
        assert!(pin8.position.x.0 > 0);

        // Row span should be 5.4mm
        let row_span = (pin8.position.x.0 - pin1.position.x.0).abs();
        assert_eq!(row_span, Nm::from_mm(5.4).0);
    }

    #[test]
    fn test_soic14_has_14_pins() {
        let fp = soic14();
        assert_eq!(fp.name, "SOIC-14");
        assert_eq!(fp.pads.len(), 14);

        // Verify all pins exist
        for i in 1..=14 {
            assert!(fp.get_pad(&i.to_string()).is_some());
        }
    }

    #[test]
    fn test_sot23_has_3_pins() {
        let fp = sot23();
        assert_eq!(fp.name, "SOT-23");
        assert_eq!(fp.pads.len(), 3);

        // Verify asymmetric layout
        let pin1 = fp.get_pad("1").unwrap();
        let pin2 = fp.get_pad("2").unwrap();
        let pin3 = fp.get_pad("3").unwrap();

        // Pins 1 and 2 on bottom
        assert_eq!(pin1.position.y, pin2.position.y);
        assert!(pin1.position.y.0 < 0);

        // Pin 3 on top
        assert!(pin3.position.y.0 > 0);

        // Pin 3 centered horizontally
        assert_eq!(pin3.position.x, Nm::ZERO);
    }

    #[test]
    fn test_sot23_5_has_5_pins() {
        let fp = sot23_5();
        assert_eq!(fp.name, "SOT-23-5");
        assert_eq!(fp.pads.len(), 5);

        // Pins 1-3 on left, 4-5 on right
        let pin1 = fp.get_pad("1").unwrap();
        let pin4 = fp.get_pad("4").unwrap();

        assert!(pin1.position.x.0 < 0);
        assert!(pin4.position.x.0 > 0);
    }

    #[test]
    fn test_tqfp32_has_32_pins() {
        let fp = tqfp32();
        assert_eq!(fp.name, "TQFP-32");
        assert_eq!(fp.pads.len(), 32);

        // Verify all pins exist
        for i in 1..=32 {
            assert!(
                fp.get_pad(&i.to_string()).is_some(),
                "Pin {} should exist",
                i
            );
        }
    }

    #[test]
    fn test_tqfp32_pin_positions() {
        let fp = tqfp32();

        // Pin 1 should be on bottom
        let pin1 = fp.get_pad("1").unwrap();
        assert!(pin1.position.y.0 < 0);

        // Pin 9 should be on right
        let pin9 = fp.get_pad("9").unwrap();
        assert!(pin9.position.x.0 > 0);

        // Pin 17 should be on top
        let pin17 = fp.get_pad("17").unwrap();
        assert!(pin17.position.y.0 > 0);

        // Pin 25 should be on left
        let pin25 = fp.get_pad("25").unwrap();
        assert!(pin25.position.x.0 < 0);
    }

    #[test]
    fn test_gullwing_pads_are_smd() {
        let fp = soic8();
        for pad in &fp.pads {
            assert!(pad.drill.is_none(), "Gull-wing pads should be SMD");
            assert!(pad.layers.contains(&Layer::TopCopper));
            assert!(pad.layers.contains(&Layer::TopPaste));
            assert!(pad.layers.contains(&Layer::TopMask));
        }
    }

    #[test]
    fn test_courtyard_includes_margin() {
        let fp = soic8();

        // Courtyard should be larger than bounds
        assert!(fp.courtyard.width() > fp.bounds.width());
        assert!(fp.courtyard.height() > fp.bounds.height());

        // Margin should be 0.5mm (0.25mm each side)
        let width_diff = fp.courtyard.width().0 - fp.bounds.width().0;
        let height_diff = fp.courtyard.height().0 - fp.bounds.height().0;

        assert_eq!(width_diff, Nm::from_mm(0.5).0);
        assert_eq!(height_diff, Nm::from_mm(0.5).0);
    }

    #[test]
    #[should_panic(expected = "Pin count must be even")]
    fn test_gullwing_rejects_odd_pin_count() {
        gullwing_footprint(
            "BAD",
            "Invalid",
            7, // Odd pin count
            Nm::from_mm(1.0),
            Nm::from_mm(0.5),
            Nm::from_mm(1.0),
            Nm::from_mm(5.0),
            (Nm::from_mm(4.0), Nm::from_mm(4.0)),
        );
    }
}
