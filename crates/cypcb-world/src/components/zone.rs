//! Zone components for keepouts and copper pours.
//!
//! Zones define rectangular regions on the board with special properties:
//! - Keepouts prevent copper from being placed in the region
//! - Copper pours fill the region with copper connected to a net

use crate::components::NetId;
use bevy_ecs::prelude::*;
use cypcb_core::Rect;

/// Zone type (keepout vs copper pour).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum ZoneKind {
    /// No copper allowed in this region.
    Keepout,
    /// Copper fill zone (pour) - connected to a net.
    CopperPour,
    /// The part of the board that bends.
    ///
    /// A rigid-flex board is one panel with rigid sections and a flexible
    /// ribbon between them. The ribbon is not a keepout - copper crosses it,
    /// that is what it is for - and it is not a pour. What it is, is an area
    /// the board is bent in during service, and a plated hole in a bend
    /// cracks: the barrel is copper and the laminate around it moves.
    Flex,
}

/// A zone entity (keepout or copper pour).
///
/// Zones define rectangular regions with special properties.
/// The layer_mask indicates which layers the zone applies to.
///
/// # Examples
///
/// ```rust
/// use cypcb_world::components::zone::{Zone, ZoneKind};
/// use cypcb_core::{Rect, Point, Nm};
///
/// // Create a keepout zone on all layers
/// let bounds = Rect::new(
///     Point::from_mm(10.0, 10.0),
///     Point::from_mm(20.0, 20.0),
/// );
/// let keepout = Zone::keepout(bounds, 0b11); // Top and bottom
///
/// assert!(keepout.is_keepout());
/// assert!(keepout.contains(Point::from_mm(15.0, 15.0)));
/// ```
#[derive(Debug, Clone, Component)]
pub struct Zone {
    /// Zone bounds in nanometers.
    pub bounds: Rect,
    /// Zone type (keepout or copper pour).
    pub kind: ZoneKind,
    /// Layer mask (which layers this zone applies to).
    /// Bit 0 = top copper, bit 1 = bottom copper, etc.
    pub layer_mask: u32,
    /// Optional name for reference.
    pub name: Option<String>,
    /// Net this zone is poured to. Always `None` for a keepout.
    ///
    /// A pour without a net is not a pour: it cannot be filled, it cannot be
    /// checked against foreign copper, and the pads it swallows are not
    /// connected to anything.
    pub net: Option<NetId>,
}

impl Zone {
    /// Create a new keepout zone.
    ///
    /// # Arguments
    ///
    /// * `bounds` - The rectangular bounds of the zone
    /// * `layer_mask` - Bit mask of layers this zone applies to
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cypcb_world::components::zone::Zone;
    /// use cypcb_core::{Rect, Point};
    ///
    /// let bounds = Rect::from_center_size(
    ///     Point::from_mm(15.0, 15.0),
    ///     (cypcb_core::Nm::from_mm(10.0), cypcb_core::Nm::from_mm(10.0)),
    /// );
    /// let zone = Zone::keepout(bounds, 0b11);
    /// assert!(zone.is_keepout());
    /// ```
    pub fn keepout(bounds: Rect, layer_mask: u32) -> Self {
        Zone {
            net: None,
            bounds,
            kind: ZoneKind::Keepout,
            layer_mask,
            name: None,
        }
    }

    /// Create a flexible region: the part of the board that bends.
    pub fn flex(bounds: Rect, layer_mask: u32) -> Self {
        Zone {
            net: None,
            bounds,
            kind: ZoneKind::Flex,
            layer_mask,
            name: None,
        }
    }

    /// Create a new copper pour zone.
    ///
    /// # Arguments
    ///
    /// * `bounds` - The rectangular bounds of the zone
    /// * `layer_mask` - Bit mask of layers this zone applies to
    pub fn copper_pour(bounds: Rect, layer_mask: u32) -> Self {
        Zone {
            net: None,
            bounds,
            kind: ZoneKind::CopperPour,
            layer_mask,
            name: None,
        }
    }

    /// Create a copper pour tied to a net.
    ///
    /// This is what a ground plane is. The pads of that net which fall inside
    /// the pour are connected by it, so the router does not have to reach them
    /// and the checker must not read them as shorts.
    pub fn copper_pour_for_net(bounds: Rect, layer_mask: u32, net: NetId) -> Self {
        Zone {
            net: Some(net),
            ..Zone::copper_pour(bounds, layer_mask)
        }
    }

    /// Set the zone name.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cypcb_world::components::zone::Zone;
    /// use cypcb_core::{Rect, Point};
    ///
    /// let bounds = Rect::new(Point::ORIGIN, Point::from_mm(10.0, 10.0));
    /// let zone = Zone::keepout(bounds, 0b11).with_name("antenna_clearance");
    /// assert_eq!(zone.name.as_deref(), Some("antenna_clearance"));
    /// ```
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Check if this is a keepout zone.
    #[inline]
    pub fn is_keepout(&self) -> bool {
        self.kind == ZoneKind::Keepout
    }

    /// Check if this is a copper pour zone.
    #[inline]
    pub fn is_copper_pour(&self) -> bool {
        self.kind == ZoneKind::CopperPour
    }

    /// Check if this is a flexible region: the part of the board that bends.
    #[inline]
    pub fn is_flex(&self) -> bool {
        self.kind == ZoneKind::Flex
    }

    /// Check if a point is inside the zone bounds.
    #[inline]
    pub fn contains(&self, point: cypcb_core::Point) -> bool {
        self.bounds.contains(point)
    }

    /// Check if this zone applies to a specific layer.
    ///
    /// # Arguments
    ///
    /// * `layer` - Layer index (0 = top, 1 = bottom, etc.)
    #[inline]
    pub fn on_layer(&self, layer: u8) -> bool {
        self.layer_mask & (1 << layer) != 0
    }

    /// Check if this zone shares any layers with the given mask.
    #[inline]
    pub fn layers_overlap(&self, mask: u32) -> bool {
        self.layer_mask & mask != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::Point;

    fn make_bounds() -> Rect {
        Rect::new(Point::from_mm(10.0, 10.0), Point::from_mm(20.0, 20.0))
    }

    #[test]
    fn test_keepout_zone() {
        let zone = Zone::keepout(make_bounds(), 0b11);
        assert!(zone.is_keepout());
        assert!(!zone.is_copper_pour());
        assert_eq!(zone.kind, ZoneKind::Keepout);
    }

    #[test]
    fn test_copper_pour_zone() {
        let zone = Zone::copper_pour(make_bounds(), 0b01);
        assert!(!zone.is_keepout());
        assert!(zone.is_copper_pour());
        assert_eq!(zone.kind, ZoneKind::CopperPour);
    }

    #[test]
    fn test_with_name() {
        let zone = Zone::keepout(make_bounds(), 0b11).with_name("antenna_clearance");
        assert_eq!(zone.name.as_deref(), Some("antenna_clearance"));
    }

    #[test]
    fn test_contains() {
        let zone = Zone::keepout(make_bounds(), 0b11);

        // Point inside
        assert!(zone.contains(Point::from_mm(15.0, 15.0)));

        // Point outside
        assert!(!zone.contains(Point::from_mm(5.0, 5.0)));
        assert!(!zone.contains(Point::from_mm(25.0, 25.0)));

        // Point on boundary
        assert!(zone.contains(Point::from_mm(10.0, 10.0)));
    }

    #[test]
    fn test_on_layer() {
        let zone = Zone::keepout(make_bounds(), 0b101); // Layers 0 and 2

        assert!(zone.on_layer(0));
        assert!(!zone.on_layer(1));
        assert!(zone.on_layer(2));
        assert!(!zone.on_layer(3));
    }

    #[test]
    fn test_layers_overlap() {
        let zone = Zone::keepout(make_bounds(), 0b101); // Layers 0 and 2

        assert!(zone.layers_overlap(0b001)); // Overlaps with layer 0
        assert!(!zone.layers_overlap(0b010)); // No overlap with layer 1
        assert!(zone.layers_overlap(0b111)); // Overlaps with layers 0 and 2
    }

    #[test]
    fn test_all_layers() {
        let zone = Zone::keepout(make_bounds(), 0xFFFFFFFF);

        for layer in 0..32 {
            assert!(zone.on_layer(layer), "should be on layer {}", layer);
        }
    }
}
