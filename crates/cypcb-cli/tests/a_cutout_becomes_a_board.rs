//! `from-dxf` reads a mechanical drawing's cutout as a board outline.
//!
//! `cargo test -p cypcb-cli --test a_cutout_becomes_a_board`
//!
//! An enclosure is drawn in a mechanical tool and the board has to fit inside
//! it. The way that fact reached a design was a person reading coordinates off
//! a drawing and typing them in, which is how a board ends up a fraction out
//! from the case it was made for. Row 8 of the KiCad parity audit.
//!
//! The fixtures here are written by hand rather than exported, because the
//! point is to read what *other* tools write: R12 polylines with their vertices
//! as separate entities, R14 lightweight polylines, loose lines in no
//! particular order, and a drawing whose numbers are inches.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples")
        .join(name)
}

fn scratch(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-fromdxf-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the scratch directory is made");
    dir
}

/// A drawing with the given entities in it, in millimetres unless told
/// otherwise.
fn drawing(dir: &Path, name: &str, insunits: u8, entities: &str) -> PathBuf {
    let text = format!(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n9\n$INSUNITS\n70\n{insunits}\n0\nENDSEC\n\
         0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n"
    );
    let path = dir.join(name);
    std::fs::write(&path, text).expect("the drawing is written");
    path
}

/// One R12 line, on a layer, between two points.
fn line(layer: &str, from: (f64, f64), to: (f64, f64)) -> String {
    format!(
        "0\nLINE\n8\n{layer}\n10\n{}\n20\n{}\n30\n0.0\n11\n{}\n21\n{}\n31\n0.0\n",
        from.0, from.1, to.0, to.1
    )
}

/// One R14 closed lightweight polyline.
fn lwpolyline(layer: &str, points: &[(f64, f64)]) -> String {
    let mut out = format!("0\nLWPOLYLINE\n8\n{layer}\n90\n{}\n70\n1\n", points.len());
    for (x, y) in points {
        out.push_str(&format!("10\n{x}\n20\n{y}\n"));
    }
    out
}

fn read(file: &Path, out: &Path, extra: &[&str]) -> Output {
    let mut command = cypcb();
    command.arg("from-dxf").arg(file).arg("-o").arg(out);
    for argument in extra {
        command.arg(argument);
    }
    command.output().expect("the binary runs")
}

fn design(file: &Path, out: &Path, extra: &[&str]) -> String {
    let result = read(file, out, extra);
    assert!(
        result.status.success(),
        "the read failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::read_to_string(out).expect("the design is readable")
}

#[test]
fn what_the_plotter_writes_is_what_the_reader_reads() {
    // The two halves of row 7 and row 8 meet here: `--dxf` writes the board
    // edge as an R12 polyline, and this reads that exact file back. A board
    // that survives the trip is a board whose outline was not retyped.
    let out = scratch("roundtrip");
    let status = cypcb()
        .arg("export")
        .arg(example("usb-diff-pair.cypcb"))
        .arg("-o")
        .arg(&out)
        .arg("--dxf")
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export failed");

    let back = out.join("back.cypcb");
    let text = design(&out.join("plot").join("usb-diff-pair-F_Cu.dxf"), &back, &[]);

    assert!(
        text.contains("size 30mm x 20mm"),
        "the board is the size the drawing says:\n{text}"
    );
    for corner in [
        "point 0mm, 0mm",
        "point 30mm, 0mm",
        "point 30mm, 20mm",
        "point 0mm, 20mm",
    ] {
        assert!(text.contains(corner), "{corner} is in the outline:\n{text}");
    }

    // And what came out is a design, not a pile of numbers.
    let checked = cypcb()
        .arg("check")
        .arg(&back)
        .output()
        .expect("the binary runs");
    assert!(
        checked.status.success(),
        "the design it wrote does not check: {}",
        String::from_utf8_lossy(&checked.stderr)
    );
}

#[test]
fn a_lightweight_polyline_is_read_too() {
    // Every tool newer than R14 writes cutouts with LWPOLYLINE, where the
    // vertices are pairs inside the one entity rather than entities of their
    // own. A reader that only knew R12 would read those drawings as empty.
    let dir = scratch("lwpolyline");
    let file = drawing(
        &dir,
        "case.dxf",
        4,
        &lwpolyline(
            "CUTOUT",
            &[(0.0, 0.0), (50.0, 0.0), (50.0, 30.0), (0.0, 30.0)],
        ),
    );
    let text = design(&file, &dir.join("case.cypcb"), &[]);
    assert!(
        text.contains("size 50mm x 30mm") && text.contains("point 50mm, 30mm"),
        "the shape came out whole:\n{text}"
    );
}

#[test]
fn loose_lines_are_followed_into_a_loop() {
    // A cutout drawn as four separate lines is the ordinary case, and the
    // lines arrive in whatever order the tool drew them - here, deliberately
    // shuffled and with two of them running backwards.
    let dir = scratch("lines");
    let entities = [
        line("EDGE", (40.0, 0.0), (40.0, 25.0)),
        line("EDGE", (0.0, 0.0), (40.0, 0.0)),
        line("EDGE", (0.0, 25.0), (0.0, 0.0)),
        line("EDGE", (0.0, 25.0), (40.0, 25.0)),
    ]
    .concat();
    let file = drawing(&dir, "plate.dxf", 4, &entities);
    let text = design(&file, &dir.join("plate.cypcb"), &[]);
    assert!(
        text.contains("size 40mm x 25mm"),
        "the four lines make one shape:\n{text}"
    );
    assert_eq!(
        text.matches("    point ").count(),
        4,
        "and it has four corners, not five:\n{text}"
    );
}

#[test]
fn a_drawing_in_inches_is_read_in_inches() {
    // A DXF number carries no unit of its own. A drawing read as millimetres
    // when it meant inches is a board 25.4 times too small, and it would check
    // clean all the way to the fabricator.
    let dir = scratch("inches");
    let file = drawing(
        &dir,
        "imperial.dxf",
        1,
        &lwpolyline("0", &[(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (0.0, 1.0)]),
    );
    let result = read(&file, &dir.join("imperial.cypcb"), &[]);
    let said = String::from_utf8_lossy(&result.stderr).to_string();
    let text = std::fs::read_to_string(dir.join("imperial.cypcb")).expect("the design is readable");

    assert!(
        said.contains("in inches"),
        "the run says what it read: {said}"
    );
    assert!(
        text.contains("size 50.8mm x 25.4mm"),
        "two inches by one is 50.8mm by 25.4mm:\n{text}"
    );
}

#[test]
fn the_shape_is_moved_to_the_boards_own_corner() {
    // A cutout drawn 400mm along a fixture is 400mm along it in the file. A
    // board is measured from its own corner, so the shape moves - and a person
    // who wants the fixture's numbers can keep them.
    let dir = scratch("origin");
    let file = drawing(
        &dir,
        "fixture.dxf",
        4,
        &lwpolyline(
            "0",
            &[
                (400.0, 100.0),
                (430.0, 100.0),
                (430.0, 120.0),
                (400.0, 120.0),
            ],
        ),
    );

    let moved = design(&file, &dir.join("moved.cypcb"), &[]);
    assert!(
        moved.contains("point 0mm, 0mm") && moved.contains("size 30mm x 20mm"),
        "the board starts at its own corner:\n{moved}"
    );

    let kept = design(&file, &dir.join("kept.cypcb"), &["--keep-origin"]);
    assert!(
        kept.contains("point 400mm, 100mm"),
        "and --keep-origin keeps the drawing's own numbers:\n{kept}"
    );
}

#[test]
fn the_cutout_wins_and_a_named_layer_wins_over_it() {
    // A drawing of a case holds the cutout and the holes in it. The cutout is
    // the big one, which is the rule when nobody says otherwise; when somebody
    // does, the layer they name is the answer even though it is smaller.
    let dir = scratch("largest");
    let entities = [
        lwpolyline("MOUNTS", &[(5.0, 5.0), (9.0, 5.0), (9.0, 9.0), (5.0, 9.0)]),
        lwpolyline(
            "CUTOUT",
            &[(0.0, 0.0), (60.0, 0.0), (60.0, 40.0), (0.0, 40.0)],
        ),
    ]
    .concat();
    let file = drawing(&dir, "case.dxf", 4, &entities);

    let biggest = read(&file, &dir.join("big.cypcb"), &[]);
    let said = String::from_utf8_lossy(&biggest.stderr).to_string();
    let text = std::fs::read_to_string(dir.join("big.cypcb")).expect("the design is readable");
    assert!(
        text.contains("size 60mm x 40mm"),
        "the cutout wins over the mounting hole:\n{text}"
    );
    assert!(
        said.contains("holds 2 closed shapes"),
        "and the run says what it passed over: {said}"
    );

    let named = design(&file, &dir.join("small.cypcb"), &["--layer", "MOUNTS"]);
    assert!(
        named.contains("size 4mm x 4mm"),
        "a named layer beats the size rule:\n{named}"
    );
}

#[test]
fn a_drawing_with_no_closed_shape_says_so_and_fails() {
    // Curves are the honest gap: this project's copper has no arcs, so an arc
    // in a drawing cannot be carried. Reading such a drawing as an empty board
    // would be worse than refusing it.
    let dir = scratch("curves");
    let file = drawing(
        &dir,
        "round.dxf",
        4,
        "0\nARC\n8\n0\n10\n0.0\n20\n0.0\n30\n0.0\n40\n10.0\n50\n0.0\n51\n90.0\n",
    );
    let result = read(&file, &dir.join("round.cypcb"), &[]);
    assert!(
        !result.status.success(),
        "a drawing it cannot read is an error"
    );
    let said = String::from_utf8_lossy(&result.stderr).to_string();
    assert!(
        said.contains("no closed shape") && said.contains("1 ARC"),
        "and the message names what was in the drawing: {said}"
    );
    assert!(
        !dir.join("round.cypcb").exists(),
        "nothing was written from a drawing that could not be read"
    );
}
