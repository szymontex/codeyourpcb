//! An arc or a circle on Edge.Cuts is part of the board edge, and the board
//! file says so again when it is written.
//!
//! `cargo test -p cypcb-kicad --test an_arc_on_the_board_edge_is_sampled_and_written_back_as_an_arc`
//!
//! The edge was read from lines, rectangles and polygons only. A board with
//! rounded corners came in as the rectangle round its lines, a circle cut out
//! of it was left out, and on 18 of the 132 boards on this machine's KiCad
//! trees the checks measured against a box that is not the edge. An arc is now
//! sampled into the polygon every reader of the outline reads, the way KiCad
//! polygonises it (`SHAPE_ARC::ConvertToPolyline`), and remembered as the arc
//! it is, so a board written back carries the `gr_arc` and `gr_circle` it came
//! with and not the line pieces they were sampled into.
//!
//! The boards here are written by hand, so they are evidence about this
//! reader, not about what KiCad emits.

use cypcb_core::Point;
use cypcb_kicad::board_writer::write_board;
use cypcb_kicad::pcb_parser::{parse_kicad_pcb_str, KicadPcbParseResult};
use cypcb_world::components::{BoardCutouts, BoardOutline, BoardSize, EdgeCurves, Zone};
use cypcb_world::BoardWorld;

const HEAD: &str = r#"(kicad_pcb (version 20240108) (generator "pcbnew") (generator_version "8.0.0")
  (general (thickness 1.6))
  (paper "A4")
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
  (net 0 "")
  (net 1 "GND")
"#;

fn line(x1: f64, y1: f64, x2: f64, y2: f64) -> String {
    format!(
        "  (gr_line (start {x1} {y1}) (end {x2} {y2}) (stroke (width 0.1) (type solid)) (layer \"Edge.Cuts\"))\n"
    )
}

fn arc(start: (f64, f64), mid: (f64, f64), end: (f64, f64)) -> String {
    format!(
        "  (gr_arc (start {} {}) (mid {} {}) (end {} {}) (stroke (width 0.1) (type solid)) (layer \"Edge.Cuts\"))\n",
        start.0, start.1, mid.0, mid.1, end.0, end.1
    )
}

fn circle(centre: (f64, f64), radius: f64) -> String {
    format!(
        "  (gr_circle (center {} {}) (end {} {}) (stroke (width 0.1) (type solid)) (fill none) (layer \"Edge.Cuts\"))\n",
        centre.0,
        centre.1,
        centre.0 + radius,
        centre.1
    )
}

/// A D: three straight sides and an arc that bulges 10mm out of the fourth.
/// The arc is a circle of radius 16.25mm about (123.75, 115): the board is 100
/// to 140 across and 100 to 130 down.
fn d_shape() -> String {
    [
        line(100.0, 100.0, 130.0, 100.0),
        arc((130.0, 100.0), (140.0, 115.0), (130.0, 130.0)),
        line(130.0, 130.0, 100.0, 130.0),
        line(100.0, 130.0, 100.0, 100.0),
    ]
    .concat()
}

/// The D again, with its arc drawn first and from the bottom to the top, so
/// the walk round the board goes along it the way it was drawn.
fn d_shape_backwards() -> String {
    [
        arc((130.0, 130.0), (140.0, 115.0), (130.0, 100.0)),
        line(130.0, 100.0, 100.0, 100.0),
        line(100.0, 100.0, 100.0, 130.0),
        line(100.0, 130.0, 130.0, 130.0),
    ]
    .concat()
}

fn board(edge: &str, rest: &str) -> String {
    format!("{HEAD}{edge}{rest})\n")
}

fn read(text: &str) -> KicadPcbParseResult {
    parse_kicad_pcb_str(text).expect("the board reads")
}

fn ring(world: &BoardWorld) -> Option<Vec<Point>> {
    let entity = world.board_entity()?;
    world
        .ecs()
        .get::<BoardOutline>(entity)
        .map(|o| o.points.clone())
}

fn cutouts(world: &BoardWorld) -> Vec<Vec<Point>> {
    let Some(entity) = world.board_entity() else {
        return Vec::new();
    };
    world
        .ecs()
        .get::<BoardCutouts>(entity)
        .map(|c| c.rings.clone())
        .unwrap_or_default()
}

fn curves(world: &BoardWorld) -> Option<EdgeCurves> {
    let entity = world.board_entity()?;
    world.ecs().get::<EdgeCurves>(entity).cloned()
}

fn size_mm(world: &BoardWorld) -> (f64, f64) {
    let entity = world.board_entity().expect("a board");
    let size = world.ecs().get::<BoardSize>(entity).expect("a size");
    (size.width.to_mm(), size.height.to_mm())
}

/// How many Edge.Cuts items of one kind a written board holds.
fn items(text: &str, kind: &str) -> usize {
    text.lines()
        .filter(|l| l.trim_start().starts_with(&format!("({kind} ")) && l.contains("Edge.Cuts"))
        .count()
}

const POUR: &str = r#"  (zone (net 1) (net_name "GND") (layer "F.Cu")
    (fill yes)
    (polygon (pts (xy 105 105) (xy 115 105) (xy 115 125) (xy 105 125))))
"#;

fn pour_bounds(world: &mut BoardWorld) -> Vec<(i64, i64, i64, i64)> {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<&Zone>();
    query
        .iter(ecs)
        .map(|z| {
            (
                z.bounds.min.x.0,
                z.bounds.min.y.0,
                z.bounds.max.x.0,
                z.bounds.max.y.0,
            )
        })
        .collect()
}

#[test]
fn an_edge_with_an_arc_is_a_ring_and_the_board_is_as_big_as_the_arc() {
    let parsed = read(&board(&d_shape(), ""));
    // 100 to 140 across: the arc reaches 10mm past the lines, which end at 130.
    assert_eq!(size_mm(&parsed.world), (40.0, 30.0));
    let outline = ring(&parsed.world).expect("an edge with an arc is a ring");
    assert!(outline.len() > 4, "the arc is sampled: {}", outline.len());
    let set = curves(&parsed.world).expect("the arc is remembered");
    assert_eq!(set.ring.arcs.len(), 1);
    assert_eq!(set.ring.arcs[0].mid, Point::from_mm(40.0, 15.0));
}

#[test]
fn an_arc_written_the_other_way_round_bulges_the_same_way() {
    let forwards = read(&board(&d_shape(), ""));
    let backwards = read(&board(&d_shape_backwards(), ""));
    assert_eq!(size_mm(&backwards.world), (40.0, 30.0));
    let (a, b) = (
        ring(&forwards.world).expect("a ring"),
        ring(&backwards.world).expect("a ring"),
    );
    // The same points, whichever end the walk set out from.
    let near = |p: &Point| {
        b.iter()
            .any(|q| (p.x.0 - q.x.0).abs() <= 5 && (p.y.0 - q.y.0).abs() <= 5)
    };
    assert!(a.iter().all(near));
}

#[test]
fn the_arc_is_sampled_the_way_kicad_polygonises_it() {
    let parsed = read(&board(&d_shape(), ""));
    let outline = ring(&parsed.world).expect("a ring");
    let set = curves(&parsed.world).expect("curves");
    let run = set.ring.arcs[0];
    // The arc is a circle of radius 16.25mm about (23.75, 15) on the board.
    // 134.76 degrees at 2.84 degrees a step is 47 steps: the two ends and one
    // point in the middle of each step.
    assert_eq!(run.last - run.first, 47 + 1);
    // The ends are the file's own.
    assert_eq!(outline[run.first], Point::from_mm(30.0, 30.0));
    assert_eq!(outline[run.last % outline.len()], Point::from_mm(30.0, 0.0));
    // Between them the points lie on the circle of radius r + e/2, where e is
    // the error 47 steps leave: 0.00513mm here, a little over the 0.005 asked
    // for because the step count is rounded to a whole number. Outside the
    // arc and never inside it.
    for sample in &outline[run.first + 1..run.last] {
        let (dx, dy) = (sample.x.to_mm() - 23.75, sample.y.to_mm() - 15.0);
        let off = dx.hypot(dy) - 16.25;
        assert!((0.00256..=0.00257).contains(&off), "{off}");
    }
}

#[test]
fn a_circle_inside_the_edge_is_a_cutout_and_is_remembered_as_one() {
    let edge = format!(
        "{}{}",
        line_loop(100.0, 100.0, 140.0, 130.0),
        circle((120.0, 115.0), 4.0)
    );
    let parsed = read(&board(&edge, ""));
    let holes = cutouts(&parsed.world);
    assert_eq!(holes.len(), 1);
    assert!(holes[0].len() > 8);
    let set = curves(&parsed.world).expect("the circle is remembered");
    assert_eq!(set.cutouts.len(), 1);
    assert_eq!(
        set.cutouts[0].circle,
        Some((Point::from_mm(20.0, 15.0), cypcb_core::Nm::from_mm(4.0)))
    );
}

fn line_loop(x1: f64, y1: f64, x2: f64, y2: f64) -> String {
    [
        line(x1, y1, x2, y1),
        line(x2, y1, x2, y2),
        line(x2, y2, x1, y2),
        line(x1, y2, x1, y1),
    ]
    .concat()
}

#[test]
fn a_round_board_is_a_circle() {
    let parsed = read(&board(&circle((120.0, 115.0), 10.0), ""));
    assert_eq!(size_mm(&parsed.world), (20.0, 20.0));
    assert!(ring(&parsed.world).expect("a ring").len() > 8);
    assert_eq!(
        curves(&parsed.world).expect("curves").ring.circle,
        Some((Point::from_mm(10.0, 10.0), cypcb_core::Nm::from_mm(10.0)))
    );
}

#[test]
fn the_arc_and_the_circle_are_written_as_what_they_are() {
    let edge = format!("{}{}", d_shape(), circle((115.0, 115.0), 3.0));
    let mut parsed = read(&board(&edge, ""));
    let text = write_board(&mut parsed.world, "cypcb");
    assert_eq!(items(&text, "gr_arc"), 1, "{text}");
    assert_eq!(items(&text, "gr_circle"), 1, "{text}");
    // Three straight sides, as in the file, and nothing sampled.
    assert_eq!(items(&text, "gr_line"), 3, "{text}");
    // The arc's middle point is the one the file gave: 10mm in from the corner.
    let arc_line = text
        .lines()
        .find(|l| l.contains("(gr_arc"))
        .expect("an arc");
    let mid = arc_line.split("(mid ").nth(1).expect("a mid");
    let mid: Vec<f64> = mid
        .split(')')
        .next()
        .unwrap()
        .split_whitespace()
        .map(|n| n.parse().unwrap())
        .collect();
    let start = arc_line
        .split("(start ")
        .nth(1)
        .unwrap()
        .split(')')
        .next()
        .unwrap();
    let start: Vec<f64> = start
        .split_whitespace()
        .map(|n| n.parse().unwrap())
        .collect();
    assert!((mid[0] - start[0] - 10.0).abs() < 1e-9, "{arc_line}");
}

/// Rounded corners tangent to the sides, 3mm in radius, with the points as a
/// hand edit leaves them: the middle of each arc a few nanometres outside the
/// circle and given to more decimals than a nanometre, and a gap of a couple
/// of nanometres where an arc meets a line. The circle through such points
/// reaches a few nanometres past the sides it is tangent to.
fn rounded_corners() -> String {
    [
        line(103.0, 100.0, 137.0, 100.0),
        arc(
            (136.9999984, 100.0000003),
            (139.1213234, 100.8786766),
            (140.0, 103.0000019),
        ),
        line(140.0, 103.0, 140.0, 127.0),
        arc(
            (140.0, 126.9999981),
            (139.1213234, 129.1213234),
            (137.0000002, 130.0),
        ),
        line(137.0, 130.0, 103.0, 130.0),
        arc(
            (103.0000017, 130.0),
            (100.8786766, 129.1213234),
            (100.0, 127.0000004),
        ),
        line(100.0, 127.0, 100.0, 103.0),
        arc(
            (100.0, 103.0000019),
            (100.8786766, 100.8786766),
            (103.0, 100.0),
        ),
    ]
    .concat()
}

#[test]
fn a_board_with_an_arc_comes_back_from_kicad_where_it_went() {
    for edge in [
        format!("{}{}", d_shape(), circle((115.0, 115.0), 3.0)),
        circle((120.0, 115.0), 10.0),
        rounded_corners(),
        [
            arc((100.0, 100.0), (105.5, 105.5), (100.0, 111.0)),
            line(100.0, 111.0, 115.0, 111.0),
            line(115.0, 111.0, 115.0, 100.0),
            line(115.0, 100.0, 100.0, 100.0),
        ]
        .concat(),
    ] {
        let mut parsed = read(&board(&edge, POUR));
        let (before_size, before_ring) = (size_mm(&parsed.world), ring(&parsed.world));
        let (before_cutouts, before_curves) = (cutouts(&parsed.world), curves(&parsed.world));
        let before_pour = pour_bounds(&mut parsed.world);

        let text = write_board(&mut parsed.world, "cypcb");
        let mut back = read(&text);

        assert_eq!(size_mm(&back.world), before_size);
        assert_eq!(ring(&back.world), before_ring);
        assert_eq!(cutouts(&back.world), before_cutouts);
        assert_eq!(curves(&back.world), before_curves);
        assert_eq!(
            pour_bounds(&mut back.world),
            before_pour,
            "the frame did not move"
        );
    }
}

#[test]
fn an_older_arc_states_a_centre_and_an_angle_and_is_the_same_arc() {
    // The D's arc as the older files wrote it: the centre, the point it starts
    // at, and 134.76 degrees; its middle point is where the new form puts it.
    let old = [
        line(100.0, 100.0, 130.0, 100.0),
        "  (gr_arc (start 123.75 115) (end 130 100) (angle 134.7609) (layer \"Edge.Cuts\") (width 0.1))\n".to_string(),
        line(130.0, 130.0, 100.0, 130.0),
        line(100.0, 130.0, 100.0, 100.0),
    ]
    .concat();
    let parsed = read(&board(&old, ""));
    assert!(ring(&parsed.world).is_some(), "the old arc is read");
    assert_eq!(curves(&parsed.world).expect("curves").ring.arcs.len(), 1);
    let (width, height) = size_mm(&parsed.world);
    assert!(
        (width - 40.0).abs() < 0.001 && (height - 30.0).abs() < 0.001,
        "{width} x {height}"
    );
}

#[test]
fn an_arc_a_few_nanometres_past_its_line_does_not_make_the_board_bigger() {
    // The board is 40 x 30, not 40.000004 x 30.000004, and where an arc meets
    // a line the polygon has the line's point, not the arc's.
    let parsed = read(&board(&rounded_corners(), ""));
    assert_eq!(size_mm(&parsed.world), (40.0, 30.0));
    assert_eq!(curves(&parsed.world).expect("curves").ring.arcs.len(), 4);
    let outline = ring(&parsed.world).expect("a ring");
    for joint in [
        (40.0, 27.0),
        (40.0, 3.0),
        (37.0, 0.0),
        (3.0, 0.0),
        (0.0, 3.0),
        (0.0, 27.0),
    ] {
        assert!(
            outline.contains(&Point::from_mm(joint.0, joint.1)),
            "{joint:?}"
        );
    }
}

#[test]
fn a_half_circle_is_the_same_polygon_wherever_it_lies() {
    // Exactly 180 degrees comes out of three rounded points as a hair more or
    // less, which must not be a different polygon.
    let mut shapes = Vec::new();
    for step in 0..40 {
        let (dx, dy) = (f64::from(step) * 7.3171, f64::from(step) * 3.1913);
        let edge = [
            arc(
                (100.0 + dx, 100.0 + dy),
                (105.5 + dx, 105.5 + dy),
                (100.0 + dx, 111.0 + dy),
            ),
            line(100.0 + dx, 111.0 + dy, 115.0 + dx, 111.0 + dy),
            line(115.0 + dx, 111.0 + dy, 115.0 + dx, 100.0 + dy),
            line(115.0 + dx, 100.0 + dy, 100.0 + dx, 100.0 + dy),
        ]
        .concat();
        let parsed = read(&board(&edge, ""));
        shapes.push(ring(&parsed.world).expect("a ring"));
    }
    // The same points to within the rounding of a nanometre, taken from the
    // first of each.
    let relative = |shape: &Vec<Point>| -> Vec<(i64, i64)> {
        shape
            .iter()
            .map(|p| (p.x.0 - shape[0].x.0, p.y.0 - shape[0].y.0))
            .collect()
    };
    let first = relative(&shapes[0]);
    for shape in &shapes[1..] {
        let this = relative(shape);
        assert_eq!(this.len(), first.len());
        for (a, b) in this.iter().zip(&first) {
            assert!(
                (a.0 - b.0).abs() <= 5 && (a.1 - b.1).abs() <= 5,
                "{a:?} {b:?}"
            );
        }
    }
}

#[test]
fn a_bezier_curve_is_counted_and_is_neither_edge_nor_hole() {
    let edge = [
        line(100.0, 100.0, 130.0, 100.0),
        "  (gr_curve (pts (xy 130 100) (xy 140 105) (xy 140 125) (xy 130 130)) (stroke (width 0.1) (type solid)) (layer \"Edge.Cuts\"))\n".to_string(),
        line(130.0, 130.0, 100.0, 130.0),
        line(100.0, 130.0, 100.0, 100.0),
    ]
    .concat();
    let parsed = read(&board(&edge, ""));
    assert!(ring(&parsed.world).is_none());
    assert!(parsed
        .metadata
        .placement_notes
        .iter()
        .any(|n| n.starts_with("1 closed shape(s) on Edge.Cuts with a bezier curve")));
}

#[test]
fn a_board_with_no_arc_and_no_circle_remembers_no_curves() {
    let parsed = read(&board(&line_loop(100.0, 100.0, 140.0, 130.0), ""));
    assert!(curves(&parsed.world).is_none());
}

#[test]
fn the_edge_keeps_the_order_the_file_draws_it_in() {
    // A circle drawn first, a loop of lines second: the circle stays first.
    let edge = format!(
        "{}{}{}{}",
        line_loop(100.0, 100.0, 140.0, 130.0),
        circle((110.0, 110.0), 2.0),
        line_loop(120.0, 110.0, 130.0, 120.0),
        circle((110.0, 120.0), 2.0)
    );
    let parsed = read(&board(&edge, ""));
    let set = curves(&parsed.world).expect("curves");
    let kinds: Vec<bool> = set.cutouts.iter().map(|c| c.circle.is_some()).collect();
    assert_eq!(kinds, [true, false, true]);
}
