//! Gerber aperture management for D-code generation.
//!
//! Apertures define the "tools" used to draw features in Gerber files.
//! Each unique pad shape gets a D-code (D10, D11, etc.) that is defined
//! once in the aperture section and reused throughout the file.

use crate::coords::{nm_to_decimal, CoordinateFormat};
use cypcb_world::components::PadShape as WorldPadShape;
use cypcb_world::footprint::PadDef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Aperture shape for Gerber D-code definition.
///
/// Represents the physical shapes that can be drawn in Gerber files.
/// All dimensions are in nanometers for consistency with internal representation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ApertureShape {
    /// Circular aperture (diameter in nm)
    Circle { diameter: i64 },
    /// Rectangular aperture (width x height in nm)
    Rectangle { width: i64, height: i64 },
    /// Oblong/oval aperture (width x height in nm, stadium shape)
    Oblong { width: i64, height: i64 },
    /// Rounded rectangle (width x height in nm, corner radius ratio 0-50%)
    RoundRect {
        width: i64,
        height: i64,
        corner_ratio: u8,
    },
}

/// Manages aperture definitions and D-code assignment.
///
/// Automatically assigns unique D-codes to aperture shapes and generates
/// the corresponding Gerber aperture definition statements.
///
/// # Examples
///
/// ```
/// use cypcb_export::apertures::{ApertureManager, ApertureShape};
/// use cypcb_export::coords::CoordinateFormat;
/// use cypcb_core::Nm;
///
/// let mut manager = ApertureManager::new();
/// let format = CoordinateFormat::FORMAT_MM_2_6;
///
/// // Create apertures - same shape returns same D-code
/// let d1 = manager.get_or_create(ApertureShape::Circle { diameter: Nm::from_mm(1.0).0 });
/// let d2 = manager.get_or_create(ApertureShape::Circle { diameter: Nm::from_mm(1.0).0 });
/// assert_eq!(d1, d2);
/// assert_eq!(d1, 10); // First D-code is 10
///
/// // Different shape gets new D-code
/// let d3 = manager.get_or_create(ApertureShape::Rectangle {
///     width: Nm::from_mm(1.0).0,
///     height: Nm::from_mm(0.5).0,
/// });
/// assert_eq!(d3, 11);
///
/// // Generate definitions
/// let definitions = manager.to_definitions(&format);
/// assert!(definitions.contains("%ADD10C,1.000000*%"));
/// ```
#[derive(Debug, Default)]
pub struct ApertureManager {
    /// Next D-code to assign (starts at 10 per Gerber convention)
    next_dcode: u16,
    /// Map from aperture shape to assigned D-code
    apertures: HashMap<ApertureShape, u16>,
}

impl ApertureManager {
    /// Create a new aperture manager.
    ///
    /// D-codes start at 10 (D01-D03 are reserved for draw/move/flash commands).
    pub fn new() -> Self {
        Self {
            next_dcode: 10,
            apertures: HashMap::new(),
        }
    }

    /// Get or create a D-code for the given aperture shape.
    ///
    /// If the shape already exists, returns its existing D-code.
    /// Otherwise, assigns a new D-code and returns it.
    ///
    /// # Arguments
    ///
    /// * `shape` - The aperture shape to get or create
    ///
    /// # Returns
    ///
    /// The D-code (10, 11, 12, ...) for this aperture shape.
    pub fn get_or_create(&mut self, shape: ApertureShape) -> u16 {
        if let Some(&dcode) = self.apertures.get(&shape) {
            dcode
        } else {
            let dcode = self.next_dcode;
            self.next_dcode += 1;
            self.apertures.insert(shape, dcode);
            dcode
        }
    }

    /// Generate Gerber aperture definition statements for all registered apertures.
    ///
    /// Returns a string containing all %ADD...% statements, one per line.
    ///
    /// # Arguments
    ///
    /// * `format` - Coordinate format for dimension conversion
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_export::apertures::{ApertureManager, ApertureShape};
    /// use cypcb_export::coords::CoordinateFormat;
    /// use cypcb_core::Nm;
    ///
    /// let mut manager = ApertureManager::new();
    /// let format = CoordinateFormat::FORMAT_MM_2_6;
    ///
    /// manager.get_or_create(ApertureShape::Circle { diameter: Nm::from_mm(1.0).0 });
    /// let defs = manager.to_definitions(&format);
    /// assert_eq!(defs, "%ADD10C,1.000000*%\n");
    /// ```
    pub fn to_definitions(&self, format: &CoordinateFormat) -> String {
        // Macros come first: a `%ADD` naming one has to follow its `%AM`.
        let mut macros = String::new();
        let mut result = String::new();
        let mut sorted_apertures: Vec<_> = self.apertures.iter().collect();
        // Sort by D-code for deterministic output
        sorted_apertures.sort_by_key(|(_, &dcode)| dcode);

        for (shape, &dcode) in sorted_apertures {
            let definition = match shape {
                ApertureShape::Circle { diameter } => {
                    let d = nm_to_decimal(*diameter, format);
                    format!("%ADD{}C,{}*%\n", dcode, d)
                }
                ApertureShape::Rectangle { width, height } => {
                    let w = nm_to_decimal(*width, format);
                    let h = nm_to_decimal(*height, format);
                    format!("%ADD{}R,{}X{}*%\n", dcode, w, h)
                }
                ApertureShape::Oblong { width, height } => {
                    let w = nm_to_decimal(*width, format);
                    let h = nm_to_decimal(*height, format);
                    format!("%ADD{}O,{}X{}*%\n", dcode, w, h)
                }
                ApertureShape::RoundRect {
                    width,
                    height,
                    corner_ratio,
                } => {
                    // Gerber has no rounded-rectangle aperture, and this used
                    // to flash a hard-cornered `R` with the corner written
                    // after it as `G04 RoundRect corner_ratio=25%` - which
                    // dropped the corners from the copper and was not a legal
                    // comment either: a `G04` has to end in `*`, and that
                    // trailing `%` opened an extended command nothing closed.
                    //
                    // What the format does have is the aperture macro, which
                    // is how KiCad draws the same pad. One macro per aperture,
                    // written with the numbers already worked out rather than
                    // with parameters and arithmetic, so no reader has to
                    // agree with us about operator precedence.
                    macros.push_str(&round_rect_macro(
                        dcode,
                        *width,
                        *height,
                        *corner_ratio,
                        format,
                    ));
                    format!("%ADD{dcode}RR{dcode}*%\n")
                }
            };
            result.push_str(&definition);
        }

        macros + &result
    }

    /// Get the number of registered apertures.
    pub fn len(&self) -> usize {
        self.apertures.len()
    }

    /// Check if no apertures are registered.
    pub fn is_empty(&self) -> bool {
        self.apertures.is_empty()
    }
}

/// Convert a pad definition to an aperture shape.
///
/// Maps the internal pad shape representation to the corresponding
/// Gerber aperture shape.
///
/// # Arguments
///
/// * `pad` - The pad definition from the footprint library
///
/// # Examples
///
/// ```
/// use cypcb_export::apertures::aperture_for_pad;
/// use cypcb_world::footprint::PadDef;
/// use cypcb_world::components::{PadShape, Layer};
/// use cypcb_core::{Nm, Point};
///
/// let pad = PadDef {
///     number: "1".into(),
///     shape: PadShape::Circle,
///     position: Point::ORIGIN,
///     size: (Nm::from_mm(1.0), Nm::from_mm(1.0)),
///     drill: None,
///     slot: None,
///     layers: vec![Layer::TopCopper],
///     mask_margin: None,
/// };
///
/// let aperture = aperture_for_pad(&pad);
/// // Circle pads use the width as diameter
/// ```
pub fn aperture_for_pad(pad: &PadDef) -> ApertureShape {
    let (width, height) = pad.size;

    match pad.shape {
        WorldPadShape::Circle => ApertureShape::Circle { diameter: width.0 },
        WorldPadShape::Rect => ApertureShape::Rectangle {
            width: width.0,
            height: height.0,
        },
        WorldPadShape::Oblong => ApertureShape::Oblong {
            width: width.0,
            height: height.0,
        },
        WorldPadShape::RoundRect { corner_ratio } => ApertureShape::RoundRect {
            width: width.0,
            height: height.0,
            corner_ratio,
        },
    }
}

/// One aperture macro drawing a rounded rectangle, named after its D-code.
///
/// Two overlapping centre-line rectangles fill the body, four circles round
/// the corners. The corner radius is the shorter side times the ratio, which
/// is what the SVG writer draws and what KiCad's `roundrect_rratio` means.
///
/// ```
/// use cypcb_export::apertures::round_rect_macro;
/// use cypcb_export::coords::CoordinateFormat;
/// use cypcb_core::Nm;
///
/// let text = round_rect_macro(
///     10u16,
///     Nm::from_mm(2.0).0,
///     Nm::from_mm(1.0).0,
///     25,
///     &CoordinateFormat::FORMAT_MM_2_6,
/// );
/// assert!(text.starts_with("%AMRR10*\n"));
/// assert!(text.ends_with("%\n"));
/// ```
pub fn round_rect_macro(
    dcode: u16,
    width: i64,
    height: i64,
    corner_ratio: u8,
    format: &CoordinateFormat,
) -> String {
    let radius = width.min(height) * i64::from(corner_ratio) / 100;
    let d = |value: i64| nm_to_decimal(value, format);

    let mut text = format!("%AMRR{dcode}*\n");
    // The body, as two rectangles that overlap in the middle.
    text.push_str(&format!(
        "21,1,{},{},0,0,0*\n",
        d(width),
        d(height - 2 * radius)
    ));
    text.push_str(&format!(
        "21,1,{},{},0,0,0*\n",
        d(width - 2 * radius),
        d(height)
    ));
    // A circle on each corner, centred a radius in from both edges.
    for (x, y) in [
        (width / 2 - radius, height / 2 - radius),
        (-(width / 2 - radius), height / 2 - radius),
        (width / 2 - radius, -(height / 2 - radius)),
        (-(width / 2 - radius), -(height / 2 - radius)),
    ] {
        text.push_str(&format!("1,1,{},{},{},0*\n", d(2 * radius), d(x), d(y)));
    }
    text.push_str("%\n");
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::Nm;
    use cypcb_core::Point;
    use cypcb_world::components::Layer;

    #[test]
    fn test_aperture_manager_new() {
        let manager = ApertureManager::new();
        assert_eq!(manager.next_dcode, 10);
        assert!(manager.is_empty());
    }

    #[test]
    fn test_get_or_create_assigns_dcode() {
        let mut manager = ApertureManager::new();
        let shape = ApertureShape::Circle {
            diameter: Nm::from_mm(1.0).0,
        };

        let dcode = manager.get_or_create(shape);
        assert_eq!(dcode, 10); // First D-code
        assert_eq!(manager.len(), 1);
    }

    #[test]
    fn test_get_or_create_reuses_dcode() {
        let mut manager = ApertureManager::new();
        let shape = ApertureShape::Circle {
            diameter: Nm::from_mm(1.0).0,
        };

        let d1 = manager.get_or_create(shape.clone());
        let d2 = manager.get_or_create(shape);
        assert_eq!(d1, d2);
        assert_eq!(manager.len(), 1);
    }

    #[test]
    fn test_get_or_create_different_shapes() {
        let mut manager = ApertureManager::new();

        let circle = ApertureShape::Circle {
            diameter: Nm::from_mm(1.0).0,
        };
        let rect = ApertureShape::Rectangle {
            width: Nm::from_mm(1.0).0,
            height: Nm::from_mm(0.5).0,
        };

        let d1 = manager.get_or_create(circle);
        let d2 = manager.get_or_create(rect);

        assert_eq!(d1, 10);
        assert_eq!(d2, 11);
        assert_eq!(manager.len(), 2);
    }

    #[test]
    fn test_to_definitions_circle() {
        let mut manager = ApertureManager::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;

        manager.get_or_create(ApertureShape::Circle {
            diameter: Nm::from_mm(1.0).0,
        });

        let defs = manager.to_definitions(&format);
        assert_eq!(defs, "%ADD10C,1.000000*%\n");
    }

    #[test]
    fn test_to_definitions_rectangle() {
        let mut manager = ApertureManager::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;

        manager.get_or_create(ApertureShape::Rectangle {
            width: Nm::from_mm(1.0).0,
            height: Nm::from_mm(0.5).0,
        });

        let defs = manager.to_definitions(&format);
        assert_eq!(defs, "%ADD10R,1.000000X0.500000*%\n");
    }

    #[test]
    fn test_to_definitions_oblong() {
        let mut manager = ApertureManager::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;

        manager.get_or_create(ApertureShape::Oblong {
            width: Nm::from_mm(1.5).0,
            height: Nm::from_mm(0.8).0,
        });

        let defs = manager.to_definitions(&format);
        assert_eq!(defs, "%ADD10O,1.500000X0.800000*%\n");
    }

    #[test]
    fn test_to_definitions_roundrect() {
        let mut manager = ApertureManager::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;

        manager.get_or_create(ApertureShape::RoundRect {
            width: Nm::from_mm(1.0).0,
            height: Nm::from_mm(0.5).0,
            corner_ratio: 25,
        });

        let defs = manager.to_definitions(&format);
        // This case asserted the fallback until 2026-09-03 - a hard-cornered
        // `R` and a comment that was not one - which is how a defect stays put
        // for months: the test agreed with it. A 1.0 by 0.5 pad at 25% has a
        // 0.125mm radius, and every number below follows from that.
        assert!(defs.starts_with("%AMRR10*\n"), "{defs}");
        assert!(defs.contains("21,1,1.000000,0.250000,0,0,0*\n"), "{defs}");
        assert!(defs.contains("21,1,0.750000,0.500000,0,0,0*\n"), "{defs}");
        assert!(
            defs.contains("1,1,0.250000,0.375000,0.125000,0*\n"),
            "{defs}"
        );
        assert!(
            defs.contains("1,1,0.250000,-0.375000,-0.125000,0*\n"),
            "{defs}"
        );
        assert!(defs.contains("%ADD10RR10*%\n"), "{defs}");
        assert!(
            !defs.contains("%ADD10R,"),
            "no bare rectangle any more: {defs}"
        );
    }

    #[test]
    fn test_to_definitions_multiple_apertures() {
        let mut manager = ApertureManager::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;

        manager.get_or_create(ApertureShape::Circle {
            diameter: Nm::from_mm(1.0).0,
        });
        manager.get_or_create(ApertureShape::Rectangle {
            width: Nm::from_mm(0.8).0,
            height: Nm::from_mm(0.6).0,
        });

        let defs = manager.to_definitions(&format);
        assert!(defs.contains("%ADD10C,1.000000*%"));
        assert!(defs.contains("%ADD11R,0.800000X0.600000*%"));
    }

    #[test]
    fn test_aperture_for_pad_circle() {
        let pad = PadDef {
            number: "1".into(),
            shape: WorldPadShape::Circle,
            position: Point::ORIGIN,
            size: (Nm::from_mm(1.0), Nm::from_mm(1.0)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
        };

        let aperture = aperture_for_pad(&pad);
        assert_eq!(
            aperture,
            ApertureShape::Circle {
                diameter: Nm::from_mm(1.0).0
            }
        );
    }

    #[test]
    fn test_aperture_for_pad_rect() {
        let pad = PadDef {
            number: "1".into(),
            shape: WorldPadShape::Rect,
            position: Point::ORIGIN,
            size: (Nm::from_mm(1.2), Nm::from_mm(0.8)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
        };

        let aperture = aperture_for_pad(&pad);
        assert_eq!(
            aperture,
            ApertureShape::Rectangle {
                width: Nm::from_mm(1.2).0,
                height: Nm::from_mm(0.8).0
            }
        );
    }

    #[test]
    fn test_aperture_for_pad_oblong() {
        let pad = PadDef {
            number: "1".into(),
            shape: WorldPadShape::Oblong,
            position: Point::ORIGIN,
            size: (Nm::from_mm(1.5), Nm::from_mm(0.8)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
        };

        let aperture = aperture_for_pad(&pad);
        assert_eq!(
            aperture,
            ApertureShape::Oblong {
                width: Nm::from_mm(1.5).0,
                height: Nm::from_mm(0.8).0
            }
        );
    }

    #[test]
    fn test_aperture_for_pad_roundrect() {
        let pad = PadDef {
            number: "1".into(),
            shape: WorldPadShape::RoundRect { corner_ratio: 25 },
            position: Point::ORIGIN,
            size: (Nm::from_mm(1.0), Nm::from_mm(0.6)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
        };

        let aperture = aperture_for_pad(&pad);
        assert_eq!(
            aperture,
            ApertureShape::RoundRect {
                width: Nm::from_mm(1.0).0,
                height: Nm::from_mm(0.6).0,
                corner_ratio: 25
            }
        );
    }
}
