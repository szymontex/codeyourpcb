//! A pair that carries one signal has to still be a pair after a save.
//!
//! `cargo test -p cypcb-world --test a_diffpair_survives_being_written_down`
//!
//! `diffpair USB { USB_DP USB_DM }` is what `DiffPairSkewRule` measures: the
//! two halves have to stay the same length, because a receiver reads the
//! difference between them. Unlike a `netclass`, which hands its figures to
//! each member and is flattened rather than lost, a pair is flattened onto
//! nothing - the world keeps it as its own statement and `board_as_dsl` did
//! not write it.
//!
//! So a board written out came home as two ordinary nets, and the skew rule
//! had nothing to check. Measured before the fix: **1 pair in, 0 out**.

use cypcb_world::dsl::board_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

const PAIRED: &str = r#"version 1

board paired {
    size 40mm x 20mm
    layers 2
}

component J1 connector "PIN-HDR-1x2" {
    value "in"
    at 10mm, 10mm
}

component J2 connector "PIN-HDR-1x2" {
    value "out"
    at 30mm, 10mm
}

net USB_DP {
    J1.1
    J2.1
}

net USB_DM {
    J1.2
    J2.2
}

diffpair USB {
    USB_DP
    USB_DM
}
"#;

fn load(source: &str) -> BoardWorld {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "parse: {:?}", parsed.errors);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "sync: {:?}", result.errors);
    world
}

/// The pairs a board states, as name and halves in the order they were given.
fn pairs(world: &BoardWorld) -> Vec<(String, String, String)> {
    world
        .diff_pairs()
        .iter()
        .map(|pair| {
            (
                pair.name.value.clone(),
                pair.positive.value.clone(),
                pair.negative.value.clone(),
            )
        })
        .collect()
}

#[test]
fn the_pair_is_still_a_pair() {
    let mut before = load(PAIRED);
    let stated = pairs(&before);
    assert_eq!(
        stated,
        vec![(
            "USB".to_string(),
            "USB_DP".to_string(),
            "USB_DM".to_string()
        )]
    );

    let written = board_as_dsl(&mut before);
    assert!(
        written.contains("diffpair USB {"),
        "the pair is written down by name:\n{written}"
    );

    let after = load(&written);
    assert_eq!(
        pairs(&after),
        stated,
        "the halves come back in the order they were given, because the rule \
         reports which one is long:\n{written}"
    );
}
