//! A pad that asks for its own mask opening gets it.
//!
//! `cargo test -p cypcb-export --test a_pad_asks_for_its_own_mask_opening`
//!
//! The solder mask opening runs past the copper by one figure from the
//! fabricator's table, and that is right for nearly every pad on a board. A
//! pad states its own when the part needs one: KiCad writes
//! `(solder_mask_margin 0.1016)` - 4 mil - inside a through-hole connector's
//! pads so the mask does not creep onto copper a hand-soldered joint has to
//! wet.
//!
//! Measured 2026-08-31 across the KiCad files in this repository: 124 pads of
//! 2623 state one, and every one of them was exported with the board's figure
//! instead, because nothing carried theirs.

use cypcb_core::{Nm, Point, Rect};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::gerber::{export_soldermask, MaskPasteConfig, Side};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// A board with one 1mm round pad, asking for the mask opening given here.
fn mask_of(margin: Option<Nm>) -> String {
    let mut world = BoardWorld::new();
    world.set_board(
        "one_pad".to_string(),
        (Nm::from_mm(20.0), Nm::from_mm(20.0)),
        2,
    );

    let mut library = FootprintLibrary::new();
    library.register(Footprint {
        name: "pad".to_string(),
        description: String::new(),
        bounds: Rect::new(Point::ORIGIN, Point::ORIGIN),
        courtyard: Rect::new(Point::ORIGIN, Point::ORIGIN),
        silk: Vec::new(),
        pads: vec![PadDef {
            number: "1".to_string(),
            shape: PadShape::Circle,
            position: Point::ORIGIN,
            size: (Nm::from_mm(1.0), Nm::from_mm(1.0)),
            drill: None,
            slot: None,
            layers: vec![Layer::TopCopper],
            mask_margin: margin,
        }],
    });

    world.spawn_component(
        RefDes::new("J1"),
        Value::new(""),
        Position::from_mm(10.0, 10.0),
        Rotation::ZERO,
        FootprintRef::new("pad"),
        NetConnections::new(),
    );

    export_soldermask(
        &mut world,
        &library,
        Side::Top,
        &CoordinateFormat::FORMAT_MM_2_6,
        &MaskPasteConfig::default(),
    )
    .expect("the mask exports")
}

#[test]
fn the_boards_figure_is_what_a_pad_that_asks_for_nothing_gets() {
    // 1mm of copper, 0.05mm of expansion each side: a 1.1mm opening.
    let gerber = mask_of(None);
    assert!(
        gerber.contains("1.100000"),
        "the opening is not the board's 0.05mm:\n{gerber}"
    );
}

#[test]
fn a_pad_that_asks_for_its_own_opening_gets_that_one() {
    // The 4 mil a KiCad connector footprint asks for: 1mm of copper and
    // 0.1016mm each side is a 1.2032mm opening, and the board's figure would
    // have made it 1.1mm.
    let gerber = mask_of(Some(Nm::from_mm(0.1016)));
    assert!(
        gerber.contains("1.203200"),
        "the pad's own 0.1016mm is not in the mask:\n{gerber}"
    );
    assert!(
        !gerber.contains("1.100000"),
        "the board's figure was used beside the pad's own:\n{gerber}"
    );
}
