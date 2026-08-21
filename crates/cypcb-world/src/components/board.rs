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

/// The fabricator a board is for: `fab jlcpcb`.
///
/// A name as the design wrote it, not a validated preset. This crate models
/// boards and has no table of fabricators to check a name against; whoever
/// resolves it to a rule set is the one that can say the name is not known.
/// Absent means the design did not say, which is not the same as saying the
/// project's default - a caller that needs a fab has to choose one and be
/// able to tell the reader it chose.
#[derive(Component, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fab(pub String);

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
        write!(
            f,
            "{:.2}mm x {:.2}mm",
            self.width.to_mm(),
            self.height.to_mm()
        )
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
            (Self::MIN_LAYERS..=Self::MAX_LAYERS).contains(&count),
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
        if (Self::MIN_LAYERS..=Self::MAX_LAYERS).contains(&count) {
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

/// The board's real outline, when it is not a rectangle.
///
/// [`BoardSize`] is a width and a height, which is all a rectangular board
/// needs and all this model could say until now. A board with a cutout, a
/// slot or a rounded corner is not that shape, and measuring clearance to a
/// bounding box passes copper that sits outside the actual edge.
///
/// Points are in board coordinates and the ring is closed implicitly: the last
/// point joins the first. Absent means rectangular, and everything keeps
/// using `BoardSize`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct BoardOutline {
    /// The outline ring, in order.
    pub points: Vec<cypcb_core::Point>,
}

impl BoardOutline {
    /// Build an outline, rejecting anything that is not a ring.
    ///
    /// Fewer than three points cannot enclose an area, and a caller that hands
    /// over two is describing a line rather than a board.
    pub fn new(points: Vec<cypcb_core::Point>) -> Option<Self> {
        if points.len() < 3 {
            return None;
        }
        Some(BoardOutline { points })
    }

    /// The outline's edges, as point pairs, closing the ring.
    pub fn edges(&self) -> impl Iterator<Item = (cypcb_core::Point, cypcb_core::Point)> + '_ {
        self.points
            .iter()
            .zip(self.points.iter().cycle().skip(1))
            .take(self.points.len())
            .map(|(a, b)| (*a, *b))
    }

    /// Whether a point is inside the ring.
    ///
    /// Ray casting along +x, counting crossings. A point exactly on an edge
    /// may fall either way, which is acceptable: a feature that close to the
    /// edge is a clearance violation whichever side of the line it lands on.
    pub fn contains(&self, point: cypcb_core::Point) -> bool {
        let (x, y) = (point.x.raw() as f64, point.y.raw() as f64);
        let mut inside = false;
        for (a, b) in self.edges() {
            let (ax, ay) = (a.x.raw() as f64, a.y.raw() as f64);
            let (bx, by) = (b.x.raw() as f64, b.y.raw() as f64);
            if (ay > y) != (by > y) {
                let cross = ax + (y - ay) / (by - ay) * (bx - ax);
                if x < cross {
                    inside = !inside;
                }
            }
        }
        inside
    }
}

/// What one layer of a stackup is made of.
///
/// The parser's `LayerType` with the spans left behind: a stackup in the world
/// is a physical fact about the board, and the place in the file it was
/// written is the parser's business.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StackupLayerKind {
    /// Copper foil - the layers traces live on.
    Copper,
    /// Prepreg: glass cloth and resin, cured in the press.
    Prepreg,
    /// Core: cured laminate, copper-clad on both faces.
    Core,
    /// Solder mask.
    Mask,
    /// Silkscreen.
    Silk,
    /// Solder paste, deposited through a stencil at assembly.
    ///
    /// Not a layer a fabricator presses, and in the model anyway. KiCad's own
    /// stackup carries `F.Paste` and `B.Paste` between the silkscreen and the
    /// mask, so a board read without a word for one would describe a different
    /// build than the file it came from. What a consumer does with it is that
    /// consumer's business: the Gerber job file leaves it out, because that
    /// file's stackup is the materials of the bare board.
    Paste,
}

impl StackupLayerKind {
    /// Whether this layer separates two copper layers electrically.
    ///
    /// Mask and silk sit on the outside of the board, so neither one can be
    /// what keeps two copper layers apart.
    #[inline]
    pub const fn is_dielectric(self) -> bool {
        matches!(self, StackupLayerKind::Prepreg | StackupLayerKind::Core)
    }

    /// The word a designer writes for this layer.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            StackupLayerKind::Copper => "copper",
            StackupLayerKind::Prepreg => "prepreg",
            StackupLayerKind::Core => "core",
            StackupLayerKind::Mask => "mask",
            StackupLayerKind::Silk => "silk",
            StackupLayerKind::Paste => "paste",
        }
    }
}

/// One layer of the board, top to bottom.
///
/// Not `Copy`: a layer carries the two names a fabricator needs, and a name is
/// a `String`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StackupLayer {
    /// What the layer is made of.
    pub kind: StackupLayerKind,
    /// What the fabricator calls this layer, when the design says.
    ///
    /// `F.Cu`, `In1.Cu`, `dielectric 2` - the names KiCad's own board file
    /// carries, which is why they are stored as written rather than parsed
    /// into a layer index. A stackup entry and a copper layer are not the same
    /// thing: a four-layer board has nine or eleven stackup entries.
    pub name: Option<String>,
    /// How thick it is, when the design says.
    pub thickness: Option<Nm>,
    /// The laminate or foil the fabricator is asked for: `FR4`, `Isola 370HR`.
    ///
    /// Held as written. Nothing in this project has a table of laminates to
    /// check it against, and a material this tool did not recognise is still
    /// the material the board is quoted on.
    pub material: Option<String>,
    /// The dielectric constant, in thousandths.
    ///
    /// `4.5` is `4500`. Fixed point rather than an `f64` because this struct
    /// is `Eq` and `Hash`, and because a laminate datasheet publishes three
    /// decimals at most - `3.66`, `4.05`, `3.48`.
    pub dk_x1000: Option<u32>,
    /// The loss tangent, in millionths.
    ///
    /// `0.0089` is `8900`. A different scale from `dk_x1000` because the
    /// numbers are: a low-loss laminate publishes `0.0021` and PTFE `0.0002`,
    /// so thousandths would round both to nothing. The suffix names the scale
    /// at every use, which is the point of writing it out.
    pub df_x1000000: Option<u32>,
}

impl StackupLayer {
    /// A layer that states only what it is and how thick.
    ///
    /// The two names are what a design adds when it cares which fabricator
    /// builds it; most designs state neither, and every caller that predates
    /// them means this.
    pub fn new(kind: StackupLayerKind, thickness: Option<Nm>) -> Self {
        StackupLayer {
            kind,
            name: None,
            thickness,
            material: None,
            dk_x1000: None,
            df_x1000000: None,
        }
    }
}

/// Which impedance form a copper layer's surroundings call for, and with what.
///
/// Produced by [`Stackup::environment_of`] and consumed by `cypcb-calc`. The
/// two variants are the two closed forms IPC-2141 states; a stack that is
/// neither produces no variant rather than the nearer of the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CopperEnvironment {
    /// An outer layer over one reference plane.
    Microstrip {
        /// The dielectric between this copper and the next copper inward.
        height: Nm,
        /// That dielectric's constant, in thousandths.
        dk_x1000: u32,
        /// This layer's own foil thickness, which both forms need as `T`.
        copper: Nm,
    },
    /// An inner layer centred between two reference planes.
    Stripline {
        /// The distance between the two planes, which is both dielectrics.
        plate_separation: Nm,
        /// Their shared dielectric constant, in thousandths.
        dk_x1000: u32,
        /// This layer's own foil thickness, which both forms need as `T`.
        copper: Nm,
    },
}

/// The layers a fabricator presses together, in order from the top.
///
/// A design states this to say what it expects to be built - which is a claim
/// that can disagree with the rest of the design, and did so silently for as
/// long as this was parsed and dropped. It is deliberately not the same thing
/// as [`LayerStack`], which counts copper layers and nothing else: a board can
/// declare `layers 4` and then describe a stackup with two.
#[derive(Component, Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Stackup {
    /// Every layer, top to bottom.
    pub layers: Vec<StackupLayer>,
}

impl Stackup {
    /// How many copper layers the stackup describes.
    #[inline]
    pub fn copper_count(&self) -> usize {
        self.layers
            .iter()
            .filter(|layer| layer.kind == StackupLayerKind::Copper)
            .count()
    }

    /// Total thickness, or `None` when any layer left it unsaid.
    ///
    /// Partial answers are worse than none here: a board reported as 0.2mm
    /// thick because two of its five layers stated a thickness reads like a
    /// measurement rather than like a gap in the design.
    pub fn total_thickness(&self) -> Option<Nm> {
        self.layers
            .iter()
            .try_fold(Nm(0), |total, layer| Some(total + layer.thickness?))
    }

    /// Where a copper layer sits, as the impedance forms need it.
    ///
    /// `copper_index` counts copper entries from the top, so 0 is the outer
    /// layer of the face the stackup starts on.
    ///
    /// Returns `None` when no form this project has applies. That is not the
    /// same as "the answer is hard": each case below is a geometry the closed
    /// forms in `cypcb-calc` are not written for, and a number produced for it
    /// anyway would read like a measurement.
    ///
    /// - **An outer copper layer** gets [`CopperEnvironment::Microstrip`], with
    ///   the height and the dielectric constant of the one dielectric between
    ///   it and the next copper inward.
    /// - **An inner copper layer** gets [`CopperEnvironment::Stripline`] only
    ///   when it is genuinely centred: the dielectric above and the dielectric
    ///   below have the **same thickness and the same `dk`**. The form is the
    ///   *symmetric* stripline, and a trace nearer one plane than the other is
    ///   an asymmetric stripline, which is a different equation this project
    ///   does not have. Most four-layer stacks put prepreg on one side of an
    ///   inner layer and core on the other, so most inner layers answer `None`
    ///   here - correctly.
    /// - A layer with no dielectric beside it, or a dielectric that states no
    ///   thickness or no `dk`, answers `None` for the same reason.
    pub fn environment_of(&self, copper_index: usize) -> Option<CopperEnvironment> {
        let coppers: Vec<usize> = self
            .layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| layer.kind == StackupLayerKind::Copper)
            .map(|(index, _)| index)
            .collect();
        let at = *coppers.get(copper_index)?;
        // `T` in both equations. A stack that does not state its foil cannot
        // answer either of them, for the same reason a missing `dk` cannot.
        let copper = self.layers[at].thickness?;

        // The nearest dielectric on each side, and nothing but surface
        // finishes allowed in between: a second copper layer between this one
        // and the dielectric would mean the two are shorted, which
        // `copper_touching_copper` reports rather than measures.
        let above = self.layers[..at]
            .iter()
            .rev()
            .take_while(|layer| layer.kind != StackupLayerKind::Copper)
            .find(|layer| layer.kind.is_dielectric());
        let below = self.layers[at + 1..]
            .iter()
            .take_while(|layer| layer.kind != StackupLayerKind::Copper)
            .find(|layer| layer.kind.is_dielectric());

        let outer = copper_index == 0 || copper_index + 1 == coppers.len();
        if outer {
            // The dielectric on the inward side: below for the top layer,
            // above for the bottom one. A one-copper stack has no inward side.
            let inward = if copper_index == 0 { below } else { above };
            let inward = inward?;
            return Some(CopperEnvironment::Microstrip {
                height: inward.thickness?,
                dk_x1000: inward.dk_x1000?,
                copper,
            });
        }

        let (above, below) = (above?, below?);
        let (top, bottom) = (above.thickness?, below.thickness?);
        let (top_dk, bottom_dk) = (above.dk_x1000?, below.dk_x1000?);
        if top != bottom || top_dk != bottom_dk {
            return None;
        }
        Some(CopperEnvironment::Stripline {
            plate_separation: Nm(top.raw() + bottom.raw()),
            dk_x1000: top_dk,
            copper,
        })
    }

    /// Pairs of adjacent copper layers with no dielectric between them.
    ///
    /// Returns the index of the first of each pair. Two copper foils pressed
    /// together are one thicker foil, so a stackup written this way describes
    /// a board whose layers are shorted to each other by construction.
    pub fn copper_touching_copper(&self) -> Vec<usize> {
        let mut found = Vec::new();
        let mut previous_copper: Option<usize> = None;
        for (index, layer) in self.layers.iter().enumerate() {
            match layer.kind {
                StackupLayerKind::Copper => {
                    if let Some(previous) = previous_copper {
                        found.push(previous);
                    }
                    previous_copper = Some(index);
                }
                kind if kind.is_dielectric() => previous_copper = None,
                // Mask and silk are surface finishes: they sit on top of
                // copper rather than between two copper layers, so they do not
                // clear the pairing.
                _ => {}
            }
        }
        found
    }
}
