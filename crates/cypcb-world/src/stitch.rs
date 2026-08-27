//! Where a stitching via may sit.
//!
//! A plane on a two-layer board is two planes, one on each side, and what ties
//! them into one is a field of vias through the pour. Every board that carries
//! a ground plane past a connector or under a fast signal has them; KiCad
//! places them by hand or by plugin and this project placed none - item 4 of
//! the KiCad parity audit.
//!
//! The rule is small and the consequences are not: a via belongs where the
//! pour is on both sides and where nothing else is. A via dropped on a track
//! is a short, and a via dropped a quarter of a millimetre from a foreign pad
//! is a board a fabricator will make and a tester will fail.

use crate::components::zone::{Zone, ZoneKind};
use crate::footprint::FootprintLibrary;
use crate::{BoardWorld, Layer};
use cypcb_core::{Nm, Point, Rect};

/// What a stitching via is, before it is placed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StitchSpec {
    /// Centre-to-centre distance between vias, along both axes.
    pub pitch: Nm,
    /// Finished hole diameter.
    pub drill: Nm,
    /// Copper diameter, hole plus the annular ring.
    pub diameter: Nm,
    /// Distance kept from copper on another net.
    pub clearance: Nm,
}

impl StitchSpec {
    /// A field at this pitch, with the ordinary via this project uses
    /// elsewhere: a 0.3mm hole in a 0.6mm pad.
    pub fn at(pitch: Nm) -> Self {
        StitchSpec {
            pitch,
            drill: Nm::from_mm(0.3),
            diameter: Nm::from_mm(0.6),
            clearance: Nm::from_mm(0.3),
        }
    }
}

/// The via centres a pour can carry at this pitch.
///
/// Walks the pour's own rectangle on a grid, and keeps a point when the via
/// would sit inside the pour with its clearance to spare and clear of every
/// piece of foreign copper on either outer layer. A via bridges the two sides,
/// so copper on *either* side decides whether it may exist - a point clear on
/// the top and over a track on the bottom is not a place for a hole.
///
/// The grid starts half a pitch inside the pour rather than on its edge, so a
/// field is symmetric and no via lands half outside the copper it is meant to
/// tie together.
pub fn stitching_vias(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    zone: &Zone,
    spec: StitchSpec,
) -> Vec<Point> {
    if zone.kind != ZoneKind::CopperPour || spec.pitch.0 <= 0 {
        return Vec::new();
    }

    // Foreign copper on both outer layers: a hole goes through the board.
    let mut blockers: Vec<Rect> = Vec::new();
    for layer in [Layer::TopCopper, Layer::BottomCopper] {
        let (foreign, _own) = copper_boxes(world, library, layer, zone);
        blockers.extend(foreign);
    }

    let ring = Nm(spec.diameter.0 / 2);
    let keep = Nm(ring.0 + spec.clearance.0);

    // The pour's own edge counts too: a via has to sit inside the copper with
    // its ring, not merely inside the outline.
    let inner = Rect::new(
        Point::new(
            Nm(zone.bounds.min.x.0 + keep.0),
            Nm(zone.bounds.min.y.0 + keep.0),
        ),
        Point::new(
            Nm(zone.bounds.max.x.0 - keep.0),
            Nm(zone.bounds.max.y.0 - keep.0),
        ),
    );
    if inner.min.x.0 > inner.max.x.0 || inner.min.y.0 > inner.max.y.0 {
        return Vec::new();
    }

    let mut placed = Vec::new();
    let half = Nm(spec.pitch.0 / 2);
    let mut y = Nm(zone.bounds.min.y.0 + half.0);
    while y.0 <= zone.bounds.max.y.0 {
        let mut x = Nm(zone.bounds.min.x.0 + half.0);
        while x.0 <= zone.bounds.max.x.0 {
            let centre = Point::new(x, y);
            if inside(inner, centre) && clear_of_all(&blockers, centre, keep) {
                placed.push(centre);
            }
            x = Nm(x.0 + spec.pitch.0);
        }
        y = Nm(y.0 + spec.pitch.0);
    }
    placed
}

/// Copper on this layer, split into foreign and the pour's own.
fn copper_boxes(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    layer: Layer,
    zone: &Zone,
) -> (Vec<Rect>, Vec<Rect>) {
    crate::copper::copper_on_layer(world, library, layer, zone.net)
}

fn inside(rect: Rect, point: Point) -> bool {
    point.x.0 >= rect.min.x.0
        && point.x.0 <= rect.max.x.0
        && point.y.0 >= rect.min.y.0
        && point.y.0 <= rect.max.y.0
}

/// Is this centre at least `keep` away from every blocker?
fn clear_of_all(blockers: &[Rect], centre: Point, keep: Nm) -> bool {
    blockers.iter().all(|blocker| {
        let dx = (blocker.min.x.0 - centre.x.0)
            .max(centre.x.0 - blocker.max.x.0)
            .max(0);
        let dy = (blocker.min.y.0 - centre.y.0)
            .max(centre.y.0 - blocker.max.y.0)
            .max(0);
        // Squared, to keep this in integers: a hole's position is not a place
        // for floating point rounding.
        let distance_sq = (dx as i128) * (dx as i128) + (dy as i128) * (dy as i128);
        distance_sq >= (keep.0 as i128) * (keep.0 as i128)
    })
}
