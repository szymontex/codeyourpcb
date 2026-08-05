/// End-to-end test for trace persistence: export → reload → verify exact match.
///
/// This test simulates the actual user workflow:
/// 1. Load a .cypcb file
/// 2. Add interactive traces (simulating manual routing)
/// 3. Export traces as DSL
/// 4. Merge exported traces into the source
/// 5. Load the merged file into a fresh engine
/// 6. Verify traces survived exactly (bit-for-bit coordinates)
/// 7. Export again and verify the output is IDENTICAL (determinism)
use cypcb_render::PcbEngine;

#[test]
fn e2e_trace_persistence_full_pipeline() {
    // Step 1: Load a realistic .cypcb file
    let original_source = r#"version 1

board blink {
    size 60mm x 40mm
    layers 2
}

component J1 connector "PIN-HDR-1x2" { value "5V PWR"
    at 5mm, 20mm }
component U1 ic "SOIC-8" { value "NE555"
    at 28mm, 20mm }
component R1 resistor "0402" { value "10k"
    at 35mm, 30mm }

net VCC { J1.1
    U1.8
    R1.1 }
net GND { J1.2
    U1.1 }
net DIS { U1.7
    R1.2 }
"#;

    let mut engine1 = PcbEngine::new();
    let err = engine1.load_source(original_source);
    assert!(err.is_empty(), "Load error: {}", err);

    // Step 2: Add traces (simulating interactive routing)
    // VCC trace: J1.1 → U1.8 → R1.1
    let vcc_segs = [
        5_000_000i64,
        20_000_000,
        15_000_000,
        20_000_000, // J1 area → middle
        15_000_000,
        20_000_000,
        28_000_000,
        20_000_000, // middle → U1
        28_000_000,
        20_000_000,
        35_000_000,
        30_000_000, // U1 → R1
    ];
    let id1 = engine1.add_trace("VCC", "Top", 250_000, &vcc_segs);
    assert_ne!(id1, u32::MAX, "Failed to add VCC trace");

    // GND trace: J1.2 → U1.1
    let gnd_segs = [
        5_000_000i64,
        20_000_000,
        10_000_000,
        25_000_000,
        10_000_000,
        25_000_000,
        28_000_000,
        25_000_000,
    ];
    let id2 = engine1.add_trace("GND", "Bottom", 200_000, &gnd_segs);
    assert_ne!(id2, u32::MAX, "Failed to add GND trace");

    // DIS trace with odd coordinates (testing precision)
    let dis_segs = [28_123_456i64, 20_654_321, 35_999_999, 30_000_001];
    let id3 = engine1.add_trace("DIS", "Top", 150_000, &dis_segs);
    assert_ne!(id3, u32::MAX, "Failed to add DIS trace");

    // Step 3: Export traces as DSL
    let exported1 = engine1.export_traces_as_dsl();
    println!("=== EXPORTED DSL (first pass) ===");
    println!("{}", exported1);
    println!("=== END ===\n");

    assert!(!exported1.is_empty(), "Export should not be empty");
    assert!(exported1.contains("trace VCC"), "Missing VCC trace");
    assert!(exported1.contains("trace GND"), "Missing GND trace");
    assert!(exported1.contains("trace DIS"), "Missing DIS trace");
    assert!(exported1.contains("layer Top"), "Missing Top layer");
    assert!(exported1.contains("layer Bottom"), "Missing Bottom layer");

    // Step 4: Merge into the source (simulating what TS does)
    let merged_source = format!(
        "{}\n// --- Routed traces (auto-generated) ---\n{}\n// --- End routed traces ---\n",
        original_source.trim(),
        exported1.trim()
    );
    println!("=== MERGED SOURCE ===");
    println!("{}", merged_source);
    println!("=== END ===\n");

    // Step 5: Load into a fresh engine
    let mut engine2 = PcbEngine::new();
    let err2 = engine2.load_source(&merged_source);
    assert!(err2.is_empty(), "Reload error: {}", err2);

    // Step 6: Verify trace count and coordinates
    let snapshot2 = engine2.build_snapshot();
    println!("Traces after reload: {}", snapshot2.traces.len());
    for t in &snapshot2.traces {
        println!(
            "  {} on {} ({} segments, width={}, locked={})",
            t.net_name,
            t.layer,
            t.segments.len(),
            t.width,
            t.locked
        );
        for (i, s) in t.segments.iter().enumerate() {
            println!(
                "    seg[{}]: ({}, {}) -> ({}, {})",
                i, s.start_x, s.start_y, s.end_x, s.end_y
            );
        }
    }

    // Should have 3 traces (VCC, GND, DIS)
    assert_eq!(
        snapshot2.traces.len(),
        3,
        "Expected 3 traces after reload, got {}",
        snapshot2.traces.len()
    );

    // Verify DIS trace coordinates survived exactly (precision test)
    let dis_trace = snapshot2
        .traces
        .iter()
        .find(|t| t.net_name == "DIS")
        .expect("DIS trace not found after reload");
    assert_eq!(dis_trace.segments.len(), 1);
    assert_eq!(
        dis_trace.segments[0].start_x as i64, 28_123_456,
        "DIS start_x mismatch"
    );
    assert_eq!(
        dis_trace.segments[0].start_y as i64, 20_654_321,
        "DIS start_y mismatch"
    );
    assert_eq!(
        dis_trace.segments[0].end_x as i64, 35_999_999,
        "DIS end_x mismatch"
    );
    assert_eq!(
        dis_trace.segments[0].end_y as i64, 30_000_001,
        "DIS end_y mismatch"
    );

    // Step 7: Export again — must be IDENTICAL (determinism)
    let exported2 = engine2.export_traces_as_dsl();
    println!("=== EXPORTED DSL (second pass) ===");
    println!("{}", exported2);
    println!("=== END ===\n");

    assert_eq!(
        exported1, exported2,
        "\n\nDETERMINISM FAILURE!\n--- First ---\n{}\n--- Second ---\n{}",
        exported1, exported2
    );

    println!("✓ All assertions passed — trace persistence is deterministic!");
}
