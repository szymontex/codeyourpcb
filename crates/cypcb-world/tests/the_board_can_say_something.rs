//! `text "REV B" { at 5mm, 2mm }`: words a person puts on the board.
//!
//! `cargo test -p cypcb-world --test the_board_can_say_something`
//!
//! The legend has carried every part's designator since the stroke font moved
//! into the model. What a board could not say was anything else: a revision, a
//! label beside a connector, a warning. Item 9 of the KiCad parity audit.

use cypcb_world::components::BoardText;
use cypcb_world::dsl::board_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld, Layer};

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

fn texts(world: &mut BoardWorld) -> Vec<BoardText> {
    let mut query = world.ecs_mut().query::<&BoardText>();
    query.iter(world.ecs()).cloned().collect()
}

const BOARD: &str = "version 1\n\nboard b {\n    size 20mm x 10mm\n    layers 2\n}\n\ntext \"REV B\" {\n    at 5mm, 2mm\n    layer top\n    height 1.5mm\n}\n\ntext \"MADE HERE\" {\n    at 5mm, 8mm\n    layer bottom\n}\n";

#[test]
fn the_words_reach_the_model_where_they_were_put() {
    let mut world = world_of(BOARD);
    let mut found = texts(&mut world);
    found.sort_by(|a, b| a.content.cmp(&b.content));

    assert_eq!(found.len(), 2, "both lines are there: {found:?}");
    assert_eq!(found[1].content, "REV B");
    assert_eq!(found[1].position.x.0, 5_000_000);
    assert_eq!(found[1].position.y.0, 2_000_000);
    assert_eq!(found[1].layer, Layer::TopSilk);
    assert_eq!(found[1].height.0, 1_500_000, "the height it asked for");
}

#[test]
fn a_line_that_says_no_height_takes_the_legends_own() {
    let mut world = world_of(BOARD);
    let bottom = texts(&mut world)
        .into_iter()
        .find(|text| text.layer == Layer::BottomSilk)
        .expect("the bottom line is on the bottom");
    assert_eq!(
        bottom.height,
        BoardText::DEFAULT_HEIGHT,
        "so it matches every designator beside it"
    );
    assert_eq!(bottom.content, "MADE HERE");
}

#[test]
fn the_words_survive_being_written_down() {
    let mut world = world_of(BOARD);
    let written = board_as_dsl(&mut world);
    assert!(
        written.contains("text \"REV B\" {"),
        "the line comes back:\n{written}"
    );
    assert!(
        written.contains("    layer bottom"),
        "and so does the side it was on:\n{written}"
    );

    let mut again = world_of(&written);
    assert_eq!(
        texts(&mut again).len(),
        2,
        "a second reading finds the same two lines"
    );
}
