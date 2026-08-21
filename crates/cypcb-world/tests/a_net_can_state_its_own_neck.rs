//! A net can say how thin its copper may get on the way into a pad.
//!
//! `cargo test -p cypcb-world --test a_net_can_state_its_own_neck`
//!
//! The request this feature came from is a netclass: `netclass Mains [current
//! 10A]` gives copper millimetres wide, correctly, and a 2.54mm pad pitch has
//! nowhere to put it. `neck 0.8mm for 4mm` landed on `trace` blocks only, so
//! the case that prompted it could not be written - a design would have had to
//! repeat the neck on every trace of the net, and a net that is autorouted has
//! no trace block to repeat it on.
//!
//! This is the reader and the model. Applying it to autorouted copper is the
//! next step and is deliberately not here: a constraint that reaches the model
//! and changes nothing is still worth landing on its own, because the two
//! halves fail in different places and a test that spans both cannot say which
//! one broke.

use cypcb_core::Nm;
use cypcb_world::components::trace::TraceNeck;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

fn load(source: &str) -> BoardWorld {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "parse: {:?}", parsed.errors);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "sync: {:?}", result.errors);
    world
}

/// A board whose net carries `constraints`, written between the brackets.
fn board(net_constraints: &str, class: Option<&str>) -> BoardWorld {
    let class = class
        .map(|c| format!("netclass Power [{c}] {{\n    SIG\n}}\n\n"))
        .unwrap_or_default();
    load(&format!(
        "version 1\n\n\
         board t {{\n    size 40mm x 20mm\n    layers 2\n}}\n\n\
         component R1 resistor \"0402\" {{\n    value 10kohm\n    at 10mm, 10mm\n}}\n\n\
         component R2 resistor \"0402\" {{\n    value 10kohm\n    at 30mm, 10mm\n}}\n\n\
         {class}net SIG [{net_constraints}] {{\n    R1.2\n    R2.1\n}}\n"
    ))
}

fn neck_of(world: &mut BoardWorld) -> Option<TraceNeck> {
    let net = world
        .nets()
        .find(|(_, name)| *name == "SIG")
        .map(|(id, _)| id)
        .expect("the net is interned");
    world.net_constraints(net).and_then(|c| c.neck)
}

#[test]
fn a_net_states_a_neck_and_the_model_carries_it() {
    let mut world = board("width 2mm, neck 0.8mm for 4mm", None);
    assert_eq!(
        neck_of(&mut world),
        Some(TraceNeck {
            width: Nm::from_mm(0.8),
            length: Nm::from_mm(4.0),
        })
    );
}

#[test]
fn a_netclass_states_it_for_every_net_in_the_class() {
    // The shape the owner asked for: said once, on the class.
    let mut world = board("width 2mm", Some("current 10A, neck 0.8mm for 4mm"));
    assert_eq!(
        neck_of(&mut world),
        Some(TraceNeck {
            width: Nm::from_mm(0.8),
            length: Nm::from_mm(4.0),
        })
    );
}

#[test]
fn a_nets_own_neck_beats_the_class_it_is_in() {
    // Same rule the other four constraints follow: the class states a default
    // and the net overrides it. This is checked because the merge is written
    // field by field, and a field added without a line there is carried by the
    // class and dropped by the net - silently.
    let mut world = board("neck 0.5mm for 2mm", Some("neck 0.8mm for 4mm"));
    assert_eq!(
        neck_of(&mut world),
        Some(TraceNeck {
            width: Nm::from_mm(0.5),
            length: Nm::from_mm(2.0),
        })
    );
}

#[test]
fn a_net_that_says_nothing_has_no_neck() {
    let mut world = board("width 2mm", None);
    assert_eq!(neck_of(&mut world), None);
}

#[test]
fn a_neck_with_no_length_is_refused() {
    // A width with no length is a second width, which is why the grammar makes
    // `for` compulsory in both places a neck can be written.
    let parsed = cypcb_parser::parse(
        "version 1\n\n\
         board t {\n    size 40mm x 20mm\n    layers 2\n}\n\n\
         net SIG [neck 0.8mm] {\n}\n",
    );
    assert!(!parsed.errors.is_empty(), "`neck 0.8mm` was accepted alone");
}
