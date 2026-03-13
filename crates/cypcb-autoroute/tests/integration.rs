//! Integration tests for cypcb-autoroute.
//!
//! Tests exercise the routing pipeline on reference boards:
//! - `grid_from_blink`: builds a RoutingGrid from blink.cypcb and verifies dimensions
//! - `route_routing_test_board`: routes routing-test.cypcb (3 components, 3 nets) end-to-end
//! - `route_blink_board`: routes blink.cypcb (8 components, 7 nets) with full quality validation
//! - `blink_apply_routes_compatibility`: validates output contract with apply_routes()
//! - `routed_output_passes_drc`: DRC integration — zero violations after routing blink.cypcb
//! - `benchmark_routing_time`: performance baselines for S08 comparison (gated behind `#[ignore]`)
//! - `benchmark_500_component`: 500-component synthetic board benchmark (<30s target)

use cypcb_autoroute::grid::RoutingGrid;
use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_parser::parse;
use cypcb_router::apply_routes;
use cypcb_router::types::{calculate_metrics, RoutingStatus};
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld, Layer};

/// Resolve a path relative to the workspace root.
fn workspace_path(relative: &str) -> std::path::PathBuf {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    // crates/cypcb-autoroute -> workspace root is ../../
    manifest_dir.join("../..").join(relative)
}

/// Parse a .cypcb file into a BoardWorld.
fn parse_board(relative_path: &str) -> BoardWorld {
    let path = workspace_path(relative_path);
    let path_str = path.display().to_string();
    let source = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("Failed to read {path_str}: {e}");
    });
    let parse_result = parse(&source);
    assert!(
        parse_result.is_ok(),
        "Parse errors in {path_str}: {:?}",
        parse_result.errors
    );

    let mut world = BoardWorld::new();
    let library = FootprintLibrary::new();
    let sync_result = sync_ast_to_world(&parse_result.value, &source, &mut world, &library);

    if sync_result.has_errors() {
        for err in &sync_result.errors {
            eprintln!("Sync warning in {path_str}: {err:?}");
        }
    }

    world
}

/// Build rules for testing (JLCPCB 2-layer defaults).
fn test_rules() -> PresetRuleSet {
    let preset = RulesPreset::from_name("jlcpcb").unwrap();
    PresetRuleSet::new(preset)
}

#[test]
fn grid_from_blink() {
    let mut world = parse_board("examples/blink.cypcb");
    let library = FootprintLibrary::new();
    let rules = test_rules();
    let config = AutorouteConfig::default();
    let resolution = config.resolve_grid_resolution(&rules);

    let grid = RoutingGrid::from_board(&mut world, &library, &rules, resolution)
        .expect("Grid construction should succeed for blink board");

    let stats = grid.stats();

    // blink.cypcb: 60mm x 40mm board
    // At ~63.5µm resolution: ~944 x 629 cells
    let expected_width_mm = 60.0;
    let expected_height_mm = 40.0;
    let expected_width_cells = (expected_width_mm * 1_000_000.0 / resolution as f64).ceil() as u32;
    let expected_height_cells =
        (expected_height_mm * 1_000_000.0 / resolution as f64).ceil() as u32;

    assert_eq!(
        stats.width, expected_width_cells,
        "Grid width should match board size"
    );
    assert_eq!(
        stats.height, expected_height_cells,
        "Grid height should match board size"
    );
    assert_eq!(stats.layers, 2, "Should have 2 copper layers");

    // The board has 8 components with pads — there should be occupied cells
    assert!(
        stats.obstacle_cell_count > 0,
        "Grid should have obstacles from component pads (got 0)"
    );

    println!("Grid stats: {stats:?}");

    // Verify specific pad positions are marked as occupied.
    // J1 is at (5mm, 20mm) with a PIN-HDR-1x2 footprint.
    // Pad 1 offset: (-1.27mm, 0), so absolute position: (3.73mm, 20mm)
    let j1_pad1_pos = cypcb_core::Point::from_mm(3.73, 20.0);
    let (j1_gx, j1_gy) = grid.nm_to_grid(j1_pad1_pos);
    let j1_occupied = !grid.is_free(j1_gx, j1_gy, 0) || !grid.is_free(j1_gx, j1_gy, 1);
    assert!(
        j1_occupied,
        "J1 pad 1 position ({j1_gx}, {j1_gy}) should be occupied on at least one layer"
    );
}

#[test]
fn route_routing_test_board() {
    let mut world = parse_board("examples/routing-test.cypcb");
    let library = FootprintLibrary::new();
    let rules = test_rules();
    let config = AutorouteConfig::default();

    let result = route_board(&mut world, &library, &rules, &config);

    let metrics = calculate_metrics(&result);
    eprintln!(
        "routing-test.cypcb: {:?} | segments={} vias={} total_length={:.1}mm",
        result.status,
        result.routes.len(),
        result.vias.len(),
        metrics.total_length.raw() as f64 / 1_000_000.0
    );

    assert!(
        matches!(result.status, RoutingStatus::Complete),
        "Expected RoutingStatus::Complete, got {:?}",
        result.status
    );

    // All segments should have non-zero width
    for seg in &result.routes {
        assert!(
            seg.width.raw() > 0,
            "Segment should have non-zero width, got {:?}",
            seg
        );
    }

    // All segments should be on valid copper layers (2-layer board)
    for seg in &result.routes {
        assert!(
            matches!(seg.layer, Layer::TopCopper | Layer::BottomCopper),
            "Segment on invalid layer {:?} (expected TopCopper or BottomCopper)",
            seg.layer
        );
    }
}

#[test]
fn route_blink_board() {
    let mut world = parse_board("examples/blink.cypcb");
    let library = FootprintLibrary::new();
    let rules = test_rules();
    let config = AutorouteConfig::default();

    let result = route_board(&mut world, &library, &rules, &config);

    let metrics = calculate_metrics(&result);
    let total_length_mm = metrics.total_length.raw() as f64 / 1_000_000.0;

    // Print metrics table for future comparison
    eprintln!("\n╔══════════════════════════════════════════════╗");
    eprintln!("║        blink.cypcb Routing Metrics           ║");
    eprintln!("╠══════════════════════════════════════════════╣");
    eprintln!("║ Status:        {:?}", result.status);
    eprintln!("║ Segments:      {}", result.routes.len());
    eprintln!("║ Vias:          {}", metrics.via_count);
    eprintln!("║ Total length:  {:.1} mm", total_length_mm);
    eprintln!("║ Layer changes: {}", metrics.layer_changes);
    eprintln!("║ Quality score: {:.1}", metrics.quality_score());
    eprintln!(
        "║ Completion:    {}",
        if metrics.is_complete() {
            "100%"
        } else {
            "partial"
        }
    );
    eprintln!("╚══════════════════════════════════════════════╝\n");

    // ---- Must-have assertions ----

    // 1. All 7 nets routed (RoutingStatus::Complete)
    assert!(
        matches!(result.status, RoutingStatus::Complete),
        "Expected RoutingStatus::Complete with 7/7 nets, got {:?}",
        result.status
    );

    // 2. All segments have correct width matching JLCPCB min_trace_width
    let jlcpcb_trace_width = cypcb_core::Nm::from_mm(0.127);
    for seg in &result.routes {
        assert!(seg.width.raw() > 0, "Segment has zero width: {:?}", seg);
        assert_eq!(
            seg.width, jlcpcb_trace_width,
            "Segment width {:?} doesn't match JLCPCB min_trace_width {:?}",
            seg.width, jlcpcb_trace_width
        );
    }

    // 3. All segments on valid copper layers (TopCopper or BottomCopper for 2-layer)
    for seg in &result.routes {
        assert!(
            matches!(seg.layer, Layer::TopCopper | Layer::BottomCopper),
            "Segment on invalid layer {:?} (expected TopCopper or BottomCopper)",
            seg.layer
        );
    }

    // 4. All vias have correct drill size matching JLCPCB min_via_drill
    let jlcpcb_via_drill = cypcb_core::Nm::from_mm(0.3);
    for via in &result.vias {
        assert_eq!(
            via.drill, jlcpcb_via_drill,
            "Via drill {:?} doesn't match JLCPCB min_via_drill {:?}",
            via.drill, jlcpcb_via_drill
        );
        assert!(
            matches!(via.start_layer, Layer::TopCopper | Layer::BottomCopper),
            "Via start_layer {:?} not on copper",
            via.start_layer
        );
        assert!(
            matches!(via.end_layer, Layer::TopCopper | Layer::BottomCopper),
            "Via end_layer {:?} not on copper",
            via.end_layer
        );
    }

    // 5. Quality bounds
    assert!(
        total_length_mm < 500.0,
        "Total length {:.1}mm exceeds generous bound of 500mm",
        total_length_mm
    );
    assert!(
        metrics.via_count < 20,
        "Via count {} exceeds bound of 20",
        metrics.via_count
    );

    // 6. Segments should be meaningful (collinear merge worked — fewer segments than raw steps)
    assert!(
        result.routes.len() > 0,
        "Should have at least one route segment"
    );
    assert!(
        result.routes.len() < 500,
        "Segment count {} suspiciously high — collinear merge may not be working",
        result.routes.len()
    );
}

#[test]
fn blink_apply_routes_compatibility() {
    let mut world = parse_board("examples/blink.cypcb");
    let library = FootprintLibrary::new();
    let rules = test_rules();
    let config = AutorouteConfig::default();

    let result = route_board(&mut world, &library, &rules, &config);

    // Must be complete for this test to be meaningful
    assert!(
        matches!(result.status, RoutingStatus::Complete),
        "Prerequisite: blink must route completely, got {:?}",
        result.status
    );

    let segment_count = result.routes.len();
    let via_count = result.vias.len();

    // Apply routes — should not panic
    apply_routes(&mut world, &result);

    // Verify entities were spawned by querying the world
    use cypcb_world::components::trace::{Trace, TraceSource, Via};

    // Scope trace query to avoid borrow conflict with via query
    let (trace_count, total_trace_segments) = {
        let ecs = world.ecs_mut();
        let mut trace_query = ecs.query::<&Trace>();
        let traces: Vec<_> = trace_query.iter(ecs).collect();
        assert!(
            !traces.is_empty(),
            "apply_routes should have spawned Trace entities"
        );

        // All traces should be autorouted and unlocked
        for trace in &traces {
            assert_eq!(
                trace.source,
                TraceSource::Autorouted,
                "Spawned trace should have Autorouted source"
            );
            assert!(!trace.locked, "Spawned trace should not be locked");
            assert!(
                trace.width.raw() > 0,
                "Spawned trace should have non-zero width"
            );
        }

        let total_segs: usize = traces.iter().map(|t| t.segments.len()).sum();
        (traces.len(), total_segs)
    };

    assert_eq!(
        total_trace_segments, segment_count,
        "Total trace segments after apply_routes should match RouteSegment count"
    );

    // Check vias exist (if any were generated)
    let via_entity_count = {
        let ecs = world.ecs_mut();
        let mut via_query = ecs.query::<&Via>();
        let via_entities: Vec<_> = via_query.iter(ecs).collect();

        for via in &via_entities {
            assert!(!via.locked, "Spawned via should not be locked");
            assert!(via.drill.raw() > 0, "Via should have non-zero drill size");
        }

        via_entities.len()
    };

    assert_eq!(
        via_entity_count, via_count,
        "Via entity count should match ViaPlacement count"
    );

    eprintln!(
        "apply_routes OK: {} trace entities ({} segments), {} vias",
        trace_count, total_trace_segments, via_entity_count
    );
}

#[test]
fn routed_output_passes_drc() {
    use cypcb_drc::{run_drc, DesignRules};

    let mut world = parse_board("examples/blink.cypcb");
    let library = FootprintLibrary::new();
    let rules = test_rules();
    let config = AutorouteConfig::default();

    // Route the board
    let result = route_board(&mut world, &library, &rules, &config);
    assert!(
        matches!(result.status, RoutingStatus::Complete),
        "Prerequisite: blink must route completely, got {:?}",
        result.status
    );

    // Apply routes to the board world (spawns Trace and Via entities)
    apply_routes(&mut world, &result);

    // Rebuild spatial index after adding traces/vias.
    // The current rebuild_spatial_index only indexes components (Position + FootprintRef),
    // so traces won't be in the spatial index. DRC clearance checks on traces are
    // not yet supported — this test validates that routing doesn't break existing
    // component-level DRC.
    world.rebuild_spatial_index(|name| {
        library.get(name).map(|fp| fp.courtyard).unwrap_or_else(|| {
            cypcb_core::Rect::from_center_size(
                cypcb_core::Point::ORIGIN,
                (cypcb_core::Nm::from_mm(1.0), cypcb_core::Nm::from_mm(1.0)),
            )
        })
    });

    // Run DRC on the routed board
    let drc_rules = DesignRules::jlcpcb_2layer();
    let drc_result = run_drc(&mut world, &drc_rules);

    // Print violations for diagnostics before asserting
    if !drc_result.passed() {
        eprintln!("\n╔══════════════════════════════════════════════╗");
        eprintln!(
            "║        DRC Violations ({} found)             ║",
            drc_result.violation_count()
        );
        eprintln!("╠══════════════════════════════════════════════╣");
        for v in &drc_result.violations {
            eprintln!("║ {:?}: {}", v.kind, v.message);
            eprintln!("║   at {:?}", v.location);
        }
        eprintln!("╚══════════════════════════════════════════════╝\n");
    }

    assert!(
        drc_result.passed(),
        "Routed blink.cypcb should pass DRC with zero violations, got {} violations",
        drc_result.violation_count()
    );

    eprintln!(
        "DRC passed: 0 violations in {}ms (rules: clearance, drill, trace_width, connectivity, keepout, edge_clearance, annular_ring)",
        drc_result.duration_ms
    );
}

/// Performance benchmarks for routing — run with `cargo test -p cypcb-autoroute -- benchmark --ignored --nocapture`
///
/// Baseline numbers recorded for S08 optimization comparison.
/// Gated behind `#[ignore]` + `#[cfg(not(target_arch = "wasm32"))]` since
/// std::time::Instant is not available in WASM.
#[test]
#[ignore]
#[cfg(not(target_arch = "wasm32"))]
fn benchmark_routing_time() {
    use std::time::Instant;

    let library = FootprintLibrary::new();
    let rules = test_rules();
    let config = AutorouteConfig::default();

    eprintln!("\n╔══════════════════════════════════════════════════════════╗");
    eprintln!("║          Autorouter Performance Benchmarks              ║");
    eprintln!("╠══════════════════════════════════════════════════════════╣");

    // Benchmark 1: Grid construction for blink.cypcb (60x40mm, 8 components)
    {
        let mut world = parse_board("examples/blink.cypcb");
        let resolution = config.resolve_grid_resolution(&rules);

        let start = Instant::now();
        let _grid = RoutingGrid::from_board(&mut world, &library, &rules, resolution)
            .expect("Grid construction should succeed");
        let elapsed = start.elapsed();

        eprintln!(
            "║ Grid construction (blink, 60x40mm):  {:>8.2}ms          ║",
            elapsed.as_secs_f64() * 1000.0
        );
    }

    // Benchmark 2: Full routing of routing-test.cypcb (3 components, 3 nets)
    {
        let mut world = parse_board("examples/routing-test.cypcb");
        let start = Instant::now();
        let result = route_board(&mut world, &library, &rules, &config);
        let elapsed = start.elapsed();

        let metrics = calculate_metrics(&result);
        eprintln!(
            "║ Route routing-test.cypcb (3 nets):   {:>8.2}ms {:?}  ║",
            elapsed.as_secs_f64() * 1000.0,
            result.status
        );
        eprintln!(
            "║   segments={} vias={} length={:.1}mm                    ║",
            result.routes.len(),
            metrics.via_count,
            metrics.total_length.raw() as f64 / 1_000_000.0
        );
    }

    // Benchmark 3: Full routing of blink.cypcb (8 components, 7 nets)
    {
        let mut world = parse_board("examples/blink.cypcb");
        let start = Instant::now();
        let result = route_board(&mut world, &library, &rules, &config);
        let elapsed = start.elapsed();

        let metrics = calculate_metrics(&result);
        eprintln!(
            "║ Route blink.cypcb (7 nets):          {:>8.2}ms {:?}  ║",
            elapsed.as_secs_f64() * 1000.0,
            result.status
        );
        eprintln!(
            "║   segments={} vias={} length={:.1}mm                    ║",
            result.routes.len(),
            metrics.via_count,
            metrics.total_length.raw() as f64 / 1_000_000.0
        );
    }

    eprintln!("╠══════════════════════════════════════════════════════════╣");
    eprintln!("║ Baselines recorded for S08 optimization comparison.     ║");
    eprintln!("╚══════════════════════════════════════════════════════════╝\n");
}

// ============================================================================
// Synthetic board generator
// ============================================================================

/// Generate a synthetic board with the given number of components for benchmarking.
///
/// Strategy:
/// - Components are placed in a grid layout on a proportionally-sized board
/// - Each component uses the "0805" footprint (2-pad SMD)
/// - Nets connect adjacent pairs (horizontal neighbors) for realistic connectivity
/// - Board dimensions scale with sqrt(component_count) to maintain reasonable density
///
/// Returns (BoardWorld, FootprintLibrary, net_count).
fn generate_synthetic_board(component_count: usize) -> (BoardWorld, FootprintLibrary, usize) {
    use cypcb_core::Nm;
    use cypcb_world::{
        FootprintRef, NetConnections, NetId, PinConnection, Position, RefDes, Rotation, Value,
    };

    let library = FootprintLibrary::new();
    let mut world = BoardWorld::new();

    // Grid layout: roughly square arrangement
    let cols = (component_count as f64).sqrt().ceil() as usize;
    let rows = (component_count + cols - 1) / cols;

    // Component spacing: 3mm pitch (0805 is ~2x1.25mm, so 3mm gives clearance)
    let pitch_mm = 3.0;
    // Board size: grid dimensions + margin
    let margin_mm = 5.0;
    let board_width_mm = (cols as f64) * pitch_mm + 2.0 * margin_mm;
    let board_height_mm = (rows as f64) * pitch_mm + 2.0 * margin_mm;

    world.set_board(
        "SyntheticBenchmark".to_string(),
        (Nm::from_mm(board_width_mm), Nm::from_mm(board_height_mm)),
        2,
    );

    // Place components in a grid and connect horizontal neighbors
    let mut net_id_counter: u32 = 0;
    let mut placed = 0;

    for row in 0..rows {
        for col in 0..cols {
            if placed >= component_count {
                break;
            }

            let x_mm = margin_mm + (col as f64) * pitch_mm + pitch_mm / 2.0;
            let y_mm = margin_mm + (row as f64) * pitch_mm + pitch_mm / 2.0;

            // Build net connections for this component's 2 pads
            let mut nets = NetConnections::new();

            // Pad "1" connects to the net shared with the left neighbor (or a new net)
            // Pad "2" connects to the net shared with the right neighbor (or a new net)
            if col > 0 {
                // Share net with left neighbor's pad "2"
                // The left neighbor's pad "2" net_id is (net_id_counter - 1) since we
                // assigned it in the previous iteration.
                let shared_net = NetId(net_id_counter - 1);
                nets.add(PinConnection::new("1", shared_net));
                world.intern_net(&format!("N{}", shared_net.0));
            } else {
                // First in row: pad 1 gets a new net (will be unconnected unless
                // we wrap around, which we don't — some nets will be single-pad)
                let net = NetId(net_id_counter);
                nets.add(PinConnection::new("1", net));
                world.intern_net(&format!("N{}", net.0));
                net_id_counter += 1;
            }

            // Pad "2" always gets a new net (right-side connection point)
            let right_net = NetId(net_id_counter);
            nets.add(PinConnection::new("2", right_net));
            world.intern_net(&format!("N{}", right_net.0));
            net_id_counter += 1;

            let refdes = format!("R{}", placed + 1);
            world.spawn_component(
                RefDes::new(&refdes),
                Value::new("100R"),
                Position::from_mm(x_mm, y_mm),
                Rotation::ZERO,
                FootprintRef::new("0805"),
                nets,
            );

            placed += 1;
        }
    }

    // Count nets that have >=2 pads (routable nets)
    // With our scheme: each horizontal pair shares a net via pad1-pad2 connections.
    // Nets shared between neighbors are the ones that matter.
    // Total unique nets = net_id_counter, but many are single-pad (unroutable).
    // The router's ratsnest extraction handles this — it only routes nets with >=2 pads.
    let total_nets = net_id_counter as usize;

    eprintln!(
        "Synthetic board: {}x{}mm, {} components in {}x{} grid, {} total nets",
        board_width_mm as u32, board_height_mm as u32, placed, cols, rows, total_nets,
    );

    (world, library, total_nets)
}

/// 500-component benchmark — target: <30s in release mode.
///
/// Run with: `cargo test --release -p cypcb-autoroute -- benchmark_500 --ignored --nocapture`
#[test]
#[ignore]
#[cfg(not(target_arch = "wasm32"))]
fn benchmark_500_component() {
    use std::time::Instant;

    let rules = test_rules();
    let config = AutorouteConfig::default();

    let (mut world, library, total_nets) = generate_synthetic_board(500);

    // Get board dimensions for reporting
    let (board_size, _) = world.board_info().expect("board should exist");
    let board_w_mm = board_size.width.to_mm();
    let board_h_mm = board_size.height.to_mm();

    // Compute expected grid dimensions for reporting
    let resolution = config.resolve_adaptive_grid_resolution(
        &rules,
        board_size.width.raw(),
        board_size.height.raw(),
    );
    let grid_w = (board_size.width.raw() + resolution - 1) / resolution;
    let grid_h = (board_size.height.raw() + resolution - 1) / resolution;

    eprintln!("\n╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║          500-Component Benchmark                            ║");
    eprintln!("╠══════════════════════════════════════════════════════════════╣");
    eprintln!("║ Components:     500                                         ║");
    eprintln!("║ Total nets:     {:<44}║", total_nets);
    eprintln!(
        "║ Board:          {:.0}x{:.0}mm{:>40}║",
        board_w_mm, board_h_mm, ""
    );
    eprintln!(
        "║ Grid:           {}x{} @ {:.0}µm{:>33}║",
        grid_w,
        grid_h,
        resolution as f64 / 1_000.0,
        ""
    );

    let start = Instant::now();
    let result = route_board(&mut world, &library, &rules, &config);
    let elapsed = start.elapsed();

    let metrics = calculate_metrics(&result);
    let elapsed_secs = elapsed.as_secs_f64();

    eprintln!("║ Status:         {:?}{:>40}║", result.status, "");
    eprintln!("║ Time:           {:.2}s{:>44}║", elapsed_secs, "");
    eprintln!("║ Segments:       {:<44}║", result.routes.len());
    eprintln!("║ Vias:           {:<44}║", metrics.via_count);
    eprintln!(
        "║ Length:         {:.1}mm{:>42}║",
        metrics.total_length.raw() as f64 / 1_000_000.0,
        ""
    );

    // Calculate routing completion rate
    let routed_nets = match result.status {
        RoutingStatus::Complete => total_nets,
        RoutingStatus::Partial { unrouted_count, .. } => total_nets.saturating_sub(unrouted_count),
        RoutingStatus::Failed { .. } => 0,
    };
    let completion_pct = if total_nets > 0 {
        (routed_nets as f64 / total_nets as f64) * 100.0
    } else {
        100.0
    };

    eprintln!(
        "║ Completion:     {:.1}% ({}/{}){:>33}║",
        completion_pct, routed_nets, total_nets, ""
    );
    eprintln!("╚══════════════════════════════════════════════════════════════╝\n");

    // Primary assertion: <30s
    assert!(
        elapsed_secs < 30.0,
        "500-component routing took {:.2}s, exceeds 30s target",
        elapsed_secs
    );

    // Secondary assertion: at least 90% of routable nets completed
    // (some may fail on a synthetic board with tight spacing)
    assert!(
        completion_pct >= 90.0,
        "Routing completion {:.1}% is below 90% threshold",
        completion_pct
    );
}
