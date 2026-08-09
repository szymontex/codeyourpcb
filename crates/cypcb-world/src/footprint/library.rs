//! Footprint library with data structures and lookup.

use std::collections::HashMap;

use bevy_ecs::prelude::Resource;

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
///     slot: None,
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
///     slot: None,
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
    ///
    /// For a slot this is the narrow dimension, which is what every rule
    /// about a drill means: the smallest bit the fab has to own, the width
    /// the plating has to reach down, the wall a router bit can break into.
    pub drill: Option<Nm>,
    /// The hole's full size when it is a slot rather than a round hole.
    ///
    /// `(width, height)` in the pad's own frame, exactly as KiCad writes
    /// `(drill oval 2.4 1.0)`, so the pair survives a round trip. `None` is a
    /// round hole, which is nearly every hole on nearly every board.
    ///
    /// A slot is milled rather than drilled, and it is not a detail: a USB
    /// connector, a barrel jack and a latching header all hold themselves to
    /// the board through one. A slot delivered as a round hole is a part that
    /// does not fit and a board that is scrap, so nothing downstream may
    /// quietly round it - the drill file writes the routed path and the KiCad
    /// file writes the oval back.
    pub slot: Option<(Nm, Nm)>,
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

    /// A drilled hole with no copper on any layer: a mounting hole, a tooling
    /// hole, a slot for a connector's latch.
    ///
    /// This is not a flag somebody has to remember to set. A hole with copper
    /// around it is plated by definition and a hole without copper cannot be,
    /// so the geometry already says which it is. KiCad calls it
    /// `np_thru_hole`; the importer drops the copper layers off such a pad and
    /// this reads the result back.
    ///
    /// It matters to three places that would otherwise get it wrong: the drill
    /// file, which must list it separately or the fabricator plates it - an
    /// M3 hole comes back a tenth of a millimetre narrower and shorted to
    /// whatever copper it touches; the copper layers, which must not flash a
    /// pad there; and the router, which must still treat it as solid, because
    /// a hole with no copper is still a hole.
    #[inline]
    pub fn is_non_plated(&self) -> bool {
        self.drill.is_some() && !self.layers.iter().any(|layer| layer.is_copper())
    }

    /// Whether this hole is a slot: milled along its length, not drilled.
    ///
    /// A pair that is square is a round hole written the long way, so it is
    /// not one - `(drill oval 1.0 1.0)` is a 1mm drill and saying otherwise
    /// would send a routing path to the fab for a hole a bit makes in one hit.
    #[inline]
    pub fn is_slot(&self) -> bool {
        matches!(self.slot, Some((width, height)) if width != height)
    }

    /// Half the distance the milling bit travels, in the pad's own frame.
    ///
    /// A slot `(w, h)` is cut with a bit the width of its narrow dimension
    /// moving along the long one, so the bit's centre stops half a bit short
    /// of each end and the travel is `long - narrow`. The two ends of the hole
    /// are the pad's position plus and minus this.
    ///
    /// `None` for a round hole, whose two ends are the same point. It lives
    /// here rather than in the drill writer because the checker needs the same
    /// two ends: a hole measured at its centre is up to its own length wrong
    /// about how close it is to anything.
    #[inline]
    pub fn slot_half_travel(&self) -> Option<Point> {
        let (width, height) = self.slot?;
        if width == height {
            return None;
        }
        Some(if width > height {
            Point::new(Nm((width.0 - height.0) / 2), Nm(0))
        } else {
            Point::new(Nm(0), Nm((height.0 - width.0) / 2))
        })
    }
}

impl Footprint {
    /// Whether this footprint is mechanical: a mounting hole, a tooling hole,
    /// a fiducial-free bracket cutout - something the board carries but nobody
    /// solders.
    ///
    /// True when it has no copper anywhere. Such a part must not reach a bill
    /// of materials, where it becomes a line somebody is asked to buy, and
    /// must not reach a placement file, where a machine is asked to put a hole
    /// on the board.
    pub fn is_mechanical(&self) -> bool {
        !self.pads.is_empty()
            && self
                .pads
                .iter()
                .all(|pad| !pad.layers.iter().any(|layer| layer.is_copper()))
    }
}

/// A piece of silkscreen artwork, in footprint coordinates.
///
/// What a fabricator prints as the legend: outlines, polarity marks, pin-one
/// dots. Held per footprint because that is where it comes from - a courtyard
/// outline is what the exporter can derive, artwork is what a real footprint
/// carries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SilkShape {
    /// A straight line of the given width.
    Segment {
        /// One end.
        start: Point,
        /// The other end.
        end: Point,
        /// Stroke width.
        width: Nm,
    },
    /// A circle outline of the given stroke width.
    Circle {
        /// Centre.
        centre: Point,
        /// Radius to the centre of the stroke.
        radius: Nm,
        /// Stroke width.
        width: Nm,
    },
}

impl SilkShape {
    /// The stroke width, whatever the shape.
    #[inline]
    pub fn width(&self) -> Nm {
        match self {
            SilkShape::Segment { width, .. } | SilkShape::Circle { width, .. } => *width,
        }
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
    /// Assembly courtyard: everything the placed part occupies, plus the
    /// excess a placement machine needs to work.
    ///
    /// Per IPC-7351 that is the body **and the land pattern** - the pads are
    /// part of what the part occupies. Build it with
    /// [`Footprint::with_ipc_courtyard`] rather than by hand; a courtyard
    /// derived from the body alone is smaller than the copper it is supposed
    /// to enclose, and everything that reasons about the space a part takes
    /// reads this rectangle.
    pub courtyard: Rect,
    /// Silkscreen artwork, in footprint coordinates.
    ///
    /// Empty means the footprint says nothing about its legend, and the only
    /// ink the exporter draws is the courtyard outline. A part fetched from a
    /// supplier arrives with real artwork, and this is where it survives.
    pub silk: Vec<SilkShape>,
}

/// Courtyard excess per side, per IPC-7351B nominal density.
///
/// The clearance a placement machine needs around everything the part
/// occupies, measured from the outside of the land pattern rather than from
/// the body.
pub const IPC_COURTYARD_EXCESS: Nm = Nm(250_000);

impl Footprint {
    /// Set the courtyard to enclose the body and every pad, plus
    /// [`IPC_COURTYARD_EXCESS`].
    ///
    /// The built-in footprints each derived their own courtyard from the body,
    /// which is smaller than the land pattern on every two-terminal chip part
    /// there is: an 0805's body is 2.0mm wide and its pads span 2.9mm, so the
    /// courtyard stopped 0.2mm inside its own copper on each side. Two
    /// consumers were reading that box and believing it - `courtyard-clearance`
    /// as the space a part needs, and `silk_text` as the height a designator
    /// has to clear - so both under-reported by the same 0.2mm.
    ///
    /// A footprint with no pads keeps a courtyard around its body alone, which
    /// is all a mechanical part has.
    #[must_use]
    pub fn with_ipc_courtyard(mut self) -> Self {
        let mut min_x = self.bounds.min.x.raw();
        let mut min_y = self.bounds.min.y.raw();
        let mut max_x = self.bounds.max.x.raw();
        let mut max_y = self.bounds.max.y.raw();

        for pad in &self.pads {
            let half_width = pad.size.0.raw() / 2;
            let half_height = pad.size.1.raw() / 2;
            min_x = min_x.min(pad.position.x.raw() - half_width);
            min_y = min_y.min(pad.position.y.raw() - half_height);
            max_x = max_x.max(pad.position.x.raw() + half_width);
            max_y = max_y.max(pad.position.y.raw() + half_height);
        }

        let excess = IPC_COURTYARD_EXCESS.raw();
        self.courtyard = Rect::new(
            Point::new(Nm(min_x - excess), Nm(min_y - excess)),
            Point::new(Nm(max_x + excess), Nm(max_y + excess)),
        );
        self
    }

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
/// Stored in the board world as a resource so every consumer - DRC rules,
/// export, the renderer - resolves pads through the same table, including the
/// footprints a design defines inline.
#[derive(Debug, Default, Clone, Resource)]
pub struct FootprintLibrary {
    footprints: HashMap<String, Footprint>,
    /// Footprints registered from a design source, mapped to whatever entry they
    /// shadowed, so [`clear_design`](FootprintLibrary::clear_design) can undo them.
    design_defined: HashMap<String, Option<Footprint>>,
}

impl FootprintLibrary {
    /// Create a new footprint library with built-in footprints.
    pub fn new() -> Self {
        let mut lib = Self::default();
        lib.register_builtin_smd();
        lib.register_builtin_tht();
        lib.register_builtin_gullwing();
        lib.register_builtin_mounting();
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

    /// Register a footprint that came from the design source rather than the
    /// built-in set.
    ///
    /// Design-defined footprints are tracked so that [`clear_design`](FootprintLibrary::clear_design)
    /// can undo them on the next sync. Without that, a footprint deleted from the
    /// source would keep resolving from a previous sync, and one that shadowed a
    /// built-in would hide it forever.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_world::footprint::{Footprint, FootprintLibrary};
    /// use cypcb_core::{Point, Rect};
    ///
    /// let empty = Rect::new(Point::ORIGIN, Point::ORIGIN);
    /// let mut lib = FootprintLibrary::new();
    /// lib.register_design(Footprint {
    ///     name: "MY_PART".to_string(),
    ///     description: String::new(),
    ///     pads: Vec::new(),
    ///     bounds: empty,
    ///     courtyard: empty,
    ///     silk: Vec::new(),
    /// });
    /// assert!(lib.contains("MY_PART"));
    ///
    /// lib.clear_design();
    /// assert!(!lib.contains("MY_PART"));
    /// assert!(lib.contains("0402")); // built-ins are untouched
    /// ```
    pub fn register_design(&mut self, footprint: Footprint) {
        let name = footprint.name.clone();
        let shadowed = self.footprints.insert(name.clone(), footprint);
        // Only the first registration records what was shadowed - re-registering
        // the same name must not snapshot the design footprint over the built-in.
        self.design_defined.entry(name).or_insert(shadowed);
    }

    /// Drop every footprint added by [`register_design`](FootprintLibrary::register_design),
    /// restoring any built-in they shadowed.
    pub fn clear_design(&mut self) {
        for (name, shadowed) in self.design_defined.drain() {
            match shadowed {
                Some(builtin) => self.footprints.insert(name, builtin),
                None => self.footprints.remove(&name),
            };
        }
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

    /// Register the built-in mounting holes.
    fn register_builtin_mounting(&mut self) {
        use super::mounting::*;

        self.register(mount_m2());
        self.register(mount_m2_5());
        self.register(mount_m3());
        self.register(mount_m4());
    }

    /// Register all built-in gull-wing IC footprints.
    fn register_builtin_gullwing(&mut self) {
        use super::gullwing::*;

        self.register(soic8());
        self.register(soic14());
        self.register(sot23());
        self.register(sot23_5());
        self.register(tqfp32());
    }
}

/// The same footprint, soldered to the other face of the board.
///
/// A part is flipped over, not moved: seen from above - which is how every
/// coordinate in this project is written - its local x axis reverses, so a pad
/// at +1mm ends up at -1mm, and every layer it touches moves to the matching
/// layer on the other side. Nothing else changes; the pads keep their numbers,
/// their sizes and their drills.
///
/// This exists so the flip happens once. The checker, the four Gerber writers,
/// the drill file, the renderer and the pick-and-place list all read a
/// footprint out of the library and place it themselves, and a mirror
/// implemented six times is a board where the copper and the solder mask
/// disagree about which side a part is on.
pub fn mirrored_to_bottom(footprint: &Footprint) -> Footprint {
    Footprint {
        name: bottom_name(&footprint.name),
        description: format!("{} (bottom side)", footprint.description),
        pads: footprint
            .pads
            .iter()
            .map(|pad| PadDef {
                number: pad.number.clone(),
                shape: pad.shape,
                position: mirror_point(pad.position),
                size: pad.size,
                drill: pad.drill,
                slot: None,
                layers: pad.layers.iter().map(|layer| flip(*layer)).collect(),
            })
            .collect(),
        bounds: mirror_rect(footprint.bounds),
        courtyard: mirror_rect(footprint.courtyard),
        silk: footprint
            .silk
            .iter()
            .map(|shape| match *shape {
                SilkShape::Segment { start, end, width } => SilkShape::Segment {
                    start: mirror_point(start),
                    end: mirror_point(end),
                    width,
                },
                SilkShape::Circle {
                    centre,
                    radius,
                    width,
                } => SilkShape::Circle {
                    centre: mirror_point(centre),
                    radius,
                    width,
                },
            })
            .collect(),
    }
}

/// What the flipped copy of a footprint is called in the library.
///
/// The suffix is not a legal footprint name a design could write, which is
/// deliberate: these entries are derived, and a design naming one directly
/// would be asking for a part whose mirroring nobody decided.
pub fn bottom_name(name: &str) -> String {
    format!("{name}@bottom")
}

/// The footprint a derived bottom-side entry was made from.
///
/// The mirrored copy is this project's own arrangement. Anything writing a
/// file for somebody else - a KiCad board, a library reference - wants the
/// name the design asked for, and its own convention for saying which face the
/// part is on.
pub fn base_name(name: &str) -> &str {
    name.strip_suffix("@bottom").unwrap_or(name)
}

/// Mirror about the footprint's own y axis.
fn mirror_point(point: Point) -> Point {
    Point::new(Nm(-point.x.0), point.y)
}

/// A rectangle mirrored is still a rectangle, with its corners swapped in x.
fn mirror_rect(rect: Rect) -> Rect {
    Rect::new(
        Point::new(Nm(-rect.max.x.0), rect.min.y),
        Point::new(Nm(-rect.min.x.0), rect.max.y),
    )
}

/// The layer on the other face of the board.
///
/// Inner layers are untouched: a part has no pads on them, and a footprint
/// that somehow named one is describing something this function has no opinion
/// about.
fn flip(layer: Layer) -> Layer {
    match layer {
        Layer::TopCopper => Layer::BottomCopper,
        Layer::BottomCopper => Layer::TopCopper,
        Layer::TopMask => Layer::BottomMask,
        Layer::BottomMask => Layer::TopMask,
        Layer::TopPaste => Layer::BottomPaste,
        Layer::BottomPaste => Layer::TopPaste,
        Layer::TopSilk => Layer::BottomSilk,
        Layer::BottomSilk => Layer::TopSilk,
        other => other,
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
            silk: Vec::new(),
            courtyard: Rect::default(),
        };

        lib.register(custom);
        assert_eq!(lib.len(), initial_count + 1);
        assert!(lib.contains("CUSTOM-1"));
    }

    #[test]
    fn test_clear_design_removes_only_design_footprints() {
        let mut lib = FootprintLibrary::new();
        let initial_count = lib.len();

        lib.register_design(Footprint {
            name: "DESIGN-1".into(),
            description: String::new(),
            pads: vec![],
            bounds: Rect::default(),
            silk: Vec::new(),
            courtyard: Rect::default(),
        });
        lib.register(Footprint {
            name: "MANUAL-1".into(),
            description: String::new(),
            pads: vec![],
            bounds: Rect::default(),
            silk: Vec::new(),
            courtyard: Rect::default(),
        });
        assert_eq!(lib.len(), initial_count + 2);

        lib.clear_design();
        assert!(
            !lib.contains("DESIGN-1"),
            "design footprint must be dropped"
        );
        assert!(lib.contains("MANUAL-1"), "manual registration must survive");
        assert!(lib.contains("0402"), "built-ins must survive");
        assert_eq!(lib.len(), initial_count + 1);
    }

    #[test]
    fn test_clear_design_restores_shadowed_builtin() {
        let mut lib = FootprintLibrary::new();
        let builtin_pads = lib.get("0402").expect("0402 is built in").pads.len();
        assert_ne!(builtin_pads, 0);

        // A design may redefine a built-in name. Dropping the design footprint
        // has to bring the built-in back, not delete it.
        lib.register_design(Footprint {
            name: "0402".into(),
            description: "redefined by the design".into(),
            pads: vec![],
            bounds: Rect::default(),
            silk: Vec::new(),
            courtyard: Rect::default(),
        });
        assert_eq!(lib.get("0402").unwrap().pads.len(), 0);

        lib.clear_design();
        assert_eq!(lib.get("0402").unwrap().pads.len(), builtin_pads);
    }

    #[test]
    fn test_register_design_twice_keeps_original_shadowed_entry() {
        let mut lib = FootprintLibrary::new();
        let builtin_pads = lib.get("0603").expect("0603 is built in").pads.len();

        for description in ["first", "second"] {
            lib.register_design(Footprint {
                name: "0603".into(),
                description: description.into(),
                pads: vec![],
                bounds: Rect::default(),
                silk: Vec::new(),
                courtyard: Rect::default(),
            });
        }

        lib.clear_design();
        assert_eq!(lib.get("0603").unwrap().pads.len(), builtin_pads);
    }

    #[test]
    fn test_pad_def_is_smd() {
        let smd = PadDef {
            number: "1".into(),
            shape: PadShape::Rect,
            position: Point::ORIGIN,
            size: (Nm::from_mm(0.5), Nm::from_mm(0.5)),
            drill: None,
            slot: None,
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
            slot: None,
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

    #[test]
    fn test_library_has_gullwing_footprints() {
        let lib = FootprintLibrary::new();

        // SOIC footprints
        assert!(lib.contains("SOIC-8"));
        assert!(lib.contains("SOIC-14"));

        // SOT footprints
        assert!(lib.contains("SOT-23"));
        assert!(lib.contains("SOT-23-5"));

        // QFP footprints
        assert!(lib.contains("TQFP-32"));
    }

    #[test]
    fn test_soic8_from_library() {
        let lib = FootprintLibrary::new();
        let fp = lib.get("SOIC-8").expect("SOIC-8 should exist");

        assert_eq!(fp.pads.len(), 8);
        assert!(fp.get_pad("1").is_some());
        assert!(fp.get_pad("8").is_some());
    }

    #[test]
    fn test_sot23_from_library() {
        let lib = FootprintLibrary::new();
        let fp = lib.get("SOT-23").expect("SOT-23 should exist");

        assert_eq!(fp.pads.len(), 3);
        assert!(fp.get_pad("1").is_some());
        assert!(fp.get_pad("2").is_some());
        assert!(fp.get_pad("3").is_some());
    }

    #[test]
    fn test_tqfp32_from_library() {
        let lib = FootprintLibrary::new();
        let fp = lib.get("TQFP-32").expect("TQFP-32 should exist");

        assert_eq!(fp.pads.len(), 32);

        // Check first and last pins
        assert!(fp.get_pad("1").is_some());
        assert!(fp.get_pad("32").is_some());

        // Check pins at each side boundary
        assert!(fp.get_pad("8").is_some()); // End of bottom side
        assert!(fp.get_pad("9").is_some()); // Start of right side
        assert!(fp.get_pad("16").is_some()); // End of right side
        assert!(fp.get_pad("17").is_some()); // Start of top side
        assert!(fp.get_pad("24").is_some()); // End of top side
        assert!(fp.get_pad("25").is_some()); // Start of left side
    }
}
