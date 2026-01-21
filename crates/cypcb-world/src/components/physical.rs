//! Physical components for board entities.
//!
//! These components represent the physical properties of PCB elements:
//! layers, footprints, and pads.

use bevy_ecs::prelude::*;
use cypcb_core::Nm;
use serde::{Deserialize, Serialize};

/// PCB layer identifier.
///
/// Supports 2-32 layer boards as per BRD-02 requirement.
/// Layer numbering follows standard PCB conventions.
///
/// # Layer Types
///
/// - **Copper layers**: Conductive layers for traces and planes
/// - **Silkscreen**: Component markings (top/bottom)
/// - **Soldermask**: Solder mask openings (top/bottom)
/// - **Solderpaste**: Paste stencil openings (top/bottom)
/// - **Outline**: Board edge definition
///
/// # Examples
///
/// ```
/// use cypcb_world::Layer;
///
/// let top = Layer::TopCopper;
/// let bottom = Layer::BottomCopper;
/// let inner = Layer::Inner(1);
///
/// assert!(top.is_copper());
/// assert!(inner.is_copper());
/// assert!(!Layer::TopSilk.is_copper());
/// ```
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Layer {
    /// Top copper layer (component side).
    TopCopper,
    /// Bottom copper layer (solder side).
    BottomCopper,
    /// Inner copper layer (1-30 for 32-layer boards).
    Inner(u8),
    /// Top silkscreen layer.
    TopSilk,
    /// Bottom silkscreen layer.
    BottomSilk,
    /// Top soldermask layer.
    TopMask,
    /// Bottom soldermask layer.
    BottomMask,
    /// Top solderpaste layer.
    TopPaste,
    /// Bottom solderpaste layer.
    BottomPaste,
    /// Board outline/edge cuts.
    Outline,
}

impl Layer {
    /// Check if this is a copper layer.
    #[inline]
    pub fn is_copper(&self) -> bool {
        matches!(self, Layer::TopCopper | Layer::BottomCopper | Layer::Inner(_))
    }

    /// Check if this is a silkscreen layer.
    #[inline]
    pub fn is_silkscreen(&self) -> bool {
        matches!(self, Layer::TopSilk | Layer::BottomSilk)
    }

    /// Check if this is a soldermask layer.
    #[inline]
    pub fn is_soldermask(&self) -> bool {
        matches!(self, Layer::TopMask | Layer::BottomMask)
    }

    /// Check if this is a solderpaste layer.
    #[inline]
    pub fn is_solderpaste(&self) -> bool {
        matches!(self, Layer::TopPaste | Layer::BottomPaste)
    }

    /// Check if this is a top-side layer.
    #[inline]
    pub fn is_top(&self) -> bool {
        matches!(
            self,
            Layer::TopCopper | Layer::TopSilk | Layer::TopMask | Layer::TopPaste
        )
    }

    /// Check if this is a bottom-side layer.
    #[inline]
    pub fn is_bottom(&self) -> bool {
        matches!(
            self,
            Layer::BottomCopper | Layer::BottomSilk | Layer::BottomMask | Layer::BottomPaste
        )
    }

    /// Convert to a bit mask value for layer sets.
    ///
    /// The bit positions are:
    /// - 0: TopCopper
    /// - 1: BottomCopper
    /// - 2-31: Inner(0-29)
    /// - Other layers don't have mask bits (return 0)
    pub fn to_copper_mask(&self) -> u32 {
        match self {
            Layer::TopCopper => 1 << 0,
            Layer::BottomCopper => 1 << 1,
            Layer::Inner(n) if *n < 30 => 1 << (2 + *n),
            _ => 0,
        }
    }
}

impl std::fmt::Display for Layer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Layer::TopCopper => write!(f, "Top Copper"),
            Layer::BottomCopper => write!(f, "Bottom Copper"),
            Layer::Inner(n) => write!(f, "Inner {}", n),
            Layer::TopSilk => write!(f, "Top Silkscreen"),
            Layer::BottomSilk => write!(f, "Bottom Silkscreen"),
            Layer::TopMask => write!(f, "Top Soldermask"),
            Layer::BottomMask => write!(f, "Bottom Soldermask"),
            Layer::TopPaste => write!(f, "Top Solderpaste"),
            Layer::BottomPaste => write!(f, "Bottom Solderpaste"),
            Layer::Outline => write!(f, "Outline"),
        }
    }
}

/// Reference to a footprint library entry.
///
/// Footprints define the physical landing pattern for components.
/// The reference string identifies the footprint in the library.
///
/// # Examples
///
/// ```
/// use cypcb_world::FootprintRef;
///
/// let smd = FootprintRef::new("0402");
/// let soic = FootprintRef::new("SOIC-8");
/// let dip = FootprintRef::new("DIP-8");
/// ```
#[derive(Component, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FootprintRef(pub String);

impl FootprintRef {
    /// Create a new footprint reference.
    #[inline]
    pub fn new(name: impl Into<String>) -> Self {
        FootprintRef(name.into())
    }

    /// Get the footprint name as a string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for FootprintRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for FootprintRef {
    fn from(s: &str) -> Self {
        FootprintRef::new(s)
    }
}

/// Pad shape for SMD and through-hole pads.
///
/// These are the basic pad shapes supported for PCB design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PadShape {
    /// Circular pad.
    Circle,
    /// Rectangular pad.
    Rect,
    /// Rounded rectangle (with corner radius as a percentage of min dimension).
    RoundRect {
        /// Corner radius as percentage of smaller dimension (0-50).
        corner_ratio: u8,
    },
    /// Oblong/oval pad (stadium shape).
    Oblong,
}

impl PadShape {
    /// Create a rounded rectangle with the given corner ratio.
    ///
    /// Corner ratio is clamped to 0-50 range.
    #[inline]
    pub fn round_rect(corner_ratio: u8) -> Self {
        PadShape::RoundRect {
            corner_ratio: corner_ratio.min(50),
        }
    }
}

impl Default for PadShape {
    fn default() -> Self {
        PadShape::Circle
    }
}

impl std::fmt::Display for PadShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PadShape::Circle => write!(f, "Circle"),
            PadShape::Rect => write!(f, "Rect"),
            PadShape::RoundRect { corner_ratio } => write!(f, "RoundRect({}%)", corner_ratio),
            PadShape::Oblong => write!(f, "Oblong"),
        }
    }
}

/// A pad definition within a footprint.
///
/// Pads are the conductive areas where components are soldered.
/// Each pad has a shape, size, optional drill, and layer mask.
///
/// # Examples
///
/// ```
/// use cypcb_world::{Pad, PadShape, Layer};
/// use cypcb_core::Nm;
///
/// // SMD pad (no drill)
/// let smd_pad = Pad::smd("1", PadShape::Rect, Nm::from_mm(0.6), Nm::from_mm(0.5));
///
/// // Through-hole pad
/// let th_pad = Pad::through_hole(
///     "1",
///     PadShape::Circle,
///     Nm::from_mm(1.6),
///     Nm::from_mm(1.6),
///     Nm::from_mm(0.8),
/// );
/// ```
#[derive(Component, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pad {
    /// Pad number/name (e.g., "1", "2", "A1", "VCC").
    pub number: String,
    /// Pad shape.
    pub shape: PadShape,
    /// Pad width in nanometers.
    pub width: Nm,
    /// Pad height in nanometers.
    pub height: Nm,
    /// Drill diameter for through-hole pads (None for SMD).
    pub drill: Option<Nm>,
    /// Copper layer mask (bit 0 = top, bit 1 = bottom, bits 2-31 = inner layers).
    pub layer_mask: u32,
}

impl Pad {
    /// Create an SMD pad (top copper only).
    pub fn smd(number: impl Into<String>, shape: PadShape, width: Nm, height: Nm) -> Self {
        Pad {
            number: number.into(),
            shape,
            width,
            height,
            drill: None,
            layer_mask: Layer::TopCopper.to_copper_mask(),
        }
    }

    /// Create a through-hole pad (top and bottom copper).
    pub fn through_hole(
        number: impl Into<String>,
        shape: PadShape,
        width: Nm,
        height: Nm,
        drill: Nm,
    ) -> Self {
        Pad {
            number: number.into(),
            shape,
            width,
            height,
            drill: Some(drill),
            layer_mask: Layer::TopCopper.to_copper_mask() | Layer::BottomCopper.to_copper_mask(),
        }
    }

    /// Check if this is an SMD pad.
    #[inline]
    pub fn is_smd(&self) -> bool {
        self.drill.is_none()
    }

    /// Check if this is a through-hole pad.
    #[inline]
    pub fn is_through_hole(&self) -> bool {
        self.drill.is_some()
    }

    /// Check if the pad exists on a given layer.
    #[inline]
    pub fn on_layer(&self, layer: Layer) -> bool {
        let mask = layer.to_copper_mask();
        mask != 0 && (self.layer_mask & mask) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_is_copper() {
        assert!(Layer::TopCopper.is_copper());
        assert!(Layer::BottomCopper.is_copper());
        assert!(Layer::Inner(1).is_copper());
        assert!(!Layer::TopSilk.is_copper());
        assert!(!Layer::Outline.is_copper());
    }

    #[test]
    fn test_layer_sides() {
        assert!(Layer::TopCopper.is_top());
        assert!(Layer::TopSilk.is_top());
        assert!(!Layer::TopCopper.is_bottom());

        assert!(Layer::BottomCopper.is_bottom());
        assert!(Layer::BottomMask.is_bottom());
        assert!(!Layer::BottomCopper.is_top());
    }

    #[test]
    fn test_layer_mask() {
        assert_eq!(Layer::TopCopper.to_copper_mask(), 0b01);
        assert_eq!(Layer::BottomCopper.to_copper_mask(), 0b10);
        assert_eq!(Layer::Inner(0).to_copper_mask(), 0b100);
        assert_eq!(Layer::Inner(1).to_copper_mask(), 0b1000);
        assert_eq!(Layer::TopSilk.to_copper_mask(), 0); // Non-copper
    }

    #[test]
    fn test_pad_smd() {
        let pad = Pad::smd("1", PadShape::Rect, Nm::from_mm(0.6), Nm::from_mm(0.5));
        assert!(pad.is_smd());
        assert!(!pad.is_through_hole());
        assert!(pad.on_layer(Layer::TopCopper));
        assert!(!pad.on_layer(Layer::BottomCopper));
    }

    #[test]
    fn test_pad_through_hole() {
        let pad = Pad::through_hole(
            "1",
            PadShape::Circle,
            Nm::from_mm(1.6),
            Nm::from_mm(1.6),
            Nm::from_mm(0.8),
        );
        assert!(!pad.is_smd());
        assert!(pad.is_through_hole());
        assert!(pad.on_layer(Layer::TopCopper));
        assert!(pad.on_layer(Layer::BottomCopper));
    }

    #[test]
    fn test_pad_shape_round_rect() {
        let shape = PadShape::round_rect(25);
        assert_eq!(shape, PadShape::RoundRect { corner_ratio: 25 });

        // Clamped to 50
        let clamped = PadShape::round_rect(100);
        assert_eq!(clamped, PadShape::RoundRect { corner_ratio: 50 });
    }
}
