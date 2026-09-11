//! Copper on a layer this importer has no word for is named, not moved.
//!
//! `cargo test -p cypcb-kicad --test copper_on_an_unknown_layer_is_refused -- --nocapture`
//!
//! The layer was read with `parse_layer_name(name).unwrap_or(Layer::TopCopper)`,
//! so a name this parser did not know put the copper on the top layer and said
//! nothing. That is the mistake this file already argues against, in its note on
//! `coordinate`: putting a part 50 mm from where the file says is worse than
//! refusing to read the file at all. Copper on the wrong layer is the same
//! mistake over a shorter distance, and a via whose span is guessed is worse
//! still, because `ViaSpanRule` grades a board on exactly that span.
//!
//! The remedy is the one the same file already uses for a pour it will not
//! approximate: refuse the feature and name it, so the board that arrives
//! without a trace says so rather than arriving with the trace somewhere else.
//!
//! The fixture is written here rather than by KiCad, and the layer name in it -
//! `In7.Signal` - is one this parser has no word for. That is all the test
//! needs: it is about what this importer does with a name it does not know, not
//! about which names KiCad emits.

use std::path::{Path, PathBuf};

use cypcb_world::components::trace::Trace;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/hand")
        .join(name)
}

#[test]
fn a_segment_on_an_unknown_layer_is_left_out_and_named() {
    let parsed =
        cypcb_kicad::parse_kicad_pcb(&fixture("a_layer_this_importer_does_not_know.kicad_pcb"))
            .expect("the fixture parses");

    for refusal in &parsed.metadata.track_refusals {
        println!("refused: {refusal}");
    }

    // Two of the three copper features name a layer this parser does not know:
    // one segment and the via. The third is on F.Cu and has to arrive.
    assert_eq!(
        parsed.metadata.track_refusals.len(),
        2,
        "one segment and one via name an unknown layer, and both have to be \
         reported: {:?}",
        parsed.metadata.track_refusals
    );
    // Every one of them, not any of them. The first version of this asserted
    // `any`, and a mutation that stripped the layer name out of the segment's
    // message passed - because the via's message still carried it. A reader
    // given one refusal that names the layer and one that does not cannot act
    // on the second.
    for why in &parsed.metadata.track_refusals {
        assert!(
            why.contains("In7.Signal"),
            "every refusal has to name the layer, or the reader cannot act on \
             it: {why}"
        );
    }

    // The control, and the whole point: the copper is gone, not relocated.
    let routes = parsed.reference_routes.expect("the good segment arrives");
    println!("{} segment(s) carried", routes.routes.len());
    assert_eq!(
        routes.routes.len(),
        1,
        "only the segment on a known layer comes through"
    );
    assert!(
        routes.vias.is_empty(),
        "the via spans a layer this importer cannot name, so it is refused too"
    );
}

#[test]
fn a_board_whose_layers_are_all_known_refuses_nothing() {
    // The positive control on the other side. Without it the assertion above
    // would pass against a parser that refused every track it ever read.
    let parsed = cypcb_kicad::parse_kicad_pcb(&fixture("one_straight_one_curved.kicad_pcb"))
        .expect("the fixture parses");

    println!(
        "{} refusal(s) on a board with no unknown layer",
        parsed.metadata.track_refusals.len()
    );
    assert!(
        parsed.metadata.track_refusals.is_empty(),
        "nothing on this board names a layer the importer does not know: {:?}",
        parsed.metadata.track_refusals
    );

    // And its copper is all there, which is what makes the emptiness meaningful.
    let routes = parsed.reference_routes.expect("the board carries copper");
    assert_eq!(routes.routes.len(), 1, "one straight segment");
    assert_eq!(routes.vias.len(), 1, "one via");
    let world = parsed.world;
    let _ = std::mem::size_of_val(&world);
}

#[test]
fn the_arc_path_refuses_the_same_way() {
    // The arc parser is a separate function with its own copy of the layer
    // read, so it needs its own case: a fix applied to one and not the other
    // is the shape of defect this project has already met twice.
    let parsed = cypcb_kicad::parse_kicad_pcb(&fixture("one_straight_one_curved.kicad_pcb"))
        .expect("the fixture parses");
    let mut world = parsed.world;
    let arcs = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query.iter(ecs).count()
    };
    println!("{arcs} arc(s) spawned from a board with no unknown layer");
    assert_eq!(arcs, 1, "the arc on a known layer is spawned as copper");
}
