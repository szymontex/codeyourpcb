//! A hole can be legal on a thin board and unbuildable on a thick one.
//!
//! `cargo test -p cypcb-drc --test a_hole_too_deep_to_plate`
//!
//! Plating a through hole is chemistry: copper is pulled down the barrel out
//! of solution, and past some depth-to-width ratio the solution stops
//! refreshing in the middle. The board comes back with a barrel that is thin
//! or open in a place no one can see. Every fab publishes the ratio it still
//! plates, and until the stackup reached the model there was no depth to
//! divide by, so the number sat in the tables unread.
//!
//! `min_via_drill` does not answer this. That rule is a floor on the drill
//! alone: the same 0.3mm via is comfortable through 1.6mm and impossible
//! through 3.2mm, and only one of those boards can be built. The distinction
//! is the whole point of the rule, and the first test below is exactly it.

use cypcb_core::{Nm, Point};
use cypcb_drc::{run_drc, Preset, PresetRules, ViolationKind};
use cypcb_world::components::trace::Via;
use cypcb_world::components::{
    FootprintRef, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::{BoardWorld, Stackup, StackupLayer, StackupLayerKind};

/// A board `thickness_mm` thick, stated the way a design states it.
///
/// `None` leaves the design silent, which is the common case: the board is
/// then whatever the fab builds as standard.
fn stackup_of(thickness_mm: f64) -> Stackup {
    // Two copper foils and one core between them - the thickness a fab
    // presses is nearly all core, so the core carries the number.
    Stackup {
        layers: vec![
            StackupLayer::new(StackupLayerKind::Copper, Some(Nm::from_mm(0.035))),
            StackupLayer::new(
                StackupLayerKind::Core,
                Some(Nm::from_mm(thickness_mm - 0.07)),
            ),
            StackupLayer::new(StackupLayerKind::Copper, Some(Nm::from_mm(0.035))),
        ],
        ..Stackup::default()
    }
}

/// A 20x20 two-layer board with one via of `drill_mm`.
fn board_with_via(drill_mm: f64, thickness_mm: Option<f64>) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        "deep".to_string(),
        (Nm::from_mm(20.0), Nm::from_mm(20.0)),
        2,
    );
    if let Some(thickness_mm) = thickness_mm {
        assert!(
            world.set_stackup(stackup_of(thickness_mm)),
            "the board takes a stackup"
        );
    }
    let net = world.intern_net("GND");
    let mut via = Via::new(Point::from_mm(10.0, 10.0), net);
    via.drill = Nm::from_mm(drill_mm);
    world.ecs_mut().spawn((via, net));
    world
}

/// The same board with a mounting hole: a drill and no copper anywhere.
fn board_with_mounting_hole(drill_mm: f64, thickness_mm: f64) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        "deep".to_string(),
        (Nm::from_mm(20.0), Nm::from_mm(20.0)),
        2,
    );
    assert!(
        world.set_stackup(stackup_of(thickness_mm)),
        "the board takes a stackup"
    );

    let mut library = FootprintLibrary::new();
    let base = library
        .get("0402")
        .expect("the library has an 0402")
        .clone();
    library.register_design(Footprint {
        name: "screw".to_string(),
        pads: vec![PadDef {
            number: String::new(),
            shape: PadShape::Circle,
            position: Point::from_mm(0.0, 0.0),
            size: (Nm::from_mm(drill_mm), Nm::from_mm(drill_mm)),
            drill: Some(Nm::from_mm(drill_mm)),
            slot: None,
            // No copper layers, so nothing plates it. `PadDef::is_non_plated`
            // reads exactly this, and so does the drill file.
            layers: Vec::new(),
        }],
        ..base
    });
    world.set_footprints(library);

    world.spawn_component(
        RefDes::new("H1"),
        Value::new(""),
        Position::from_mm(10.0, 10.0),
        Rotation::ZERO,
        FootprintRef::new("screw"),
        NetConnections::new(),
    );
    world
}

fn depth_faults(world: &mut BoardWorld) -> Vec<String> {
    run_drc(world, &Preset::JlcpcbStandard2Layer.rules())
        .violations
        .into_iter()
        .filter(|violation| violation.kind == ViolationKind::DrillAspectRatio)
        .map(|violation| violation.message)
        .collect()
}

#[test]
fn the_same_via_passes_on_a_thin_board_and_fails_on_a_thick_one() {
    // 0.3mm is JLCPCB's own minimum via drill, so the drill rule is content
    // with it on any board. Through 1.6mm it is 5.3:1 and ordinary; through
    // 3.2mm it is 10.7:1, past the 8:1 that process plates.
    let thin = depth_faults(&mut board_with_via(0.3, Some(1.6)));
    let thick = depth_faults(&mut board_with_via(0.3, Some(3.2)));

    assert_eq!(thin, Vec::<String>::new(), "1.6mm is an ordinary board");
    assert_eq!(thick.len(), 1, "{thick:?}");
    assert!(
        thick[0].contains("10.7:1") && thick[0].contains("8.0:1"),
        "the message names the ratio it reached and the one allowed: {}",
        thick[0]
    );
    assert!(
        thick[0].contains("0.400mm"),
        "and the drill that would work: {}",
        thick[0]
    );
}

#[test]
fn the_drill_rule_says_nothing_about_this_board() {
    // The distinction the rule exists for: a 0.3mm via on a 3.2mm board is a
    // hole no fab plates, and every rule about the drill alone passes it.
    let mut world = board_with_via(0.3, Some(3.2));

    let drill_faults = run_drc(&mut world, &Preset::JlcpcbStandard2Layer.rules())
        .violations
        .into_iter()
        .filter(|violation| {
            matches!(
                violation.kind,
                ViolationKind::ViaDrill | ViolationKind::DrillSize
            )
        })
        .count();

    assert_eq!(drill_faults, 0, "the drill clears every floor there is");
    assert_eq!(depth_faults(&mut board_with_via(0.3, Some(3.2))).len(), 1);
}

#[test]
fn a_board_that_states_no_thickness_takes_the_fabs_own() {
    // JLCPCB builds 1.6mm as standard and states so in its table, so a design
    // that says nothing is graded through 1.6mm rather than through a
    // constant somebody typed here.
    let silent = depth_faults(&mut board_with_via(0.19, None));

    assert_eq!(silent.len(), 1, "{silent:?}");
    assert!(
        silent[0].contains("1.60mm"),
        "the fab's own thickness is in the message: {}",
        silent[0]
    );
}

#[test]
fn the_boundary_is_where_the_fab_put_it() {
    // 8:1 through 1.6mm is 0.2mm exactly. That hole is buildable; anything
    // under it is not.
    assert_eq!(
        depth_faults(&mut board_with_via(0.2, Some(1.6))),
        Vec::<String>::new()
    );
    assert_eq!(
        depth_faults(&mut board_with_via(0.1999, Some(1.6))).len(),
        1
    );
}

#[test]
fn a_hole_nobody_plates_is_not_asked_to_plate() {
    // A mounting hole is drilled and left bare. There is no copper in it to
    // come out thin, so however deep the board it is not this rule's business
    // - and a rule that reported it would fire on every screw hole in every
    // thick board ever designed.
    let faults = depth_faults(&mut board_with_mounting_hole(0.3, 3.2));

    assert_eq!(faults, Vec::<String>::new());
}

#[test]
fn every_preset_publishes_both_numbers_for_this() {
    for preset in Preset::all() {
        let rules = preset.rules();
        assert!(
            rules.max_drill_aspect_ratio > 0,
            "{} states no drill aspect ratio",
            preset.name()
        );
        assert!(
            rules.board_thickness > Nm(0),
            "{} states no board thickness",
            preset.name()
        );
    }
}
