//! Footprint library with data structures and lookup.

use std::collections::HashMap;

use cypcb_core::{Nm, Point, Rect};

use crate::components::{Layer, PadShape};

/// A single pad definition within a footprint.
///
/// Pads define the conductive areas where components connect to the PCB.
/// Each pad has a number/name, shape, position relative to the footprint
/// origin, size, optional drill (for through-hole), and layer information.
///
/// # Examples
///
/// ```
/// use cypcb_world::footprint::PadDef;
/// use cypcb_world::components::{PadShape, Layer};
/// use cypcb_core::{Nm, Point};
///
/// // SMD pad (no drill)
/// let smd_pad = PadDef {
///     number: "1".into(),
///     shape: PadShape::Rect,
///     position: Point::from_mm(-0.5, 0.0),
///     size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
///     drill: None,
///     layers: vec![Layer::TopCopper, Layer::TopPaste, Layer::TopMask],
/// };
///
/// // Through-hole pad (with drill)
/// let tht_pad = PadDef {
///     number: "1".into(),
///     shape: PadShape::Circle,
///     position: Point::from_mm(0.0, 0.0),
///     size: (Nm::from_mm(1.8), Nm::from_mm(1.8)),
///     drill: Some(Nm::from_mm(1.0)),
///     layers: vec![Layer::TopCopper, Layer::BottomCopper],
/// };
/// ```
#[derive(Debug, Clone)]
pub struct PadDef {
    /// Pad number/name (e.g., "1", "2", "A1", "VCC").
    pub number: String,
    /// Pad shape.
    pub shape: PadShape,
    /// Position relative to footprint origin.
    pub position: Point,
    /// Pad size as (width, height) in nanometers.
    pub size: (Nm, Nm),
    /// Drill diameter for through-hole pads (None for SMD).
    pub drill: Option<Nm>,
    /// Layers this pad appears on.
    pub layers: Vec<Layer>,
}

impl PadDef {
    /// Check if this is an SMD pad (no drill hole).
    #[inline]
    pub fn is_smd(&self) -> bool {
        self.drill.is_none()
    }

    /// Check if this is a through-hole pad (has drill hole).
    #[inline]
    pub fn is_through_hole(&self) -> bool {
        self.drill.is_some()
    }
}

/// A complete footprint definition.
///
/// A footprint represents the physical landing pattern for a component,
/// including all pads, their positions, and bounding boxes.
///
/// # Examples
///
/// ```
/// use cypcb_world::footprint::FootprintLibrary;
///
/// let lib = FootprintLibrary::new();
/// let fp = lib.get("0402").unwrap();
///
/// println!("Footprint: {}", fp.name);
/// println!("Description: {}", fp.description);
/// println!("Pads: {}", fp.pads.len());
/// ```
#[derive(Debug, Clone)]
pub struct Footprint {
    /// Footprint name/identifier (e.g., "0402", "DIP-8", "SOIC-8").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Pad definitions.
    pub pads: Vec<PadDef>,
    /// Bounding box of the component body.
    pub bounds: Rect,
    /// Assembly courtyard (clearance area).
    pub courtyard: Rect,
}

impl Footprint {
    /// Get a pad by its number/name.
    pub fn get_pad(&self, number: &str) -> Option<&PadDef> {
        self.pads.iter().find(|p| p.number == number)
    }

    /// Get the number of pads.
    #[inline]
    pub fn pad_count(&self) -> usize {
        self.pads.len()
    }
}

/// Library of known footprints.
///
/// The library is pre-populated with common SMD and through-hole footprints.
/// Custom footprints can be registered using [`register`](FootprintLibrary::register).
///
/// # Examples
///
/// ```
/// use cypcb_world::footprint::FootprintLibrary;
///
/// let lib = FootprintLibrary::new();
///
/// // Look up built-in footprints
/// assert!(lib.get("0402").is_some());
/// assert!(lib.get("0603").is_some());
/// assert!(lib.get("DIP-8").is_some());
///
/// // Iterate over all footprints
/// for (name, fp) in lib.iter() {
///     println!("{}: {} pads", name, fp.pads.len());
/// }
/// ```
#[derive(Debug, Default)]
pub struct FootprintLibrary {
    footprints: HashMap<String, Footprint>,
}

impl FootprintLibrary {
    /// Create a new footprint library with built-in footprints.
    pub fn new() -> Self {
        let mut lib = Self::default();
        lib.register_builtin_smd();
        lib.register_builtin_tht();
        lib
    }

    /// Look up a footprint by name.
    ///
    /// Returns `None` if the footprint is not found.
    pub fn get(&self, name: &str) -> Option<&Footprint> {
        self.footprints.get(name)
    }

    /// Register a new footprint in the library.
    ///
    /// If a footprint with the same name already exists, it is replaced.
    pub fn register(&mut self, footprint: Footprint) {
        self.footprints.insert(footprint.name.clone(), footprint);
    }

    /// Iterate over all footprints in the library.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Footprint)> {
        self.footprints.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Get the number of footprints in the library.
    #[inline]
    pub fn len(&self) -> usize {
        self.footprints.len()
    }

    /// Check if the library is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.footprints.is_empty()
    }

    /// Check if a footprint exists in the library.
    #[inline]
    pub fn contains(&self, name: &str) -> bool {
        self.footprints.contains_key(name)
    }

    /// Register all built-in SMD footprints.
    fn register_builtin_smd(&mut self) {
        use super::smd::*;

        self.register(chip_0402());
        self.register(chip_0603());
        self.register(chip_0805());
        self.register(chip_1206());
        self.register(chip_2512());
    }

    /// Register all built-in through-hole footprints.
    fn register_builtin_tht(&mut self) {
        use super::tht::*;

        self.register(axial_300mil());
        self.register(dip8());
        self.register(pin_header_1x2());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_has_builtin_footprints() {
        let lib = FootprintLibrary::new();

        // SMD footprints
        assert!(lib.contains("0402"));
        assert!(lib.contains("0603"));
        assert!(lib.contains("0805"));
        assert!(lib.contains("1206"));
        assert!(lib.contains("2512"));

        // THT footprints
        assert!(lib.contains("AXIAL-300"));
        assert!(lib.contains("DIP-8"));
        assert!(lib.contains("PIN-HDR-1x2"));
    }

    #[test]
    fn test_footprint_lookup() {
        let lib = FootprintLibrary::new();

        let fp = lib.get("0402").expect("0402 should exist");
        assert_eq!(fp.name, "0402");
        assert_eq!(fp.pads.len(), 2);
    }

    #[test]
    fn test_custom_footprint_registration() {
        let mut lib = FootprintLibrary::new();
        let initial_count = lib.len();

        let custom = Footprint {
            name: "CUSTOM-1".into(),
            description: "Custom test footprint".into(),
            pads: vec![],
            bounds: Rect::default(),
            courtyard: Rect::default(),
        };

        lib.register(custom);
        assert_eq!(lib.len(), initial_count + 1);
        assert!(lib.contains("CUSTOM-1"));
    }

    #[test]
    fn test_pad_def_is_smd() {
        let smd = PadDef {
            number: "1".into(),
            shape: PadShape::Rect,
            position: Point::ORIGIN,
            size: (Nm::from_mm(0.5), Nm::from_mm(0.5)),
            drill: None,
            layers: vec![Layer::TopCopper],
        };
        assert!(smd.is_smd());
        assert!(!smd.is_through_hole());

        let tht = PadDef {
            number: "1".into(),
            shape: PadShape::Circle,
            position: Point::ORIGIN,
            size: (Nm::from_mm(1.5), Nm::from_mm(1.5)),
            drill: Some(Nm::from_mm(0.8)),
            layers: vec![Layer::TopCopper, Layer::BottomCopper],
        };
        assert!(!tht.is_smd());
        assert!(tht.is_through_hole());
    }

    #[test]
    fn test_footprint_get_pad() {
        let lib = FootprintLibrary::new();
        let fp = lib.get("0402").unwrap();

        assert!(fp.get_pad("1").is_some());
        assert!(fp.get_pad("2").is_some());
        assert!(fp.get_pad("3").is_none());
    }
}
