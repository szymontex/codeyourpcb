//! A pour drawn as a polygon is cut from its ring, with the clearance a
//! rectangle's has.
//!
//! `cargo test -p cypcb-world --test a_pour_that_is_a_polygon_is_filled_inside_its_ring`
//!
//! The fill of a rectangular pour is the rectangle minus every foreign piece of
//! copper, grown by the clearance. `fill_outlined_zone` is the same cut of a
//! ring. What is held: nothing on the board leaves the ring whole, a foreign pad
//! is cut out with the clearance, a pad of the pour's own net is cut with the
//! thermal gap and bridged by spokes that stay inside the ring, and - the
//! reason this is not a tolerance - every corner of every piece is inside the
//! ring and outside every clearance, asked of the exact corners.

use cypcb_core::pour::{self, PourOptions};
use cypcb_core::pour_shape::twice_area;
use cypcb_core::{Nm, Point, Rect};
use cypcb_world::components::zone::{Hatch, Zone, ZoneOutline};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PinConnection, Position, RefDes, Rotation, Value,
};
use cypcb_world::copper::{copper_on_layer, fill_outlined_zone, fill_zone, outlined_pours};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

fn p(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

/// An L on a 40 x 30 board: 20 x 20 with the upper right quarter cut away,
/// from (5, 5).
fn l_ring() -> ZoneOutline {
    ZoneOutline::new(vec![
        p(5.0, 5.0),
        p(25.0, 5.0),
        p(25.0, 15.0),
        p(15.0, 15.0),
        p(15.0, 25.0),
        p(5.0, 25.0),
    ])
    .expect("an L")
}

/// A hexagon with no edge on an axis, on the same board.
fn slanted_ring() -> ZoneOutline {
    ZoneOutline::new(vec![
        p(6.0, 6.0),
        p(30.0, 7.3),
        p(33.1, 20.0),
        p(21.7, 21.2),
        p(19.3, 27.9),
        p(7.1, 26.4),
    ])
    .expect("a hexagon")
}

struct Board {
    world: BoardWorld,
    library: FootprintLibrary,
    gnd: cypcb_world::NetId,
}

/// A board with `pours` GND on the top layer in `ring`, and one 0402 on net
/// `net` of each `(x, y, net)` given.
fn board(ring: ZoneOutline, parts: &[(f64, f64, &str)]) -> Board {
    let mut world = BoardWorld::new();
    world.set_board("b".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
    let gnd = world.intern_net("GND");
    world.spawn_outlined_zone(
        Zone::copper_pour_for_net(Rect::default(), Layer::TopCopper.to_copper_mask(), gnd),
        ring,
    );
    for (i, (x, y, net)) in parts.iter().enumerate() {
        let net = world.intern_net(net);
        let mut nets = NetConnections::new();
        nets.add(PinConnection::new("1", net));
        world.spawn_component(
            RefDes::new(format!("R{}", i + 1)),
            Value::new("10k"),
            Position::from_mm(*x, *y),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            nets,
        );
    }
    Board {
        world,
        library: FootprintLibrary::new(),
        gnd,
    }
}

fn fill(board: &mut Board) -> cypcb_world::copper::FilledPour {
    let rows = outlined_pours(&mut board.world, Layer::TopCopper);
    assert_eq!(rows.len(), 1, "one pour on the layer");
    let (_, zone, outline) = &rows[0];
    fill_outlined_zone(
        &mut board.world,
        &board.library,
        Layer::TopCopper,
        zone,
        outline,
        &PourOptions::default(),
    )
}

fn grown(rect: Rect, by: Nm) -> Rect {
    pour::grown(rect, by)
}

#[test]
fn with_nothing_on_the_board_the_pour_is_its_ring() {
    let mut b = board(l_ring(), &[]);
    let filled = fill(&mut b);
    let area: i128 = filled.shapes.iter().map(|s| twice_area(s)).sum();
    assert_eq!(
        area,
        twice_area(l_ring().corners()),
        "the whole L, to the nanometre"
    );
    assert!(filled.pieces.is_empty(), "a polygon pour has no rectangles");
    assert!(filled.spoke_shapes.is_empty());
}

#[test]
fn a_foreign_pad_is_cut_out_with_the_clearance() {
    let mut b = board(l_ring(), &[(10.0, 10.0, "SIG")]);
    let filled = fill(&mut b);
    let (obstacles, _) = copper_on_layer(&mut b.world, &b.library, Layer::TopCopper, Some(b.gnd));
    assert_eq!(obstacles.len(), 2, "the two pads of the 0402");
    let clearance = PourOptions::default().clearance;

    let ring_area = twice_area(l_ring().corners());
    let holes: Vec<Rect> = obstacles.iter().map(|o| grown(*o, clearance)).collect();
    // The grown pads overlap each other at this clearance, so what is taken is
    // their union, which the rectangle algebra gives: the box round them less
    // what is left of it after they are cut out.
    let around = Rect::new(p(0.0, 0.0), p(40.0, 30.0));
    let left_of_box: i128 = pour::fill(around, &holes, Nm::ZERO)
        .iter()
        .map(|piece| pour::area(*piece))
        .sum();
    let cut = 2 * (pour::area(around) - left_of_box);
    let area: i128 = filled.shapes.iter().map(|s| twice_area(s)).sum();
    assert_eq!(area, ring_area - cut);
}

#[test]
fn every_corner_is_in_the_ring_and_out_of_every_clearance() {
    let parts = [
        (9.0, 9.0, "SIG"),
        (14.3, 11.7, "SIG"),
        (22.1, 17.3, "SIG"),
        (28.0, 9.0, "SIG"),
        (12.0, 22.5, "SIG"),
    ];
    let mut checked = 0usize;
    for ring in [l_ring(), slanted_ring()] {
        let mut b = board(ring.clone(), &parts);
        let filled = fill(&mut b);
        let (obstacles, _) =
            copper_on_layer(&mut b.world, &b.library, Layer::TopCopper, Some(b.gnd));
        let clearance = PourOptions::default().clearance;
        for shape in filled.all_shapes() {
            for &corner in shape {
                assert!(ring.contains(corner), "{corner:?} outside the ring");
                for o in &obstacles {
                    let g = grown(*o, clearance);
                    let strictly_in = corner.x.0 > g.min.x.0
                        && corner.x.0 < g.max.x.0
                        && corner.y.0 > g.min.y.0
                        && corner.y.0 < g.max.y.0;
                    assert!(!strictly_in, "{corner:?} inside the clearance {g:?}");
                }
                checked += 1;
            }
        }
    }
    assert!(
        checked > 100,
        "{checked} corners checked; the walk has stopped seeing them"
    );
}

#[test]
fn a_pad_of_the_pours_own_net_is_cut_with_the_gap_and_bridged() {
    let mut b = board(l_ring(), &[(10.0, 10.0, "GND")]);
    let filled = fill(&mut b);
    assert!(
        !filled.spoke_shapes.is_empty(),
        "the pad is bridged to the plane"
    );
    let ring = l_ring();
    for shape in &filled.spoke_shapes {
        for &corner in shape {
            assert!(
                ring.contains(corner),
                "a spoke corner {corner:?} outside the ring"
            );
        }
    }
    // The pad itself is cut out with the thermal gap, and the other pad of the
    // 0402 - on no net, so foreign - with the clearance. The plane is the ring
    // less the union of those boxes, which are wholly inside the ring.
    let (foreign, own) = copper_on_layer(&mut b.world, &b.library, Layer::TopCopper, Some(b.gnd));
    assert_eq!(own.len(), 1);
    let options = PourOptions::default();
    let mut holes: Vec<Rect> = foreign
        .iter()
        .map(|o| grown(*o, options.clearance))
        .collect();
    holes.push(grown(own[0], options.thermal_gap));
    for hole in &holes {
        for corner in hole.corners() {
            assert!(
                ring.contains(corner),
                "a hole corner {corner:?} outside the ring"
            );
        }
    }
    let around = Rect::new(p(0.0, 0.0), p(40.0, 30.0));
    let left_of_box: i128 = pour::fill(around, &holes, Nm::ZERO)
        .iter()
        .map(|piece| pour::area(*piece))
        .sum();
    let cut = 2 * (pour::area(around) - left_of_box);
    let plane: i128 = filled.shapes.iter().map(|s| twice_area(s)).sum();
    assert_eq!(plane, twice_area(ring.corners()) - cut);
}

#[test]
fn a_spoke_that_would_cross_the_edge_of_the_ring_stops_at_it() {
    // A ground pad a fifth of a millimetre inside the slanted lower edge: the
    // spoke that reaches down from it would end outside the ring.
    let mut b = board(slanted_ring(), &[(15.0, 6.9, "GND")]);
    let filled = fill(&mut b);
    let ring = slanted_ring();
    assert!(!filled.spoke_shapes.is_empty());
    for shape in &filled.spoke_shapes {
        for &corner in shape {
            assert!(
                ring.contains(corner),
                "a spoke corner {corner:?} outside the ring"
            );
        }
    }
    // The control that the spoke does reach the edge: one of its corners is on
    // the line of the edge, to within the rounding.
    let reaches = filled.spoke_shapes.iter().flatten().any(|c| {
        let y_edge = 6.0 + (c.x.to_mm() - 6.0) * (7.3 - 6.0) / (30.0 - 6.0);
        (c.y.to_mm() - y_edge).abs() < 1e-3
    });
    assert!(reaches, "{:?}", filled.spoke_shapes);
}

#[test]
fn a_foreign_part_makes_no_spokes() {
    let mut b = board(l_ring(), &[(10.0, 10.0, "SIG")]);
    assert!(fill(&mut b).spoke_shapes.is_empty());
}

#[test]
fn a_pad_outside_the_ring_cuts_nothing() {
    let near = ring_area(&mut board(l_ring(), &[]));
    let far = ring_area(&mut board(l_ring(), &[(32.0, 25.0, "SIG")]));
    assert_eq!(near, far, "a part in the notch is on no copper of the pour");
}

fn ring_area(b: &mut Board) -> i128 {
    fill(b).shapes.iter().map(|s| twice_area(s)).sum()
}

#[test]
fn a_hatched_outlined_pour_is_refused_and_not_filled_solid() {
    let mut b = board(l_ring(), &[]);
    let (entity, _, _) = outlined_pours(&mut b.world, Layer::TopCopper).remove(0);
    b.world.ecs_mut().entity_mut(entity).insert(Hatch {
        width: Nm::from_mm(0.25),
        pitch: Nm::from_mm(0.75),
    });
    assert!(
        outlined_pours(&mut b.world, Layer::TopCopper).is_empty(),
        "a mesh is not cut from a ring, and a sheet is not what was asked for"
    );
}

#[test]
fn control_without_the_hatch_the_pour_is_there() {
    let mut b = board(l_ring(), &[]);
    assert_eq!(outlined_pours(&mut b.world, Layer::TopCopper).len(), 1);
    assert!(
        outlined_pours(&mut b.world, Layer::BottomCopper).is_empty(),
        "the other layer has none"
    );
}

#[test]
fn a_pour_that_is_a_rectangle_has_no_shapes() {
    let mut world = BoardWorld::new();
    world.set_board("b".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
    let gnd = world.intern_net("GND");
    let zone = Zone::copper_pour_for_net(
        Rect::new(p(5.0, 5.0), p(25.0, 25.0)),
        Layer::TopCopper.to_copper_mask(),
        gnd,
    );
    let filled = fill_zone(
        &mut world,
        &FootprintLibrary::new(),
        Layer::TopCopper,
        &zone,
        None,
        &PourOptions::default(),
    );
    assert_eq!(filled.pieces.len(), 1);
    assert!(filled.shapes.is_empty() && filled.spoke_shapes.is_empty());
}

#[test]
fn a_pour_whose_outline_crosses_itself_is_not_filled() {
    // A bow tie: the even-odd reading and the winding one are different shapes.
    let bow = ZoneOutline::new(vec![p(5.0, 5.0), p(25.0, 25.0), p(25.0, 5.0), p(5.0, 20.0)])
        .expect("a bow tie encloses an area");
    assert!(!bow.is_simple());
    let mut b = board(bow, &[]);
    assert!(outlined_pours(&mut b.world, Layer::TopCopper).is_empty());
}

#[test]
fn control_a_simple_outline_is_simple() {
    assert!(l_ring().is_simple());
    assert!(slanted_ring().is_simple());
}

#[test]
fn a_spoke_keeps_the_clearance_of_foreign_copper_too() {
    // A ground pad with a signal pad a millimetre from it: the spoke that
    // reaches toward the signal pad stops at that pad's clearance.
    let mut b = board(l_ring(), &[(10.0, 10.0, "GND"), (11.2, 10.0, "SIG")]);
    let filled = fill(&mut b);
    let (obstacles, _) = copper_on_layer(&mut b.world, &b.library, Layer::TopCopper, Some(b.gnd));
    let clearance = PourOptions::default().clearance;
    assert!(!filled.spoke_shapes.is_empty());
    for shape in &filled.spoke_shapes {
        for &c in shape {
            for o in &obstacles {
                let g = grown(*o, clearance);
                assert!(
                    !(c.x.0 > g.min.x.0
                        && c.x.0 < g.max.x.0
                        && c.y.0 > g.min.y.0
                        && c.y.0 < g.max.y.0),
                    "a spoke corner {c:?} inside the clearance {g:?}"
                );
            }
        }
    }
}
