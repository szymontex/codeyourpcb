//! A solder paste stencil is a steel sheet, and it can tear.
//!
//! `cargo test -p cypcb-drc --test the_stencil_has_to_survive_being_made`
//!
//! Every fab preset in this project publishes a `min_paste_clearance` and
//! until now **nothing read it**: the field appeared thirteen times inside
//! `cypcb-rules` and nowhere else in the workspace. So a board could put two
//! SMD pads close enough that the steel between their stencil apertures tears,
//! the two openings become one, and the parts bridge with solder on reflow -
//! and every check passed.
//!
//! The twin of the solder mask bridge rule: same geometry, a different sheet.
//! A paste aperture is the pad itself, because `paste_reduction` is 0.0 and a
//! reduction is a stencil design decision rather than a number a fabricator
//! publishes.

use cypcb_core::{Nm, Point};
use cypcb_drc::{run_drc, Preset, PresetRules, ViolationKind};
use cypcb_world::components::{FootprintRef, NetConnections, Position, RefDes, Rotation, Value};
use cypcb_world::components::{Layer, PadShape};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// Two SMD pads of one part, `gap` apart edge to edge.
fn board_with_pad_gap(gap_mm: f64, drilled: bool) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        "stencil".to_string(),
        (Nm::from_mm(20.0), Nm::from_mm(20.0)),
        2,
    );

    // 1mm square pads, centred either side of the part origin.
    let half = 0.5;
    let offset = half + gap_mm / 2.0;
    let pad = |number: &str, x: f64| PadDef {
        number: number.to_string(),
        shape: PadShape::Rect,
        position: Point::from_mm(x, 0.0),
        size: (Nm::from_mm(1.0), Nm::from_mm(1.0)),
        drill: drilled.then(|| Nm::from_mm(0.4)),
        slot: None,
        layers: vec![Layer::TopCopper, Layer::TopMask, Layer::TopPaste],
    };

    let mut library = FootprintLibrary::new();
    let base = library
        .get("0402")
        .expect("the library has an 0402")
        .clone();
    library.register_design(Footprint {
        name: "pair".to_string(),
        pads: vec![pad("1", -offset), pad("2", offset)],
        ..base
    });
    world.set_footprints(library);

    world.spawn_component(
        RefDes::new("U1"),
        Value::new(""),
        Position::from_mm(10.0, 10.0),
        Rotation::ZERO,
        FootprintRef::new("pair"),
        NetConnections::new(),
    );
    world
}

fn paste_faults(world: &mut BoardWorld) -> Vec<String> {
    run_drc(world, &Preset::JlcpcbStandard2Layer.rules())
        .violations
        .into_iter()
        .filter(|violation| violation.kind == ViolationKind::PasteClearance)
        .map(|violation| violation.message)
        .collect()
}

/// JLCPCB publishes 0.127mm.
const PUBLISHED: f64 = 0.127;

#[test]
fn a_web_thinner_than_the_fab_allows_is_reported() {
    let faults = paste_faults(&mut board_with_pad_gap(0.1, false));

    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(
        faults[0].contains("0.100mm") && faults[0].contains("0.127mm"),
        "the message carries what it is and what it has to be: {}",
        faults[0]
    );
}

#[test]
fn a_web_the_fab_allows_says_nothing() {
    // The control, a hair over the published minimum.
    let faults = paste_faults(&mut board_with_pad_gap(PUBLISHED + 0.01, false));

    assert_eq!(faults, Vec::<String>::new());
}

#[test]
fn a_through_hole_pad_has_no_stencil_aperture() {
    // Wave-soldered or hand-soldered: there is no opening to tear, so the
    // same geometry that fires for SMD has to stay silent here.
    let faults = paste_faults(&mut board_with_pad_gap(0.1, true));

    assert_eq!(faults, Vec::<String>::new());
}

#[test]
fn the_rule_is_quiet_on_the_ordinary_board() {
    // An 0402's own two pads sit 0.4mm apart, comfortably over any fab's
    // number - a rule that fires on every chip resistor is a rule people turn
    // off.
    let mut world = BoardWorld::new();
    world.set_board(
        "plain".to_string(),
        (Nm::from_mm(20.0), Nm::from_mm(20.0)),
        2,
    );
    world.spawn_component(
        RefDes::new("R1"),
        Value::new("10k"),
        Position::from_mm(10.0, 10.0),
        Rotation::ZERO,
        FootprintRef::new("0402"),
        NetConnections::new(),
    );

    assert_eq!(paste_faults(&mut world), Vec::<String>::new());
}

#[test]
fn every_preset_publishes_a_number_for_this() {
    // The rule reads the fab's own figure, so a preset that states none would
    // check against zero and never fire.
    for preset in Preset::all() {
        assert!(
            preset.rules().min_paste_clearance > Nm(0),
            "{} states no paste clearance",
            preset.name()
        );
    }
}
