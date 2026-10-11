//! A KiCad pad's `(drill ... (offset x y))` puts its copper off its hole, and
//! the files, the model and the KiCad writer all say so.
//!
//! `cargo test -p cypcb-cli --test a_hole_off_the_middle_of_its_pad_comes_from_kicad_and_goes_back`
//!
//! The offset moves the pad's copper and not its hole: `(at)` is the hole, the
//! anchor, and the copper stands at `at` plus the offset turned by the pad's
//! angle (`PAD::ShapePos` in `pcbnew/pad.cpp`, KiCad master, read 2026-10-11).
//! The reader dropped it, so every such pad was flashed on its hole: measured
//! on the TO-92 for hand soldering in KiCad's footprint library (`(drill 0.75
//! (offset 0 0.4))` on 1.1 by 1.8mm lands), 0.40mm of copper misplaced on each
//! of three pads, 44 percent of each land. The board here is this repository's
//! own: one 2.0 by 3.0mm pad with a 0.8mm hole and the offset (0.5, 0.25),
//! standing square, stood at 30 degrees with its part, and turned 90 degrees
//! in its footprint; and one pad whose hole is in the middle, the control.
//!
//! What is held: copper minus hole, per pad, is the offset turned by the pad's
//! angle, worked out here from KiCad's own rotation and not from the model's;
//! the hole is drilled where `(at)` puts it; the pad the writer gives back is
//! the pad that was read.

use std::path::{Path, PathBuf};
use std::process::Command;

const BOARD: &str = r#"(kicad_pcb (version 20240108) (generator "pcbnew") (generator_version "8.0.0")
  (general (thickness 1.6))
  (paper "A4")
  (layers
    (0 "F.Cu" signal)
    (31 "B.Cu" signal)
    (36 "B.SilkS" user "B.Silkscreen")
    (37 "F.SilkS" user "F.Silkscreen")
    (38 "B.Mask" user)
    (39 "F.Mask" user)
    (44 "Edge.Cuts" user)
  )
  (setup (pad_to_mask_clearance 0))
  (net 0 "")
  (gr_rect (start 40 30) (end 140 50) (stroke (width 0.1) (type default)) (fill none) (layer "Edge.Cuts"))
  (footprint "T106:OFFSET" (layer "F.Cu") (at 50 40)
    (property "Reference" "J1" (at 0 0) (layer "F.SilkS"))
    (pad "1" thru_hole rect (at 0 0) (size 2 3) (drill 0.8 (offset 0.5 0.25)) (layers "*.Cu" "*.Mask"))
  )
  (footprint "T106:OFFSET" (layer "F.Cu") (at 70 40 30)
    (property "Reference" "J2" (at 0 0) (layer "F.SilkS"))
    (pad "1" thru_hole rect (at 0 0 30) (size 2 3) (drill 0.8 (offset 0.5 0.25)) (layers "*.Cu" "*.Mask"))
  )
  (footprint "T106:OFFSET" (layer "F.Cu") (at 90 40)
    (property "Reference" "J3" (at 0 0) (layer "F.SilkS"))
    (pad "1" thru_hole rect (at 0 0 90) (size 2 3) (drill 0.8 (offset 0.5 0.25)) (layers "*.Cu" "*.Mask"))
  )
  (footprint "T106:MIDDLE" (layer "F.Cu") (at 110 40)
    (property "Reference" "J4" (at 0 0) (layer "F.SilkS"))
    (pad "1" thru_hole rect (at 0 0) (size 2 3) (drill 0.8) (layers "*.Cu" "*.Mask"))
  )
  (footprint "T106:OFFSET" (layer "F.Cu") (at 130 40)
    (property "Reference" "J5" (at 0 0) (layer "F.SilkS"))
    (pad "1" thru_hole rect (at 0.5 0.25) (size 2 3) (drill 0.8) (layers "*.Cu" "*.Mask"))
  )
)
"#;

/// The pad angle each part states for its pad, degrees, and the part order.
const ANGLES: [f64; 5] = [0.0, 30.0, 90.0, 0.0, 0.0];

/// Positions in millimetres.
type Points = Vec<(f64, f64)>;

fn scratch(who: &str) -> cypcb_fixtures::ScratchPath {
    let home = cypcb_fixtures::scratch_dir(&format!("cypcb-kicad-hole-{who}"));
    let dir = home.join("out");
    std::fs::create_dir_all(&dir).expect("the directory is creatable");
    home.holding(dir)
}

fn export(board: &Path, out: &Path) {
    let status = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .env("SOURCE_DATE_EPOCH", "0")
        .arg("export")
        .arg(board)
        .arg("-o")
        .arg(out)
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export of {} failed", board.display());
}

fn read(path: PathBuf) -> String {
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Every `X..Y..` position of the lines `keep` accepts, in millimetres, in the
/// order of the file's x.
fn positions(text: &str, scale: f64, keep: impl Fn(&str) -> bool) -> Vec<(f64, f64)> {
    let mut found: Vec<(f64, f64)> = text
        .lines()
        .filter(|line| keep(line))
        .filter_map(|line| {
            let rest = line.strip_prefix('X')?;
            let (x, y) = rest.trim_end_matches("D03*").split_once('Y')?;
            Some((
                x.parse::<f64>().ok()? / scale,
                y.parse::<f64>().ok()? / scale,
            ))
        })
        .collect();
    found.sort_by(|a, b| a.partial_cmp(b).unwrap());
    found
}

/// Every hole and every flash of the top copper the files give, in millimetres,
/// in the order of the part's x.
fn holes_and_copper(dir: &Path, stem: &str) -> (Points, Points) {
    let drill = read(dir.join(format!("drill/{stem}-PTH.drl")));
    let holes = positions(&drill, 1.0, |line| line.starts_with('X'));
    let copper = read(dir.join(format!("gerber/{stem}-F_Cu.gbr")));
    let flashes = positions(&copper, 1e6, |line| line.ends_with("D03*"));
    assert_eq!(holes.len(), 5, "five holes: {holes:?}");
    assert_eq!(flashes.len(), 5, "five pads: {flashes:?}");
    (holes, flashes)
}

/// Copper minus hole of each of the five pads, as the files give them.
fn copper_from_hole(dir: &Path, stem: &str) -> Vec<(f64, f64)> {
    let (holes, flashes) = holes_and_copper(dir, stem);
    holes
        .iter()
        .zip(&flashes)
        .map(|(hole, flash)| (flash.0 - hole.0, flash.1 - hole.1))
        .collect()
}

/// What KiCad says it is: the offset (0.5, 0.25) in KiCad's frame, y down,
/// turned by the pad's angle the way `RotatePoint` turns - counter-clockwise on
/// the screen - and then into this project's frame, y up.
fn kicad_copper_from_hole(degrees: f64) -> (f64, f64) {
    let (ox, oy) = (0.5, 0.25);
    let (sin, cos) = degrees.to_radians().sin_cos();
    let (x, y) = (ox * cos + oy * sin, -ox * sin + oy * cos);
    (x, -y)
}

fn close(got: (f64, f64), want: (f64, f64)) -> bool {
    (got.0 - want.0).abs() < 3e-6 && (got.1 - want.1).abs() < 3e-6
}

#[test]
fn the_copper_stands_off_the_hole_by_the_offset_turned_by_the_pads_angle() {
    let dir = scratch("read");
    let board = dir.join("board.kicad_pcb");
    std::fs::write(&board, BOARD).expect("the board is writable");
    export(&board, &dir.join("files"));

    let vectors = copper_from_hole(&dir.join("files"), "board");
    for (i, vector) in vectors.iter().take(3).enumerate() {
        let want = kicad_copper_from_hole(ANGLES[i]);
        assert!(
            close(*vector, want),
            "pad {} at {} degrees: copper is {vector:?} from the hole, KiCad puts it {want:?}",
            i + 1,
            ANGLES[i]
        );
    }
    // The control: a hole with no offset is in the middle of its copper.
    assert!(close(vectors[3], (0.0, 0.0)), "{:?}", vectors[3]);
    // And a part that shares its footprint's name with the first, whose copper
    // stands where the first's does but whose hole is under it, is a footprint
    // of its own: the same copper and another hole, not the first one's.
    assert!(close(vectors[4], (0.0, 0.0)), "{:?}", vectors[4]);
}

#[test]
fn control_the_three_vectors_are_three_different_ones() {
    // A reader that ignored the offset, or turned it by the wrong angle, would
    // give a vector that agrees with at most one of these.
    let wants: Vec<(f64, f64)> = ANGLES
        .iter()
        .take(3)
        .map(|a| kicad_copper_from_hole(*a))
        .collect();
    for i in 0..3 {
        assert!(wants[i].0.hypot(wants[i].1) > 0.5, "{:?}", wants[i]);
        for j in (i + 1)..3 {
            assert!(!close(wants[i], wants[j]), "{:?} {:?}", wants[i], wants[j]);
        }
    }
}

#[test]
fn the_board_written_back_is_the_board_that_was_read() {
    let dir = scratch("trip");
    let board = dir.join("board.kicad_pcb");
    std::fs::write(&board, BOARD).expect("the board is writable");
    let mut parsed = cypcb_kicad::parse_kicad_pcb(&board).expect("the board reads");

    let written = cypcb_kicad::write_board(&mut parsed.world, "cypcb");
    // The hole is where `(at)` goes and the copper's offset goes in the drill:
    // the pad that was written as `(at 0 0) ... (drill 0.8 (offset 0.5 0.25))`.
    assert_eq!(
        written.matches("(drill 0.8 (offset 0.5 0.25))").count(),
        3,
        "{written}"
    );
    assert_eq!(written.matches("(drill 0.8)").count(), 2, "{written}");

    let again = dir.join("again.kicad_pcb");
    std::fs::write(&again, &written).expect("the board is writable");
    export(&board, &dir.join("first"));
    export(&again, &dir.join("second"));
    let (first, second) = (
        copper_from_hole(&dir.join("first"), "board"),
        copper_from_hole(&dir.join("second"), "again"),
    );
    for (i, (a, b)) in first.iter().zip(&second).enumerate() {
        assert!(close(*a, *b), "pad {}: {a:?} written back as {b:?}", i + 1);
    }

    // The vectors alone do not say where the pads are: a writer that put the
    // copper at `(at)` and the offset on top of it moves both by the same
    // amount. The hole and the copper of each pad against the control's, which
    // has no offset and stands where it stood, are held too.
    let (first_holes, first_copper) = holes_and_copper(&dir.join("first"), "board");
    let (second_holes, second_copper) = holes_and_copper(&dir.join("second"), "again");
    let from_the_control =
        |points: &[(f64, f64)], i: usize| (points[i].0 - points[3].0, points[i].1 - points[3].1);
    for i in 0..5 {
        for (what, a, b) in [
            ("hole", &first_holes, &second_holes),
            ("copper", &first_copper, &second_copper),
        ] {
            let (was, is) = (from_the_control(a, i), from_the_control(b, i));
            assert!(
                close(was, is),
                "pad {} {what}: {was:?} written back as {is:?}",
                i + 1
            );
        }
    }
}

#[test]
fn a_footprint_file_reads_the_same_way() {
    let text = r#"(footprint "OFFSET" (version 20240108) (generator "pcbnew")
  (layer "F.Cu")
  (pad "1" thru_hole rect (at 1 2 90) (size 2 3) (drill 0.8 (offset 0.5 0.25)) (layers "*.Cu" "*.Mask"))
)"#;
    let footprint = cypcb_kicad::import_footprint_from_str(text).expect("the footprint reads");
    let pad = &footprint.pads[0];
    // The hole is the file's `(at)`: 1mm right and 2mm up the sheet's y, which
    // is down, so -2mm in this project's frame.
    let hole = pad.hole_centre();
    assert!(
        close((hole.x.0 as f64 / 1e6, hole.y.0 as f64 / 1e6), (1.0, -2.0)),
        "{hole:?}"
    );
    // The copper is off it by the offset turned by the pad's own 90 degrees.
    let want = kicad_copper_from_hole(90.0);
    let copper = (
        (pad.position.x.0 - hole.x.0) as f64 / 1e6,
        (pad.position.y.0 - hole.y.0) as f64 / 1e6,
    );
    assert!(close(copper, want), "{copper:?} against {want:?}");
}

#[test]
fn from_kicad_says_the_language_cannot_state_it() {
    let dir = scratch("said");
    let board = dir.join("board.kicad_pcb");
    std::fs::write(&board, BOARD).expect("the board is writable");
    let written = dir.join("board.cypcb");
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("from-kicad")
        .arg(&board)
        .arg("-o")
        .arg(&written)
        .output()
        .expect("the binary runs");
    assert!(output.status.success());
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(
        said.contains(
            "pad(s) with the hole off the middle of the copper written with it in the middle"
        ),
        "{said}"
    );
    assert!(written.exists(), "the board still arrives");
}
