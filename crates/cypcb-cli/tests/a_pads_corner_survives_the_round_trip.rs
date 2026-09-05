//! A pad's corner survives being written down and read back.
//!
//! `cargo test -p cypcb-cli --test a_pads_corner_survives_the_round_trip`
//!
//! KiCad states how round a rounded pad is on every one it writes -
//! `(roundrect_rratio 0.2)` - and this language could not say it at all. The
//! importer read the figure, the writer wrote the bare word `roundrect`, and
//! reading that back gave the 25% fallback: a board taken out of KiCad and
//! saved from here came back with corners a fifth larger than the ones it was
//! drawn with. A pad's corner is the copper nearest its neighbour, which is
//! what a clearance is measured from.

use std::path::{Path, PathBuf};
use std::process::Command;

use cypcb_world::components::PadShape;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the crate sits two levels below the repo root")
}

/// The shape of the one pad the source's one footprint states.
fn only_pad_shape(source: &str) -> PadShape {
    let parsed = cypcb_parser::parse(source);
    assert!(
        parsed.errors.is_empty(),
        "the design does not parse: {:?}",
        parsed.errors
    );
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let sync = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(sync.errors.is_empty(), "sync: {:?}", sync.errors);

    let (_, footprint) = library
        .iter()
        .find(|(name, _)| *name == "F")
        .expect("the design states a footprint called F");
    assert_eq!(footprint.pads.len(), 1, "the design states one pad");
    footprint.pads[0].shape
}

fn design_with(corner: &str) -> String {
    format!(
        "version 1\n\
         \n\
         board b {{\n\
         \x20   size 10mm x 10mm\n\
         \x20   layers 2\n\
         }}\n\
         \n\
         footprint F {{\n\
         \x20   courtyard 2mm x 2mm\n\
         \x20   pad 1 roundrect at 0mm, 0mm size 1mm x 1mm{corner}\n\
         }}\n\
         \n\
         component U1 ic \"F\" {{\n\
         \x20   at 5mm, 5mm\n\
         }}\n"
    )
}

#[test]
fn the_corner_a_design_states_reaches_the_board() {
    assert_eq!(
        only_pad_shape(&design_with(" corner 20%")),
        PadShape::RoundRect { corner_ratio: 20 }
    );
    // A design that states none keeps the 25% this project has always used.
    // Nothing in the file says 25, so it is a fallback rather than a reading.
    assert_eq!(
        only_pad_shape(&design_with("")),
        PadShape::RoundRect { corner_ratio: 25 }
    );
}

#[test]
fn a_corner_larger_than_half_the_pad_is_refused() {
    // Half the short side is a stadium and there is nothing past it to draw.
    // Refused rather than quietly held to half: a number somebody wrote and
    // the tool changed is worse than one it would not take.
    let source = design_with(" corner 90%");
    let parsed = cypcb_parser::parse(&source);
    let complaint = format!("{:?}", parsed.errors);
    assert!(
        complaint.contains("50%"),
        "the refusal names the limit: {complaint}"
    );
}

#[test]
fn the_writer_says_the_corner_the_reader_read() {
    let source = design_with(" corner 20%");
    let parsed = cypcb_parser::parse(&source);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    sync_ast_to_world(&parsed.value, &source, &mut world, &mut library);

    let written = cypcb_world::dsl::board_as_dsl(&mut world);
    assert!(
        written.contains("corner 20%"),
        "the design written out does not say the corner:\n{written}"
    );
    assert_eq!(
        only_pad_shape(&written),
        PadShape::RoundRect { corner_ratio: 20 },
        "the design written out reads back as a different pad"
    );
}

#[test]
fn a_board_out_of_kicad_keeps_the_corners_it_was_drawn_with() {
    // Four pads on this board state `(roundrect_rratio 0.2)`; every other
    // rounded pad in this repository states 0.25.
    let board =
        repo_root().join("tests/fixtures/kicad-tools/tests/fixtures/routing-diagnostic.kicad_pcb");
    let out = std::env::temp_dir().join("cypcb-corner-round-trip.cypcb");
    let run = cypcb()
        .arg("from-kicad")
        .arg(&board)
        .arg("-o")
        .arg(&out)
        .output()
        .expect("the binary runs");
    assert!(
        run.status.success(),
        "from-kicad: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    let design = std::fs::read_to_string(&out).expect("from-kicad wrote a design");
    assert!(
        design.contains("corner 20%"),
        "the board states 0.2 on four pads and the design says nothing about them"
    );
    std::fs::remove_file(&out).ok();
}
