//! A CAM system stacks a board by what each Gerber says it is.
//!
//! `cargo test -p cypcb-export --test every_copper_file_says_which_layer_it_is`
//!
//! Gerber X2 puts the layer in the file: `%TF.FileFunction,Copper,L2,Inr%`.
//! It is how a fabricator's tooling knows which foil goes where, and on a
//! four-layer board this project wrote **L1 twice** - once for the top copper
//! and once for the first inner layer, then L2 for the second inner and L4 for
//! the bottom. L3 was never claimed by anything.
//!
//! The cause was two conventions meeting: `Layer::Inner` is zero-based
//! everywhere in this workspace - the DSL writes `inner1` and `sync.rs` reads
//! it as `Inner(0)` - and the export computed the Gerber number as `n + 1`,
//! which is the formula for a one-based index. Nothing caught it because no
//! test read the attribute out of a file, and a four-layer export is otherwise
//! indistinguishable from a correct one.
//!
//! The second thing checked here is the name. `copper.rs` and `mask.rs` read
//! the board's size and threw it away for the literal `"board"`, while
//! `silk.rs` and `outline.rs` used the design's own name, so one directory of
//! files described two projects.

use cypcb_core::Nm;
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::gerber::copper::export_copper_layer;
use cypcb_export::gerber::mask::{export_soldermask, MaskPasteConfig};
use cypcb_export::gerber::outline::export_outline;
use cypcb_export::gerber::silk::{export_silkscreen, SilkConfig};
use cypcb_export::gerber::Side;
use cypcb_world::components::Layer;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

const NAME: &str = "flight_controller";

fn board(copper_layers: u8) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        NAME.to_string(),
        (Nm::from_mm(40.0), Nm::from_mm(30.0)),
        copper_layers,
    );
    world
}

/// Every copper layer of a board of this size, in the order they are pressed.
fn copper_layers(count: u8) -> Vec<Layer> {
    let mut layers = vec![Layer::TopCopper];
    layers.extend((0..count.saturating_sub(2)).map(Layer::Inner));
    layers.push(Layer::BottomCopper);
    layers
}

/// The layer number a file claims: `Copper,L2,Inr` is 2.
///
/// The number alone, because the first version of this test compared whole
/// attributes and passed on the defect - `Copper,L1,Top` and `Copper,L1,Inr`
/// are different strings and the same layer, which is the fault itself.
fn layer_number(gerber: &str) -> u32 {
    let function = file_function(gerber);
    function
        .split(',')
        .nth(1)
        .and_then(|part| part.trim_start_matches('L').parse().ok())
        .unwrap_or_else(|| panic!("no layer number in {function}"))
}

/// The `TF.FileFunction` attribute out of an exported file.
fn file_function(gerber: &str) -> String {
    gerber
        .lines()
        .find_map(|line| line.split("TF.FileFunction,").nth(1))
        .map(|rest| rest.trim_end_matches('*').to_string())
        .unwrap_or_else(|| panic!("no file function in:\n{gerber}"))
}

fn export(world: &mut BoardWorld, layer: Layer) -> String {
    let library = FootprintLibrary::new();
    export_copper_layer(world, &library, layer, &CoordinateFormat::FORMAT_MM_2_6)
        .expect("the layer exports")
}

#[test]
fn a_four_layer_board_claims_l1_to_l4_once_each() {
    let mut world = board(4);

    let claimed: Vec<String> = copper_layers(4)
        .into_iter()
        .map(|layer| file_function(&export(&mut world, layer)))
        .collect();

    assert_eq!(
        claimed,
        vec![
            "Copper,L1,Top".to_string(),
            "Copper,L2,Inr".to_string(),
            "Copper,L3,Inr".to_string(),
            "Copper,L4,Bot".to_string(),
        ],
        "the four files have to describe four different layers"
    );
}

#[test]
fn no_two_copper_files_claim_the_same_layer() {
    // The property the numbering has to hold whatever the count: a repeated
    // number is a board a CAM system cannot stack, and it is what shipped.
    for count in [2u8, 4, 6, 8] {
        let mut world = board(count);
        let mut seen: Vec<u32> = copper_layers(count)
            .into_iter()
            .map(|layer| layer_number(&export(&mut world, layer)))
            .collect();

        let before = seen.len();
        seen.sort();
        seen.dedup();
        assert_eq!(
            seen.len(),
            before,
            "{count}-layer board: two files claim the same layer - {seen:?}"
        );
    }
}

#[test]
fn the_numbers_run_from_the_top_to_the_bottom_with_no_gaps() {
    // L3 was missing entirely from a four-layer export, which is the half of
    // the fault a duplicate check alone would not have caught.
    for count in [2u8, 4, 6, 8] {
        let mut world = board(count);
        let numbers: Vec<u32> = copper_layers(count)
            .into_iter()
            .map(|layer| layer_number(&export(&mut world, layer)))
            .collect();

        let expected: Vec<u32> = (1..=u32::from(count)).collect();
        assert_eq!(numbers, expected, "{count}-layer board");
    }
}

#[test]
fn every_file_names_the_same_board_and_it_is_the_design() {
    let mut world = board(4);
    let library = FootprintLibrary::new();
    let format = CoordinateFormat::FORMAT_MM_2_6;

    let mut files = vec![
        export(&mut world, Layer::TopCopper),
        export(&mut world, Layer::Inner(0)),
        export(&mut world, Layer::BottomCopper),
    ];
    files.push(
        export_soldermask(
            &mut world,
            &library,
            Side::Top,
            &format,
            &MaskPasteConfig::default(),
        )
        .expect("the mask exports"),
    );
    files.push(
        export_silkscreen(
            &mut world,
            &library,
            Side::Top,
            &format,
            &SilkConfig::default(),
        )
        .expect("silk exports"),
    );
    files.push(export_outline(&world, &format).expect("the outline exports"));

    let named: Vec<&str> = files
        .iter()
        .map(|gerber| {
            gerber
                .lines()
                .find_map(|line| line.strip_prefix("G04 Board: "))
                .map(|rest| rest.trim_end_matches('*'))
                .unwrap_or("no name at all")
        })
        .collect();

    assert!(
        named.iter().all(|name| *name == NAME),
        "one set of files, one board name - got {named:?}"
    );
}
