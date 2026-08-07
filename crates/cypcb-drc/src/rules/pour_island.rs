//! Copper in a pour that reaches nothing.
//!
//! A pour is its outline minus every obstacle, so a plane crossed by two
//! traces comes out in pieces. A piece that no pad of the pour's own net
//! bridges to is copper connected to nothing - dead weight at best, an antenna
//! at worst - and it is invisible in a Gerber preview, because it looks
//! exactly like the rest of the plane.
//!
//! The connection between a piece and a pad is the thermal spoke: the fill
//! cuts the pad's gap out of the plane and the spokes bridge it. So a piece is
//! connected if a spoke touches it, or if it touches another piece that is -
//! two rectangles sharing an edge are one sheet of copper.

use cypcb_core::pour::PourOptions;
use cypcb_core::Rect;
#[cfg(test)]
use cypcb_core::{Nm, Point};
use cypcb_world::components::Layer;
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for pour copper that connects to nothing.
pub struct PourIslandRule;

impl DrcRule for PourIslandRule {
    fn name(&self) -> &'static str {
        "pour-island"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        use cypcb_world::components::zone::ZoneKind;

        let zones: Vec<_> = world
            .zones()
            .into_iter()
            .filter(|(_, zone)| zone.kind == ZoneKind::CopperPour)
            .collect();
        if zones.is_empty() {
            return Vec::new();
        }

        let library = world.footprints().clone();
        let options = PourOptions {
            clearance: rules.min_clearance,
            ..PourOptions::default()
        };

        let mut violations = Vec::new();

        for (entity, zone) in zones {
            for layer in [Layer::TopCopper, Layer::BottomCopper] {
                if zone.layer_mask & layer.to_copper_mask() == 0 {
                    continue;
                }

                let filled =
                    cypcb_world::copper::fill_zone(world, &library, layer, &zone, &options);
                if filled.pieces.is_empty() {
                    continue;
                }

                for island in unconnected_islands(&filled.pieces, &filled.spokes) {
                    violations.push(DrcViolation::pour_island(entity, island));
                }
            }
        }

        violations
    }
}

/// Group the pieces into sheets of touching copper and return the sheets no
/// spoke reaches, each as one representative rectangle.
fn unconnected_islands(pieces: &[Rect], spokes: &[Rect]) -> Vec<Rect> {
    let mut parent: Vec<usize> = (0..pieces.len()).collect();

    fn find(parent: &mut [usize], index: usize) -> usize {
        let mut root = index;
        while parent[root] != root {
            root = parent[root];
        }
        let mut walk = index;
        while parent[walk] != root {
            let next = parent[walk];
            parent[walk] = root;
            walk = next;
        }
        root
    }

    for i in 0..pieces.len() {
        for j in (i + 1)..pieces.len() {
            if touches(&pieces[i], &pieces[j]) {
                let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                if a != b {
                    parent[a] = b;
                }
            }
        }
    }

    // A spoke joins the pad to the plane, so the sheet it lands on is
    // connected. A spoke crossing several sheets joins them to each other too.
    let mut connected = vec![false; pieces.len()];
    for spoke in spokes {
        let mut reached: Vec<usize> = Vec::new();
        for (index, piece) in pieces.iter().enumerate() {
            if touches(spoke, piece) {
                reached.push(find(&mut parent, index));
            }
        }
        for root in &reached {
            connected[*root] = true;
        }
        if let Some(first) = reached.first() {
            for root in &reached[1..] {
                let (a, b) = (find(&mut parent, *first), find(&mut parent, *root));
                if a != b {
                    parent[a] = b;
                    connected[b] = true;
                }
            }
        }
    }

    let mut reported: Vec<usize> = Vec::new();
    let mut islands = Vec::new();
    for index in 0..pieces.len() {
        let root = find(&mut parent, index);
        if connected[root] || connected[index] {
            continue;
        }
        if reported.contains(&root) {
            continue;
        }
        reported.push(root);
        islands.push(pieces[index]);
    }
    islands
}

/// Whether two rectangles share any copper, edges included.
///
/// Touching counts: the fill splits one sheet into rectangles that meet
/// exactly, and copper that meets is copper that conducts.
fn touches(a: &Rect, b: &Rect) -> bool {
    a.min.x.0 <= b.max.x.0
        && b.min.x.0 <= a.max.x.0
        && a.min.y.0 <= b.max.y.0
        && b.min.y.0 <= a.max.y.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x1: f64, y1: f64, x2: f64, y2: f64) -> Rect {
        Rect {
            min: Point::from_mm(x1, y1),
            max: Point::from_mm(x2, y2),
        }
    }

    #[test]
    fn a_piece_a_spoke_reaches_is_not_an_island() {
        let pieces = vec![rect(0.0, 0.0, 5.0, 5.0)];
        let spokes = vec![rect(4.0, 2.0, 6.0, 2.5)];
        assert!(unconnected_islands(&pieces, &spokes).is_empty());
    }

    #[test]
    fn a_piece_nothing_reaches_is_reported_once() {
        let pieces = vec![rect(0.0, 0.0, 5.0, 5.0), rect(20.0, 20.0, 25.0, 25.0)];
        let spokes = vec![rect(4.0, 2.0, 6.0, 2.5)];

        let islands = unconnected_islands(&pieces, &spokes);
        assert_eq!(islands.len(), 1, "one sheet of copper reaches nothing");
        assert_eq!(islands[0].min.x, Nm::from_mm(20.0));
    }

    #[test]
    fn copper_that_touches_connected_copper_is_connected() {
        // Two rectangles meeting exactly at x = 5mm are one sheet, and only
        // the left one has a spoke. Reporting the right one would be reporting
        // copper that conducts.
        let pieces = vec![rect(0.0, 0.0, 5.0, 5.0), rect(5.0, 0.0, 10.0, 5.0)];
        let spokes = vec![rect(-1.0, 2.0, 1.0, 2.5)];
        assert!(unconnected_islands(&pieces, &spokes).is_empty());
    }

    #[test]
    fn a_plane_with_no_spokes_at_all_is_one_island_not_several() {
        // A pour on a net with no pads under it: every piece is disconnected,
        // and a designer needs to be told once rather than per rectangle.
        let pieces = vec![rect(0.0, 0.0, 5.0, 5.0), rect(5.0, 0.0, 10.0, 5.0)];
        assert_eq!(unconnected_islands(&pieces, &[]).len(), 1);
    }
}
