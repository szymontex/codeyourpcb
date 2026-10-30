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
#[cfg(test)]
use cypcb_core::Nm;
use cypcb_core::{Point, Rect};
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for pour copper that connects to nothing.
pub struct PourIslandRule;

/// The pour this checker measures, as the fab will make it.
///
/// Every number here is the house's, and one of them said so for two days
/// while being the wrong one of the house's numbers. `PourOptions::default()`
/// carries a deliberately generous 0.254mm relief, which is not what JLCPCB's
/// advanced process makes: it publishes 0.2mm, so its plane reaches 0.054mm
/// further around every pad of the pour's own net. A checker filling from the
/// default measures a plane nobody is being sent, which is the same defect the
/// exporter carried until 2026-09-11 - there the numbers never left the
/// preset, here they never reached `DesignRules` at all.
///
/// The clearance was that defect twice over. It read `min_clearance`, which is
/// what a house publishes for two traces, where a pour beside foreign copper
/// is a wider figure the same house publishes separately - and every preset
/// this project ships states the two differently. The comment above claimed
/// the number was the house's, which is what made it hard to see: the sentence
/// was true of the field's origin and false of the question it answers.
fn pour_options(rules: &DesignRules) -> PourOptions {
    PourOptions {
        clearance: rules.min_copper_pour_clearance,
        edge_clearance: rules.min_edge_clearance,
        thermal_gap: rules.thermal_relief_gap,
        spoke_width: rules.thermal_relief_spoke_width,
    }
}

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
        let any_outlined = world
            .outlined_zones()
            .iter()
            .any(|(_, zone, _)| zone.is_copper_pour());
        if zones.is_empty() && !any_outlined {
            return Vec::new();
        }

        let library = world.footprints().clone();
        let options = pour_options(rules);
        // Every copper layer of the board, as the screen and the Gerber have it.
        let layers = world.copper_layers();

        let mut violations = Vec::new();

        for (entity, zone) in zones {
            for &layer in &layers {
                if zone.layer_mask & layer.to_copper_mask() == 0 {
                    continue;
                }

                // The mesh the pour asked for, if it asked for one: a hatched
                // plane is islands by construction, and filling it solid here
                // would measure a plane nobody is getting.
                let hatch = world
                    .ecs()
                    .get::<cypcb_world::components::Hatch>(entity)
                    .copied();
                let filled =
                    cypcb_world::copper::fill_zone(world, &library, layer, &zone, hatch, &options);
                //
                // A pour cut at a slant, or held to a board edge that is not
                // square to the axes, comes back as convex pieces, and the
                // pieces the cut spared stay rectangles. Rectangles and convex
                // pieces are one sheet when they touch, so a pour made of both
                // is measured as convex pieces throughout.
                if filled.shapes.is_empty() && filled.spoke_shapes.is_empty() {
                    if !filled.pieces.is_empty() {
                        for island in unconnected_islands(&filled.pieces, &filled.spokes) {
                            violations.push(DrcViolation::pour_island(entity, island));
                        }
                    }
                } else {
                    let (pieces, spokes) = as_shapes(&filled);
                    if !pieces.is_empty() {
                        for island in unconnected_island_shapes(&pieces, &spokes) {
                            violations.push(DrcViolation::pour_island(entity, island));
                        }
                    }
                }
            }
        }

        // The pours drawn as a polygon.
        for &layer in &layers {
            for (entity, zone, outline) in cypcb_world::copper::outlined_pours(world, layer) {
                let filled = cypcb_world::copper::fill_outlined_zone(
                    world, &library, layer, &zone, &outline, &options,
                );
                if filled.shapes.is_empty() {
                    continue;
                }
                for island in unconnected_island_shapes(&filled.shapes, &filled.spoke_shapes) {
                    violations.push(DrcViolation::pour_island(entity, island));
                }
            }
        }

        violations
    }
}

/// A fill as convex pieces and spokes alone: its rectangles as the rings of
/// their corners, then the convex pieces it already has.
fn as_shapes(filled: &cypcb_world::copper::FilledPour) -> (Vec<Vec<Point>>, Vec<Vec<Point>>) {
    let ring = |rect: &Rect| {
        vec![
            rect.min,
            Point::new(rect.max.x, rect.min.y),
            rect.max,
            Point::new(rect.min.x, rect.max.y),
        ]
    };
    let join = |rects: &[Rect], shapes: &[Vec<Point>]| -> Vec<Vec<Point>> {
        rects
            .iter()
            .map(ring)
            .chain(shapes.iter().cloned())
            .collect()
    };
    (
        join(&filled.pieces, &filled.shapes),
        join(&filled.spokes, &filled.spoke_shapes),
    )
}

/// Group the pieces into sheets of touching copper and return the sheets no
/// spoke reaches, each as one representative rectangle.
fn unconnected_islands(pieces: &[Rect], spokes: &[Rect]) -> Vec<Rect> {
    islands(pieces, spokes, touches, |piece| *piece)
}

/// The same for a pour drawn as a polygon, whose pieces are convex rings.
///
/// Two pieces that meet at one corner are one sheet, as two rectangles that
/// meet at one corner are: `touches` is closed, and so is
/// `cypcb_core::pour_shape::shapes_touch`.
fn unconnected_island_shapes(pieces: &[Vec<Point>], spokes: &[Vec<Point>]) -> Vec<Rect> {
    islands(
        pieces,
        spokes,
        |a, b| cypcb_core::pour_shape::shapes_touch(a, b),
        |piece| cypcb_core::pour_shape::shape_bounds(piece),
    )
}

fn islands<T>(
    pieces: &[T],
    spokes: &[T],
    touches: impl Fn(&T, &T) -> bool,
    representative: impl Fn(&T) -> Rect,
) -> Vec<Rect> {
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
    let mut found = Vec::new();
    for index in 0..pieces.len() {
        let root = find(&mut parent, index);
        if connected[root] || connected[index] {
            continue;
        }
        if reported.contains(&root) {
            continue;
        }
        reported.push(root);
        found.push(representative(&pieces[index]));
    }
    found
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

    /// Every preset states one clearance for two traces and a wider one for a
    /// pour beside foreign copper, and the fill has to take the second.
    ///
    /// The denominator is the point: if a preset ever set the two equal, this
    /// test would keep passing on that preset while measuring nothing, so the
    /// count of presets that distinguish them is asserted beside the values.
    #[test]
    fn the_pour_is_filled_at_the_distance_the_house_states_for_a_pour() {
        let presets: [(&str, DesignRules); 8] = [
            ("jlcpcb_2layer", DesignRules::jlcpcb_2layer()),
            ("jlcpcb_4layer", DesignRules::jlcpcb_4layer()),
            (
                "jlcpcb_advanced_2layer",
                DesignRules::jlcpcb_advanced_2layer(),
            ),
            (
                "jlcpcb_advanced_4layer",
                DesignRules::jlcpcb_advanced_4layer(),
            ),
            ("oshpark_2layer", DesignRules::oshpark_2layer()),
            ("oshpark_4layer", DesignRules::oshpark_4layer()),
            ("pcbway_standard", DesignRules::pcbway_standard()),
            ("prototype", DesignRules::prototype()),
        ];

        let mut distinguishing = 0usize;
        for (name, rules) in &presets {
            let filled = pour_options(rules).clearance;
            assert_eq!(
                filled, rules.min_copper_pour_clearance,
                "{name} fills its pour at {filled:?} where the house publishes \
                 {:?} for a pour",
                rules.min_copper_pour_clearance
            );
            if rules.min_copper_pour_clearance != rules.min_clearance {
                distinguishing += 1;
            }
            eprintln!(
                "{name}: trace {:?}, pour {:?}",
                rules.min_clearance, rules.min_copper_pour_clearance
            );
        }

        assert_eq!(
            distinguishing,
            presets.len(),
            "every preset here states a different figure for a pour than for two traces, which \
             is why the fill cannot take `min_clearance`. If this drops, one preset has set the \
             two equal and this test measures nothing on it."
        );
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

    #[test]
    fn the_relief_this_rule_fills_with_is_the_fabs() {
        use crate::presets::{Preset, PresetRules};

        // Two houses that publish different reliefs, so a constant cannot
        // satisfy both: JLCPCB's standard process 0.254mm, its advanced one
        // 0.2mm. The pair is the control - one number alone would pass against
        // `PourOptions::default()`, which happens to hold 0.254mm too.
        let standard = pour_options(&Preset::JlcpcbStandard2Layer.rules());
        assert_eq!(standard.thermal_gap, Nm::from_mm(0.254));
        assert_eq!(standard.spoke_width, Nm::from_mm(0.254));

        let advanced = pour_options(&Preset::JlcpcbAdvanced2Layer.rules());
        assert_eq!(advanced.thermal_gap, Nm::from_mm(0.2));
        assert_eq!(advanced.spoke_width, Nm::from_mm(0.2));

        assert_ne!(
            advanced.thermal_gap,
            PourOptions::default().thermal_gap,
            "the advanced process publishes a relief the default does not carry"
        );
        // This read 0.1mm until 2026-09-13, which is what the same house
        // publishes for two traces. A test can pin a defect as firmly as it
        // pins a behaviour, and this one did for two days.
        assert_eq!(advanced.clearance, Nm::from_mm(0.2));
        assert_ne!(
            advanced.clearance,
            Preset::JlcpcbAdvanced2Layer.rules().min_clearance,
            "the figure a pour keeps from foreign copper is not the figure two traces keep \
             from each other, and this house publishes both"
        );
    }

    fn ring(points: &[(f64, f64)]) -> Vec<Point> {
        points.iter().map(|&(x, y)| Point::from_mm(x, y)).collect()
    }

    fn rect_as_piece(r: Rect) -> Vec<Point> {
        vec![
            r.min,
            Point::new(r.max.x, r.min.y),
            r.max,
            Point::new(r.min.x, r.max.y),
        ]
    }

    /// An L, 20 x 20 with the upper right quarter cut away, with the lower left
    /// square (0..10, 0..10) taken out of it: what is left is two squares that
    /// meet at one point, the inner corner (10, 10).
    fn two_squares_meeting_at_the_inner_corner() -> Vec<Vec<Point>> {
        let l = ring(&[
            (0.0, 0.0),
            (20.0, 0.0),
            (20.0, 10.0),
            (10.0, 10.0),
            (10.0, 20.0),
            (0.0, 20.0),
        ]);
        cypcb_core::pour_shape::fill_ring(&l, &[rect(0.0, 0.0, 10.0, 10.0)], Nm::ZERO)
    }

    #[test]
    fn one_touching_point_joins_two_pieces_of_a_ring_as_it_joins_two_rectangles() {
        let pieces = two_squares_meeting_at_the_inner_corner();
        assert_eq!(pieces.len(), 2, "{pieces:?}");
        // A spoke on the lower right square only.
        let spoke = rect(15.0, 4.0, 16.0, 6.0);

        // The pieces as rectangles, which is what the checker has always been given.
        let as_rects = [rect(10.0, 0.0, 20.0, 10.0), rect(0.0, 10.0, 10.0, 20.0)];
        assert_eq!(unconnected_islands(&as_rects, &[spoke]), Vec::<Rect>::new());

        // The same two pieces as the fill cut them from the ring: the same answer.
        let spokes = [rect_as_piece(spoke)];
        assert_eq!(
            unconnected_island_shapes(&pieces, &spokes),
            Vec::<Rect>::new()
        );
    }

    #[test]
    fn a_nanometre_apart_is_an_island_for_both() {
        let pieces = two_squares_meeting_at_the_inner_corner();
        let spoke = rect(15.0, 4.0, 16.0, 6.0);
        // Move the upper piece a nanometre left, away from the corner.
        let apart: Vec<Vec<Point>> = pieces
            .iter()
            .map(|piece| {
                let upper = piece.iter().all(|c| c.y.0 >= Nm::from_mm(10.0).0);
                piece
                    .iter()
                    .map(|c| {
                        if upper {
                            Point::new(Nm(c.x.0 - 1), c.y)
                        } else {
                            *c
                        }
                    })
                    .collect()
            })
            .collect();
        let found = unconnected_island_shapes(&apart, &[rect_as_piece(spoke)]);
        assert_eq!(found.len(), 1, "the piece that no spoke reaches: {found:?}");

        let mut upper = rect(0.0, 10.0, 10.0, 20.0);
        upper.min.x = Nm(upper.min.x.0 - 1);
        upper.max.x = Nm(upper.max.x.0 - 1);
        assert_eq!(
            unconnected_islands(&[rect(10.0, 0.0, 20.0, 10.0), upper], &[spoke]).len(),
            1
        );
    }

    #[test]
    fn the_representative_of_an_island_is_the_box_of_its_piece() {
        let pieces = two_squares_meeting_at_the_inner_corner();
        let found = unconnected_island_shapes(&pieces, &[]);
        assert_eq!(
            found.len(),
            1,
            "joined at the corner, they are one sheet with no spoke"
        );
        assert!(found[0].max.x.0 >= Nm::from_mm(10.0).0);
    }
}
