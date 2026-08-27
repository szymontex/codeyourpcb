//! `diffpair USB { USB_DP USB_DM match }`: the pair asks, the tool folds.
//!
//! `cargo test -p cypcb-world --test the_pair_can_ask_to_be_matched`
//!
//! A differential pair only works if both halves arrive together, and the
//! checker has reported the difference since it was written without being able
//! to close it. `match` is the word that asks, and the meander is what answers.

use cypcb_world::components::trace::Trace;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

fn world_of(source: &str) -> (BoardWorld, Vec<String>) {
    let parsed = cypcb_parser::parse(source);
    assert!(
        !parsed.has_errors(),
        "the source parses: {:?}",
        parsed.errors
    );
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(
        result.errors.is_empty(),
        "the design syncs: {:?}",
        result.errors
    );
    let warnings = result
        .warnings
        .iter()
        .map(|warning| warning.message.clone())
        .collect();
    (world, warnings)
}

/// Copper on this net, in nanometres.
fn length_of(world: &mut BoardWorld, net: &str) -> i64 {
    let Some(id) = world.get_net(net) else {
        return 0;
    };
    let mut query = world.ecs_mut().query::<&Trace>();
    query
        .iter(world.ecs())
        .filter(|trace| trace.net_id == id)
        .map(|trace| trace.total_length().0)
        .sum()
}

/// Two headers, one half routed 4mm longer than the other.
fn board(matched: bool) -> String {
    let word = if matched { "\n    match" } else { "" };
    format!(
        "version 1\n\nboard b {{\n    size 40mm x 20mm\n    layers 2\n}}\n\n\
         component J1 connector \"PIN-HDR-1x2\" {{ at 4mm, 6mm }}\n\
         component J2 connector \"PIN-HDR-1x2\" {{ at 34mm, 6mm }}\n\
         component J3 connector \"PIN-HDR-1x2\" {{ at 4mm, 14mm }}\n\
         component J4 connector \"PIN-HDR-1x2\" {{ at 30mm, 14mm }}\n\n\
         net DP {{\n    J1.1\n    J2.1\n}}\n\n\
         net DM {{\n    J3.1\n    J4.1\n}}\n\n\
         diffpair USB {{\n    DP\n    DM{word}\n}}\n\n\
         trace DP {{\n    from J1.1\n    to J2.1\n    layer Top\n    width 0.2mm\n}}\n\n\
         trace DM {{\n    from J3.1\n    to J4.1\n    layer Top\n    width 0.2mm\n}}\n"
    )
}

#[test]
fn a_pair_that_says_nothing_is_left_alone() {
    let (mut world, warnings) = world_of(&board(false));
    let long = length_of(&mut world, "DP");
    let short = length_of(&mut world, "DM");
    assert!(
        long - short > 3_000_000,
        "the two halves start four millimetres apart: {long} against {short}"
    );
    assert!(
        warnings.is_empty(),
        "and nothing is said about it: {warnings:?}"
    );
}

#[test]
fn a_pair_that_asks_gets_its_halves_matched() {
    let (mut world, _) = world_of(&board(true));
    let long = length_of(&mut world, "DP");
    let short = length_of(&mut world, "DM");
    let skew = (long - short).abs();

    // The meander is quantised: its amplitude is three trace widths, so one
    // tooth adds six of them - 1.2mm on a 0.2mm track - and the match lands
    // within that rather than exactly. Measured here: 4mm of skew becomes
    // 0.8mm the other way, four teeth of 1.2mm against 4mm asked.
    let one_tooth = 6 * 200_000;
    assert!(
        skew <= one_tooth,
        "the halves are matched to within a tooth: {skew} nanometres left, \
         {long} against {short}"
    );
    assert!(
        skew < 4_000_000,
        "and the match is an improvement on the 4mm it started with: {skew}"
    );
}

#[test]
fn the_short_half_is_the_one_that_grows() {
    let (mut before, _) = world_of(&board(false));
    let (mut after, _) = world_of(&board(true));

    assert_eq!(
        length_of(&mut before, "DP"),
        length_of(&mut after, "DP"),
        "the long half is not touched"
    );
    assert!(
        length_of(&mut after, "DM") > length_of(&mut before, "DM"),
        "and the short one is the one that gains copper"
    );
}
