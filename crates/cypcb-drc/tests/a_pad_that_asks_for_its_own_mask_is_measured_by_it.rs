//! A pad that asks for its own mask opening is measured by that one.
//!
//! `cargo test -p cypcb-drc --test a_pad_that_asks_for_its_own_mask_is_measured_by_it`
//!
//! The mask opening is the pad grown by the fabricator's expansion, and a pad
//! states its own when the part needs one - 124 pads in this repository's
//! KiCad files ask for 4 mil, against the 2 mil a house table gives by
//! default. Measured 2026-08-31: of this crate's 37 rules, `solder-mask-bridge`
//! is the only one that measures a mask opening at all, and it grew every pad
//! by the board's figure. So a pad asking for more than the default had its
//! web measured wider than the board would be made with, and the checker
//! passed a board whose openings touch.

use cypcb_core::{Nm, Point, Rect};
use cypcb_drc::rules::SolderMaskBridgeRule;
use cypcb_drc::{DesignRules, DrcRule};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// Two parts 2mm apart, each an 0402-shaped pair of pads asking for the
/// opening given here.
///
/// The facing pads leave 0.4mm between their copper, so the web is 0.4mm less
/// twice whatever opens the mask.
fn board(margin: Option<Nm>) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board("two".to_string(), (Nm::from_mm(30.0), Nm::from_mm(20.0)), 2);

    let pad = |x_mm: f64, number: &str| PadDef {
        number: number.to_string(),
        shape: PadShape::Rect,
        position: Point::from_mm(x_mm, 0.0),
        size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
        drill: None,
        slot: None,
        layers: vec![Layer::TopCopper, Layer::TopPaste, Layer::TopMask],
        mask_margin: margin,
    };

    let mut library = FootprintLibrary::new();
    library.register(Footprint {
        name: "chip".to_string(),
        description: String::new(),
        bounds: Rect::new(Point::ORIGIN, Point::ORIGIN),
        courtyard: Rect::new(Point::ORIGIN, Point::ORIGIN),
        silk: Vec::new(),
        pads: vec![pad(-0.5, "1"), pad(0.5, "2")],
    });
    world.set_footprints(library);

    for (refdes, x_mm) in [("R1", 10.0), ("R2", 12.0)] {
        world.spawn_component(
            RefDes::new(refdes),
            Value::new("10k"),
            Position::from_mm(x_mm, 10.0),
            Rotation::ZERO,
            FootprintRef::new("chip"),
            NetConnections::new(),
        );
    }
    world
}

#[test]
fn a_pad_asking_for_a_wider_opening_is_reported_where_a_silent_one_passes() {
    let rules = DesignRules::jlcpcb_2layer();

    // Silent pads: the board's 0.05mm each side leaves 0.3mm of web, which
    // clears the 0.10mm this fab can hold.
    let mut silent = board(None);
    assert!(
        SolderMaskBridgeRule.check(&mut silent, &rules).is_empty(),
        "the board's own figure leaves a web this fab can hold"
    );

    // The same board where every pad asks for 0.3mm: the openings run into
    // each other, and the exporter would make them that way.
    let mut asking = board(Some(Nm::from_mm(0.3)));
    let violations = SolderMaskBridgeRule.check(&mut asking, &rules);
    assert!(
        !violations.is_empty(),
        "the pads ask for an opening that eats the web and nothing was reported"
    );
    assert_eq!(
        violations[0].kind,
        cypcb_drc::ViolationKind::SolderMaskBridge
    );
}

#[test]
fn a_pad_asking_for_a_narrower_opening_is_not_reported_by_the_boards_figure() {
    // The fab opens the mask by 0.3mm, which would eat the web - but these
    // pads ask for 0.05mm and are made that way, so there is nothing to
    // report. A checker measuring the board's figure here would refuse a board
    // that is fine.
    let generous = DesignRules {
        solder_mask_expansion: Nm::from_mm(0.3),
        ..DesignRules::jlcpcb_2layer()
    };

    let mut silent = board(None);
    assert!(
        !SolderMaskBridgeRule
            .check(&mut silent, &generous)
            .is_empty(),
        "at 0.3mm a silent pad's opening does eat the web"
    );

    let mut asking = board(Some(Nm::from_mm(0.05)));
    assert!(
        SolderMaskBridgeRule
            .check(&mut asking, &generous)
            .is_empty(),
        "these pads are made with their own 0.05mm and the web survives"
    );
}
