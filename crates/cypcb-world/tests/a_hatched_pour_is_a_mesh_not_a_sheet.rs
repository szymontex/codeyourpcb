//! A pour asked to be a mesh comes out as one.
//!
//! `cargo test -p cypcb-world --test a_hatched_pour_is_a_mesh_not_a_sheet`
//!
//! IPC-2223 asks for a hatched polygon in a flex area: a sheet of copper over
//! a fold takes the strain across an unbroken surface and cracks where the
//! fold begins. `hatch 0.3mm pitch 1mm` is the design asking for lines of
//! copper 0.3mm wide, a millimetre apart centre to centre, crossing both ways.
//!
//! Three things are checked here, and the third is the one that keeps the
//! other two from being decoration: the copper comes back as lines rather than
//! as a rectangle, each line is the width the design asked for, and the whole
//! thing survives being written down and read again.

use cypcb_core::pour::PourOptions;
use cypcb_world::components::{Hatch, Layer, Zone};
use cypcb_world::copper::fill_zone;
use cypcb_world::dsl::board_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

const MESHED: &str = r#"version 1

board panel {
    size 20mm x 20mm
    layers 2
}

net GND {
}

zone GND {
    bounds 2mm, 2mm to 12mm, 12mm
    layer top
    net GND
    hatch 0.3mm pitch 1mm
}
"#;

fn load(source: &str) -> (BoardWorld, FootprintLibrary) {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "parse: {:?}", parsed.errors);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "sync: {:?}", result.errors);
    (world, library)
}

fn the_pour(world: &mut BoardWorld) -> (Zone, Option<Hatch>) {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<(&Zone, Option<&Hatch>)>();
    query
        .iter(ecs)
        .find(|(zone, _)| zone.is_copper_pour())
        .map(|(zone, hatch)| (zone.clone(), hatch.copied()))
        .expect("the board states a pour")
}

#[test]
fn the_copper_comes_back_as_lines_rather_than_a_sheet() {
    let (mut world, library) = load(MESHED);
    let (zone, hatch) = the_pour(&mut world);
    assert!(hatch.is_some(), "the design asked for a mesh");

    let meshed = fill_zone(
        &mut world,
        &library,
        Layer::TopCopper,
        &zone,
        hatch,
        &PourOptions::default(),
    );
    let solid = fill_zone(
        &mut world,
        &library,
        Layer::TopCopper,
        &zone,
        None,
        &PourOptions::default(),
    );

    assert_eq!(
        solid.pieces.len(),
        1,
        "a solid fill of a bare zone is one rectangle"
    );
    assert!(
        meshed.pieces.len() > 10,
        "a 10mm square hatched at 1mm is twenty lines, not {}",
        meshed.pieces.len()
    );

    // Every line is the width the design asked for, in one direction or the
    // other. A mesh of lines the wrong width is copper the fab cannot etch and
    // a plane the designer did not ask for.
    for piece in &meshed.pieces {
        let width = piece.max.x.0 - piece.min.x.0;
        let height = piece.max.y.0 - piece.min.y.0;
        assert!(
            width == 300_000 || height == 300_000,
            "a line 0.3mm across one way: {width}nm by {height}nm"
        );
    }
}

#[test]
fn a_pour_that_asked_for_nothing_is_still_a_sheet() {
    // The half that keeps the filler honest: hatching every pour would cut
    // every plane on every board this project has ever filled.
    let (mut world, library) = load(&MESHED.replace("    hatch 0.3mm pitch 1mm\n", ""));
    let (zone, hatch) = the_pour(&mut world);
    assert!(hatch.is_none());

    let filled = fill_zone(
        &mut world,
        &library,
        Layer::TopCopper,
        &zone,
        hatch,
        &PourOptions::default(),
    );
    assert_eq!(filled.pieces.len(), 1, "one rectangle, as before");
}

#[test]
fn the_mesh_survives_being_written_down() {
    let (mut world, _) = load(MESHED);
    let written = board_as_dsl(&mut world);
    let line = written
        .lines()
        .find(|line| line.trim_start().starts_with("hatch "))
        .unwrap_or_else(|| panic!("the mesh comes back out of the writer:\n{written}"));
    assert!(line.contains("pitch"), "with both figures: {line}");

    let (mut again, _) = load(&format!("version 1\n\n{written}"));
    let (_, hatch) = the_pour(&mut again);
    assert_eq!(
        hatch,
        Some(Hatch {
            width: cypcb_core::Nm::from_mm(0.3),
            pitch: cypcb_core::Nm::from_mm(1.0),
        }),
        "and reads back as the same mesh"
    );
}
