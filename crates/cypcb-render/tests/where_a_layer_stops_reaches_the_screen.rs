//! Where a layer stops reaches the thing that draws.
//!
//! `cargo test -p cypcb-render --test where_a_layer_stops_reaches_the_screen`
//!
//! The 3D view drew the ribbon of a rigid-flex board by applying a rule: a
//! stiffener is not in the bend. That is true of a stiffener and of nothing
//! else, so a coverlay ending before the rigid part - the ordinary case, since
//! the rigid ends take solder mask and a coverlay costs more - was a fact the
//! picture could not hold. The language says it now, and this is the wire it
//! travels on: if the clause does not reach the snapshot, the view is back to
//! inferring and nothing downstream can tell.

use cypcb_render::PcbEngine;

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

fn snapshot(source: &str) -> cypcb_render::BoardSnapshot {
    let mut engine = PcbEngine::new();
    let errors = engine.load_source(source);
    assert!(errors.is_empty(), "{errors}");
    engine.build_snapshot()
}

#[test]
fn the_clause_arrives_with_the_layer_it_bounds() {
    let ribbon = snapshot(RIBBON);
    let stackup = ribbon.stackup.expect("the board states a stackup");

    let coverlay = stackup
        .layers
        .iter()
        .find(|layer| layer.kind == "coverlay")
        .expect("the stack has a coverlay");
    assert_eq!(coverlay.coverage_region, "bend");
    assert!(
        coverlay.coverage_covers,
        "the coverlay is over the ribbon and nowhere else"
    );

    let stiffener = stackup
        .layers
        .iter()
        .find(|layer| layer.kind == "stiffener")
        .expect("and a stiffener");
    assert_eq!(stiffener.coverage_region, "bend");
    assert!(
        !stiffener.coverage_covers,
        "the stiffener is everywhere but the ribbon"
    );

    // A layer that says nothing runs the whole panel, and says so by saying
    // nothing: an empty region rather than a name the reader has to test.
    let copper = stackup
        .layers
        .iter()
        .find(|layer| layer.kind == "copper")
        .expect("and copper");
    assert_eq!(
        copper.coverage_region, "",
        "copper is pressed across the whole panel"
    );
}

#[test]
fn the_build_over_each_area_arrives_worked_out() {
    // The panel draws a column per area, and the filter that decides which
    // layers are in one lives on the model - the same question the handoff
    // document asks. Sending the answer rather than the ingredients is what
    // stops the screen and the fabricator's file disagreeing about one board.
    let ribbon = snapshot(RIBBON);
    let stackup = ribbon.stackup.expect("the board states a stackup");

    let names: Vec<&str> = stackup
        .areas
        .iter()
        .map(|area| area.name.as_str())
        .collect();
    assert_eq!(names, vec!["bend"], "the one area a layer stops at");

    let bend = &stackup.areas[0];
    // Coverlay, both foils and the core: everything but the stiffener, which
    // says `outside bend`.
    assert_eq!(bend.layers, vec![0, 1, 2, 3]);
    // 109_998nm rather than a round 110_000: the fixture states its foils in
    // ounces, and half an ounce is 17.499 micrometres rather than 17.5. The
    // figure is the design's own arithmetic, which is the point.
    assert_eq!(
        bend.thickness_nm,
        Some(109_998),
        "coverlay, both half-ounce foils and the core"
    );

    // And a board whose layers stop nowhere sends no areas at all, which is
    // the panel's own case for drawing one table without a heading.
    let plain = snapshot(
        "version 1\n\nboard slab {\n    size 20mm x 20mm\n    layers 2\n    stackup {\n        copper 0.035mm\n        core 1mm\n        copper 0.035mm\n    }\n}\n",
    );
    assert!(plain
        .stackup
        .expect("that board states one too")
        .areas
        .is_empty());
}
