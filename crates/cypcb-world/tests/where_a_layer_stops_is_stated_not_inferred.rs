//! Where a stackup layer stops, said in the language and read back out.
//!
//! `cargo test -p cypcb-world --test where_a_layer_stops_is_stated_not_inferred`
//!
//! A rigid-flex build is not one stack: a stiffener cannot run through the
//! ribbon it is bonded on to stiffen, and a coverlay is often over the ribbon
//! and nowhere else. Until `covers` and `outside` existed the language could
//! state the layer and not where it stopped, so everything downstream inferred
//! it - the 3D view read "a stiffener is not in the bend", which is true of a
//! stiffener and of nothing else.
//!
//! Three questions here, and the third is the one that keeps the other two
//! honest: the clause reaches the model, it comes back out of the writer word
//! for word, and a clause naming an area the design never declared is refused
//! rather than stored.

use cypcb_world::components::LayerCoverage;
use cypcb_world::dsl::board_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld, SyncResult};

/// A ribbon between two rigid ends, with three layers that stop somewhere.
const RIBBON: &str = r#"version 1

board wearable {
    size 60mm x 16mm
    layers 2
    stackup {
        coverlay 0.025mm material "Kapton" covers bend
        copper 0.5oz
        core 0.05mm material "Kapton" dk 3.4
        copper 0.5oz
        stiffener 0.2mm material "FR4" outside bend
    }
}

flex bend {
    bounds 22mm, 0mm to 38mm, 16mm
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
fn the_layer_that_stops_says_where_and_the_model_holds_it() {
    let (world, result) = sync(RIBBON);
    assert!(result.errors.is_empty(), "sync: {:?}", result.errors);

    let stackup = world.stackup().expect("the board states a stackup");
    let coverage: Vec<Option<&LayerCoverage>> = stackup
        .layers
        .iter()
        .map(|layer| layer.coverage.as_ref())
        .collect();

    assert_eq!(
        coverage[0],
        Some(&LayerCoverage::Only("bend".to_string())),
        "the coverlay is over the ribbon and nowhere else"
    );
    // The two copper layers and the core run the whole panel, and say nothing.
    assert_eq!(
        coverage[1], None,
        "copper is pressed across the whole panel"
    );
    assert_eq!(coverage[2], None, "so is the core");
    assert_eq!(coverage[3], None);
    assert_eq!(
        coverage[4],
        Some(&LayerCoverage::Outside("bend".to_string())),
        "the stiffener is everywhere but the ribbon"
    );

    // The question a 3D view asks, answered by the design rather than by a
    // rule about what a stiffener usually is.
    assert!(
        coverage[0]
            .expect("the coverlay states its area")
            .includes_region(),
        "the coverlay is in the bend"
    );
    assert!(
        !coverage[4]
            .expect("the stiffener states its area")
            .includes_region(),
        "the stiffener is not in the bend"
    );
}

#[test]
fn the_writer_gives_the_clause_back_word_for_word() {
    let (mut world, _) = sync(RIBBON);
    let written = board_as_dsl(&mut world);

    // The thickness is written in the writer's own form - six decimals of a
    // millimetre - so the clause is read off the end of the line rather than
    // matched against a whole one this test would have to keep in step.
    let coverlay = written
        .lines()
        .find(|line| line.trim_start().starts_with("coverlay"))
        .expect("the stackup comes back with its coverlay");
    assert!(
        coverlay.ends_with("covers bend"),
        "the coverlay comes back with its area: {coverlay}"
    );
    let stiffener = written
        .lines()
        .find(|line| line.trim_start().starts_with("stiffener"))
        .expect("and with its stiffener");
    assert!(
        stiffener.ends_with("outside bend"),
        "so does the stiffener: {stiffener}"
    );

    // And what comes out reads back in as the same board: a save that loses a
    // boundary is the defect this file exists to catch, and a save that writes
    // something the reader refuses is the same defect from the other side.
    // `board_as_dsl` writes the areas the board names too, so the version line
    // is all this needs.
    let source = format!("version 1\n\n{written}");
    let (again, result) = sync(&source);
    assert!(result.errors.is_empty(), "re-sync: {:?}", result.errors);
    let stackup = again.stackup().expect("the written board states a stackup");
    assert_eq!(
        stackup.layers[0].coverage,
        Some(LayerCoverage::Only("bend".to_string())),
        "the coverlay's area survives the second trip"
    );
    assert_eq!(
        stackup.layers[4].coverage,
        Some(LayerCoverage::Outside("bend".to_string())),
        "so does the stiffener's"
    );
}

#[test]
fn an_area_the_design_never_declared_is_refused() {
    // The same board with the ribbon named `ribbon` and the stiffener still
    // bounded by `bend`. A stackup layer bounded by a rectangle nobody drew is
    // a build nobody can press.
    let source = RIBBON.replace("flex bend {", "flex ribbon {");
    let (world, result) = sync(&source);

    assert_eq!(
        result.errors.len(),
        2,
        "both clauses name an area that is gone: {:?}",
        result.errors
    );
    let reported = format!("{:?}", result.errors);
    assert!(
        reported.contains("UnknownCoverageRegion"),
        "the fault is named rather than swallowed: {reported}"
    );
    assert!(
        reported.contains("ribbon"),
        "and the help says which areas the design does declare: {reported}"
    );

    // Refused rather than stored: a model holding a boundary against nothing
    // would hand every reader of it a question it cannot answer.
    let stackup = world.stackup().expect("the rest of the board still syncs");
    assert!(
        stackup.layers.iter().all(|layer| layer.coverage.is_none()),
        "no layer keeps a boundary that resolves to nothing"
    );
}
