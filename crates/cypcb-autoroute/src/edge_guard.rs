//! The edge of the board, as a check a piece of copper is held to.
//!
//! The grid keeps the search off the edge, and the two stages that move
//! copper afterwards - the smoother, which cuts corners, and the via
//! optimizer, which joins and moves vias - work on geometry and have never
//! seen the grid. Both ask this before they put a piece down.

use cypcb_core::{Nm, Point};
use cypcb_drc::rules::clearance::segment_distance;
use cypcb_world::components::BoardEdge;
use cypcb_world::BoardWorld;

/// The edge of the board and the clearance copper keeps from it.
#[derive(Debug, Clone)]
pub struct EdgeGuard {
    edge: BoardEdge,
    clearance: Nm,
}

impl EdgeGuard {
    /// The edge of this board - the ring it states, or the rectangle of its
    /// size - and the clearance to keep from it. `None` for a board with no
    /// size, which has no edge to keep from.
    pub fn of_board(world: &BoardWorld, clearance: Nm) -> Option<Self> {
        BoardEdge::of_or_size(world).map(|edge| EdgeGuard { edge, clearance })
    }

    /// Whether a segment of `width` from `p1` to `p2` stays on the board and
    /// keeps the clearance: both ends on the board, and no edge nearer the
    /// centre line than the clearance and half the width. A via is the same
    /// question with its own width and the one point twice.
    pub fn allows(&self, p1: [i64; 2], p2: [i64; 2], width: Nm) -> bool {
        let on_board = |p: [i64; 2]| self.edge.contains(Point::new(Nm(p[0]), Nm(p[1])));
        let reach = self.clearance.0 + width.0 / 2;
        on_board(p1)
            && on_board(p2)
            && self
                .edge
                .edges()
                .all(|(a, b)| segment_distance(p1, p2, [a.x.0, a.y.0], [b.x.0, b.y.0]) >= reach)
    }
}
