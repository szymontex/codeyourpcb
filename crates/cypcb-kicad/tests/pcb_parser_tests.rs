//! Unit tests for the KiCad PCB S-expression parser.

use cypcb_core::Nm;
use cypcb_kicad::pcb_parser::{parse_kicad_pcb_str, KicadPcbError};
use std::fs;
use std::path::Path;

fn load_minimal_fixture() -> String {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("minimal.kicad_pcb");
    fs::read_to_string(fixture_path).expect("Failed to read minimal.kicad_pcb fixture")
}

#[test]
fn test_parse_minimal_fixture() {
    let content = load_minimal_fixture();
    let result = parse_kicad_pcb_str(&content).expect("Failed to parse minimal fixture");

    assert_eq!(
        result.metadata.version, 20240108,
        "Version should be KiCad 8"
    );
    assert_eq!(
        result.metadata.component_count, 2,
        "Should have 2 components (R1 + LED1)"
    );
    assert_eq!(
        result.metadata.net_count, 2,
        "Should have 2 nets (VCC + GND), excluding net 0"
    );

    // Board size: 30mm x 20mm in nm
    let expected_width = Nm::from_mm(30.0);
    let expected_height = Nm::from_mm(20.0);

    let (board_size, _layers) = result.world.board_info().expect("Board should be set");
    assert_eq!(
        board_size.width, expected_width,
        "Board width should be 30mm"
    );
    assert_eq!(
        board_size.height, expected_height,
        "Board height should be 20mm"
    );
}

#[test]
fn test_component_positions() {
    let content = load_minimal_fixture();
    let result = parse_kicad_pcb_str(&content).expect("Failed to parse");

    let mut world = result.world;

    // R1 is at (10, 8) mm
    let r1_entity = world.find_by_refdes("R1").expect("R1 should exist");
    let r1_pos = world
        .get::<cypcb_world::Position>(r1_entity)
        .expect("R1 should have Position");
    assert_eq!(r1_pos.0.x, Nm::from_mm(10.0), "R1 x position");
    assert_eq!(r1_pos.0.y, Nm::from_mm(8.0), "R1 y position");

    // R1 should have 0 rotation
    let r1_rot = world
        .get::<cypcb_world::Rotation>(r1_entity)
        .expect("R1 should have Rotation");
    assert_eq!(r1_rot.0, 0, "R1 should have 0 rotation");

    // LED1 is at (20, 12) mm with 90° rotation
    let led1_entity = world.find_by_refdes("LED1").expect("LED1 should exist");
    let led1_pos = world
        .get::<cypcb_world::Position>(led1_entity)
        .expect("LED1 should have Position");
    assert_eq!(led1_pos.0.x, Nm::from_mm(20.0), "LED1 x position");
    assert_eq!(led1_pos.0.y, Nm::from_mm(12.0), "LED1 y position");

    let led1_rot = world
        .get::<cypcb_world::Rotation>(led1_entity)
        .expect("LED1 should have Rotation");
    assert_eq!(
        led1_rot.0, 90_000,
        "LED1 should have 90° rotation (90000 millideg)"
    );
}

#[test]
fn test_pad_net_assignments() {
    let content = load_minimal_fixture();
    let result = parse_kicad_pcb_str(&content).expect("Failed to parse");

    let mut world = result.world;

    // Get VCC and GND net IDs
    let vcc_id = world.get_net("VCC").expect("VCC net should be interned");
    let gnd_id = world.get_net("GND").expect("GND net should be interned");

    // R1: pad 1 → VCC, pad 2 → GND
    let r1_entity = world.find_by_refdes("R1").expect("R1 should exist");
    let r1_nets = world
        .get::<cypcb_world::NetConnections>(r1_entity)
        .expect("R1 should have NetConnections");
    assert_eq!(r1_nets.pin_net("1"), Some(vcc_id), "R1 pad 1 should be VCC");
    assert_eq!(r1_nets.pin_net("2"), Some(gnd_id), "R1 pad 2 should be GND");

    // LED1: pad 1 → VCC, pad 2 → GND
    let led1_entity = world.find_by_refdes("LED1").expect("LED1 should exist");
    let led1_nets = world
        .get::<cypcb_world::NetConnections>(led1_entity)
        .expect("LED1 should have NetConnections");
    assert_eq!(
        led1_nets.pin_net("1"),
        Some(vcc_id),
        "LED1 pad 1 should be VCC"
    );
    assert_eq!(
        led1_nets.pin_net("2"),
        Some(gnd_id),
        "LED1 pad 2 should be GND"
    );
}

#[test]
fn test_reference_routes_extracted() {
    let content = load_minimal_fixture();
    let result = parse_kicad_pcb_str(&content).expect("Failed to parse");

    let ref_routes = result
        .reference_routes
        .as_ref()
        .expect("Should have reference routes");

    assert_eq!(ref_routes.routes.len(), 1, "Should have 1 trace segment");
    assert_eq!(ref_routes.vias.len(), 1, "Should have 1 via");

    // Verify segment positions
    let seg = &ref_routes.routes[0];
    assert_eq!(seg.start, cypcb_core::Point::from_mm(10.5, 8.0));
    assert_eq!(seg.end, cypcb_core::Point::from_mm(20.0, 8.0));
    assert_eq!(seg.width, Nm::from_mm(0.25));

    // Verify via position
    let via = &ref_routes.vias[0];
    assert_eq!(via.position, cypcb_core::Point::from_mm(20.0, 8.0));
    assert_eq!(via.drill, Nm::from_mm(0.4));

    // Verify metadata counts
    assert_eq!(result.metadata.trace_segment_count, 1);
    assert_eq!(result.metadata.via_count, 1);
}

#[test]
fn test_footprint_library_registered() {
    let content = load_minimal_fixture();
    let result = parse_kicad_pcb_str(&content).expect("Failed to parse");

    let library = &result.library;

    // Both footprints should be registered by their library link name
    assert!(
        library.contains("Resistor_SMD:R_0402"),
        "Resistor footprint should be registered"
    );
    assert!(
        library.contains("LED_THT:LED_D3.0mm"),
        "LED footprint should be registered"
    );

    // Check pad counts
    let resistor_fp = library.get("Resistor_SMD:R_0402").unwrap();
    assert_eq!(resistor_fp.pads.len(), 2, "Resistor should have 2 pads");

    let led_fp = library.get("LED_THT:LED_D3.0mm").unwrap();
    assert_eq!(led_fp.pads.len(), 2, "LED should have 2 pads");

    // Verify LED pad is through-hole
    let led_pad1 = led_fp.get_pad("1").expect("LED should have pad 1");
    assert!(led_pad1.is_through_hole(), "LED pad should be through-hole");
    assert_eq!(led_pad1.drill, Some(Nm::from_mm(0.9)));
}

#[test]
fn test_net_zero_skipped() {
    let content = load_minimal_fixture();
    let result = parse_kicad_pcb_str(&content).expect("Failed to parse");

    // Net 0 ("") should not be interned
    let world = &result.world;
    assert!(
        world.get_net("").is_none(),
        "Empty net name should not be interned"
    );

    // Only VCC and GND should exist
    assert_eq!(world.net_count(), 2, "Should have exactly 2 interned nets");
    assert!(world.get_net("VCC").is_some(), "VCC should be interned");
    assert!(world.get_net("GND").is_some(), "GND should be interned");
}

#[test]
fn test_empty_input_returns_sexpr_error() {
    let result = parse_kicad_pcb_str("");
    match result {
        Err(KicadPcbError::SexprParseError(_)) => {} // Expected
        Err(other) => panic!("Expected SexprParseError, got {:?}", other),
        Ok(_) => panic!("Expected error for empty input"),
    }
}

#[test]
fn test_unsupported_version_returns_error() {
    let input = r#"(kicad_pcb (version 1))"#;
    let result = parse_kicad_pcb_str(input);
    match result {
        Err(KicadPcbError::UnsupportedVersion { version }) => {
            assert_eq!(version, 1);
        }
        Err(other) => panic!("Expected UnsupportedVersion, got {:?}", other),
        Ok(_) => panic!("Expected error for unsupported version"),
    }
}

#[test]
fn test_module_keyword_backward_compat() {
    // KiCad 5/6 used "module" instead of "footprint"
    let input = r#"(kicad_pcb (version 20211014)
      (layers
        (0 "F.Cu" signal)
        (31 "B.Cu" signal)
      )
      (net 0 "")
      (net 1 "VCC")
      (module "Resistor_SMD:R_0603"
        (at 5 5)
        (fp_text reference "R1" (at 0 0) (layer "F.SilkS"))
        (fp_text value "100" (at 0 1) (layer "F.SilkS"))
        (pad "1" smd rect (at -0.5 0) (size 0.6 0.5) (layers "F.Cu") (net 1 "VCC"))
        (pad "2" smd rect (at 0.5 0) (size 0.6 0.5) (layers "F.Cu") (net 1 "VCC"))
      )
    )"#;

    let result = parse_kicad_pcb_str(input).expect("Should parse KiCad 6 module keyword");
    assert_eq!(result.metadata.component_count, 1);
    assert_eq!(result.metadata.version, 20211014);
}

#[test]
fn test_layer_count_extraction() {
    let content = load_minimal_fixture();
    let result = parse_kicad_pcb_str(&content).expect("Failed to parse");
    assert_eq!(
        result.metadata.layer_count, 2,
        "Minimal fixture has 2 copper layers"
    );
}
