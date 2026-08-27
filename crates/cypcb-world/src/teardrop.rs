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

use cypcb_core::{Nm, Point};

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
