//! A board goes to KiCad and back without walking off its own outline.
//!
//! `cargo test -p cypcb-cli --test the_round_trip_puts_the_board_back_where_it_was`
//!
//! Everything a KiCad board carries - pads, traces, vias, pours - is read
//! relative to the board's own corner, because the model puts that corner at
//! zero and KiCad lays boards out anywhere on a sheet. The **outline** was
//! read in file coordinates, and nothing compared the two.
//!
//! Measured on the USB fixture before the fix: `from-kicad`, `to-kicad`,
//! `from-kicad` again moved every part 10mm left and 5mm up while the outline
//! stayed where the sheet had it, at 141mm, 100mm. A second trip moves it
//! again. Copper drifting off the board it belongs to is the kind of defect a
//! round trip exists to catch, and the round trip had never been run twice.
//!
//! Both ways of writing an edge are held here: KiCad writes loose `gr_line`
//! segments, which is what this project's own writer emits, and a `gr_poly`
//! ring, which is what pcbnew writes for a polygon.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn cypcb(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "`cypcb {}` failed:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn scratch(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-round-trip-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    dir
}

/// Every number written after a keyword, in millimetres.
///
/// `point 40.000000mm, 0.000000mm` and `point 40mm, 0mm` are the same board
/// written by two hands, so the comparison is on the numbers rather than on
/// the text.
fn numbers_after(source: &str, keyword: &str) -> Vec<(f64, f64)> {
    let mut found: Vec<(f64, f64)> = source
        .lines()
        .filter_map(|line| line.trim().strip_prefix(keyword))
        .filter_map(|rest| {
            let (x, y) = rest.trim().split_once(',')?;
            Some((
                x.trim().trim_end_matches("mm").parse::<f64>().ok()?,
                y.trim().trim_end_matches("mm").parse::<f64>().ok()?,
            ))
        })
        .collect();
    found.sort_by(|a, b| a.partial_cmp(b).expect("no NaN in a board"));
    found
}

#[test]
fn a_board_with_a_real_outline_comes_back_the_same_shape() {
    // `examples/cutout.cypcb` is a U: a slot cut down from the top edge, so
    // its outline is eight points and cannot be recovered from the size.
    let dir = scratch("cutout");
    let kicad = dir.join("cutout.kicad_pcb");
    let back = dir.join("cutout.cypcb");

    cypcb(&[
        "to-kicad",
        "examples/cutout.cypcb",
        "-o",
        kicad.to_str().expect("a path that is text"),
    ]);
    cypcb(&[
        "from-kicad",
        kicad.to_str().expect("a path that is text"),
        "-o",
        back.to_str().expect("a path that is text"),
    ]);

    let before = std::fs::read_to_string(repo_root().join("examples/cutout.cypcb"))
        .expect("the example is there");
    let after = std::fs::read_to_string(&back).expect("the design came back");

    assert_eq!(
        numbers_after(&before, "point "),
        numbers_after(&after, "point "),
        "the outline came back somewhere else"
    );
    assert_eq!(
        numbers_after(&before, "at "),
        numbers_after(&after, "at "),
        "the parts came back somewhere else, which is the same fault seen from \
         the other side"
    );
}

#[test]
fn a_second_trip_changes_nothing() {
    // The defect this test was written for only shows on the second trip: the
    // first import is right, and it is writing that design out and reading it
    // back that moved everything by the board's own origin.
    let dir = scratch("twice");
    let first = dir.join("a.cypcb");
    let kicad = dir.join("a.kicad_pcb");
    let second = dir.join("b.cypcb");

    cypcb(&[
        "from-kicad",
        "tests/fixtures/usb_c_named_pads.kicad_pcb",
        "-o",
        first.to_str().expect("a path that is text"),
    ]);
    cypcb(&[
        "to-kicad",
        first.to_str().expect("a path that is text"),
        "-o",
        kicad.to_str().expect("a path that is text"),
    ]);
    cypcb(&[
        "from-kicad",
        kicad.to_str().expect("a path that is text"),
        "-o",
        second.to_str().expect("a path that is text"),
    ]);

    let a = std::fs::read_to_string(&first).expect("the first design");
    let b = std::fs::read_to_string(&second).expect("the second design");
    assert_eq!(
        a, b,
        "a design that has been through KiCad twice has to be the design it \
         was after once"
    );
}

/// A board whose edge is a `gr_poly` ring rather than loose segments.
///
/// pcbnew writes this form for a polygon, and this project's own writer never
/// does - so the branch that reads it has no other test.
const POLY_EDGE: &str = r#"(kicad_pcb (version 20240108) (generator "pcbnew") (generator_version "8.0.0")

  (general
    (thickness 1.6)
  )

  (paper "A4")

  (layers
    (0 "F.Cu" signal)
    (31 "B.Cu" signal)
    (44 "Edge.Cuts" user)
  )

  (net 0 "")
  (net 1 "GND")

  (footprint "Resistor_SMD:R_0402_1005Metric"
    (layer "F.Cu")
    (at 120 80 0)
    (property "Reference" "R1")
    (property "Value" "10k")
    (pad "1" smd rect (at -0.51 0) (size 0.54 0.64) (layers "F.Cu" "F.Paste" "F.Mask") (net 1 "GND"))
    (pad "2" smd rect (at 0.51 0) (size 0.54 0.64) (layers "F.Cu" "F.Paste" "F.Mask") (net 0 ""))
  )

  (gr_poly
    (pts
      (xy 110 70) (xy 150 70) (xy 150 95) (xy 135 95) (xy 135 85) (xy 110 85)
    )
    (layer "Edge.Cuts")
    (width 0.05)
  )
)
"#;

#[test]
fn a_polygon_edge_is_read_from_the_boards_corner_too() {
    let dir = scratch("poly");
    let board = dir.join("poly.kicad_pcb");
    std::fs::write(&board, POLY_EDGE).expect("the fixture is writable");
    let out = dir.join("poly.cypcb");

    cypcb(&[
        "from-kicad",
        board.to_str().expect("a path that is text"),
        "-o",
        out.to_str().expect("a path that is text"),
    ]);
    let source = std::fs::read_to_string(&out).expect("the design came back");

    // The ring spans 110,70 to 150,95 on the sheet and has a bite out of its
    // bottom left, so it is 40 by 25 and cannot be recovered from the size.
    // Every corner of it belongs at the board's own origin.
    assert!(
        source.contains("size 40.000000mm x 25.000000mm"),
        "the board is the size of its ring:\n{source}"
    );
    assert_eq!(
        numbers_after(&source, "point "),
        vec![
            (0.0, 0.0),
            (0.0, 15.0),
            (25.0, 15.0),
            (25.0, 25.0),
            (40.0, 0.0),
            (40.0, 25.0),
        ],
        "the ring is written from the board's corner, not from the sheet's:\n{source}"
    );
    assert_eq!(
        numbers_after(&source, "at "),
        vec![(10.0, 10.0)],
        "and so is the part, which is the same fault seen from the other \
         side:\n{source}"
    );
}
