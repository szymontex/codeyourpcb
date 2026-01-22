//! DSN Export Integration Tests
//!
//! These tests verify that the DSN export produces valid Specctra format
//! output that could be read by FreeRouting.

use cypcb_core::{Nm, Point};
use cypcb_router::export_dsn;
use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{
    BoardWorld, FootprintRef, Layer, NetConnections, PinConnection, Position, RefDes,
    Rotation, Value,
};

/// Create a test board with 3 components and 2 nets.
///
/// Board: 50mm x 30mm, 2 layers
/// Components: R1 (0402), R2 (0402), C1 (0603)
/// Nets: VCC (R1.1, C1.1), GND (R1.2, R2.1, R2.2, C1.2)
fn create_test_board() -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    let library = FootprintLibrary::new();

    // Create 50mm x 30mm 2-layer board
    world.set_board(
        "TestBoard".to_string(),
        (Nm::from_mm(50.0), Nm::from_mm(30.0)),
        2,
    );

    // Create nets
    let vcc = world.intern_net("VCC");
    let gnd = world.intern_net("GND");

    // R1: 10k resistor at (10, 15)mm
    // Pin 1 -> VCC, Pin 2 -> GND
    let mut r1_nets = NetConnections::new();
    r1_nets.add(PinConnection::new("1", vcc));
    r1_nets.add(PinConnection::new("2", gnd));

    world.spawn_component(
        RefDes::new("R1"),
        Value::new("10k"),
        Position::from_mm(10.0, 15.0),
        Rotation::ZERO,
        FootprintRef::new("0402"),
        r1_nets,
    );

    // R2: 4.7k resistor at (25, 15)mm
    // Both pins to GND (current sense, for example)
    let mut r2_nets = NetConnections::new();
    r2_nets.add(PinConnection::new("1", gnd));
    r2_nets.add(PinConnection::new("2", gnd));

    world.spawn_component(
        RefDes::new("R2"),
        Value::new("4.7k"),
        Position::from_mm(25.0, 15.0),
        Rotation::ZERO,
        FootprintRef::new("0402"),
        r2_nets,
    );

    // C1: 100nF capacitor at (17, 8)mm
    // Pin 1 -> VCC, Pin 2 -> GND
    let mut c1_nets = NetConnections::new();
    c1_nets.add(PinConnection::new("1", vcc));
    c1_nets.add(PinConnection::new("2", gnd));

    world.spawn_component(
        RefDes::new("C1"),
        Value::new("100nF"),
        Position::from_mm(17.0, 8.0),
        Rotation::DEG_90,
        FootprintRef::new("0603"),
        c1_nets,
    );

    (world, library)
}

#[test]
fn test_full_board_export() {
    let (mut world, library) = create_test_board();
    let mut output = Vec::new();

    let result = export_dsn(&mut world, &library, &mut output);
    assert!(result.is_ok(), "DSN export should succeed");

    let dsn = String::from_utf8(output).expect("DSN should be valid UTF-8");

    // Verify top-level structure
    assert!(
        dsn.starts_with("(pcb"),
        "DSN should start with (pcb declaration"
    );
    assert!(
        dsn.ends_with(")\n"),
        "DSN should end with closing paren and newline"
    );

    // Verify all major sections present
    assert!(dsn.contains("(parser"), "DSN should have parser section");
    assert!(
        dsn.contains("(resolution mil 10)"),
        "DSN should have resolution"
    );
    assert!(dsn.contains("(unit mil)"), "DSN should have unit");
    assert!(dsn.contains("(structure"), "DSN should have structure section");
    assert!(
        dsn.contains("(placement"),
        "DSN should have placement section"
    );
    assert!(dsn.contains("(library"), "DSN should have library section");
    assert!(dsn.contains("(network"), "DSN should have network section");
    assert!(dsn.contains("(wiring"), "DSN should have wiring section");
}

#[test]
fn test_board_boundary_in_dsn() {
    let (mut world, library) = create_test_board();
    let mut output = Vec::new();

    export_dsn(&mut world, &library, &mut output).unwrap();
    let dsn = String::from_utf8(output).unwrap();

    // Verify boundary (50mm x 30mm = ~1969 x 1181 mils)
    assert!(dsn.contains("(boundary"), "DSN should have boundary");
    assert!(dsn.contains("(rect pcb 0 0"), "Boundary should start at origin");

    // 50mm = 50000000nm / 25400 = ~1968.5 mils
    // 30mm = 30000000nm / 25400 = ~1181.1 mils
    assert!(
        dsn.contains("1968.5") || dsn.contains("1968.50"),
        "Width should be ~1968.5 mils"
    );
    assert!(
        dsn.contains("1181.1") || dsn.contains("1181.10"),
        "Height should be ~1181.1 mils"
    );
}

#[test]
fn test_component_placement_in_dsn() {
    let (mut world, library) = create_test_board();
    let mut output = Vec::new();

    export_dsn(&mut world, &library, &mut output).unwrap();
    let dsn = String::from_utf8(output).unwrap();

    // All three components should be placed
    assert!(dsn.contains("(place \"R1\""), "R1 should be placed");
    assert!(dsn.contains("(place \"R2\""), "R2 should be placed");
    assert!(dsn.contains("(place \"C1\""), "C1 should be placed");

    // Footprints should be in component groups
    assert!(
        dsn.contains("(component \"0402\""),
        "0402 footprint should be used"
    );
    assert!(
        dsn.contains("(component \"0603\""),
        "0603 footprint should be used"
    );
}

#[test]
fn test_net_connections_in_dsn() {
    let (mut world, library) = create_test_board();
    let mut output = Vec::new();

    export_dsn(&mut world, &library, &mut output).unwrap();
    let dsn = String::from_utf8(output).unwrap();

    // Both nets should be defined
    assert!(dsn.contains("(net \"VCC\""), "VCC net should be defined");
    assert!(dsn.contains("(net \"GND\""), "GND net should be defined");

    // VCC should have R1-1 and C1-1
    // Note: The actual DSN format uses refdes-pin format
    assert!(
        dsn.contains("R1-1") && dsn.contains("C1-1"),
        "VCC should connect R1.1 and C1.1"
    );

    // GND should have R1-2, R2-1, R2-2, C1-2
    assert!(dsn.contains("R1-2"), "GND should connect R1.2");
    assert!(dsn.contains("R2-1"), "GND should connect R2.1");
    assert!(dsn.contains("R2-2"), "GND should connect R2.2");
    assert!(dsn.contains("C1-2"), "GND should connect C1.2");
}

#[test]
fn test_padstack_definitions_in_dsn() {
    let (mut world, library) = create_test_board();
    let mut output = Vec::new();

    export_dsn(&mut world, &library, &mut output).unwrap();
    let dsn = String::from_utf8(output).unwrap();

    // Padstack definitions should exist in library section
    assert!(dsn.contains("(padstack"), "DSN should have padstack definitions");

    // Shape definitions for pads
    assert!(dsn.contains("(shape"), "Padstacks should have shapes");
}

#[test]
fn test_layer_definitions_in_dsn() {
    let (mut world, library) = create_test_board();
    let mut output = Vec::new();

    export_dsn(&mut world, &library, &mut output).unwrap();
    let dsn = String::from_utf8(output).unwrap();

    // 2-layer board should have F.Cu and B.Cu
    assert!(dsn.contains("(layer F.Cu"), "DSN should have top copper layer");
    assert!(
        dsn.contains("(layer B.Cu"),
        "DSN should have bottom copper layer"
    );
    assert!(
        dsn.contains("(type signal)"),
        "Layers should be signal type"
    );
}

#[test]
fn test_design_rules_in_dsn() {
    let (mut world, library) = create_test_board();
    let mut output = Vec::new();

    export_dsn(&mut world, &library, &mut output).unwrap();
    let dsn = String::from_utf8(output).unwrap();

    // Default design rules
    assert!(dsn.contains("(rule"), "DSN should have design rules");
    assert!(dsn.contains("(width"), "Rules should include trace width");
    assert!(dsn.contains("(clearance"), "Rules should include clearance");
}

#[test]
fn test_net_class_in_dsn() {
    let (mut world, library) = create_test_board();
    let mut output = Vec::new();

    export_dsn(&mut world, &library, &mut output).unwrap();
    let dsn = String::from_utf8(output).unwrap();

    // Net class should group all nets
    assert!(dsn.contains("(class default"), "DSN should have default net class");
}

#[test]
fn test_locked_trace_export() {
    let (mut world, library) = create_test_board();

    // Get VCC net ID
    let vcc = world.get_net("VCC").expect("VCC net should exist");

    // Add a locked trace for manual power routing
    let trace = Trace {
        segments: vec![
            TraceSegment::new(Point::from_mm(10.0, 15.0), Point::from_mm(17.0, 15.0)),
            TraceSegment::new(Point::from_mm(17.0, 15.0), Point::from_mm(17.0, 8.0)),
        ],
        width: Nm::from_mm(0.3), // 0.3mm = ~12 mil
        layer: Layer::TopCopper,
        net_id: vcc,
        locked: true,
        source: TraceSource::Manual,
    };

    world.spawn_entity(trace);

    let mut output = Vec::new();
    export_dsn(&mut world, &library, &mut output).unwrap();
    let dsn = String::from_utf8(output).unwrap();

    // Locked trace should appear in wiring section
    assert!(dsn.contains("(wire"), "Locked trace should be exported as wire");
    assert!(dsn.contains("(path F.Cu"), "Wire should be on F.Cu layer");
    assert!(dsn.contains("(type fix)"), "Locked wire should be marked as fixed");
    assert!(dsn.contains("(net \"VCC\")"), "Wire should reference VCC net");
}

#[test]
fn test_rotation_export() {
    let (mut world, library) = create_test_board();
    let mut output = Vec::new();

    export_dsn(&mut world, &library, &mut output).unwrap();
    let dsn = String::from_utf8(output).unwrap();

    // C1 is rotated 90 degrees
    // The placement should include 90.0 rotation
    // Find the C1 placement line
    let c1_place = dsn.find("(place \"C1\"").expect("C1 should be placed");
    let c1_line_end = dsn[c1_place..].find('\n').unwrap();
    let c1_line = &dsn[c1_place..c1_place + c1_line_end];

    assert!(
        c1_line.contains("90.0") || c1_line.contains("90"),
        "C1 should have 90 degree rotation"
    );
}

#[test]
fn test_image_definitions_in_dsn() {
    let (mut world, library) = create_test_board();
    let mut output = Vec::new();

    export_dsn(&mut world, &library, &mut output).unwrap();
    let dsn = String::from_utf8(output).unwrap();

    // Footprints should be defined as images in library
    assert!(
        dsn.contains("(image \"0402\""),
        "0402 image should be defined"
    );
    assert!(
        dsn.contains("(image \"0603\""),
        "0603 image should be defined"
    );

    // Images should have pins
    assert!(dsn.contains("(pin"), "Images should have pin definitions");
}

/// Test that the DSN output is well-formed (matching parens).
#[test]
fn test_dsn_balanced_parens() {
    let (mut world, library) = create_test_board();
    let mut output = Vec::new();

    export_dsn(&mut world, &library, &mut output).unwrap();
    let dsn = String::from_utf8(output).unwrap();

    // Count parentheses - they should be balanced
    let open_count = dsn.chars().filter(|&c| c == '(').count();
    let close_count = dsn.chars().filter(|&c| c == ')').count();

    assert_eq!(
        open_count, close_count,
        "DSN should have balanced parentheses: {} open, {} close",
        open_count, close_count
    );
}

/// Test coordinate conversion accuracy.
#[test]
fn test_coordinate_conversion() {
    let mut world = BoardWorld::new();
    let library = FootprintLibrary::new();

    // Create a simple board with component at known position
    world.set_board(
        "CoordTest".to_string(),
        (Nm::from_mm(100.0), Nm::from_mm(100.0)),
        2,
    );

    // Place component at exactly 25.4mm (= 1 inch = 1000 mil)
    world.spawn_component(
        RefDes::new("U1"),
        Value::new("TEST"),
        Position::from_mm(25.4, 25.4),
        Rotation::ZERO,
        FootprintRef::new("0402"),
        NetConnections::new(),
    );

    let mut output = Vec::new();
    export_dsn(&mut world, &library, &mut output).unwrap();
    let dsn = String::from_utf8(output).unwrap();

    // 25.4mm = 1000 mil exactly
    assert!(
        dsn.contains("1000.0000"),
        "25.4mm should convert to 1000.0000 mil"
    );
}

/// Fixture: Generate DSN file for manual FreeRouting testing.
///
/// This test creates a DSN file that can be manually opened in FreeRouting
/// to verify compatibility. The file is written to /tmp/test_board.dsn
/// which can be loaded into FreeRouting GUI for visual verification.
#[test]
#[ignore] // Run manually with: cargo test --test dsn_integration test_generate_freerouting_fixture -- --ignored
fn test_generate_freerouting_fixture() {
    let (mut world, library) = create_test_board();
    let mut output = Vec::new();

    export_dsn(&mut world, &library, &mut output).unwrap();
    let dsn = String::from_utf8(output).unwrap();

    // Write to temp file for manual inspection
    let path = "/tmp/test_board.dsn";
    std::fs::write(path, &dsn).expect("Failed to write DSN fixture");

    println!("DSN fixture written to: {}", path);
    println!("To test with FreeRouting:");
    println!("  java -jar freerouting.jar -de {}", path);
    println!();
    println!("DSN content preview:");
    println!("---------------------");
    for line in dsn.lines().take(50) {
        println!("{}", line);
    }
    println!("...");
}
