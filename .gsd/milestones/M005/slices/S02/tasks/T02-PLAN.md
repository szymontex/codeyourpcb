---
estimated_steps: 5
estimated_files: 3
---

# T02: Add zero-unrouted proof test and rebuild WASM

**Slice:** S02 — Routing Quality — 0 Unrouted on Blink LED
**Milestone:** M005

## Description

Add the S02→S03 boundary artifact: an explicit `test_blink_led_zero_unrouted` integration test that asserts `unrouted == 0` on the Blink LED board with detailed metrics output. Then rebuild the WASM binary so the PathFinder fix is available in the browser for S03 E2E tests and S01 Worker routing.

## Steps

1. Open `crates/cypcb-autoroute/tests/integration.rs` and add a new test `test_blink_led_zero_unrouted` after the existing `route_blink_board` test. Use these existing helpers already defined in the file:
   - `workspace_path(relative: &str) -> PathBuf` — resolves paths relative to workspace root
   - `parse_board(relative_path: &str) -> BoardWorld` — parses `.cypcb` files
   - `test_rules() -> PresetRuleSet` — returns JLCPCB 2-layer rules (search the file for its definition)

2. The test should:
   ```rust
   #[test]
   fn test_blink_led_zero_unrouted() {
       let mut world = parse_board("examples/blink.cypcb");
       let library = FootprintLibrary::new();
       let rules = test_rules();
       let config = AutorouteConfig::default();

       let result = route_board(&mut world, &library, &rules, &config);
       let metrics = calculate_metrics(&result);

       // Print detailed metrics for debugging
       eprintln!("\n=== Blink LED Zero-Unrouted Proof ===");
       eprintln!("Status:     {:?}", result.status);
       eprintln!("Segments:   {}", result.routes.len());
       eprintln!("Vias:       {}", metrics.via_count);
       eprintln!("Length:     {:.1} mm", metrics.total_length.raw() as f64 / 1_000_000.0);
       eprintln!("Unrouted:   {}", metrics.unrouted_nets);
       eprintln!("=====================================\n");

       // Primary assertion: zero unrouted
       assert_eq!(
           metrics.unrouted_nets, 0,
           "Expected 0 unrouted nets on Blink LED, got {}",
           metrics.unrouted_nets
       );

       // Status must be Complete
       assert!(
           matches!(result.status, RoutingStatus::Complete),
           "Expected RoutingStatus::Complete, got {:?}",
           result.status
       );

       // Sanity checks
       assert!(result.routes.len() > 0, "Must have route segments");
       assert!(metrics.via_count < 20, "Via count {} exceeds 20", metrics.via_count);
   }
   ```

3. Run the new test:
   ```bash
   cargo test --release -p cypcb-autoroute -- test_blink_led_zero_unrouted --nocapture
   ```

4. Run the full autoroute test suite for regression check:
   ```bash
   cargo test --release -p cypcb-autoroute
   ```

5. Rebuild the WASM binary so the PathFinder fix is available in the browser:
   ```bash
   cd viewer && bash build-wasm.sh
   ```
   If `wasm-pack` is not installed, install it: `cargo install wasm-pack`. If `wasm32-unknown-unknown` target is missing, the script adds it automatically.

## Must-Haves

- [ ] `test_blink_led_zero_unrouted` test exists in `integration.rs`
- [ ] Test asserts `unrouted_nets == 0` and `RoutingStatus::Complete`
- [ ] Test passes in release mode
- [ ] Full `cargo test --release -p cypcb-autoroute` passes (no regressions)
- [ ] WASM binary rebuilt in `viewer/pkg/`

## Verification

- `cargo test --release -p cypcb-autoroute -- test_blink_led_zero_unrouted --nocapture` — passes, prints 0 unrouted
- `cargo test --release -p cypcb-autoroute` — all tests pass
- `ls -la viewer/pkg/cypcb_render_bg.wasm` — file exists and is recent

## Inputs

- `crates/cypcb-autoroute/tests/integration.rs` — existing test file with `route_blink_board` test and helpers
- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — already fixed by T01 (ghost cell bug removed)
- `viewer/build-wasm.sh` — WASM build script

## Observability Impact

- **New signal:** `test_blink_led_zero_unrouted` prints a diagnostic block with Status, Segments, Vias, Length, and Unrouted count to stderr on every run. Run with `--nocapture` to see it.
- **Inspection:** `cargo test --release -p cypcb-autoroute -- test_blink_led_zero_unrouted --nocapture` — grep for `Unrouted:` to check routing health.
- **Failure state:** If routing regresses, the test fails with `assert_eq!(metrics.unrouted_nets, 0, ...)` showing the exact unrouted count. Status mismatch is also caught explicitly.
- **WASM artifact:** `viewer/pkg/cypcb_render_bg.wasm` timestamp indicates when the last WASM build occurred — downstream slices can verify freshness with `ls -la`.

## Expected Output

- `crates/cypcb-autoroute/tests/integration.rs` — new `test_blink_led_zero_unrouted` test added
- `viewer/pkg/cypcb_render_bg.wasm` — rebuilt WASM binary containing the PathFinder fix
