//! The router says Complete only when every net pin on the board was routed,
//! whatever library it was handed.
//!
//! `cargo test -p cypcb-autoroute --test complete_covers_every_pin_on_the_board`
//!
//! Complete is decided over the nets `extract_ratsnest` builds, and that walk
//! steps over a part whose footprint the library it gets does not hold. Two
//! routing tests synchronised `examples/blink.cypcb` with one library and
//! routed it with a fresh one: `LED_0805`, which the design defines, was not
//! in it, the LED's pads left their nets, and the router said Complete over
//! two pins it never saw.
//!
//! No command does that - each routes with the library it synchronised with.
//! The router is what reports, so the router is where the count is kept.

use cypcb_autoroute::orchestrator::{extract_ratsnest, pins_the_library_cannot_place};
use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_parser::parse;
use cypcb_router::types::RoutingStatus;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

fn blink() -> (BoardWorld, FootprintLibrary) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/blink.cypcb");
    let source = std::fs::read_to_string(&path).expect("the example is on disk");
    let parsed = parse(&source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let sync = sync_ast_to_world(&parsed.value, &source, &mut world, &mut library);
    assert!(sync.errors.is_empty(), "{:?}", sync.errors);
    (world, library)
}

fn rules() -> PresetRuleSet {
    PresetRuleSet::new(RulesPreset::from_name("jlcpcb").unwrap())
}

/// The nets the router will report on.
fn ratsnest_nets(world: &mut BoardWorld, library: &FootprintLibrary) -> Vec<String> {
    extract_ratsnest(world, library)
        .into_iter()
        .map(|net| net.net_name)
        .collect()
}

#[test]
fn the_premise_the_design_defines_a_footprint_the_built_ins_do_not_hold() {
    let (mut world, library) = blink();
    assert!(library.get("LED_0805").is_some());
    assert!(FootprintLibrary::new().get("LED_0805").is_none());
    // The LED's two pins are what a fresh library cannot place.
    assert_eq!(pins_the_library_cannot_place(&mut world, &library), 0);
    assert_eq!(
        pins_the_library_cannot_place(&mut world, &FootprintLibrary::new()),
        2
    );
    // Without its LED pad `LED_ANODE` is one pad, nothing to join, and it
    // leaves the list Complete is decided over - with R3's pad on it.
    let anode = "LED_ANODE".to_string();
    assert!(ratsnest_nets(&mut world, &library).contains(&anode));
    assert!(!ratsnest_nets(&mut world, &FootprintLibrary::new()).contains(&anode));
}

#[test]
fn routed_with_the_library_it_was_synchronised_with_the_board_is_complete() {
    // The control: the same board and router, nothing left out, is Complete.
    // Without it the test below would pass on a board that never routes.
    let (mut world, library) = blink();
    let result = route_board(&mut world, &library, &rules(), &AutorouteConfig::default());
    assert_eq!(result.status, RoutingStatus::Complete);
}

#[test]
fn routed_with_a_library_that_lacks_a_part_it_is_not_complete() {
    let (mut world, _library) = blink();
    let fresh = FootprintLibrary::new();
    let result = route_board(&mut world, &fresh, &rules(), &AutorouteConfig::default());
    match result.status {
        RoutingStatus::Partial { unrouted_count } => assert!(
            unrouted_count >= 2,
            "the LED's two pins are unrouted: {unrouted_count}"
        ),
        other => panic!("two pins were never seen, and the router said {other:?}"),
    }
}
