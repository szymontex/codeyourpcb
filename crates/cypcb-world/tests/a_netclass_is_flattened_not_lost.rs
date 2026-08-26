//! A class states a rule once; the file that comes back states it per net.
//!
//! `cargo test -p cypcb-world --test a_netclass_is_flattened_not_lost`
//!
//! `netclass Power [width 0.5mm clearance 0.3mm] { VCC GND }` is the grammar's
//! way of saying a thing once for a group. Sync hands each figure to every
//! member and keeps no membership, so `board_as_dsl` has nothing to write the
//! word back from - a board written out comes home with the numbers on each
//! net and the grouping gone.
//!
//! **That is the right answer and this test is here to keep it.** Rebuilding a
//! class out of nets that happen to share figures would invent a grouping the
//! design never stated, which is the kind of guess this project keeps finding
//! in its own history. What matters is that the board is checked the same
//! either way, and that is what is asserted below.

use cypcb_world::dsl::board_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::registry::NetConstraints;
use cypcb_world::{sync_ast_to_world, BoardWorld, NetId};

const CLASSED: &str = r#"version 1

board classed {
    size 40mm x 20mm
    layers 2
}

netclass Power [width 0.5mm clearance 0.3mm] {
    VCC
    GND
}

component R1 resistor "0402" {
    value "10k"
    at 10mm, 10mm
}

component R2 resistor "0402" {
    value "10k"
    at 30mm, 10mm
}

net VCC {
    R1.1
    R2.1
}

net GND {
    R1.2
    R2.2
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

/// What each named net asks for, sorted by name so a failure reads in order.
fn asked(world: &mut BoardWorld) -> Vec<(String, NetConstraints)> {
    let ids: Vec<(NetId, String)> = world
        .nets()
        .map(|(id, name)| (id, name.to_string()))
        .collect();
    let mut out: Vec<(String, NetConstraints)> = ids
        .into_iter()
        .filter_map(|(id, name)| world.net_constraints(id).map(|asks| (name, asks)))
        .collect();
    out.sort_by(|left, right| left.0.cmp(&right.0));
    out
}

#[test]
fn the_numbers_survive_and_the_word_does_not() {
    let mut before = load(CLASSED);
    let stated = asked(&mut before);
    assert_eq!(
        stated.len(),
        2,
        "both members of the class carry its figures: {stated:?}"
    );
    for (name, asks) in &stated {
        assert_eq!(asks.width.map(|w| w.to_mm()), Some(0.5), "{name}");
        assert_eq!(asks.clearance.map(|c| c.to_mm()), Some(0.3), "{name}");
    }

    let written = board_as_dsl(&mut before);
    assert!(
        !written.contains("netclass"),
        "a class nobody stated is a class nobody should read back:\n{written}"
    );
    assert!(
        written.contains("net GND [width 0.500000mm clearance 0.300000mm]")
            && written.contains("net VCC [width 0.500000mm clearance 0.300000mm]"),
        "each member states for itself what the class stated once:\n{written}"
    );

    // Checked the same either way, which is the whole claim.
    let mut after = load(&written);
    assert_eq!(asked(&mut after), stated, "written:\n{written}");
}
