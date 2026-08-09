//! A board with a slotted pad has to open, and come back out with its slot.
//!
//! `cargo test -p cypcb-kicad --test a_slot_is_not_a_round_hole`
//!
//! KiCad writes a slot as `(drill oval 2.4 1.0)`: a hole milled along its
//! length with a bit the width of its narrow dimension. Every USB connector,
//! barrel jack and latching header holds itself to the board through one.
//!
//! This reader took `list[1]` of a drill as a number, so `oval` failed the
//! coordinate check and **the whole board was refused** - with a message
//! advising the user to look for a stray comma, which there never was. The
//! `.kicad_mod` reader did the opposite and worse: it took the width, called
//! the height "for oval drills" in a comment, and imported a round hole. A
//! slot delivered round is a part that does not fit and a board that is scrap.
//!
//! Both halves are checked here, and so is the way back out: a board written
//! by `to-kicad` has to carry the oval, not the narrow dimension alone.

use cypcb_core::Nm;
use cypcb_kicad::board_writer::write_board;
use cypcb_kicad::pcb_parser::{parse_kicad_pcb, KicadPcbParseResult};
use cypcb_world::footprint::FootprintLibrary;

const FIXTURE: &str = "tests/fixtures/slotted.kicad_pcb";

fn imported() -> KicadPcbParseResult {
    parse_kicad_pcb(std::path::Path::new(FIXTURE)).expect("a board with a slot is a board")
}

/// One pad's hole: what it is drilled with, and its full size when it is a slot.
type Hole = (Option<Nm>, Option<(Nm, Nm)>);

/// The pads of the LED footprint, which is where the slot is.
fn slotted_pads(library: &FootprintLibrary) -> Vec<Hole> {
    library
        .iter()
        .filter(|(name, _)| name.contains("LED"))
        .flat_map(|(_, footprint)| footprint.pads.iter().map(|pad| (pad.drill, pad.slot)))
        .collect()
}

#[test]
fn a_board_with_a_slot_opens_at_all() {
    let mut imported = imported();

    assert_eq!(imported.world.component_count(), 2, "both parts arrived");
}

#[test]
fn the_slot_keeps_both_of_its_dimensions() {
    let imported = imported();
    let pads = slotted_pads(&imported.library);

    let slots: Vec<_> = pads.iter().filter_map(|(_, slot)| *slot).collect();
    assert_eq!(
        slots,
        vec![(Nm::from_mm(2.4), Nm::from_mm(1.0))],
        "one slot, as written: {pads:?}"
    );
}

#[test]
fn the_drill_is_the_narrow_dimension() {
    // What every rule about a drill means: the smallest bit the fab has to
    // own, the width the plating reaches down, the wall a router breaks into.
    // Taking 2.4mm would tell the checker a 2.4mm bit makes this hole.
    let imported = imported();

    let drills: Vec<_> = slotted_pads(&imported.library)
        .into_iter()
        .filter_map(|(drill, slot)| slot.map(|_| drill))
        .collect();

    assert_eq!(drills, vec![Some(Nm::from_mm(1.0))]);
}

#[test]
fn a_round_hole_is_still_a_round_hole() {
    // The control. Pad 1 of the same footprint is `(drill 0.9)`, and a reader
    // that saw slots everywhere would be no better than one that saw none.
    let imported = imported();

    let round: Vec<_> = slotted_pads(&imported.library)
        .into_iter()
        .filter(|(_, slot)| slot.is_none())
        .map(|(drill, _)| drill)
        .collect();

    assert_eq!(round, vec![Some(Nm::from_mm(0.9))]);
}

#[test]
fn the_slot_survives_the_way_back_out() {
    let mut imported = imported();
    imported.world.set_footprints(imported.library.clone());
    let written = write_board(&mut imported.world, "cypcb-test");

    assert!(
        written.contains("(drill oval 2.4 1)"),
        "the board goes back out with its slot:\n{}",
        written
            .lines()
            .filter(|line| line.contains("drill"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        written.contains("(drill 0.9)"),
        "and the round hole stays round"
    );
}
