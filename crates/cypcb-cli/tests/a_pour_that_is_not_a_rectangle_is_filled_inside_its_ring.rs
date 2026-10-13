//! A pour drawn as a polygon is copper exactly inside its ring.
//!
//! `cargo test -p cypcb-cli --test a_pour_that_is_not_a_rectangle_is_filled_inside_its_ring`
//!
//! The importer keeps a KiCad pour that is not a rectangle as its ring, and the
//! fill used to leave it out: read as its bounding box it would have been copper
//! across the corner the designer cut away. The fill now cuts copper from the
//! ring itself. For an L with nothing on the board, the Gerber of its layer
//! holds exactly the L, to the nanometre, and no region reaches the notch.
//!
//! The same board with the pour as a rectangle is the control: its copper is the
//! rectangle, so the L's smaller area is the ring's and not a layer that is
//! short for another reason.

use std::path::Path;
use std::process::Command;

fn board(pour_points: &str) -> String {
    format!(
        r#"(kicad_pcb (version 20240108) (generator "pcbnew")
  (general (thickness 1.6))
  (paper "A4")
  (layers
    (0 "F.Cu" signal)
    (31 "B.Cu" signal)
    (44 "Edge.Cuts" user)
  )
  (net 0 "")
  (net 1 "GND")
  (gr_rect (start 100 100) (end 140 130) (layer "Edge.Cuts") (width 0.05))
  (zone
    (net 1)
    (net_name "GND")
    (layer "F.Cu")
    (polygon
      (pts {pour_points})
    )
  )
)
"#
    )
}

const L: &str = "(xy 102 102) (xy 138 102) (xy 138 115) (xy 120 115) (xy 120 128) (xy 102 128)";
const RECTANGLE: &str = "(xy 102 102) (xy 138 102) (xy 138 128) (xy 102 128)";

/// What the command said and the top copper Gerber it wrote.
fn export(who: &str, pour_points: &str) -> (String, String) {
    let dir = cypcb_fixtures::scratch_dir(&format!("cypcb-polygon-pour-{who}"));
    let source = dir.join("poured.kicad_pcb");
    std::fs::write(&source, board(pour_points)).expect("the board is written");
    let out = dir.join("out");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args([
            "export",
            source.to_str().expect("text"),
            "-o",
            out.to_str().expect("text"),
        ])
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "the export runs:\n{said}");

    let gerber = std::fs::read_to_string(Path::new(&out).join("gerber/poured-F_Cu.gbr"))
        .expect("the top copper layer was written");
    (said, gerber)
}

/// The area, in square millimetres, of the regions of a Gerber layer, as the
/// sum of what each region draws.
fn region_area_mm2(gerber: &str) -> f64 {
    let mut total = 0.0;
    for region in gerber.split("G36*").skip(1) {
        let body = region.split("G37*").next().expect("a region ends");
        let points: Vec<(f64, f64)> = body
            .lines()
            .filter_map(|line| {
                let line = line.strip_prefix('X')?;
                let (x, rest) = line.split_once('Y')?;
                let y = rest.trim_end_matches("D01*").trim_end_matches("D02*");
                Some((x.parse::<f64>().ok()? / 1e6, y.parse::<f64>().ok()? / 1e6))
            })
            .collect();
        let twice: f64 = (0..points.len())
            .map(|i| {
                let (a, b) = (points[i], points[(i + 1) % points.len()]);
                a.0 * b.1 - b.0 * a.1
            })
            .sum();
        total += twice.abs() / 2.0;
    }
    total
}

#[test]
fn a_pour_that_is_an_l_is_copper_exactly_inside_its_ring() {
    let (said, gerber) = export("l", L);
    assert!(gerber.contains("G36*"), "the pour makes copper:\n{gerber}");
    // The L is 36 x 26 with the corner 18 x 13 cut away: 936 - 234 = 702.
    let area = region_area_mm2(&gerber);
    assert!(
        (area - 702.0).abs() < 1e-6,
        "{area} mm2 of copper, the ring is 702"
    );
    assert!(
        !said.contains("outline"),
        "a pour that is filled has nothing to excuse:\n{said}"
    );
}

#[test]
fn no_region_reaches_the_notch() {
    let (_, gerber) = export("notch", L);
    // The notch is the file's x 120..138, y 115..128, which is x 20..38 and
    // y 2..15 on the board, Y up. Every corner of every region is out of it, or
    // on its edge.
    for line in gerber.lines().filter(|l| l.starts_with('X')) {
        let (x, rest) = line[1..].split_once('Y').expect("a coordinate");
        let y = rest.trim_end_matches("D01*").trim_end_matches("D02*");
        let (x, y) = (
            x.parse::<f64>().unwrap() / 1e6,
            y.parse::<f64>().unwrap() / 1e6,
        );
        let inside = x > 20.0 + 1e-9 && x < 38.0 - 1e-9 && y > 2.0 + 1e-9 && y < 15.0 - 1e-9;
        assert!(!inside, "a corner ({x}, {y}) inside the notch");
    }
}

#[test]
fn control_the_same_pour_as_a_rectangle_is_the_rectangle() {
    let (said, gerber) = export("rectangle", RECTANGLE);
    let area = region_area_mm2(&gerber);
    assert!(
        (area - 936.0).abs() < 1e-6,
        "{area} mm2, the rectangle is 936"
    );
    assert!(!said.contains("outline"), "and nothing to excuse:\n{said}");
}
