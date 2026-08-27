//! `zone GND { ... stitch 5mm }`: the pour states the rule, the tool places the vias.
//!
//! `cargo test -p cypcb-world --test the_pour_states_its_stitching`
//!
//! A plane on a two-layer board is two planes, and a field of vias is what
//! makes them one. Where the vias go is arithmetic; whether a board wants them
//! is a decision, so it belongs in the design beside the pour it stitches.

use cypcb_world::components::trace::Via;
use cypcb_world::components::Stitched;
use cypcb_world::dsl::board_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

fn world_of(source: &str) -> BoardWorld {
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
    world
}

fn via_count(world: &mut BoardWorld) -> usize {
    let mut query = world.ecs_mut().query::<&Via>();
    query.iter(world.ecs()).count()
}

const STITCHED: &str = "version 1\n\nboard b {\n    size 20mm x 20mm\n    layers 2\n}\n\nnet GND {\n}\n\nzone GND {\n    bounds 2mm, 2mm to 18mm, 18mm\n    layer all\n    net GND\n    stitch 4mm\n}\n";

const PLAIN: &str = "version 1\n\nboard b {\n    size 20mm x 20mm\n    layers 2\n}\n\nnet GND {\n}\n\nzone GND {\n    bounds 2mm, 2mm to 18mm, 18mm\n    layer all\n    net GND\n}\n";

#[test]
fn a_stitched_pour_gets_its_vias_and_a_plain_one_gets_none() {
    let mut stitched = world_of(STITCHED);
    let mut plain = world_of(PLAIN);

    // A 16mm square at a 4mm pitch, starting half a pitch in: four by four.
    assert_eq!(via_count(&mut stitched), 16, "the field is four by four");
    assert_eq!(
        via_count(&mut plain),
        0,
        "a pour that does not ask for stitching gets no holes drilled in it"
    );
}

#[test]
fn the_vias_are_marked_as_the_tools_rather_than_the_designers() {
    let mut world = world_of(STITCHED);
    let marked = {
        let mut query = world.ecs_mut().query::<(&Via, &Stitched)>();
        query.iter(world.ecs()).count()
    };
    assert_eq!(marked, 16, "every one of them came from the rule");
}

#[test]
fn a_via_keeps_off_a_pad_even_on_its_own_net() {
    // Electrically a via on a ground pad is nothing. In a reflow oven it is
    // solder wicking down the barrel and a starved joint above it, which is
    // why every tool that places these keeps them off pads. Measured on
    // `examples/stitched-plane.cypcb`, where the connector's ground pin sits
    // inside the pour: with only foreign copper blocked, the checker read
    // `J1 <-> via: 0.00mm actual, 0.13mm required`.
    let source = "version 1\n\nboard b {\n    size 20mm x 20mm\n    layers 2\n}\n\ncomponent J1 connector \"PIN-HDR-1x2\" {\n    at 9mm, 9mm\n}\n\nnet GND {\n    J1.1\n    J1.2\n}\n\nzone GND {\n    bounds 2mm, 2mm to 18mm, 18mm\n    layer all\n    net GND\n    stitch 4mm\n}\n";
    let mut world = world_of(source);

    let vias: Vec<(i64, i64)> = {
        let mut query = world.ecs_mut().query::<&Via>();
        query
            .iter(world.ecs())
            .map(|via| (via.position.x.0, via.position.y.0))
            .collect()
    };
    assert!(!vias.is_empty(), "the pour is still stitched");

    // Both pins are on the pour's own net and both are inside it. No via may
    // sit within a millimetre of either, which is more than the ring plus the
    // clearance and enough to catch an overlap.
    for (x, y) in &vias {
        for pin_x in [9_000_000 - 1_270_000, 9_000_000 + 1_270_000] {
            let dx = (x - pin_x).abs();
            let dy = (y - 9_000_000).abs();
            assert!(
                dx > 1_000_000 || dy > 1_000_000,
                "a via at ({x}, {y}) is on the connector's own pad at ({pin_x}, 9000000)"
            );
        }
    }
}

#[test]
fn the_rule_survives_being_written_down_and_the_vias_do_not() {
    // The point of the marker. A stitched pour writes one line; writing the
    // vias as copper would turn a rule into a hundred holes and stitch the
    // stitching on the next trip through.
    let mut world = world_of(STITCHED);
    let written = board_as_dsl(&mut world);
    // The same six decimals every dimension this writer emits carries, so a
    // pitch reads like the bounds above it rather than like a different tool.
    assert!(
        written.contains("    stitch 4.000000mm"),
        "the rule comes back:\n{written}"
    );
    assert!(
        !written.contains("via "),
        "and the vias it produced do not:\n{written}"
    );

    let mut again = world_of(&written);
    assert_eq!(
        via_count(&mut again),
        16,
        "so a second reading places the same field, not a second one"
    );
}
