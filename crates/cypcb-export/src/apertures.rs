//! Gerber aperture management for D-code generation.
//!
//! Apertures define the "tools" used to draw features in Gerber files.
//! Each unique pad shape gets a D-code (D10, D11, etc.) that is defined
//! once in the aperture section and reused throughout the file.

use crate::coords::{nm_to_decimal, CoordinateFormat};
use cypcb_world::components::{rotate_about_origin, PadShape as WorldPadShape};
use cypcb_world::footprint::PadOutline;
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
    /// A rectangle stood at an angle (millidegrees counterclockwise) that a
    /// plain `R` aperture cannot state.
    TurnedRect {
        width: i64,
        height: i64,
        millideg: i32,
    },
    /// An oblong stood at an angle (millidegrees counterclockwise) that a
    /// plain `O` aperture cannot state.
    TurnedOblong {
        width: i64,
        height: i64,
        millideg: i32,
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
                ApertureShape::TurnedRect {
                    width,
                    height,
                    millideg,
                } => {
                    // Same discipline as the rounded rectangle above: one
                    // macro per aperture, numbers worked out. The rotation is
                    // the one thing left as a parameter for the reader to
                    // apply, because that is what the primitive is for - the
                    // specification's own rotating-rectangle macro passes the
                    // angle into primitive 21 the same way (revision 2026.05,
                    // section 4.5.1, `Box`; read 2026-10-07).
                    macros.push_str(&turned_rect_macro(
                        dcode, *width, *height, *millideg, format,
                    ));
                    format!("%ADD{dcode}TR{dcode}*%\n")
                }
                ApertureShape::TurnedOblong {
                    width,
                    height,
                    millideg,
                } => {
                    // A stadium cannot be rotated by parameter the way a
                    // rectangle can without trusting every reader with a
                    // rotated circle, so the angle goes into the geometry:
                    // a thick line between the two cap centres, with a circle
                    // on each. KiCad's `HorizOval` macro is the same drawing,
                    // and its authors note they avoid shape-level rotation
                    // because readers break on it
                    // (`include/plotters/gbr_plotter_aperture_macros.h`,
                    // read 2026-10-07).
                    macros.push_str(&turned_oblong_macro(
                        dcode, *width, *height, *millideg, format,
                    ));
                    format!("%ADD{dcode}TO{dcode}*%\n")
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

/// The aperture a pad is flashed with, once it is turned with its part.
///
/// It takes the pad's [`PadOutline`] rather than its definition, so the width
/// and height are the ones along the board's axes: a part turned a quarter
/// turn gets its pads' sides swapped, which the definition alone cannot say.
///
/// A turn that is a multiple of 90 degrees ends here, on the plain apertures:
/// the swap in [`PadOutline::size`] has already stated it. A turn between
/// quarter turns cannot be stated by any width and height - a 2 by 1mm pad at
/// 30 degrees is still 2 by 1 along its own axes - so a rectangle or an
/// oblong becomes a macro aperture carrying the angle. This is what KiCad
/// does for the same pads (`FlashPadRect` swaps or flashes a `RotRect` macro;
/// `FlashPadOval` swaps or a `HorizOval` macro - pcbnew's
/// `common/plotters/GERBER_plotter.cpp` and
/// `include/plotters/gbr_plotter_aperture_macros.h`, read 2026-10-07), and
/// what the Gerber specification's own rotating-rectangle example does
/// (revision 2026.05, section 4.5.1, the `Box` macro; read 2026-10-07).
///
/// ```
/// use cypcb_export::apertures::{aperture_for_pad, ApertureShape};
/// use cypcb_world::footprint::PadDef;
/// use cypcb_world::components::{PadShape, Layer, Rotation};
/// use cypcb_core::{Nm, Point};
///
/// let pad = PadDef {
///     number: "1".into(),
///     shape: PadShape::Rect,
///     position: Point::ORIGIN,
///     size: (Nm::from_mm(1.0), Nm::from_mm(1.45)),
///     drill: None,
///     slot: None,
///     layers: vec![Layer::TopCopper],
///     mask_margin: None,
///     rotation: Rotation::ZERO,
/// };
///
/// let turned = aperture_for_pad(&pad.outline(Point::ORIGIN, Rotation::DEG_90));
/// assert_eq!(
///     turned,
///     ApertureShape::Rectangle { width: Nm::from_mm(1.45).0, height: Nm::from_mm(1.0).0 }
/// );
///
/// // A turn between quarter turns keeps its angle.
/// let stood = aperture_for_pad(&pad.outline(Point::ORIGIN, Rotation::from_degrees(30.0)));
/// assert_eq!(
///     stood,
///     ApertureShape::TurnedRect {
///         width: Nm::from_mm(1.0).0,
///         height: Nm::from_mm(1.45).0,
///         millideg: 30_000,
///     }
/// );
/// ```
pub fn aperture_for_pad(pad: &PadOutline) -> ApertureShape {
    let (width, height) = pad.size;
    // The turn left over above the quarter turns the swap in `size` has
    // already taken up: the slope the pad stands at. Zero for every multiple
    // of 90 degrees, and 0-90 degrees otherwise, because a shape symmetric
    // under a half turn stands at the same slope whichever quarter it is.
    let slope = pad.turn.0 % 90_000;

    match pad.shape {
        WorldPadShape::Circle => ApertureShape::Circle { diameter: width.0 },
        WorldPadShape::Rect if slope != 0 => ApertureShape::TurnedRect {
            width: width.0,
            height: height.0,
            millideg: slope,
        },
        WorldPadShape::Rect => ApertureShape::Rectangle {
            width: width.0,
            height: height.0,
        },
        WorldPadShape::Oblong if slope != 0 => ApertureShape::TurnedOblong {
            width: width.0,
            height: height.0,
            millideg: slope,
        },
        WorldPadShape::Oblong => ApertureShape::Oblong {
            width: width.0,
            height: height.0,
        },
        // A rounded rectangle stood at an angle has no aperture yet: it is
        // flashed unturned, the way every pad was before the angle was kept.
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

/// Millidegrees as a decimal degree figure, `17.500` for 17_500.
///
/// Integer arithmetic all the way: the angle a board states is whole
/// millidegrees, and a float formatting it would give a reader one digit of
/// the writer's rounding to disagree with.
fn millideg_to_decimal(millideg: i32) -> String {
    format!("{}.{:03}", millideg / 1000, millideg % 1000)
}

/// One aperture macro drawing a rectangle stood at an angle, named after its
/// D-code.
///
/// A single center-line primitive, rotated by its last parameter. The
/// specification's `Box` macro - its own example of a rotating rectangle -
/// rotates the same primitive the same way, and warns that the rotation is
/// around the macro's origin, not the primitive's centre: the centre here is
/// the origin, so the rectangle turns in place (revision 2026.05, section
/// 4.5.1.5; read 2026-10-07).
///
/// ```
/// use cypcb_export::apertures::turned_rect_macro;
/// use cypcb_export::coords::CoordinateFormat;
/// use cypcb_core::Nm;
///
/// let text = turned_rect_macro(
///     10u16,
///     Nm::from_mm(2.0).0,
///     Nm::from_mm(1.0).0,
///     30_000,
///     &CoordinateFormat::FORMAT_MM_2_6,
/// );
/// assert_eq!(text, "%AMTR10*\n21,1,2.000000,1.000000,0,0,30.000*\n%\n");
/// ```
pub fn turned_rect_macro(
    dcode: u16,
    width: i64,
    height: i64,
    millideg: i32,
    format: &CoordinateFormat,
) -> String {
    let d = |value: i64| nm_to_decimal(value, format);
    format!(
        "%AMTR{dcode}*\n21,1,{},{},0,0,{}*\n%\n",
        d(width),
        d(height),
        millideg_to_decimal(millideg)
    )
}

/// One aperture macro drawing an oblong stood at an angle, named after its
/// D-code.
///
/// A thick vector line between the two cap centres with a circle on each, the
/// angle carried by the centres' coordinates rather than by a rotation
/// parameter. The cap centres are the half span of the long axis over the
/// short, turned by
/// [`rotate_about_origin`](cypcb_world::components::rotate_about_origin) -
/// the one place a point is turned - so a pad's aperture leans exactly the
/// way its own offset was placed.
///
/// ```
/// use cypcb_export::apertures::turned_oblong_macro;
/// use cypcb_export::coords::CoordinateFormat;
/// use cypcb_core::Nm;
///
/// // 2.4 by 1.0mm at 30 degrees: caps 0.7mm either side of the centre,
/// // 0.606218 across and 0.35 up.
/// let text = turned_oblong_macro(
///     11u16,
///     Nm::from_mm(2.4).0,
///     Nm::from_mm(1.0).0,
///     30_000,
///     &CoordinateFormat::FORMAT_MM_2_6,
/// );
/// assert_eq!(
///     text,
///     "%AMTO11*\n20,1,1.000000,0.606218,0.350000,-0.606218,-0.350000,0*\n\
///      1,1,1.000000,0.606218,0.350000,0*\n\
///      1,1,1.000000,-0.606218,-0.350000,0*\n%\n"
/// );
/// ```
pub fn turned_oblong_macro(
    dcode: u16,
    width: i64,
    height: i64,
    millideg: i32,
    format: &CoordinateFormat,
) -> String {
    let long = width.max(height);
    let short = width.min(height);
    // The long axis runs along the pad's own x; a pad taller than wide at
    // this slope has its long axis a quarter turn further round.
    let axis = if width >= height {
        millideg
    } else {
        millideg + 90_000
    };
    // The cap centres through the one place a point is turned, so the pad's
    // aperture leans exactly the way its own offset was placed.
    let half_span = (long - short) / 2;
    let turned = rotate_about_origin(
        cypcb_core::Point::new(cypcb_core::Nm(half_span), cypcb_core::Nm(0)),
        f64::from(axis) / 1000.0,
    );
    let (x, y) = (turned.x.0, turned.y.0);
    let d = |value: i64| nm_to_decimal(value, format);

    let mut text = format!("%AMTO{dcode}*\n");
    // The body between the caps, as a line the width of the short side.
    text.push_str(&format!(
        "20,1,{},{},{},{},{},0*\n",
        d(short),
        d(x),
        d(y),
        d(-x),
        d(-y)
    ));
    // A cap on each end, centred where the line stops.
    text.push_str(&format!("1,1,{},{},{},0*\n", d(short), d(x), d(y)));
    text.push_str(&format!("1,1,{},{},{},0*\n", d(short), d(-x), d(-y)));
    text.push_str("%\n");
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::Nm;
    use cypcb_core::Point;
    use cypcb_world::components::{Layer, Rotation};
    use cypcb_world::footprint::PadDef;

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
            rotation: Rotation::ZERO,
        };

        let aperture = aperture_for_pad(&pad.outline(Point::ORIGIN, Rotation::ZERO));
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
            rotation: Rotation::ZERO,
        };

        let aperture = aperture_for_pad(&pad.outline(Point::ORIGIN, Rotation::ZERO));
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
            rotation: Rotation::ZERO,
        };

        let aperture = aperture_for_pad(&pad.outline(Point::ORIGIN, Rotation::ZERO));
        assert_eq!(
            aperture,
            ApertureShape::Oblong {
                width: Nm::from_mm(1.5).0,
                height: Nm::from_mm(0.8).0
            }
        );
    }

    #[test]
    fn test_aperture_for_pad_turned_rect() {
        let pad = PadDef {
            number: "1".into(),
            shape: WorldPadShape::Rect,
            position: Point::ORIGIN,
            size: (Nm::from_mm(2.0), Nm::from_mm(1.0)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
            rotation: Rotation::ZERO,
        };

        // 135 degrees: sides swapped by the quarter turn, slope 45 kept.
        let aperture = aperture_for_pad(&pad.outline(Point::ORIGIN, Rotation::from_degrees(135.0)));
        assert_eq!(
            aperture,
            ApertureShape::TurnedRect {
                width: Nm::from_mm(1.0).0,
                height: Nm::from_mm(2.0).0,
                millideg: 45_000,
            }
        );
    }

    #[test]
    fn test_aperture_for_pad_turned_oblong() {
        let pad = PadDef {
            number: "1".into(),
            shape: WorldPadShape::Oblong,
            position: Point::ORIGIN,
            size: (Nm::from_mm(2.4), Nm::from_mm(1.0)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: None,
            rotation: Rotation::ZERO,
        };

        let aperture = aperture_for_pad(&pad.outline(Point::ORIGIN, Rotation::from_degrees(17.5)));
        assert_eq!(
            aperture,
            ApertureShape::TurnedOblong {
                width: Nm::from_mm(2.4).0,
                height: Nm::from_mm(1.0).0,
                millideg: 17_500,
            }
        );
    }

    #[test]
    fn test_to_definitions_turned_rect_and_oblong() {
        let mut manager = ApertureManager::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;

        manager.get_or_create(ApertureShape::TurnedRect {
            width: Nm::from_mm(2.0).0,
            height: Nm::from_mm(1.0).0,
            millideg: 30_000,
        });
        let defs = manager.to_definitions(&format);
        assert!(defs.contains("%AMTR10*\n"), "{defs}");
        assert!(
            defs.contains("21,1,2.000000,1.000000,0,0,30.000*\n"),
            "{defs}"
        );
        assert!(defs.contains("%ADD10TR10*%\n"), "{defs}");
    }

    #[test]
    fn test_to_definitions_turned_oblong() {
        let mut manager = ApertureManager::new();
        let format = CoordinateFormat::FORMAT_MM_2_6;

        manager.get_or_create(ApertureShape::TurnedOblong {
            width: Nm::from_mm(2.4).0,
            height: Nm::from_mm(1.0).0,
            millideg: 30_000,
        });
        let defs = manager.to_definitions(&format);
        assert!(defs.contains("%AMTO10*\n"), "{defs}");
        assert!(
            defs.contains("20,1,1.000000,0.606218,0.350000,-0.606218,-0.350000,0*\n"),
            "{defs}"
        );
        assert!(
            defs.contains("1,1,1.000000,0.606218,0.350000,0*\n"),
            "{defs}"
        );
        assert!(
            defs.contains("1,1,1.000000,-0.606218,-0.350000,0*\n"),
            "{defs}"
        );
        assert!(defs.contains("%ADD10TO10*%\n"), "{defs}");
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
            rotation: Rotation::ZERO,
        };

        let aperture = aperture_for_pad(&pad.outline(Point::ORIGIN, Rotation::ZERO));
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
