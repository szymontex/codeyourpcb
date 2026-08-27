//! A dimension is drawn for a person, on the plot and nowhere else.
//!
//! `cargo test -p cypcb-cli --test the_plot_states_what_the_board_measures`
//!
//! The measurement has to reach an eye to be worth writing: a number in the
//! source that never appears on anything a fabricator opens is a number nobody
//! checks the board against. It reaches the SVG plot, which is a picture for a
//! person, and it stays out of the Gerbers, which are what a house builds from.

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
    let dir = std::env::temp_dir().join(format!("cypcb-dimension-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn export(out: &Path) {
    let status = cypcb()
        .arg("export")
        .arg(example("board-dimensions.cypcb"))
        .arg("-o")
        .arg(out)
        .arg("--svg")
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export failed");
}

#[test]
fn the_plot_carries_the_measured_figure() {
    let out = scratch("figure");
    export(&out);
    let svg = std::fs::read_to_string(out.join("plot").join("board-dimensions-F_Cu.svg"))
        .expect("the top plot is readable");

    assert!(
        svg.contains(">40.000mm<"),
        "the width the board expects to hold is on the picture:\n{svg}"
    );
    assert!(svg.contains(">25.000mm<"), "and so is the height:\n{svg}");
}

#[test]
fn the_line_stands_off_the_edge_it_measures() {
    // A dimension drawn on top of the outline is a dimension nobody can read
    // apart from the outline. `offset 3mm` in the example puts it beside.
    let out = scratch("offset");
    export(&out);
    let svg = std::fs::read_to_string(out.join("plot").join("board-dimensions-F_Cu.svg"))
        .expect("the top plot is readable");

    assert!(
        svg.contains("x1=\"0.000\" y1=\"3.000\" x2=\"40.000\" y2=\"3.000\""),
        "the width's line runs 3mm above the edge it is about:\n{svg}"
    );
    assert!(
        svg.contains("x1=\"3.000\" y1=\"0.000\" x2=\"3.000\" y2=\"25.000\"")
            || svg.contains("x1=\"-3.000\" y1=\"0.000\" x2=\"-3.000\" y2=\"25.000\""),
        "and the height's line runs 3mm beside its own edge:\n{svg}"
    );
    assert!(
        svg.contains("x1=\"0.000\" y1=\"0.000\" x2=\"0.000\" y2=\"3.000\""),
        "with a witness line back to the point actually measured:\n{svg}"
    );
}

#[test]
fn nothing_a_house_builds_from_carries_it() {
    let out = scratch("copper");
    export(&out);

    // Every file a fabricator manufactures from, checked one by one: a
    // dimension that reached copper would be a short, and one that reached the
    // legend would print `40.000mm` across a finished board.
    let gerber = out.join("gerber");
    let mut checked = 0;
    for entry in std::fs::read_dir(&gerber).expect("the gerber directory exists") {
        let path = entry.expect("a directory entry").path();
        let body = std::fs::read_to_string(&path).expect("the file is readable");
        assert!(
            !body.contains("40.000mm") && !body.contains("25.000mm"),
            "{} carries a measurement meant for a person",
            path.display()
        );
        checked += 1;
    }
    assert!(
        checked >= 4,
        "the copper, mask, legend and outline were read"
    );
}
