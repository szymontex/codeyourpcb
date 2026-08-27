//! The serpentine that makes one track as long as another.
//!
//! A differential pair only works if both halves arrive together, and a bus
//! only works if its bits do. The checker has measured that skew since it was
//! written and could do nothing about it: `diff-pair-skew` reports the
//! difference and stops there. KiCad meanders the short half until the two
//! match; item 5 of the KiCad parity audit is that this could not.
//!
//! This is the shape, not yet the feature: given two points and how much
//! copper has to be added between them, it returns the polyline that adds it.
//!
//! ```text
//!        ___     ___          A tooth leaves the axis by `amplitude`,
//!   ____|   |___|   |____     runs `pitch` along it, and comes back.
//!                            Each one adds twice the amplitude.
//! ```
//!
//! # Why square rather than round
//!
//! A rounded meander is shorter for its area and is what a fast signal wants;
//! it is also arcs, and this project's copper has no arcs - the DRC measures
//! straight segments and so does the router. A square meander is what an
//! arc-free tool can draw and check honestly. The shape can change when arcs
//! arrive; the arithmetic here will not.

use cypcb_core::{Nm, Point};

/// How a meander is shaped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeanderSpec {
    /// How far a tooth leaves the axis.
    pub amplitude: Nm,
    /// How much axis length one tooth consumes.
    pub pitch: Nm,
}

/// What a meander turned out to be.
#[derive(Debug, Clone, PartialEq)]
pub struct Meander {
    /// The path from start to end, corners included.
    pub points: Vec<Point>,
    /// How much longer this is than the straight run it replaces.
    pub added: Nm,
    /// How many teeth it took.
    pub teeth: u32,
}

/// The length of a polyline.
pub fn path_length(points: &[Point]) -> Nm {
    let mut total = 0i64;
    for pair in points.windows(2) {
        let dx = (pair[1].x.0 - pair[0].x.0) as f64;
        let dy = (pair[1].y.0 - pair[0].y.0) as f64;
        total += (dx * dx + dy * dy).sqrt().round() as i64;
    }
    Nm(total)
}

/// Meander between two points so the path is `extra` longer than the straight
/// run, or `None` when it cannot be.
///
/// Returns `None` when the run is too short to hold a tooth, when the spec has
/// no amplitude or pitch, or when nothing has to be added - a caller asking for
/// zero extra wants the straight line, and this returns the absence rather
/// than a meander of no size.
///
/// The added length is quantised: one tooth adds twice the amplitude, so the
/// result overshoots rather than undershoots. A pair matched to within a tooth
/// is what a fabricator's tolerance is stated in anyway, and a caller that
/// needs the exact figure gets it back in `added`.
pub fn meander(start: Point, end: Point, extra: Nm, spec: MeanderSpec) -> Option<Meander> {
    if extra.0 <= 0 || spec.amplitude.0 <= 0 || spec.pitch.0 <= 0 {
        return None;
    }

    let dx = (end.x.0 - start.x.0) as f64;
    let dy = (end.y.0 - start.y.0) as f64;
    let run = (dx * dx + dy * dy).sqrt();
    if run <= 0.0 {
        return None;
    }

    let per_tooth = 2 * spec.amplitude.0;
    let teeth = ((extra.0 + per_tooth - 1) / per_tooth).max(1);
    let needed = teeth * spec.pitch.0;
    if needed as f64 > run {
        return None;
    }

    // The axis, and the direction that leaves it.
    let (ux, uy) = (dx / run, dy / run);
    let (nx, ny) = (-uy, ux);

    let at = |along: f64, off: f64| Point {
        x: Nm(start.x.0 + (ux * along + nx * off).round() as i64),
        y: Nm(start.y.0 + (uy * along + ny * off).round() as i64),
    };

    // The teeth sit in the middle of the run, so a tuned track keeps its ends
    // where the pads are and does not crowd one of them.
    let lead = (run - needed as f64) / 2.0;
    let amplitude = spec.amplitude.0 as f64;
    let pitch = spec.pitch.0 as f64;

    let mut points = vec![start];
    points.push(at(lead, 0.0));
    for tooth in 0..teeth {
        let base = lead + tooth as f64 * pitch;
        // Alternating sides, so the meander stays on the axis rather than
        // walking away from it.
        let side = if tooth % 2 == 0 { 1.0 } else { -1.0 };
        points.push(at(base, side * amplitude));
        points.push(at(base + pitch, side * amplitude));
        points.push(at(base + pitch, 0.0));
    }
    points.push(end);

    let added = Nm(path_length(&points).0 - run.round() as i64);
    Some(Meander {
        points,
        added,
        teeth: teeth as u32,
    })
}
