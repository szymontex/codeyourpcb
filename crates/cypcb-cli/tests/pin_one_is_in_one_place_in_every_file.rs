//! Pin 1 of a part is in one place on the board, whichever file says where.
//!
//! `cargo test -p cypcb-cli --test pin_one_is_in_one_place_in_every_file`
//!
//! Until 2026-09-25 the KiCad file and the Gerber written from one design
//! disagreed about it: the KiCad writer kept the board's upward Y as the
//! sheet's downward one, so a SOT-23-5 with pad 1 at the bottom left of the
//! Gerber had it at the top left of the `.kicad_pcb`. Nothing compared the two
//! files, and each one read back into this project agreed with itself.
//!
//! A Gerber flash does not say which pad it is, so the part here is drawn to
//! be told apart: pad 1 is a 1.2mm square and the other three are 0.6mm, laid
//! out with no symmetry that maps pad 1 onto another pad. It is placed turned
//! a quarter, so the direction a rotation turns is checked in the same breath:
//! a writer that turned it the other way would put pad 1 somewhere else.

use std::path::Path;
use std::process::Command;

const DESIGN: &str = r#"version 1

board pin_one {
    size 20mm x 20mm
    layers 2
}

footprint ASYMMETRIC {
    description "Pad 1 is the big one, top left; no symmetry maps it elsewhere"
    courtyard 5mm x 5mm

    pad 1 rect at -1.5mm, 1.0mm size 1.2mm x 1.2mm
    pad 2 rect at -1.5mm, -1.0mm size 0.6mm x 0.6mm
    pad 3 rect at 1.5mm, -1.0mm size 0.6mm x 0.6mm
    pad 4 rect at 1.5mm, 0.3mm size 0.6mm x 0.6mm
}

component U1 ic "ASYMMETRIC" {
    value "X"
    at 8mm, 12mm
    rotate 90
}
"#;

/// Pad 1 turned a quarter counter-clockwise about U1: (-1.5, 1.0) becomes
/// (-1.0, -1.5), so on the board it is at (7.0, 10.5).
const PAD_ONE: (f64, f64) = (7.0, 10.5);

fn cypcb(args: &[&str]) {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(args)
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "cypcb {args:?} failed:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The numbers after each `key` in `text`, up to the next `)`.
fn numbers_after(text: &str, key: &str) -> Vec<Vec<f64>> {
    text.match_indices(key)
        .map(|(at, _)| {
            text[at + key.len()..]
                .split(')')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .filter_map(|n| n.parse().ok())
                .collect()
        })
        .collect()
}

/// Where pad 1 lands on the board, read from the `.kicad_pcb`.
fn pad_one_in_kicad(text: &str) -> (f64, f64) {
    // The board's corner on the sheet: the outline's smallest X and, because
    // the sheet's Y grows down, its largest Y.
    let corners: Vec<Vec<f64>> = text
        .lines()
        .filter(|line| line.contains("Edge.Cuts") && line.contains("gr_line"))
        .flat_map(|line| {
            let mut ends = numbers_after(line, "(start ");
            ends.extend(numbers_after(line, "(end "));
            ends
        })
        .collect();
    assert!(!corners.is_empty(), "the file has no outline:\n{text}");
    let left = corners.iter().map(|p| p[0]).fold(f64::MAX, f64::min);
    let bottom = corners.iter().map(|p| p[1]).fold(f64::MIN, f64::max);

    let footprint = text
        .find("(footprint \"cypcb:ASYMMETRIC\"")
        .map(|at| &text[at..])
        .expect("the part is in the file");
    let at = numbers_after(footprint, "(at ")
        .into_iter()
        .next()
        .expect("the part is placed");
    let (x, y, angle) = (at[0], at[1], at.get(2).copied().unwrap_or(0.0));
    let pad = footprint
        .find("(pad \"1\"")
        .map(|at| &footprint[at..])
        .expect("pad 1 is in the file");
    let local = numbers_after(pad, "(at ")
        .into_iter()
        .next()
        .expect("pad 1 is placed");

    // KiCad turns a footprint counter-clockwise as the sheet is drawn, Y down:
    // a local offset (u, v) lands at (u cos + v sin, -u sin + v cos).
    let (sin, cos) = angle.to_radians().sin_cos();
    let sheet_x = x + local[0] * cos + local[1] * sin;
    let sheet_y = y - local[0] * sin + local[1] * cos;
    (sheet_x - left, bottom - sheet_y)
}

/// Where the 1.2mm square is flashed on the top copper.
fn pad_one_in_gerber(text: &str) -> (f64, f64) {
    let aperture = text
        .lines()
        .find_map(|line| {
            let rest = line.strip_prefix("%ADD")?;
            let (code, shape) = rest.split_at(rest.find(|c: char| !c.is_ascii_digit())?);
            shape.starts_with("R,1.2").then(|| format!("D{code}*"))
        })
        .unwrap_or_else(|| panic!("no 1.2mm square aperture:\n{text}"));
    let mut selected = false;
    let mut flashes = Vec::new();
    for line in text.lines() {
        if line.starts_with('D') && line.ends_with('*') && !line.contains('X') {
            selected = line == aperture;
        } else if selected && line.ends_with("D03*") {
            let x = line[1..line.find('Y').unwrap()].parse::<f64>().unwrap() / 1e6;
            let y = line[line.find('Y').unwrap() + 1..line.find("D03").unwrap()]
                .parse::<f64>()
                .unwrap()
                / 1e6;
            flashes.push((x, y));
        }
    }
    assert_eq!(flashes.len(), 1, "one flash of pad 1's aperture:\n{text}");
    flashes[0]
}

fn near(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 0.001 && (a.1 - b.1).abs() < 0.001
}

#[test]
fn the_gerber_and_the_kicad_file_put_pin_one_in_one_place() {
    let dir = cypcb_fixtures::scratch_dir("cypcb-pin-one");
    let design = dir.join("pin_one.cypcb");
    std::fs::write(&design, DESIGN).expect("the scratch dir is writable");
    let design = design.to_str().expect("a path that is text");

    let kicad = dir.join("pin_one.kicad_pcb");
    cypcb(&["to-kicad", design, "-o", kicad.to_str().unwrap()]);
    let out = dir.join("export");
    cypcb(&["export", design, "-o", out.to_str().unwrap(), "--force"]);

    let gerber = std::fs::read_to_string(Path::new(&out).join("gerber/pin_one-F_Cu.gbr"))
        .expect("the top copper was written");
    let kicad = std::fs::read_to_string(&kicad).expect("the KiCad file was written");

    let in_gerber = pad_one_in_gerber(&gerber);
    let in_kicad = pad_one_in_kicad(&kicad);
    assert!(
        near(in_gerber, PAD_ONE),
        "the Gerber flashes pad 1 at {in_gerber:?}, the design puts it at {PAD_ONE:?}"
    );
    assert!(
        near(in_kicad, PAD_ONE),
        "the KiCad file puts pad 1 at {in_kicad:?} on the board, the design at {PAD_ONE:?}"
    );
}

#[test]
fn a_kicad_file_read_back_keeps_pin_one_where_it_was() {
    let dir = cypcb_fixtures::scratch_dir("cypcb-pin-one-back");
    let design = dir.join("pin_one.cypcb");
    std::fs::write(&design, DESIGN).expect("the scratch dir is writable");
    let kicad = dir.join("pin_one.kicad_pcb");
    cypcb(&[
        "to-kicad",
        design.to_str().unwrap(),
        "-o",
        kicad.to_str().unwrap(),
    ]);
    let back = dir.join("back.cypcb");
    cypcb(&[
        "from-kicad",
        kicad.to_str().unwrap(),
        "-o",
        back.to_str().unwrap(),
    ]);
    let out = dir.join("export");
    cypcb(&[
        "export",
        back.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--force",
    ]);
    let gerber = std::fs::read_dir(out.join("gerber"))
        .expect("the gerbers were written")
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy().ends_with("-F_Cu.gbr"))
        .map(|entry| std::fs::read_to_string(entry.path()).unwrap())
        .expect("the top copper was written");
    let in_gerber = pad_one_in_gerber(&gerber);
    assert!(
        near(in_gerber, PAD_ONE),
        "after a trip through KiCad pad 1 is at {in_gerber:?}, the design put it at {PAD_ONE:?}"
    );
}
