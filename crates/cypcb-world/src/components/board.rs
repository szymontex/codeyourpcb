//! Board-level components for the board entity.
//!
//! These components define properties of the PCB board itself:
//! size, layer stackup, and the board marker.

use bevy_ecs::prelude::*;
use cypcb_core::Nm;
use serde::{Deserialize, Serialize};

/// Marker component identifying the board entity.
///
/// There should be exactly one entity with this component per design.
/// The board entity holds board-level properties like size and layer stack.
///
/// # Examples
///
/// ```
/// use bevy_ecs::prelude::*;
/// use cypcb_world::{Board, BoardSize};
/// use cypcb_core::Nm;
///
/// let mut world = World::new();
/// world.spawn((
///     Board,
///     BoardSize::from_mm(100.0, 80.0),
/// ));
/// ```
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Board;

/// Board dimensions.
///
/// Defines the width and height of the PCB in nanometers.
/// The board origin is at the bottom-left corner.
///
/// # Examples
///
/// ```
/// use cypcb_world::BoardSize;
/// use cypcb_core::Nm;
///
/// let size = BoardSize::from_mm(100.0, 80.0);
/// assert_eq!(size.width.0, 100_000_000);
/// assert_eq!(size.height.0, 80_000_000);
/// ```
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BoardSize {
    /// Board width in nanometers.
    pub width: Nm,
    /// Board height in nanometers.
    pub height: Nm,
}

impl BoardSize {
    /// Create a new board size from Nm values.
    #[inline]
    pub const fn new(width: Nm, height: Nm) -> Self {
        BoardSize { width, height }
    }

    /// Create a board size from millimeter dimensions.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_world::BoardSize;
    ///
    /// let size = BoardSize::from_mm(100.0, 80.0);
    /// assert!((size.width.to_mm() - 100.0).abs() < 0.001);
    /// ```
    #[inline]
    pub fn from_mm(width: f64, height: f64) -> Self {
        BoardSize {
            width: Nm::from_mm(width),
            height: Nm::from_mm(height),
        }
    }

    /// Create a board size from mil dimensions.
    #[inline]
    pub fn from_mil(width: f64, height: f64) -> Self {
        BoardSize {
            width: Nm::from_mil(width),
            height: Nm::from_mil(height),
        }
    }

    /// Create a board size from inch dimensions.
    #[inline]
    pub fn from_inch(width: f64, height: f64) -> Self {
        BoardSize {
            width: Nm::from_inch(width),
            height: Nm::from_inch(height),
        }
    }

    /// Get the board area in square nanometers.
    ///
    /// Returns i128 to avoid overflow for large boards.
    #[inline]
    pub fn area(&self) -> i128 {
        self.width.0 as i128 * self.height.0 as i128
    }

    /// Get the board area in square millimeters.
    #[inline]
    pub fn area_mm2(&self) -> f64 {
        self.width.to_mm() * self.height.to_mm()
    }

    /// Check if a point is within the board boundaries.
    ///
    /// Assumes origin at (0, 0) bottom-left.
    #[inline]
    pub fn contains(&self, x: Nm, y: Nm) -> bool {
        x.0 >= 0 && x.0 <= self.width.0 && y.0 >= 0 && y.0 <= self.height.0
    }
}

impl Default for BoardSize {
    /// Default board size: 100mm x 100mm.
    fn default() -> Self {
        BoardSize::from_mm(100.0, 100.0)
    }
}

impl std::fmt::Display for BoardSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2}mm x {:.2}mm", self.width.to_mm(), self.height.to_mm())
    }
}

/// Layer stack configuration.
///
/// Defines the number of copper layers in the board.
/// Supports 2-32 layers as per BRD-02 requirement.
///
/// # Layer Numbering
///
/// - 2-layer board: Top, Bottom
/// - 4-layer board: Top, Inner1, Inner2, Bottom
/// - 6-layer board: Top, Inner1, Inner2, Inner3, Inner4, Bottom
/// - etc.
///
/// # Examples
///
/// ```
/// use cypcb_world::LayerStack;
///
/// let two_layer = LayerStack::new(2);
/// assert!(two_layer.is_valid());
/// assert!(!two_layer.has_inner_layers());
///
/// let four_layer = LayerStack::new(4);
/// assert!(four_layer.has_inner_layers());
/// assert_eq!(four_layer.inner_count(), 2);
/// ```
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LayerStack {
    /// Number of copper layers (2-32).
    pub count: u8,
}

impl LayerStack {
    /// Minimum supported layer count.
    pub const MIN_LAYERS: u8 = 2;

    /// Maximum supported layer count.
    pub const MAX_LAYERS: u8 = 32;

    /// Create a new layer stack with the given layer count.
    ///
    /// # Panics
    ///
    /// Panics if count is not in range 2-32.
    #[inline]
    pub fn new(count: u8) -> Self {
        assert!(
            count >= Self::MIN_LAYERS && count <= Self::MAX_LAYERS,
            "Layer count must be 2-32, got {}",
            count
        );
        LayerStack { count }
    }

    /// Create a layer stack, clamping to valid range.
    #[inline]
    pub fn new_clamped(count: u8) -> Self {
        LayerStack {
            count: count.clamp(Self::MIN_LAYERS, Self::MAX_LAYERS),
        }
    }

    /// Try to create a layer stack, returning None if invalid.
    #[inline]
    pub fn try_new(count: u8) -> Option<Self> {
        if count >= Self::MIN_LAYERS && count <= Self::MAX_LAYERS {
            Some(LayerStack { count })
        } else {
            None
        }
    }

    /// Check if this is a valid layer count.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.count >= Self::MIN_LAYERS && self.count <= Self::MAX_LAYERS
    }

    /// Check if this board has inner layers.
    #[inline]
    pub fn has_inner_layers(&self) -> bool {
        self.count > 2
    }

    /// Get the number of inner copper layers.
    #[inline]
    pub fn inner_count(&self) -> u8 {
        self.count.saturating_sub(2)
    }

    /// Common layer stackups.
    pub const TWO_LAYER: LayerStack = LayerStack { count: 2 };
    pub const FOUR_LAYER: LayerStack = LayerStack { count: 4 };
    pub const SIX_LAYER: LayerStack = LayerStack { count: 6 };
    pub const EIGHT_LAYER: LayerStack = LayerStack { count: 8 };
}

impl Default for LayerStack {
    /// Default to 2-layer board.
    fn default() -> Self {
        LayerStack::TWO_LAYER
    }
}

impl std::fmt::Display for LayerStack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-layer", self.count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_board_size_from_mm() {
        let size = BoardSize::from_mm(100.0, 80.0);
        assert_eq!(size.width.0, 100_000_000);
        assert_eq!(size.height.0, 80_000_000);
    }

    #[test]
    fn test_board_size_area() {
        let size = BoardSize::from_mm(100.0, 80.0);
        assert!((size.area_mm2() - 8000.0).abs() < 0.001);
    }

    #[test]
    fn test_board_size_contains() {
        let size = BoardSize::from_mm(100.0, 80.0);

        assert!(size.contains(Nm::from_mm(50.0), Nm::from_mm(40.0)));
        assert!(size.contains(Nm::ZERO, Nm::ZERO));
        assert!(size.contains(Nm::from_mm(100.0), Nm::from_mm(80.0)));

        assert!(!size.contains(Nm::from_mm(101.0), Nm::from_mm(40.0)));
        assert!(!size.contains(Nm::from_mm(-1.0), Nm::from_mm(40.0)));
    }

    #[test]
    fn test_layer_stack_new() {
        let stack = LayerStack::new(4);
        assert_eq!(stack.count, 4);
        assert!(stack.is_valid());
    }

    #[test]
    #[should_panic(expected = "Layer count must be 2-32")]
    fn test_layer_stack_invalid() {
        LayerStack::new(1);
    }

    #[test]
    fn test_layer_stack_try_new() {
        assert!(LayerStack::try_new(2).is_some());
        assert!(LayerStack::try_new(32).is_some());
        assert!(LayerStack::try_new(1).is_none());
        assert!(LayerStack::try_new(33).is_none());
    }

    #[test]
    fn test_layer_stack_inner_count() {
        assert_eq!(LayerStack::new(2).inner_count(), 0);
        assert_eq!(LayerStack::new(4).inner_count(), 2);
        assert_eq!(LayerStack::new(6).inner_count(), 4);

        assert!(!LayerStack::new(2).has_inner_layers());
        assert!(LayerStack::new(4).has_inner_layers());
    }

    #[test]
    fn test_layer_stack_constants() {
        assert_eq!(LayerStack::TWO_LAYER.count, 2);
        assert_eq!(LayerStack::FOUR_LAYER.count, 4);
        assert_eq!(LayerStack::SIX_LAYER.count, 6);
        assert_eq!(LayerStack::EIGHT_LAYER.count, 8);
    }
}
