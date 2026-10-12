//! A pour drawn as a polygon is kept in the board and makes no copper.
//!
//! `cargo test -p cypcb-cli --test a_pour_that_is_not_a_rectangle_makes_no_copper`
//!
//! The fill is cut from rectangles. A pour whose outline is an L is carried in
//! the board with its corners - `to-kicad` writes them back - but nothing
//! here makes copper of a ring yet, and read as its bounding box it would be
//! copper across the corner the designer cut away. So the Gerber of the
//! layer it is on has no region of it, and the command says why, once.
//!
//! The same board with the pour as a rectangle is the control: it makes a
//! region, so the missing one is the polygon's and not a layer that is empty
//! for another reason.

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

#[test]
fn a_pour_that_is_an_l_makes_no_copper_and_the_command_says_why() {
    let (said, gerber) = export("l", L);
    assert!(
        !gerber.contains("G36*"),
        "no region of the pour, and no box of it either:\n{gerber}"
    );
    assert!(
        said.contains("the zone on net GND is a 6-point outline"),
        "the pour is named:\n{said}"
    );
    assert!(
        said.contains("not filled"),
        "and what became of it:\n{said}"
    );
    assert!(
        said.contains("bounding box would put copper where the shape was drawn to avoid it"),
        "and why a box is not the answer:\n{said}"
    );
}

#[test]
fn control_the_same_pour_as_a_rectangle_makes_a_region() {
    let (said, gerber) = export("rectangle", RECTANGLE);
    assert!(
        gerber.contains("G36*"),
        "the layer has copper to make:\n{gerber}"
    );
    assert!(!said.contains("outline"), "and nothing to excuse:\n{said}");
}
