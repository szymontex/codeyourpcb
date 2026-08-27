//! `--dxf` hands a copper layer to a mechanical tool.
//!
//! `cargo test -p cypcb-cli --test a_layer_reaches_a_mechanical_tool`
//!
//! An enclosure is drawn in a CAD tool, and what that tool asks of a board is
//! where the copper, the holes and the edge are. It does not read Gerber and it
//! does not read SVG. DXF is what it reads, and this is the other half of item
//! 7 of the KiCad parity audit.
//!
//! DXF is pairs of lines - a group code, then its value - so a test can read
//! back exactly what was written, the same way the SVG tests do.

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
    let dir = std::env::temp_dir().join(format!("cypcb-dxf-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Export with DXF plots and read one back.
fn plot(board: &str, out: &Path, suffix: &str) -> String {
    let status = cypcb()
        .arg("export")
        .arg(example(board))
        .arg("-o")
        .arg(out)
        .arg("--dxf")
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export failed");
    let name = board.trim_end_matches(".cypcb");
    std::fs::read_to_string(out.join("plot").join(format!("{name}-{suffix}.dxf")))
        .expect("the drawing is readable")
}

#[test]
fn the_file_says_which_dxf_it_is_and_in_what_units() {
    // A DXF number carries no unit. A drawing that does not say millimetres is
    // a drawing a mechanical tool may open at 25.4 times the size.
    let dxf = plot("usb-diff-pair.cypcb", &scratch("header"), "F_Cu");
    assert!(
        dxf.contains("$ACADVER\n1\nAC1009\n"),
        "the version every mechanical tool reads:\n{}",
        &dxf[..dxf.len().min(200)]
    );
    assert!(
        dxf.contains("$INSUNITS\n70\n4\n"),
        "and the units it is drawn in"
    );
    assert!(
        dxf.trim_end().ends_with("EOF"),
        "and the file ends properly"
    );
}

#[test]
fn every_line_of_the_file_is_half_of_a_pair() {
    // The failure this format is famous for: one stray line and every group
    // after it is read as the wrong thing.
    let dxf = plot("usb-diff-pair.cypcb", &scratch("pairs"), "F_Cu");
    let lines: Vec<&str> = dxf.lines().collect();
    assert_eq!(lines.len() % 2, 0, "the file is a whole number of pairs");
    for (index, line) in lines.iter().enumerate() {
        if index % 2 == 0 {
            assert!(
                line.trim().parse::<u16>().is_ok(),
                "line {} should be a group code, and is {line:?}",
                index + 1
            );
        }
    }
}

#[test]
fn the_board_edge_is_a_closed_shape_of_its_own_size() {
    // usb-diff-pair is 30mm by 20mm. The edge comes first, on its own layer,
    // so a tool that wants the outline alone can switch the copper off.
    let dxf = plot("usb-diff-pair.cypcb", &scratch("edge"), "F_Cu");
    let entities = dxf.split("0\nSECTION\n2\nENTITIES\n").nth(1).unwrap();
    assert!(
        entities.starts_with("0\nPOLYLINE\n8\nEdge_Cuts\n66\n1\n70\n1\n"),
        "a closed polyline on Edge_Cuts leads the drawing:\n{}",
        &entities[..entities.len().min(200)]
    );
    assert!(
        entities.contains("10\n30.000\n20\n0.000\n")
            && entities.contains("10\n30.000\n20\n20.000\n"),
        "with the board's own corners in it"
    );
}

#[test]
fn the_copper_is_on_the_layer_it_is_on() {
    let out = scratch("layers");
    let top = plot("usb-diff-pair.cypcb", &out, "F_Cu");
    let bottom = std::fs::read_to_string(out.join("plot").join("usb-diff-pair-B_Cu.dxf"))
        .expect("the bottom drawing is readable");

    assert!(top.contains("8\nF_Cu\n"), "the top drawing names the top");
    assert!(
        !top.contains("8\nB_Cu\n"),
        "and carries nothing from the bottom"
    );
    // Through-hole pads are on both layers; the tracks are only on the top.
    assert!(
        bottom.contains("8\nB_Cu\n"),
        "the bottom drawing carries its pads:\n{bottom}"
    );
}

#[test]
fn a_track_carries_the_width_it_runs_at() {
    // Copper is not a hairline. A mechanical tool asking how much room a track
    // needs gets the answer from the polyline's own width.
    let dxf = plot("usb-diff-pair.cypcb", &scratch("width"), "F_Cu");
    let tracks: Vec<&str> = dxf
        .split("0\nPOLYLINE\n")
        .filter(|block| block.starts_with("8\nF_Cu\n66\n1\n70\n0\n"))
        .collect();
    assert_eq!(tracks.len(), 2, "the board has two tracks:\n{dxf}");
    for track in &tracks {
        assert!(
            track.contains("40\n0.200\n41\n0.200\n"),
            "drawn at the width it is: {track}"
        );
    }
}

#[test]
fn a_measurement_reaches_the_drawing_as_text() {
    let dxf = plot("board-dimensions.cypcb", &scratch("dimension"), "F_Cu");
    assert!(
        dxf.contains("0\nTEXT\n8\nDimensions\n"),
        "the figure is a text entity on its own layer:\n{dxf}"
    );
    assert!(
        dxf.contains("1\n40.000mm\n") && dxf.contains("1\n25.000mm\n"),
        "and it says what the ends give"
    );
}

#[test]
fn the_drawing_is_the_right_way_up() {
    // A board's Y grows up and so does a DXF's, so nothing is flipped. The SVG
    // plotter flips because SVG's grows down; the two writing different
    // numbers for the same pad is the point of checking.
    let out = scratch("axis");
    let status = cypcb()
        .arg("export")
        .arg(example("usb-diff-pair.cypcb"))
        .arg("-o")
        .arg(&out)
        .arg("--dxf")
        .arg("--svg")
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export failed");

    let dxf = std::fs::read_to_string(out.join("plot").join("usb-diff-pair-F_Cu.dxf"))
        .expect("the drawing is readable");
    let svg = std::fs::read_to_string(out.join("plot").join("usb-diff-pair-F_Cu.svg"))
        .expect("the picture is readable");

    // The same track end, in both files, in board coordinates.
    assert!(
        svg.contains("y1=\"6.730\""),
        "the SVG is in board coordinates"
    );
    assert!(
        dxf.contains("20\n6.730\n"),
        "and so is the DXF, with one flip in the SVG's own group and none here"
    );
    assert!(
        svg.contains("<g transform=\"translate(0 20.000) scale(1 -1)\">"),
        "which is where the SVG's flip lives"
    );
    assert!(
        !dxf.contains("scale") && !dxf.to_lowercase().contains("transform"),
        "and the DXF has no flip at all"
    );
}

#[test]
fn a_board_that_does_not_ask_gets_no_drawings() {
    let out = scratch("silent");
    let status = cypcb()
        .arg("export")
        .arg(example("usb-diff-pair.cypcb"))
        .arg("-o")
        .arg(&out)
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export failed");
    assert!(
        !out.join("plot").exists(),
        "the file set a house receives is unchanged unless a drawing is asked for"
    );
}
