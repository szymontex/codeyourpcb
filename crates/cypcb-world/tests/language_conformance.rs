//! Everything the language can say, and proof that it arrives.
//!
//! The via bug this guards against was not a parser bug: the grammar read the
//! via, the AST held it, and `sync_trace` dropped it on the way into the
//! board. Nothing noticed, because no test asked whether what a file says is
//! what the model ends up holding.
//!
//! One fixture uses every construct the DSL has, and one test per construct
//! asserts it landed. A feature added to the grammar without a line here is a
//! feature nobody has checked reaches the board.

use cypcb_world::components::trace::{Trace, Via};
use cypcb_world::components::zone::{Zone, ZoneKind};
use cypcb_world::components::{BoardOutline, Position, RefDes, Rotation, Side, TypedValue};
use cypcb_world::footprint::{FootprintLibrary, SilkShape};
use cypcb_world::{sync_ast_to_world, BoardWorld};

const EVERYTHING: &str = r#"version 1

board conformance {
    size 40mm x 30mm
    layers 2
}

outline {
    point 0mm, 0mm
    point 40mm, 0mm
    point 40mm, 15mm
    point 20mm, 15mm
    point 20mm, 30mm
    point 0mm, 30mm
}

footprint MARKED {
    description "carries its own legend"
    courtyard 3mm x 2mm

    pad 1 rect at -0.5mm, 0mm size 0.6mm x 0.5mm
    pad 2 rect at 0.5mm, 0mm size 0.6mm x 0.5mm

    silk line -1mm, 0.6mm to 1mm, 0.6mm width 0.15mm
    silk circle -1.2mm, 0mm radius 0.15mm
}

interface TwoPort {
    pin IN
    pin OUT
}

module Divider {
    implements TwoPort
    pin IN
    pin OUT

    component RTOP resistor "0402" {
        value 10kohm
        at 1mm, 0mm
    }

    net IN {
        RTOP.1
    }

    net OUT {
        RTOP.2
    }
}

component U1 ic "MARKED" {
    value 4.7kohm
    at 8mm, 8mm
    rotate 90
}

component R1 resistor "0402" {
    value 10kohm
    at 16mm, 8mm
}

netclass Power [width 0.5mm clearance 0.3mm] {
    VCC
    GND
}

net VCC {
    U1.1
    R1.1
}

net GND [current 2A] {
    U1.2
    R1.2
}

zone GND_POUR {
    bounds 0mm, 0mm to 20mm, 15mm
    layer top
    net GND
}

keepout NO_COPPER {
    bounds 30mm, 20mm to 38mm, 28mm
    layer all
}

trace VCC {
    from U1.1
    to R1.1
    layer Top
}

trace GND {
    layer Bottom
    width 0.400000mm
    path 8.000000mm,12.000000mm -> 16.000000mm,12.000000mm
}

trace GND {
    via 12.000000mm,12.000000mm drill 0.400000mm
}

use Divider as DIV1 at 30mm, 5mm rotate 90 {
    IN = VCC
    OUT = SENSE
}

assert board.width <= 100mm
assert GND.current >= 1A
"#;

fn board() -> BoardWorld {
    let parsed = cypcb_parser::parse(EVERYTHING);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);

    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, EVERYTHING, &mut world, &mut library);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    world
}

fn refdes_list(world: &mut BoardWorld) -> Vec<String> {
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<&RefDes>();
    let mut names: Vec<String> = query.iter(ecs).map(|r| r.as_str().to_string()).collect();
    names.sort();
    names
}

#[test]
fn the_board_and_its_outline_arrive() {
    let world = board();
    let (size, layers) = world.board_info().expect("a board");
    assert_eq!(size.width, cypcb_core::Nm::from_mm(40.0));
    assert_eq!(layers.count, 2);

    let entity = world.board_entity().expect("a board entity");
    let outline = world
        .ecs()
        .get::<BoardOutline>(entity)
        .expect("the outline block");
    assert_eq!(outline.points.len(), 6);
    assert!(!outline.contains(cypcb_core::Point::from_mm(30.0, 25.0)));
}

#[test]
fn components_arrive_with_placement_rotation_side_and_typed_value() {
    let mut world = board();
    assert_eq!(refdes_list(&mut world), vec!["DIV1_RTOP", "R1", "U1"]);

    let ecs = world.ecs_mut();
    let mut query = ecs.query::<(&RefDes, &Position, &Rotation, &Side, &TypedValue)>();
    let u1 = query
        .iter(ecs)
        .find(|(refdes, ..)| refdes.as_str() == "U1")
        .expect("U1 with all of it");

    assert_eq!(u1.1 .0, cypcb_core::Point::from_mm(8.0, 8.0));
    assert_eq!(u1.2 .0, 90_000, "rotate 90 is 90000 millidegrees");
    assert_eq!(*u1.3, Side::Top);
    assert_eq!(u1.4.unit, cypcb_core::physical_units::PhysicalUnit::KiloOhm);
    assert_eq!(u1.4.value, 4.7);
}

#[test]
fn an_inline_footprint_arrives_with_its_pads_and_legend() {
    let world = board();
    let library = world.footprints().clone();
    let marked = library.get("MARKED").expect("the footprint block");

    assert_eq!(marked.pads.len(), 2);
    assert_eq!(marked.silk.len(), 2, "one line and one circle");
    assert!(marked
        .silk
        .iter()
        .any(|shape| matches!(shape, SilkShape::Segment { .. })));
    assert!(marked
        .silk
        .iter()
        .any(|shape| matches!(shape, SilkShape::Circle { .. })));
}

#[test]
fn net_constraints_and_classes_arrive() {
    let world = board();

    let gnd = world.get_net("GND").expect("GND");
    let vcc = world.get_net("VCC").expect("VCC");

    let gnd_rules = world.net_constraints(gnd).expect("GND states things");
    assert_eq!(gnd_rules.current_ma, Some(2000.0), "its own block");
    assert_eq!(
        gnd_rules.width,
        Some(cypcb_core::Nm::from_mm(0.5)),
        "and its class, for what the block leaves unsaid"
    );

    let vcc_rules = world.net_constraints(vcc).expect("VCC is in the class");
    assert_eq!(vcc_rules.clearance, Some(cypcb_core::Nm::from_mm(0.3)));
}

#[test]
fn zones_arrive_with_their_kind_and_net() {
    let mut world = board();
    let gnd = world.get_net("GND").expect("GND");

    let zones: Vec<Zone> = world.zones().into_iter().map(|(_, zone)| zone).collect();
    assert_eq!(zones.len(), 2, "a pour and a keepout");

    let pour = zones
        .iter()
        .find(|z| z.kind == ZoneKind::CopperPour)
        .expect("the pour");
    assert_eq!(
        pour.net,
        Some(gnd),
        "a pour that cannot name its net is not one"
    );

    let keepout = zones
        .iter()
        .find(|z| z.kind == ZoneKind::Keepout)
        .expect("the keepout");
    assert_eq!(keepout.net, None, "a keepout is not poured to anything");
}

#[test]
fn traces_and_vias_arrive_including_a_via_on_its_own() {
    let mut world = board();

    let (traces, vias) = {
        let ecs = world.ecs_mut();
        let mut trace_query = ecs.query::<&Trace>();
        let traces: Vec<Trace> = trace_query.iter(ecs).cloned().collect();
        let mut via_query = ecs.query::<&Via>();
        let vias: Vec<Via> = via_query.iter(ecs).copied().collect();
        (traces, vias)
    };

    assert_eq!(traces.len(), 2, "one pin-to-pin, one geometric");
    assert_eq!(vias.len(), 1, "the via block has no path, and still counts");
    assert_eq!(vias[0].drill, cypcb_core::Nm::from_mm(0.4));

    let geometric = traces
        .iter()
        .find(|t| t.width == cypcb_core::Nm::from_mm(0.4))
        .expect("the path trace keeps its stated width");
    assert_eq!(geometric.segments.len(), 1);
}

#[test]
fn a_module_instance_arrives_placed_and_wired() {
    let mut world = board();

    // Its part is named after the instance and sits where the instance says.
    let ecs = world.ecs_mut();
    let mut query = ecs.query::<(&RefDes, &Position)>();
    let placed = query
        .iter(ecs)
        .find(|(refdes, _)| refdes.as_str() == "DIV1_RTOP")
        .expect("the instance brought its part");

    // RTOP sits at 1mm, 0mm inside the module; DIV1 is at 30mm, 5mm turned a
    // quarter turn, which puts it at 30mm, 6mm.
    assert_eq!(placed.1 .0, cypcb_core::Point::from_mm(30.0, 6.0));

    // A pin wired through is the design's net, not a local one.
    assert!(world.get_net("SENSE").is_some());
    assert!(
        world.get_net("DIV1_IN").is_none(),
        "IN is wired to VCC, so it must not also exist as a local net"
    );
}

#[test]
fn assertions_arrive_for_the_checker() {
    let world = board();
    assert_eq!(
        world.assertions().len(),
        2,
        "both assert statements have to reach the model, or nothing checks them"
    );
}

#[test]
fn an_interface_contract_is_checked_rather_than_stored() {
    // `interface` parsed and was read by nothing for as long as it existed.
    // The fixture's module signs `TwoPort` and exposes both its pins, so a
    // clean sync is the proof; the failing direction is checked in the sync
    // crate's own tests, where the module is missing a pin.
    let source = EVERYTHING;
    let parsed = cypcb_parser::parse(source);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);

    let complaints: Vec<String> = result
        .errors
        .iter()
        .filter(|e| {
            matches!(
                e,
                cypcb_world::SyncError::InterfaceNotSatisfied { .. }
                    | cypcb_world::SyncError::UnknownInterface { .. }
            )
        })
        .map(|e| e.to_string())
        .collect();
    assert!(
        complaints.is_empty(),
        "the module exposes every pin TwoPort declares: {complaints:?}"
    );
}
