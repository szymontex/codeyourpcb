//! The plot draws the curve the board states, not the chords it became.
//!
//! `cargo test -p cypcb-cli --test the_plot_draws_the_curve`
//!
//! A curve reaches copper as the chords everything here measures - that is
//! what the checker, the router and the Gerbers read, and it is right. A
//! picture is a different question: SVG has an arc in a path, DXF R12 has an
//! `ARC` entity holding exactly what this model holds, and PDF has Beziers,
//! which approximate a quarter turn to about a part in a thousand. A plot made
//! of a dozen chords is a picture of the flattening rather than of the board.

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
    let dir = std::env::temp_dir().join(format!("cypcb-plotcurve-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the scratch directory is made");
    dir
}

/// Plot a design in all three formats and hand back the top copper of each.
fn plots(design: &Path, dir: &Path) -> (String, String, String) {
    let status = cypcb()
        .arg("export")
        .arg(design)
        .arg("-o")
        .arg(dir)
        .arg("--svg")
        .arg("--dxf")
        .arg("--pdf")
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export failed");
    let name = design
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("a name");
    let read = |suffix: &str| {
        std::fs::read_to_string(dir.join("plot").join(format!("{name}-F_Cu.{suffix}")))
            .expect("the plot is readable")
    };
    (read("svg"), read("dxf"), read("pdf"))
}

/// A one-curve design written into the scratch directory.
fn design(dir: &Path, name: &str, arc: &str) -> PathBuf {
    let source = format!(
        "version 1\n\nboard b {{\n    size 30mm x 30mm\n    layers 2\n    fab jlcpcb\n}}\n\n\
         net SIG {{\n}}\n\ntrace SIG {{\n    layer top\n    width 0.25mm\n    \
         path 8mm, 6mm -> 12mm, 6mm\n    {arc}\n}}\n"
    );
    let path = dir.join(name);
    std::fs::write(&path, source).expect("the design is written");
    path
}

#[test]
fn the_svg_draws_an_arc_rather_than_a_dozen_lines() {
    let (svg, _, _) = plots(&example("curved-track.cypcb"), &scratch("svg"));

    assert!(
        svg.contains("<path d=\"M 12.000 6.000 A 4.000 4.000 0 0 0 8.000 10.000\""),
        "the curve is one arc between its own ends:\n{svg}"
    );
    assert_eq!(
        svg.matches("<line").count(),
        2,
        "and the only straight lines left are the two straight runs:\n{svg}"
    );
    assert!(
        svg.contains("stroke-width=\"0.250\""),
        "drawn at the width the copper runs at"
    );
}

#[test]
fn the_dxf_carries_the_arc_a_mechanical_tool_can_measure() {
    let (_, dxf, _) = plots(&example("curved-track.cypcb"), &scratch("dxf"));

    assert!(dxf.contains("0\nARC\n"), "the drawing holds an arc:\n{dxf}");
    assert!(
        dxf.contains("10\n12.000\n20\n10.000\n30\n0.0\n40\n4.000\n"),
        "about the centre the board states, at the radius it turns at:\n{dxf}"
    );
    // DXF always draws counter-clockwise, so a clockwise curve is the same arc
    // read the other way round: 180 degrees to 270, not 270 to 180.
    assert!(
        dxf.contains("50\n180.000\n51\n270.000\n"),
        "and the two angles are in the order DXF reads them:\n{dxf}"
    );
}

#[test]
fn the_pdf_draws_the_curve_as_curves() {
    let (_, _, pdf) = plots(&example("curved-track.cypcb"), &scratch("pdf"));

    assert_eq!(
        pdf.matches(" c\n").count(),
        1,
        "a quarter turn is one Bezier:\n{pdf}"
    );
    // 12mm and 6mm in points, where the curve begins.
    assert!(
        pdf.contains("34.016 17.008 m\n"),
        "starting where the copper does:\n{pdf}"
    );
}

#[test]
fn which_way_the_curve_turns_reaches_every_plot() {
    // The same two ends describe two arcs, and a plot that picks the wrong one
    // draws copper on the other side of the board.
    let dir = scratch("direction");
    let widdershins = design(&dir, "ccw.cypcb", "arc centre 12mm, 10mm sweep 90");
    let (svg, dxf, _) = plots(&widdershins, &dir.join("ccw"));

    assert!(
        svg.contains("A 4.000 4.000 0 0 1 16.000 10.000"),
        "counter-clockwise is the other sweep flag, and the other end:\n{svg}"
    );
    assert!(
        dxf.contains("50\n270.000\n51\n0.000\n"),
        "and the DXF angles run the other way about:\n{dxf}"
    );
}

#[test]
fn a_curve_past_a_half_turn_says_so() {
    // An SVG arc between two points is one of four, and the long way round is
    // a flag of its own. A PDF Bezier is only good for about a quarter turn,
    // so a long curve is drawn in pieces.
    let dir = scratch("long");
    let long = design(&dir, "long.cypcb", "arc centre 12mm, 10mm sweep 270");
    let (svg, _, pdf) = plots(&long, &dir.join("long"));

    assert!(
        svg.contains("A 4.000 4.000 0 1 1 "),
        "three quarters of a turn is the long way round:\n{svg}"
    );
    assert_eq!(
        pdf.matches(" c\n").count(),
        3,
        "and three Beziers, one per quarter:\n{pdf}"
    );
}

#[test]
fn a_board_of_straight_copper_is_plotted_exactly_as_it_was() {
    let (svg, dxf, _) = plots(&example("usb-diff-pair.cypcb"), &scratch("straight"));
    assert!(!svg.contains("<path d=\"M"), "no arcs in the picture");
    assert!(!dxf.contains("0\nARC\n"), "and none in the drawing");
    // The PDF is not asked the same question: a round pad is four Beziers
    // there, so `c` on the page means a curve of some kind rather than a
    // curved track. The board above has no pads, which is why counting them
    // is a fair test there and not here.
}
