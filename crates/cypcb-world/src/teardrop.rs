//! Teardrops: the copper fillet where a track meets a pad.
//!
//! A track meeting a pad at a right angle is a stress riser. Drilling and
//! flexing tear it away at exactly that line, which is why every board house
//! that quotes a flex process asks for teardrops and why KiCad has had them
//! since 7.0. This project had none - item 1 of the KiCad parity audit on
//! 2026-08-27.
//!
//! The shape here is the straight teardrop rather than the curved one: a
//! four-cornered piece of copper spanning from two points on the pad's edge to
//! the width of the track a little way along it. KiCad draws both and calls the
//! straight one a fillet; a fabricator receives the same copper either way, and
//! a straight edge is one an arc-free export can already write.
//!
//! ```text
//!        A___________C
//!       /            |     A, B on the pad edge, +-phi from the track
//!    ( pad )         |     C, D at the track's own width, past the edge
//!       \____________|
//!        B           D
//! ```

use bevy_ecs::prelude::Component;
use cypcb_core::{Nm, Point, Rect};

use crate::components::trace::Trace;
use crate::components::{place_pad, FootprintRef, Layer, NetId, Position, Rotation};
use crate::footprint::FootprintLibrary;
use crate::world::{in_build_order, BoardWorld};

/// How large a teardrop is, as ratios of the pad it grows from.
///
/// KiCad states the same two numbers as percentages of the pad size and
/// defaults to 50% and 90%; those are the defaults here, for the ordinary
/// reason that a fabricator reading two boards should not have to ask which
/// tool drew them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeardropRatios {
    /// How far the fillet runs along the track, as a fraction of pad diameter.
    pub length: f64,
    /// How wide it is where it leaves the pad, as a fraction of pad diameter.
    pub width: f64,
}

impl Default for TeardropRatios {
    fn default() -> Self {
        TeardropRatios {
            length: 0.5,
            width: 0.9,
        }
    }
}

/// The fillet where a track leaves a pad, or `None` when there is nothing to
/// fill.
///
/// `pad_radius` is the pad's inscribed radius - half its smaller dimension -
/// so a long pad is filleted as the narrow thing it is at the point the track
/// leaves it. `toward` is any point on the track away from the pad; only its
/// direction is read.
///
/// Nothing is returned when the track is already as wide as the fillet would
/// be, when the pad has no size, or when the track doubles back on the pad
/// centre and states no direction at all. A teardrop narrower than its own
/// track is not copper anybody wants: it would pinch the track rather than
/// support it.
pub fn teardrop(
    pad_centre: Point,
    pad_radius: Nm,
    toward: Point,
    track_width: Nm,
    ratios: TeardropRatios,
) -> Option<[Point; 4]> {
    if pad_radius.0 <= 0 || track_width.0 <= 0 {
        return None;
    }

    let dx = (toward.x.0 - pad_centre.x.0) as f64;
    let dy = (toward.y.0 - pad_centre.y.0) as f64;
    let distance = (dx * dx + dy * dy).sqrt();
    if distance <= f64::EPSILON {
        return None;
    }
    let (ux, uy) = (dx / distance, dy / distance);

    let radius = pad_radius.0 as f64;
    let half_track = track_width.0 as f64 / 2.0;

    // Where the fillet meets the pad, as a half-width. Clamped to the pad: a
    // ratio above 1 would ask for copper the pad does not have to offer.
    let anchor_half = (ratios.width.clamp(0.0, 1.0) * radius).min(radius);
    if anchor_half <= half_track {
        return None;
    }

    // The angle those anchors sit at, measured from the track's direction.
    let phi = (anchor_half / radius).clamp(-1.0, 1.0).asin();
    let (sin_phi, cos_phi) = phi.sin_cos();

    let rotate = |sin: f64| -> (f64, f64) {
        // Rotating the unit direction by +-phi.
        (ux * cos_phi - uy * sin, ux * sin + uy * cos_phi)
    };
    let (ax, ay) = rotate(sin_phi);
    let (bx, by) = rotate(-sin_phi);

    let reach = radius + ratios.length.max(0.0) * 2.0 * radius;
    let (nx, ny) = (-uy, ux);

    let point = |x: f64, y: f64| Point {
        x: Nm(pad_centre.x.0 + x.round() as i64),
        y: Nm(pad_centre.y.0 + y.round() as i64),
    };

    let a = point(ax * radius, ay * radius);
    let b = point(bx * radius, by * radius);
    let c = point(ux * reach + nx * half_track, uy * reach + ny * half_track);
    let d = point(ux * reach - nx * half_track, uy * reach - ny * half_track);

    Some([a, c, d, b])
}

/// The inscribed radius of a pad of this size: half its smaller side.
pub fn inscribed_radius(width: Nm, height: Nm) -> Nm {
    Nm(width.0.min(height.0) / 2)
}

/// A teardrop as the copper it is: a polygon on a layer, on the net of the
/// track it grows from.
///
/// KiCad keeps a teardrop as a zone on the track's layer and net
/// (`createTeardrop` in `pcbnew/teardrop/teardrop.cpp` makes a `ZONE` and
/// adds it to the board), so the copper is part of the board and every reader
/// of copper reads it with the rest. The entity carries a [`NetId`] beside
/// this component, the way a trace and a via do, and the rules that compare
/// nets read it from there.
///
/// Built in one place, [`BoardWorld::rebuild_teardrops`], from the ratios the
/// board states and the tracks and pads as they are at that moment.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct TeardropCopper {
    /// The corners, in order round the shape.
    pub outline: Vec<Point>,
    /// The copper layer it is on: its track's.
    pub layer: Layer,
    /// Its place in the order the builder made it: tracks in the order of the
    /// source, segments in the order of the track. The files are written in
    /// this order and not in the order of the entities, which a rebuild
    /// reverses when it hands out indices that were freed a moment before.
    pub order: u32,
    /// The part whose land it grows from and that land's pad number, for the
    /// copper the builder made. `None` is copper that came from a file - a
    /// KiCad teardrop zone - which the builder leaves alone when it rebuilds,
    /// because the polygon in the file is the polygon, and which the checker's
    /// comparison of the board against its tracks does not account for.
    pub land: Option<(bevy_ecs::entity::Entity, String)>,
}

impl TeardropCopper {
    /// The box around the outline.
    pub fn bounds(&self) -> Rect {
        let mut points = self.outline.iter();
        let first = points.next().copied().unwrap_or(Point::ORIGIN);
        let mut bounds = Rect::new(first, first);
        for p in points {
            bounds.min = Point::new(bounds.min.x.min(p.x), bounds.min.y.min(p.y));
            bounds.max = Point::new(bounds.max.x.max(p.x), bounds.max.y.max(p.y));
        }
        bounds
    }

    /// Whether the point is inside the outline or on its edge.
    ///
    /// Any simple polygon: a teardrop KiCad draws curved has sides that bow
    /// inwards, so the shape is not convex and a test that assumes it is would
    /// be wrong for exactly the copper that arrives from a KiCad board.
    pub fn contains(&self, p: Point) -> bool {
        polygon_contains(&self.outline, p)
    }

    /// Twice the area of the outline, in nm squared: the shoelace sum, taken
    /// absolute. Twice, so that it stays an integer.
    pub fn double_area(&self) -> i128 {
        let n = self.outline.len();
        let mut sum: i128 = 0;
        for i in 0..n {
            let (a, b) = (self.outline[i], self.outline[(i + 1) % n]);
            sum += a.x.0 as i128 * b.y.0 as i128 - b.x.0 as i128 * a.y.0 as i128;
        }
        sum.abs()
    }
}

/// Every teardrop the board's tracks and pads give, for these ratios.
///
/// A teardrop grows wherever the end of a track segment lands inside the
/// inscribed circle of a pad that is on the track's layer: one per segment end,
/// from the first such pad, so a net with three pads - one `Trace` holding
/// several chains - is filleted at each of its joins. A track crossing a pad on
/// its way elsewhere is not an entry and gets nothing. The shape is
/// [`teardrop`]; this decides where there is one.
///
/// The polygon takes the track's net. When the pad is on another net the track
/// is a short and the clearance rule says so; the geometry does not change.
pub fn teardrops_of(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    ratios: TeardropRatios,
) -> Vec<(TeardropCopper, NetId)> {
    // Every pad as a layer mask, a centre and the radius a fillet starts from.
    // Collected once: a board has far more track ends than pads.
    let mut pads: Vec<(u32, Point, Nm, bevy_ecs::entity::Entity, String)> = Vec::new();
    for (part, position, footprint_ref, rotation) in in_build_order::<(
        bevy_ecs::entity::Entity,
        &Position,
        &FootprintRef,
        &Rotation,
    )>(world.ecs_mut())
    {
        let Some(footprint) = library.get(&footprint_ref.0) else {
            continue;
        };
        for pad in &footprint.pads {
            let centre = place_pad(position.0, pad.position, *rotation);
            pads.push((
                pad.copper_mask(),
                centre,
                inscribed_radius(pad.size.0, pad.size.1),
                part,
                pad.number.clone(),
            ));
        }
    }
    if pads.is_empty() {
        return Vec::new();
    }

    let mut found = Vec::new();
    for trace in in_build_order::<&Trace>(world.ecs_mut()) {
        let mask = trace.layer.to_copper_mask();
        for segment in &trace.segments {
            // Both ends of every segment, as the Gerber writer always took them.
            for (end, toward) in [(segment.start, segment.end), (segment.end, segment.start)] {
                for (pad_mask, centre, radius, part, number) in &pads {
                    if pad_mask & mask == 0 {
                        continue;
                    }
                    let dx = (end.x.0 - centre.x.0) as f64;
                    let dy = (end.y.0 - centre.y.0) as f64;
                    if dx * dx + dy * dy > (radius.0 as f64) * (radius.0 as f64) {
                        continue;
                    }
                    if let Some(shape) = teardrop(*centre, *radius, toward, trace.width, ratios) {
                        found.push((
                            TeardropCopper {
                                outline: shape.to_vec(),
                                layer: trace.layer,
                                order: found.len() as u32,
                                land: Some((*part, number.clone())),
                            },
                            trace.net_id,
                        ));
                    }
                    break;
                }
            }
        }
    }
    found
}

/// Every teardrop the board holds, with its entity and net, in the order the
/// builder made them.
pub fn teardrop_copper(
    world: &mut BoardWorld,
) -> Vec<(bevy_ecs::entity::Entity, TeardropCopper, NetId)> {
    let mut all: Vec<_> =
        in_build_order::<(bevy_ecs::entity::Entity, &TeardropCopper, &NetId)>(world.ecs_mut())
            .into_iter()
            .map(|(entity, copper, net)| (entity, copper.clone(), *net))
            .collect();
    all.sort_by_key(|(_, copper, _)| copper.order);
    all
}

/// Whether `p` is inside the polygon or on its edge: even-odd, with the edge
/// itself counted in.
pub fn polygon_contains(outline: &[Point], p: Point) -> bool {
    let n = outline.len();
    if n < 3 {
        return false;
    }
    let (px, py) = (p.x.0 as i128, p.y.0 as i128);
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let (ax, ay, bx, by) = (a.x.0 as i128, a.y.0 as i128, b.x.0 as i128, b.y.0 as i128);
        // On this edge: collinear and between its ends.
        let cross = (bx - ax) * (py - ay) - (by - ay) * (px - ax);
        if cross == 0
            && px >= ax.min(bx)
            && px <= ax.max(bx)
            && py >= ay.min(by)
            && py <= ay.max(by)
        {
            return true;
        }
        // A ray towards -x from the point crosses this edge.
        if (ay > py) != (by > py) {
            // x of the crossing, compared without division: sign of the cross
            // product says which side of the edge the point is on.
            let right_of = if by > ay { cross < 0 } else { cross > 0 };
            if right_of {
                inside = !inside;
            }
        }
    }
    inside
}
