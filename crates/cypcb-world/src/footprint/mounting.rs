//! Mounting holes: the holes a board is screwed down by.
//!
//! A mounting hole is a footprint with one pad, and that pad has a drill and
//! no copper. That is not a trick of representation - it is what the thing is,
//! and it is how KiCad carries them too, as `np_thru_hole` pads inside stock
//! `MountingHole_*` footprints. Modelling them as footprints rather than as a
//! new kind of board object means the placement, the DRC, the router and the
//! drill files all treat them with code that already exists.
//!
//! The pad has no number, like KiCad's. Nothing connects to a mounting hole,
//! so there is no pin for a net to name.
//!
//! Sizes are the close-fit clearance drills for the metric screw series -
//! 2.2mm for M2, 2.7 for M2.5, 3.2 for M3, 4.3 for M4. They are the drill,
//! not the screw: an M3 screw passes through a 3.2mm hole.

use cypcb_core::{Nm, Point, Rect};

use super::library::{Footprint, PadDef};
use crate::components::PadShape;

/// How much wider than the hole the courtyard is drawn.
///
/// A screw head and its washer overhang the hole, and nothing may be placed
/// under them. 2mm of radius covers a washer for the sizes here; a board that
/// needs a wider standoff says so with a keepout.
const HEAD_CLEARANCE: Nm = Nm(2_000_000);

/// One mounting hole, drilled to `drill` and carrying no copper.
fn mounting_hole(name: &str, screw: &str, drill: Nm) -> Footprint {
    let outer = drill + HEAD_CLEARANCE + HEAD_CLEARANCE;

    Footprint {
        name: name.into(),
        description: format!(
            "{screw} mounting hole, {:.1}mm clearance drill, not plated",
            drill.to_mm()
        ),
        pads: vec![PadDef {
            // KiCad writes no number on an NPTH pad, and nothing here needs
            // one: a mounting hole has no pin for a net to connect to.
            number: String::new(),
            shape: PadShape::Circle,
            position: Point::ORIGIN,
            size: (drill, drill),
            drill: Some(drill),
            slot: None,
            // The whole point. `PadDef::is_non_plated` reads this, the drill
            // file splits on it, and the router blocks the hole because of it.
            layers: Vec::new(),
        }],
        bounds: Rect::from_center_size(Point::ORIGIN, (drill, drill)),
        courtyard: Rect::from_center_size(Point::ORIGIN, (outer, outer)),
        silk: Vec::new(),
    }
}

/// M2 mounting hole, 2.2mm drill.
pub fn mount_m2() -> Footprint {
    mounting_hole("MOUNT-M2", "M2", Nm::from_mm(2.2))
}

/// M2.5 mounting hole, 2.7mm drill.
pub fn mount_m2_5() -> Footprint {
    mounting_hole("MOUNT-M2.5", "M2.5", Nm::from_mm(2.7))
}

/// M3 mounting hole, 3.2mm drill.
pub fn mount_m3() -> Footprint {
    mounting_hole("MOUNT-M3", "M3", Nm::from_mm(3.2))
}

/// M4 mounting hole, 4.3mm drill.
pub fn mount_m4() -> Footprint {
    mounting_hole("MOUNT-M4", "M4", Nm::from_mm(4.3))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mounting_hole_is_a_hole_without_copper() {
        for footprint in [mount_m2(), mount_m2_5(), mount_m3(), mount_m4()] {
            let pad = &footprint.pads[0];
            assert!(
                pad.is_non_plated(),
                "{} came out plated, so the fabricator narrows it and shorts it",
                footprint.name
            );
            assert_eq!(
                pad.drill,
                Some(pad.size.0),
                "{}: the hole is the pad - there is no annular ring to be wider than",
                footprint.name
            );
        }
    }

    #[test]
    fn the_drill_is_the_clearance_hole_not_the_screw() {
        // An M3 screw is 3mm across and does not pass through a 3mm hole.
        assert_eq!(mount_m3().pads[0].drill, Some(Nm::from_mm(3.2)));
        assert_eq!(mount_m2().pads[0].drill, Some(Nm::from_mm(2.2)));
    }

    #[test]
    fn the_courtyard_covers_the_screw_head() {
        let m3 = mount_m3();
        assert!(
            m3.courtyard.width() > m3.pads[0].size.0,
            "a part could be placed under the washer"
        );
    }
}
