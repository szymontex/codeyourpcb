//! The board is the outer shape on Edge.Cuts, and the shapes inside it are holes.
//!
//! `cargo test -p cypcb-kicad --test a_board_edge_is_the_outer_shape_and_the_shapes_inside_it_are_cutouts`
//!
//! KiCad reads Edge.Cuts as a set of closed shapes and counts, for each, the
//! shapes around it: an even count is a board edge, an odd count a hole
//! (`buildContourHierarchy` and `addOutlinesToPolygon` in
//! `convert_shape_list_to_polygon.cpp`). This importer took the first
//! `gr_poly` as the ring and the bounding box of every line, rectangle and
//! polygon as the size, so a board with a slot drawn before its edge came in
//! with the slot as its outline: 3 of the 132 boards on this machine's KiCad
//! trees, one of which was 223.1 x 141.3mm and came back from a trip as
//! 37.8 x 35.7mm, every part and zone moved by the same vector.
//!
//! The boards here are written by hand, so they are evidence about this
//! reader, not about what KiCad emits.

use cypcb_core::Point;
use cypcb_kicad::board_writer::write_board;
use cypcb_kicad::pcb_parser::{parse_kicad_pcb_str, KicadPcbParseResult};
use cypcb_world::components::{BoardCutouts, BoardOutline, BoardSize, Zone};
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

/// Four lines round a rectangle, written in the order a person draws them.
fn lines_around(x1: f64, y1: f64, x2: f64, y2: f64) -> String {
    [
        line(x1, y1, x2, y1),
        line(x2, y1, x2, y2),
        line(x2, y2, x1, y2),
        line(x1, y2, x1, y1),
    ]
    .concat()
}

fn rect(x1: f64, y1: f64, x2: f64, y2: f64) -> String {
    format!(
        "  (gr_rect (start {x1} {y1}) (end {x2} {y2}) (stroke (width 0.1) (type solid)) (fill no) (layer \"Edge.Cuts\"))\n"
    )
}

fn poly(points: &[(f64, f64)]) -> String {
    let pts: Vec<String> = points
        .iter()
        .map(|(x, y)| format!("(xy {x} {y})"))
        .collect();
    format!(
        "  (gr_poly (pts {}) (stroke (width 0.1) (type solid)) (fill no) (layer \"Edge.Cuts\"))\n",
        pts.join(" ")
    )
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

fn size_mm(world: &BoardWorld) -> (f64, f64) {
    let entity = world.board_entity().expect("a board");
    let size = world.ecs().get::<BoardSize>(entity).expect("a size");
    (size.width.to_mm(), size.height.to_mm())
}

fn mm(x: f64, y: f64) -> Point {
    Point::from_mm(x, y)
}

fn sorted(points: &[Point]) -> Vec<(i64, i64)> {
    let mut all: Vec<(i64, i64)> = points.iter().map(|p| (p.x.0, p.y.0)).collect();
    all.sort_unstable();
    all
}

// The board is 100..140 by 100..130 in the file: 40 x 30, with the origin at
// its bottom-left corner, which is (100, 130) in the file.
const OUTER: (f64, f64, f64, f64) = (100.0, 100.0, 140.0, 130.0);

#[test]
fn a_hole_drawn_before_the_edge_is_a_hole_and_not_the_edge() {
    // The hole is the first `gr_poly`, which the reader used to call the ring.
    let hole = poly(&[
        (110.0, 110.0),
        (110.0, 120.0),
        (120.0, 120.0),
        (120.0, 110.0),
    ]);
    let edge = format!(
        "{hole}{}",
        poly(&[
            (100.0, 100.0),
            (140.0, 100.0),
            (140.0, 130.0),
            (100.0, 130.0)
        ])
    );
    let parsed = read(&board(&edge, ""));
    assert_eq!(size_mm(&parsed.world), (40.0, 30.0));
    let outline = ring(&parsed.world).expect("the outer shape is the ring");
    assert_eq!(
        sorted(&outline),
        sorted(&[mm(0.0, 30.0), mm(40.0, 30.0), mm(40.0, 0.0), mm(0.0, 0.0)])
    );
    let holes = cutouts(&parsed.world);
    assert_eq!(holes.len(), 1);
    assert_eq!(
        sorted(&holes[0]),
        sorted(&[
            mm(10.0, 20.0),
            mm(20.0, 20.0),
            mm(20.0, 10.0),
            mm(10.0, 10.0)
        ])
    );
}

#[test]
fn a_hole_in_an_edge_of_lines_is_a_hole() {
    // Two loops of lines. The reader gave up on this: segments left over.
    let edge = format!(
        "{}{}",
        lines_around(OUTER.0, OUTER.1, OUTER.2, OUTER.3),
        lines_around(110.0, 110.0, 120.0, 120.0)
    );
    let parsed = read(&board(&edge, ""));
    assert_eq!(size_mm(&parsed.world), (40.0, 30.0));
    assert_eq!(ring(&parsed.world).expect("a ring").len(), 4);
    assert_eq!(cutouts(&parsed.world).len(), 1);
}

#[test]
fn a_rectangle_is_its_own_box_and_a_shape_inside_it_is_a_hole() {
    let edge = format!(
        "{}{}",
        poly(&[
            (110.0, 110.0),
            (110.0, 120.0),
            (120.0, 120.0),
            (120.0, 110.0)
        ]),
        rect(OUTER.0, OUTER.1, OUTER.2, OUTER.3)
    );
    let parsed = read(&board(&edge, ""));
    assert_eq!(size_mm(&parsed.world), (40.0, 30.0));
    assert!(ring(&parsed.world).is_none(), "a rectangle needs no ring");
    assert_eq!(cutouts(&parsed.world).len(), 1);
}

#[test]
fn a_rectangle_inside_a_rectangle_is_a_hole() {
    let edge = format!(
        "{}{}",
        rect(OUTER.0, OUTER.1, OUTER.2, OUTER.3),
        rect(110.0, 110.0, 120.0, 120.0)
    );
    let parsed = read(&board(&edge, ""));
    assert!(ring(&parsed.world).is_none());
    assert_eq!(cutouts(&parsed.world).len(), 1);
}

#[test]
fn a_circle_inside_the_edge_is_a_hole() {
    let edge = format!(
        "{}  (gr_circle (center 120 115) (end 122 115) (stroke (width 0.1) (type solid)) (fill no) (layer \"Edge.Cuts\"))\n",
        lines_around(OUTER.0, OUTER.1, OUTER.2, OUTER.3)
    );
    let parsed = read(&board(&edge, ""));
    assert_eq!(ring(&parsed.world).expect("a ring").len(), 4);
    assert_eq!(cutouts(&parsed.world).len(), 1);
    assert!(parsed.metadata.placement_notes.is_empty());
}

#[test]
fn a_hole_inside_an_edge_with_an_arc_is_a_hole_in_that_edge() {
    // A D: three lines and an arc, which bulges to 140. The hole inside it has
    // an odd count of shapes round it and stays a hole; the D is the ring.
    let edge = format!(
        "{}{}{}{}{}",
        line(100.0, 100.0, 130.0, 100.0),
        "  (gr_arc (start 130 100) (mid 140 115) (end 130 130) (stroke (width 0.1) (type solid)) (layer \"Edge.Cuts\"))\n",
        line(130.0, 130.0, 100.0, 130.0),
        line(100.0, 130.0, 100.0, 100.0),
        rect(105.0, 110.0, 115.0, 120.0)
    );
    let parsed = read(&board(&edge, ""));
    assert!(ring(&parsed.world).is_some(), "the D is the ring");
    assert_eq!(cutouts(&parsed.world).len(), 1);
    // The arc reaches 10mm past the lines.
    assert_eq!(size_mm(&parsed.world), (40.0, 30.0));
    assert!(parsed.metadata.placement_notes.is_empty());
}

#[test]
fn of_two_separate_outlines_the_larger_is_the_board() {
    let edge = format!(
        "{}{}",
        lines_around(150.0, 100.0, 160.0, 110.0),
        lines_around(OUTER.0, OUTER.1, OUTER.2, OUTER.3)
    );
    let parsed = read(&board(&edge, ""));
    assert_eq!(size_mm(&parsed.world), (40.0, 30.0));
    assert!(cutouts(&parsed.world).is_empty());
    assert!(parsed
        .metadata
        .placement_notes
        .iter()
        .any(|n| n.starts_with("1 further outer shape(s)")));
}

#[test]
fn a_polygon_that_repeats_its_first_point_loses_the_repeat() {
    let edge = format!(
        "{}{}",
        lines_around(OUTER.0, OUTER.1, OUTER.2, OUTER.3),
        poly(&[
            (110.0, 110.0),
            (110.0, 120.0),
            (120.0, 120.0),
            (120.0, 110.0),
            (110.0, 110.0)
        ])
    );
    let parsed = read(&board(&edge, ""));
    assert_eq!(cutouts(&parsed.world)[0].len(), 4);
}

#[test]
fn a_board_with_one_shape_has_no_cutouts() {
    for edge in [
        lines_around(OUTER.0, OUTER.1, OUTER.2, OUTER.3),
        rect(OUTER.0, OUTER.1, OUTER.2, OUTER.3),
        poly(&[
            (100.0, 100.0),
            (140.0, 100.0),
            (140.0, 130.0),
            (100.0, 130.0),
        ]),
    ] {
        let parsed = read(&board(&edge, ""));
        assert!(cutouts(&parsed.world).is_empty(), "{edge}");
        assert_eq!(size_mm(&parsed.world), (40.0, 30.0), "{edge}");
        assert!(parsed.metadata.placement_notes.is_empty(), "{edge}");
    }
}

/// A pour well inside the outer shape, to see whether the frame moves.
const POUR: &str = r#"  (zone (net 1) (net_name "GND") (layer "F.Cu")
    (fill yes)
    (polygon (pts (xy 125 105) (xy 135 105) (xy 135 125) (xy 125 125))))
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
fn a_board_with_a_hole_comes_back_from_kicad_where_it_went() {
    // The hole is drawn first and is the first polygon in the file.
    let edge = format!(
        "{}{}",
        poly(&[
            (110.0, 110.0),
            (110.0, 120.0),
            (120.0, 120.0),
            (120.0, 110.0)
        ]),
        lines_around(OUTER.0, OUTER.1, OUTER.2, OUTER.3)
    );
    let mut parsed = read(&board(&edge, POUR));
    let before_pour = pour_bounds(&mut parsed.world);
    let before_cutouts = cutouts(&parsed.world);
    let before_ring = ring(&parsed.world);
    let before_size = size_mm(&parsed.world);

    let text = write_board(&mut parsed.world, "cypcb");
    let mut back = read(&text);

    assert_eq!(size_mm(&back.world), before_size);
    assert_eq!(ring(&back.world), before_ring);
    assert_eq!(cutouts(&back.world), before_cutouts);
    assert_eq!(
        pour_bounds(&mut back.world),
        before_pour,
        "the frame did not move"
    );
    // Eight lines on Edge.Cuts: four round the edge, four round the hole.
    assert_eq!(text.matches("(layer \"Edge.Cuts\")").count(), 8, "{text}");
}

#[test]
fn a_board_whose_edge_is_a_rectangle_with_a_hole_keeps_both() {
    let edge = format!(
        "{}{}",
        rect(OUTER.0, OUTER.1, OUTER.2, OUTER.3),
        rect(110.0, 110.0, 120.0, 120.0)
    );
    let mut parsed = read(&board(&edge, POUR));
    let before_pour = pour_bounds(&mut parsed.world);
    let text = write_board(&mut parsed.world, "cypcb");
    let mut back = read(&text);
    assert_eq!(size_mm(&back.world), (40.0, 30.0));
    assert_eq!(cutouts(&back.world), cutouts(&parsed.world));
    assert_eq!(pour_bounds(&mut back.world), before_pour);
}

#[test]
fn a_board_with_no_hole_is_written_as_it_was() {
    // Four lines in, four lines out, and nothing added.
    let mut parsed = read(&board(
        &lines_around(OUTER.0, OUTER.1, OUTER.2, OUTER.3),
        "",
    ));
    let text = write_board(&mut parsed.world, "cypcb");
    assert_eq!(text.matches("(layer \"Edge.Cuts\")").count(), 4, "{text}");
}

#[test]
fn the_language_says_it_has_no_word_for_a_hole() {
    let edge = format!(
        "{}{}",
        lines_around(OUTER.0, OUTER.1, OUTER.2, OUTER.3),
        lines_around(110.0, 110.0, 120.0, 120.0)
    );
    let mut parsed = read(&board(&edge, ""));
    let written = cypcb_world::dsl::board_as_dsl_reporting(&mut parsed.world);
    assert!(
        written
            .not_written
            .iter()
            .any(|n| n.starts_with("1 cutout(s) not written")),
        "{:?}",
        written.not_written
    );
}

#[test]
fn every_board_the_repository_holds_keeps_its_edge_through_a_trip() {
    let root = cypcb_fixtures::tree::repo_root();
    let boards: Vec<String> = cypcb_fixtures::tree::files_git_tracks(&root)
        .into_iter()
        .filter(|f| f.ends_with(".kicad_pcb"))
        .collect();
    assert!(!boards.is_empty());
    for path in &boards {
        let text = std::fs::read_to_string(root.join(path)).expect("a tracked board reads");
        let Ok(mut parsed) = parse_kicad_pcb_str(&text) else {
            continue;
        };
        let (before_ring, before_cutouts) = (ring(&parsed.world), cutouts(&parsed.world));
        let before_size = parsed.world.board_entity().map(|_| size_mm(&parsed.world));
        let written = write_board(&mut parsed.world, "cypcb");
        let back = read(&written);
        if before_ring.is_some() {
            assert_eq!(ring(&back.world), before_ring, "{path}");
        }
        assert_eq!(cutouts(&back.world), before_cutouts, "{path}");
        if let Some(size) = before_size {
            assert_eq!(size_mm(&back.world), size, "{path}");
        }
    }
}
