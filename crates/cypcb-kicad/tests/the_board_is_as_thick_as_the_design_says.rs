//! One command, one board, one thickness.
//!
//! `cargo test -p cypcb-kicad --test the_board_is_as_thick_as_the_design_says`
//!
//! `(general (thickness 1.6))` was written as a literal, whatever the design
//! said. A board declaring
//!
//! ```text
//! stackup { copper 0.035mm prepreg 0.2mm copper 0.0175mm core 1.065mm ... }
//! ```
//!
//! sums to 1.57mm, and the checker reports that, and the Gerber job file
//! writes `"BoardThickness": 1.57`. The KiCad file said 1.6. Two files out of
//! the same export disagreeing about how thick the board is, which is worse
//! than either number being wrong on its own: whoever notices has to work out
//! which one to believe.
//!
//! 1.6 stays for a design that states no stackup. That is the ordinary
//! two-layer build and what this line always claimed - it is a default, not a
//! measurement, and the difference is the point.

use cypcb_core::Nm;
use cypcb_kicad::board_writer::write_board;
use cypcb_world::{BoardWorld, Stackup, StackupLayer, StackupLayerKind};

use StackupLayerKind::{Copper, Core, Prepreg};

fn board(stackup: Option<&[(StackupLayerKind, Option<f64>)]>) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        "thick".to_string(),
        (Nm::from_mm(30.0), Nm::from_mm(20.0)),
        4,
    );
    if let Some(spec) = stackup {
        world.set_stackup(Stackup {
            layers: spec
                .iter()
                .map(|(kind, thickness)| StackupLayer::new(*kind, thickness.map(Nm::from_mm)))
                .collect(),
        });
    }
    world
}

/// The number in `(general (thickness N))`.
fn stated_thickness(text: &str) -> String {
    text.lines()
        .find_map(|line| line.split("(thickness ").nth(1))
        .and_then(|rest| rest.split(')').next())
        .map(str::to_string)
        .unwrap_or_else(|| panic!("no thickness at all:\n{text}"))
}

/// 0.035 + 0.2 + 0.0175 + 1.065 + 0.0175 + 0.2 + 0.035 = 1.57
const FOUR_LAYER: &[(StackupLayerKind, Option<f64>)] = &[
    (Copper, Some(0.035)),
    (Prepreg, Some(0.2)),
    (Copper, Some(0.0175)),
    (Core, Some(1.065)),
    (Copper, Some(0.0175)),
    (Prepreg, Some(0.2)),
    (Copper, Some(0.035)),
];

#[test]
fn a_declared_stackup_decides_the_thickness() {
    let text = write_board(&mut board(Some(FOUR_LAYER)), "cypcb");

    assert_eq!(stated_thickness(&text), "1.57");
}

#[test]
fn it_is_the_same_number_the_job_file_writes() {
    // The whole point: one design, two files, one answer. `1.570mm of
    // material` is what the checker prints and `"BoardThickness": 1.57` is
    // what the Gerber job file carries, both from `Stackup::total_thickness`.
    let stackup = board(Some(FOUR_LAYER));
    let total = stackup
        .stackup()
        .and_then(|stackup| stackup.total_thickness())
        .expect("every layer states one");

    let text = write_board(&mut board(Some(FOUR_LAYER)), "cypcb");
    assert_eq!(stated_thickness(&text), format!("{:.2}", total.to_mm()));
}

#[test]
fn a_design_with_no_stackup_keeps_the_ordinary_build() {
    // Every example in this repository is in this case, and none of them may
    // move.
    let text = write_board(&mut board(None), "cypcb");

    assert_eq!(stated_thickness(&text), "1.6");
}

#[test]
fn a_stackup_that_states_no_thicknesses_does_not_invent_one() {
    // A partial sum is not a thickness, so there is nothing to write and the
    // default stands.
    let bare: &[(StackupLayerKind, Option<f64>)] = &[(Copper, None), (Core, None), (Copper, None)];
    let text = write_board(&mut board(Some(bare)), "cypcb");

    assert_eq!(stated_thickness(&text), "1.6");
}
