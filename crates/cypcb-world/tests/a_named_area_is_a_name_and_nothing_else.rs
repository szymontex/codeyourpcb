//! An area with a name and nothing else.
//!
//! `cargo test -p cypcb-world --test a_named_area_is_a_name_and_nothing_else`
//!
//! `covers` and `outside` take an area the design names, and for one week the
//! only thing a design could name was the ribbon of a rigid-flex board. That
//! left the ordinary order unsayable: a stiffener is bonded under **one**
//! rigid end, and a build with a second core on one end only is a thing
//! fabricators quote every day. `outside bend` says something else, and
//! something wrong - that the stiffener is under both ends.
//!
//! So `region` exists, and what it has to be is nothing: not poured, not kept
//! out of, not bending. Three questions here - a layer can point at one, the
//! writer gives it back, and it carries no copper meaning of its own.

use cypcb_world::components::zone::ZoneKind;
use cypcb_world::components::LayerCoverage;
use cypcb_world::dsl::board_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld, SyncResult};

/// A ribbon, a rigid end with a name, and a stiffener under that end only.
const ONE_END: &str = r#"version 1

board wearable {
    size 60mm x 16mm
    layers 2
    stackup {
        coverlay 0.025mm material "Kapton" covers bend
        copper 0.5oz
        core 0.05mm material "Kapton" dk 3.4
        copper 0.5oz
        stiffener 0.2mm material "FR4" covers connector_end
    }
}

flex bend {
    bounds 22mm, 0mm to 38mm, 16mm
    layer all
}

region connector_end {
    bounds 0mm, 0mm to 22mm, 16mm
    layer all
}
"#;

fn sync(source: &str) -> (BoardWorld, SyncResult) {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "parse: {:?}", parsed.errors);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    (world, result)
}

#[test]
fn a_layer_can_stop_at_an_end_rather_than_at_the_ribbon() {
    let (world, result) = sync(ONE_END);
    assert!(result.errors.is_empty(), "sync: {:?}", result.errors);

    let stackup = world.stackup().expect("the board states a stackup");
    let stiffener = stackup
        .layers
        .iter()
        .find(|layer| layer.kind.as_str() == "stiffener")
        .expect("the stack has a stiffener");
    assert_eq!(
        stiffener.coverage,
        Some(LayerCoverage::Only("connector_end".to_string())),
        "the stiffener is under the end J1 is soldered to, and under no other"
    );

    // The point of the word: this is not the ribbon, and the design says which
    // of the two ends it is.
    let coverlay = stackup
        .layers
        .iter()
        .find(|layer| layer.kind.as_str() == "coverlay")
        .expect("and a coverlay");
    assert_ne!(
        coverlay.coverage, stiffener.coverage,
        "the two layers stop at different areas"
    );
}

#[test]
fn a_named_area_is_not_copper_and_not_a_keepout() {
    let (mut world, _) = sync(ONE_END);

    let named: Vec<(String, ZoneKind)> = world
        .zones()
        .into_iter()
        .map(|(_, zone)| (zone.name.clone().unwrap_or_default(), zone.kind))
        .collect();

    let end = named
        .iter()
        .find(|(name, _)| name == "connector_end")
        .expect("the design's named area reaches the model");
    assert_eq!(
        end.1,
        ZoneKind::Region,
        "a region is its own kind: called a keepout it would clear copper, \
         called a pour it would fill some"
    );

    // And it pours to nothing, which is what stops the rest of the project
    // treating it as copper: every filler, plotter and rule here asks for the
    // kind it wants.
    let (_, zone) = world
        .zones()
        .into_iter()
        .find(|(_, zone)| zone.name.as_deref() == Some("connector_end"))
        .expect("the same area again");
    assert!(zone.net.is_none(), "a named area is poured to no net");
    assert!(zone.is_region(), "and says so when asked");
    assert!(!zone.is_keepout() && !zone.is_copper_pour() && !zone.is_flex());
}

#[test]
fn the_writer_gives_the_area_back_as_a_region() {
    let (mut world, _) = sync(ONE_END);
    let written = board_as_dsl(&mut world);

    assert!(
        written.contains("region connector_end {"),
        "the area comes back as a region rather than as a pour:\n{written}"
    );

    // Read what was written: a save that turns a named area into a zone gives
    // the board a copper pour nobody asked for, which is the defect the flex
    // region had before it was named in the writer.
    let (again, result) = sync(&format!("version 1\n\n{written}"));
    assert!(result.errors.is_empty(), "re-sync: {:?}", result.errors);
    let stackup = again.stackup().expect("the written board states a stackup");
    assert_eq!(
        stackup.layers[4].coverage,
        Some(LayerCoverage::Only("connector_end".to_string())),
        "and the layer that points at it still points at it"
    );
}
