//! A pour drawn as a polygon is copper in the Gerber and in the IPC-2581 file,
//! exactly inside its ring.
//!
//! `cargo test -p cypcb-export --test a_pour_that_is_a_polygon_is_in_the_fabrication_files`
//!
//! Gerber coordinates in the 2.6 format are whole nanometres, so the area of
//! the regions of a layer is an integer and is asked of the file as written:
//! for a rectilinear ring with a foreign part on it, exactly the ring less the
//! part's pads grown by the fab's clearance; for a ring with slanted edges, the
//! ring less the same, to within the nanometre each cut on a slanted edge may
//! give up, and never more than the ring holds.

use cypcb_core::pour::{self, PourOptions};
use cypcb_core::pour_shape::twice_area;
use cypcb_core::{Nm, Point, Rect};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::gerber::export_copper_layer_with;
use cypcb_export::ipc2581::{export_ipc2581_with, HouseTolerances};
use cypcb_world::components::zone::{Zone, ZoneOutline};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PinConnection, Position, RefDes, Rotation, Value,
};
use cypcb_world::copper::copper_on_layer;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

fn p(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

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

fn board(ring: ZoneOutline) -> (BoardWorld, FootprintLibrary, cypcb_world::NetId) {
    let mut world = BoardWorld::new();
    world.set_board("b".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
    let gnd = world.intern_net("GND");
    let sig = world.intern_net("SIG");
    world.spawn_outlined_zone(
        Zone::copper_pour_for_net(Rect::default(), Layer::TopCopper.to_copper_mask(), gnd),
        ring,
    );
    for (i, (x, y)) in [(10.0, 10.0), (18.0, 8.0)].iter().enumerate() {
        let mut nets = NetConnections::new();
        nets.add(PinConnection::new("1", sig));
        world.spawn_component(
            RefDes::new(format!("R{}", i + 1)),
            Value::new("10k"),
            Position::from_mm(*x, *y),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            nets,
        );
    }
    (world, FootprintLibrary::new(), gnd)
}

const CLEARANCE_MM: f64 = 0.254;

fn options() -> PourOptions {
    PourOptions {
        clearance: Nm::from_mm(CLEARANCE_MM),
        ..Default::default()
    }
}

fn top_copper(world: &mut BoardWorld, library: &FootprintLibrary) -> String {
    export_copper_layer_with(
        world,
        library,
        Layer::TopCopper,
        &CoordinateFormat::FORMAT_MM_2_6,
        &options(),
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
    )
    .expect("the layer exports")
}

/// The regions of a Gerber layer as rings of whole nanometres.
fn regions(gerber: &str) -> Vec<Vec<Point>> {
    gerber
        .split("G36*")
        .skip(1)
        .map(|region| {
            let body = region.split("G37*").next().expect("a region ends");
            let mut points: Vec<Point> = body
                .lines()
                .filter_map(|line| {
                    let line = line.strip_prefix('X')?;
                    let (x, rest) = line.split_once('Y')?;
                    let y = rest.trim_end_matches("D01*").trim_end_matches("D02*");
                    Some(Point::new(Nm(x.parse().ok()?), Nm(y.parse().ok()?)))
                })
                .collect();
            // A region closes on its first point.
            if points.len() > 1 && points.first() == points.last() {
                points.pop();
            }
            points
        })
        .collect()
}

fn copper_area(gerber: &str) -> i128 {
    regions(gerber).iter().map(|r| twice_area(r).abs()).sum()
}

/// What the ring holds after the pads of the board are taken out, grown by the
/// clearance, as twice an area in nm2: the box round the board less what is
/// left of it, for the pads that are inside the ring.
fn expected(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    gnd: cypcb_world::NetId,
    ring: &ZoneOutline,
) -> i128 {
    let (obstacles, _) = copper_on_layer(world, library, Layer::TopCopper, Some(gnd));
    let holes: Vec<Rect> = obstacles
        .iter()
        .map(|o| pour::grown(*o, Nm::from_mm(CLEARANCE_MM)))
        .collect();
    let around = Rect::new(p(0.0, 0.0), p(40.0, 30.0));
    let left: i128 = pour::fill(around, &holes, Nm::ZERO)
        .iter()
        .map(|piece| pour::area(*piece))
        .sum();
    // Only meaningful where the holes are wholly inside the ring.
    for hole in &holes {
        for corner in hole.corners() {
            assert!(
                ring.contains(corner),
                "a hole corner {corner:?} outside the ring"
            );
        }
    }
    twice_area(ring.corners()) - 2 * (pour::area(around) - left)
}

#[test]
fn a_rectilinear_ring_is_exactly_the_ring_less_the_grown_pads() {
    let (mut world, library, gnd) = board(l_ring());
    let want = expected(&mut world, &library, gnd, &l_ring());
    let gerber = top_copper(&mut world, &library);
    assert_eq!(copper_area(&gerber), want, "to the nanometre");
}

#[test]
fn every_corner_in_the_file_is_in_the_ring_and_out_of_the_clearance() {
    for ring in [l_ring(), slanted_ring()] {
        let (mut world, library, gnd) = board(ring.clone());
        let (obstacles, _) = copper_on_layer(&mut world, &library, Layer::TopCopper, Some(gnd));
        let gerber = top_copper(&mut world, &library);
        let shapes = regions(&gerber);
        assert!(!shapes.is_empty());
        for shape in &shapes {
            for &corner in shape {
                assert!(ring.contains(corner), "{corner:?} written outside the ring");
                for o in &obstacles {
                    let g = pour::grown(*o, Nm::from_mm(CLEARANCE_MM));
                    let strictly_in = corner.x.0 > g.min.x.0
                        && corner.x.0 < g.max.x.0
                        && corner.y.0 > g.min.y.0
                        && corner.y.0 < g.max.y.0;
                    assert!(
                        !strictly_in,
                        "{corner:?} written inside the clearance {g:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_slanted_ring_is_the_ring_less_the_grown_pads_within_the_rounding() {
    let (mut world, library, gnd) = board(slanted_ring());
    let want = expected(&mut world, &library, gnd, &slanted_ring());
    let gerber = top_copper(&mut world, &library);
    let got = copper_area(&gerber);
    assert!(
        got <= want,
        "the rounding only ever removes copper: {got} > {want}"
    );
    // Each piece gives up less than its perimeter's width in nanometres: far
    // below a part in a million of the plane.
    assert!(
        (want - got) * 1_000_000 < want,
        "{} nm2 of {want} lost",
        want - got
    );
}

#[test]
fn the_ipc2581_file_holds_the_same_copper() {
    let (mut world, library, _) = board(l_ring());
    let (xml, _) = export_ipc2581_with(
        &mut world,
        &library,
        HouseTolerances::default(),
        cypcb_export::stamp::Stamp::UNIX_EPOCH,
        &options(),
    );
    let gerber = top_copper(&mut world, &library);
    let pieces = regions(&gerber).len();
    assert!(pieces > 1, "the pads split the L");
    // One polygon for each piece the Gerber has, on the pour's net.
    let set = xml
        .split("<Set net=\"GND\">")
        .nth(1)
        .expect("the pour is a set of its net");
    let set = set.split("</Set>").next().unwrap();
    assert_eq!(set.matches("<Polygon>").count(), pieces, "{set}");
    assert!(set.contains("<PolyStepSegment"));
}

#[test]
fn control_a_board_with_no_pour_has_no_region() {
    let mut world = BoardWorld::new();
    world.set_board("b".to_string(), (Nm::from_mm(40.0), Nm::from_mm(30.0)), 2);
    let gerber = top_copper(&mut world, &FootprintLibrary::new());
    assert!(!gerber.contains("G36*"));
}
