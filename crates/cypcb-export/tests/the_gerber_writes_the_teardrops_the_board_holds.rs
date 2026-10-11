//! The copper file writes the teardrops the board holds, and no others.
//!
//! `cargo test -p cypcb-export --test the_gerber_writes_the_teardrops_the_board_holds`
//!
//! The writer used to work the fillets out of the tracks as it went, and the
//! checker never saw them. A teardrop is copper in the board now, built once,
//! and the writer is one reader of it among the others. What is held here:
//! a teardrop that is in the board reaches the file of its layer, one that is
//! not does not, and the file of another layer does not get it.

use cypcb_core::{Nm, Point};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::gerber::export_copper_layer;
use cypcb_export::stamp::Stamp;
use cypcb_world::components::{Layer, NetId};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::teardrop::TeardropCopper;
use cypcb_world::BoardWorld;

fn board() -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board("t".to_string(), (Nm::from_mm(20.0), Nm::from_mm(20.0)), 2);
    (world, FootprintLibrary::new())
}

fn copper(world: &mut BoardWorld, library: &FootprintLibrary, layer: Layer) -> String {
    export_copper_layer(
        world,
        library,
        layer,
        &CoordinateFormat::FORMAT_MM_2_6,
        Stamp::UNIX_EPOCH,
    )
    .expect("the layer exports")
}

fn region(points: &[(f64, f64)]) -> String {
    let mut text = String::from("G36*\n");
    for (i, (x, y)) in points.iter().chain(points.first()).enumerate() {
        let command = if i == 0 { "D02" } else { "D01" };
        text.push_str(&format!(
            "X{}Y{}{}*\n",
            (x * 1_000_000.0).round() as i64,
            (y * 1_000_000.0).round() as i64,
            command
        ));
    }
    text.push_str("G37*\n");
    text
}

#[test]
fn a_teardrop_in_the_board_is_a_region_in_its_layers_file() {
    let triangle = [(2.0, 2.0), (3.0, 2.0), (3.0, 3.0)];
    let (mut world, library) = board();
    world.spawn_entity((
        TeardropCopper {
            outline: triangle
                .iter()
                .map(|(x, y)| Point::from_mm(*x, *y))
                .collect(),
            layer: Layer::TopCopper,
            order: 0,
            land: None,
        },
        NetId::new(0),
    ));
    let top = copper(&mut world, &library, Layer::TopCopper);
    assert!(
        top.contains(&region(&triangle)),
        "the held teardrop is in the top copper:\n{top}"
    );
    let bottom = copper(&mut world, &library, Layer::BottomCopper);
    assert!(
        !bottom.contains("G36*"),
        "and not in the bottom copper:\n{bottom}"
    );
}

#[test]
fn control_a_board_holding_none_writes_none() {
    let (mut world, library) = board();
    assert!(!copper(&mut world, &library, Layer::TopCopper).contains("G36*"));
}
