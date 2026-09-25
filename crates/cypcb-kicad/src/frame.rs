//! Where a KiCad coordinate becomes a board coordinate, and back.
//!
//! KiCad's Y grows down the sheet. A board's grows up - the convention
//! `cypcb_world::footprint` states for the board and every footprint on it.
//! So every Y that crosses this crate's boundary is negated, and this module
//! is the only place that does it: the board reader, the board writer, the
//! routing writer and the `.kicad_mod` reader all call it.
//!
//! Until 2026-09-25 nothing negated anything. A board read from KiCad sat in
//! the model as its own mirror image, and a board written to KiCad came out
//! as the mirror of the Gerber written from the same design: a SOT-23-5 with
//! pad 1 at the bottom left of the Gerber had it at the top left in the
//! `.kicad_pcb`. The two mirrors cancelled on a round trip, which is why no
//! round-trip test saw it.
//!
//! An angle crosses unchanged. KiCad turns a footprint counter-clockwise as
//! the sheet is drawn on screen, Y down; this project turns it
//! counter-clockwise seen from the top, Y up. Both are the part turning the
//! same way under the viewer's eye, so the number is the same number.

use cypcb_core::{Nm, Point};

/// The board's corner the model counts from, in file millimetres.
///
/// It is the bottom-left corner of the outline's bounding box: the smallest
/// X, and the largest Y the file states, because the file's largest Y is the
/// bottom of the drawing. Measured from there, every point on the board has
/// a Y of zero or more, as it does in a design written in this language.
pub type Origin = (f64, f64);

/// The origin for a board whose outline spans these file coordinates.
pub fn origin(min_x: f64, max_y: f64) -> Origin {
    (min_x, max_y)
}

/// A point on the sheet, in file millimetres, as a board point.
pub fn board_point(origin: Origin, x: f64, y: f64) -> Point {
    Point::from_mm(x - origin.0, origin.1 - y)
}

/// A point on the sheet as a pair of board millimetres, for the readers that
/// keep plain numbers until they have the whole shape.
pub fn board_mm(origin: Origin, x: f64, y: f64) -> (f64, f64) {
    (x - origin.0, origin.1 - y)
}

/// A point inside a footprint, relative to the footprint's origin, as a
/// footprint point.
pub fn local_point(x: f64, y: f64) -> Point {
    Point::from_mm(x, -y)
}

/// A board point as it is written on the sheet, in nanometres, with the
/// board's corner at `origin` (also in nanometres, in file coordinates).
pub fn sheet_nm(origin: Point, point: Point) -> (Nm, Nm) {
    (Nm(origin.x.0 + point.x.0), Nm(origin.y.0 - point.y.0))
}

/// A footprint point as it is written inside a footprint.
pub fn local_nm(point: Point) -> (Nm, Nm) {
    (point.x, Nm(-point.y.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_near_the_top_of_the_sheet_is_near_the_top_of_the_board() {
        // An outline from (100, 50) to (140, 80) on the sheet: its top edge is
        // y = 50 in the file and y = 30 on the board.
        let origin = origin(100.0, 80.0);
        assert_eq!(board_point(origin, 100.0, 80.0), Point::from_mm(0.0, 0.0));
        assert_eq!(board_point(origin, 140.0, 50.0), Point::from_mm(40.0, 30.0));
    }

    #[test]
    fn a_point_crosses_back_to_where_it_came_from() {
        let origin_nm = Point::from_mm(100.0, 80.0);
        let origin = (100.0, 80.0);
        let on_board = board_point(origin, 112.5, 61.25);
        let (x, y) = sheet_nm(origin_nm, on_board);
        assert_eq!((x, y), (Nm::from_mm(112.5), Nm::from_mm(61.25)));

        let local = local_point(-1.2, -0.95);
        assert_eq!(local, Point::from_mm(-1.2, 0.95));
        assert_eq!(local_nm(local), (Nm::from_mm(-1.2), Nm::from_mm(-0.95)));
    }
}
