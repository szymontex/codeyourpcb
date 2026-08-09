//! A slot can be written in the language, not only imported from KiCad.
//!
//! `cargo test -p cypcb-world --test a_design_can_ask_for_a_slot`
//!
//! A slot reached the model, the drill file, the KiCad file and the screen -
//! and the one place it could not come from was a `.cypcb` file. `sync.rs`
//! wrote `slot: None` with a comment saying the language had no way to say it,
//! so every slot in the system was an import-only feature and a design written
//! in this tool could not describe the hole its own connector needs.
//!
//! `drill 2.4mm x 1.0mm` says it now, written the way `size W x H` is because
//! it is the same question asked of the hole rather than of the copper. One
//! number is still a round hole, so nothing about an existing design changes.

use cypcb_core::Nm;
use cypcb_parser::parse;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// Build the world a source describes, and fail loudly if it would not build.
fn world_of(source: &str) -> (BoardWorld, FootprintLibrary) {
    let parsed = parse(source);
    assert!(
        parsed.errors.is_empty(),
        "the source parses: {:?}",
        parsed.errors
    );
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "clean sync: {:?}", result.errors);
    (world, library)
}

fn source(pad: &str) -> String {
    format!(
        "version 1

board holes {{
    size 30mm x 20mm
    layers 2
}}

footprint ANCHOR {{
    description \"one anchor\"
    courtyard 6mm x 4mm

    {pad}
}}

component J1 connector \"ANCHOR\" {{
    value \"anchor\"
    at 15mm, 10mm
}}
"
    )
}

/// The pad the source defined, out of the library the sync filled.
fn only_pad(library: &FootprintLibrary) -> (Option<Nm>, Option<(Nm, Nm)>) {
    let footprint = library.get("ANCHOR").expect("the design's own footprint");
    let pad = footprint.pads.first().expect("the footprint has its pad");
    (pad.drill, pad.slot)
}

#[test]
fn two_numbers_are_a_slot() {
    let (_, library) = world_of(&source(
        "pad 1 oblong at 0mm, 0mm size 3.2mm x 1.8mm drill 2.4mm x 1.0mm",
    ));

    assert_eq!(
        only_pad(&library),
        (
            // The narrow dimension is what every rule about a drill means: the
            // bit the fab has to own, the width the plating reaches down.
            Some(Nm::from_mm(1.0)),
            Some((Nm::from_mm(2.4), Nm::from_mm(1.0)))
        )
    );
}

#[test]
fn one_number_is_still_a_round_hole() {
    let (_, library) = world_of(&source(
        "pad 1 circle at 0mm, 0mm size 1.6mm x 1.6mm drill 0.9mm",
    ));

    assert_eq!(only_pad(&library), (Some(Nm::from_mm(0.9)), None));
}

#[test]
fn a_slot_written_tall_is_read_tall() {
    // The pair is stored as written rather than sorted, because which way the
    // hole runs is what the drill file mills along and what the screen draws.
    let (_, library) = world_of(&source(
        "pad 1 oblong at 0mm, 0mm size 1.8mm x 3.2mm drill 1.0mm x 2.4mm",
    ));

    assert_eq!(
        only_pad(&library),
        (
            Some(Nm::from_mm(1.0)),
            Some((Nm::from_mm(1.0), Nm::from_mm(2.4)))
        )
    );
}

#[test]
fn a_square_pair_is_a_round_hole() {
    // `drill 1.0mm x 1.0mm` is a 1mm drill written the long way. Storing a
    // slot there would send the fab a zero-length milling path for a hole one
    // hit makes.
    let (_, library) = world_of(&source(
        "pad 1 circle at 0mm, 0mm size 1.6mm x 1.6mm drill 1.0mm x 1.0mm",
    ));

    assert_eq!(only_pad(&library), (Some(Nm::from_mm(1.0)), None));
}
