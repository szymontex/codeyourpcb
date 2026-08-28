//! A curve survives a trip through KiCad.
//!
//! `cargo test -p cypcb-cli --test a_curve_survives_a_trip_through_kicad`
//!
//! KiCad holds a track arc natively - `(arc (start ...) (mid ...) (end ...))` -
//! and this model holds a centre and a sweep. Neither is wrong, and the
//! conversion between them is arithmetic about arcs rather than about either
//! tool, so `cypcb_world::arc` owns both directions.
//!
//! Writing a curve out as the dozen chords it was flattened into would hand a
//! KiCad user copper they cannot edit as the one curve it is, and reading one
//! back as twelve anonymous segments would lose it on the next save.

use std::path::{Path, PathBuf};
use std::process::Command;

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
    let dir = std::env::temp_dir().join(format!("cypcb-kicad-arc-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the scratch directory is made");
    dir
}

/// Write a design out as a KiCad board and read the file back.
fn to_kicad(design: &str, dir: &Path) -> String {
    let board = dir.join("board.kicad_pcb");
    let out = cypcb()
        .arg("to-kicad")
        .arg(example(design))
        .arg("-o")
        .arg(&board)
        .output()
        .expect("the binary runs");
    assert!(
        out.status.success(),
        "the write failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::read_to_string(&board).expect("the board is readable")
}

/// Read a KiCad board back as a design.
fn from_kicad(dir: &Path) -> String {
    let back = dir.join("back.cypcb");
    let out = cypcb()
        .arg("from-kicad")
        .arg(dir.join("board.kicad_pcb"))
        .arg("-o")
        .arg(&back)
        .output()
        .expect("the binary runs");
    assert!(
        out.status.success(),
        "the read failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::read_to_string(&back).expect("the design is readable")
}

/// The numbers inside one `(name x y)` of an s-expression line.
fn point_after(line: &str, name: &str) -> (f64, f64) {
    let at = line
        .find(&format!("({name} "))
        .unwrap_or_else(|| panic!("the arc states its {name}: {line}"));
    let rest = &line[at + name.len() + 2..];
    let inside = &rest[..rest.find(')').expect("the pair closes")];
    let mut numbers = inside.split_whitespace();
    (
        numbers.next().expect("an x").parse().expect("a number"),
        numbers.next().expect("a y").parse().expect("a number"),
    )
}

#[test]
fn a_curve_is_written_as_kicads_own_arc() {
    let board = to_kicad("curved-track.cypcb", &scratch("write"));
    assert_eq!(
        board.matches("  (arc ").count(),
        1,
        "the one curve is one arc:\n{board}"
    );
    assert_eq!(
        board.matches("  (segment ").count(),
        2,
        "and the two straight runs are still segments"
    );
}

#[test]
fn the_three_points_kicad_stores_are_on_the_curve() {
    // KiCad states an arc by three points on it, and a mid-point that is not
    // half way round is a different curve - a reader takes it at its word.
    let board = to_kicad("curved-track.cypcb", &scratch("mid"));
    let line = board
        .lines()
        .find(|line| line.trim_start().starts_with("(arc "))
        .expect("the arc is in the file");

    let start = point_after(line, "start");
    let mid = point_after(line, "mid");
    let end = point_after(line, "end");

    // The example turns about a centre 4mm from each end. Whatever the sheet
    // origin is, all three points are that far from one point.
    let radius = |point: (f64, f64), centre: (f64, f64)| {
        ((point.0 - centre.0).powi(2) + (point.1 - centre.1).powi(2)).sqrt()
    };
    // The centre is the corner of the right angle the ends make.
    let centre = (start.0, end.1);
    for (name, point) in [("start", start), ("mid", mid), ("end", end)] {
        assert!(
            (radius(point, centre) - 4.0).abs() < 0.001,
            "{name} sits {:.4}mm from the centre, and the radius is 4mm",
            radius(point, centre)
        );
    }
    // And the mid-point is between the ends rather than at one of them.
    assert!(
        (radius(mid, start) - radius(mid, end)).abs() < 0.001,
        "the mid-point is half way round: {mid:?}"
    );
}

#[test]
fn the_curve_comes_home_as_a_curve() {
    let dir = scratch("roundtrip");
    to_kicad("curved-track.cypcb", &dir);
    let design = from_kicad(&dir);

    assert!(
        design.contains("arc start 12.000000mm,6.000000mm"),
        "the design says arc again, from where it started:\n{design}"
    );
    assert!(
        design.contains("centre 12.000000mm,10.000000mm") && design.contains("sweep 90 clockwise"),
        "about the same centre and the same way round:\n{design}"
    );
}

#[test]
fn the_copper_that_comes_home_is_the_copper_that_left() {
    // The curve is the sentence; the chords are the board. Both have to
    // survive, and the plot is where the copper can be counted.
    let dir = scratch("copper");
    to_kicad("curved-track.cypcb", &dir);
    from_kicad(&dir);

    let plot = |design: &Path, out: &Path| -> String {
        let status = cypcb()
            .arg("export")
            .arg(design)
            .arg("-o")
            .arg(out)
            .arg("--svg")
            .status()
            .expect("the binary runs");
        assert!(status.success(), "the export failed");
        let name = design
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("a name");
        let svg = std::fs::read_to_string(out.join("plot").join(format!("{name}-F_Cu.svg")))
            .expect("the plot is readable");
        // Everything the plot draws, sorted, with the board's own name and
        // title left out: two plots of the same copper are the same drawing.
        // Sorted because the order shapes come out in follows the world's
        // archetypes rather than anything about the board, and a trip through
        // KiCad spawns the curve at a different moment.
        let mut drawn: Vec<&str> = svg
            .lines()
            .filter(|line| line.contains("<line") || line.contains("<path"))
            .collect();
        drawn.sort_unstable();
        drawn.join("\n")
    };

    let before = plot(&example("curved-track.cypcb"), &dir.join("before"));
    let after = plot(&dir.join("back.cypcb"), &dir.join("after"));
    assert_eq!(
        after, before,
        "the same copper is drawn after the trip as before it"
    );
    assert!(
        before.contains("A 4.000 4.000"),
        "and the curve is still a curve on both sides of it:\n{before}"
    );
}

#[test]
fn a_board_with_no_curve_gets_no_arcs() {
    let board = to_kicad("usb-diff-pair.cypcb", &scratch("straight"));
    assert!(
        !board.contains("  (arc "),
        "a board of straight copper is written exactly as it was:\n{board}"
    );
}
