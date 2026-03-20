# S02: Routing Quality — 0 Unrouted on Blink LED — UAT

**Milestone:** M005
**Written:** 2026-03-19

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: This slice is a pure Rust bug fix + test addition. The proof is cargo test output — no browser, UI, or runtime needed. The WASM binary is a build artifact verified by file existence and freshness.

## Preconditions

- Working Rust toolchain with `cargo` available
- Working directory is the project root (or M005 worktree)
- `examples/blink.cypcb` fixture exists (Blink LED template board)
- `viewer/build-wasm.sh` is executable (for WASM rebuild verification)

## Smoke Test

Run `cargo test --release -p cypcb-autoroute -- test_blink_led_zero_unrouted --nocapture` and verify the output contains `Unrouted: 0` and the test passes.

## Test Cases

### 1. Zero Unrouted Proof Test

1. Run `cargo test --release -p cypcb-autoroute -- test_blink_led_zero_unrouted --nocapture`
2. Check stderr output for the diagnostic block
3. **Expected:** Output contains `Status: Complete`, `Unrouted: 0`, `Segments: 45`, `Vias: 6`. Test result is `ok`.

### 2. Route Blink Board Full Metrics

1. Run `cargo test --release -p cypcb-autoroute -- route_blink_board --nocapture`
2. Check the metrics table in output
3. **Expected:** `Status: Complete`, `Completion: 100%`, `Segments: 45`, `Vias: 6`, `Total length: 182.5 mm`. Test result is `ok`.

### 3. Full Integration Suite Regression Check

1. Run `cargo test --release -p cypcb-autoroute --test integration`
2. Check test results summary
3. **Expected:** 6 passed, 2 ignored, 0 failed. No regressions in existing tests (`grid_from_blink`, `route_routing_test_board`, `routed_output_passes_drc`, `blink_apply_routes_compatibility`).

### 4. WASM Binary Contains Fix

1. Run `ls -la viewer/pkg/cypcb_render_bg.wasm`
2. Check file exists and timestamp is after the fix commit
3. **Expected:** File exists, size ~637KB, timestamp is 2026-03-19 or later.

### 5. Ghost Cell Bug Removed

1. Open `crates/cypcb-autoroute/src/pathfinder_v2.rs`
2. Search for `mark_route` in the rip-up section (around the `clear_route` call)
3. **Expected:** No `mark_route(x, y, layer, u32::MAX)` calls exist in the rip-up block. The rip-up block should contain only `congestion_map.unmark_net(&cells)` followed by `grid.clear_route(net_id)`.

## Edge Cases

### Pre-existing Benchmark Failure

1. Run `cargo test --release -p cypcb-autoroute` (full crate suite, not just integration)
2. **Expected:** `benchmark_regression` in `benchmark_validation.rs` fails with composite score ~15543.6 vs threshold 5501.0. This is a pre-existing issue, NOT a regression from S02. All other tests pass.

### Determinism of Routing Output

1. Run `cargo test --release -p cypcb-autoroute -- test_blink_led_zero_unrouted --nocapture` twice
2. Compare Segments, Vias, and Length values
3. **Expected:** Values are identical across runs (45 segments, 6 vias, 182.5mm). Routing is deterministic for the same input.

## Failure Signals

- `test_blink_led_zero_unrouted` fails with `Expected 0 unrouted nets on Blink LED, got N` — ghost cells have regressed or another PathFinder bug was introduced
- `route_blink_board` shows `Status: Partial` or `Completion: <100%` — rip-up loop regression
- `routed_output_passes_drc` fails — the fix may have introduced DRC violations
- `viewer/pkg/cypcb_render_bg.wasm` missing or size drastically different from ~637KB — WASM build broken

## Requirements Proved By This UAT

- R204 (0 Unrouted on Blink LED) — Test cases 1 and 2 directly prove all 25 connections route with 0 unrouted. Test case 3 proves no regressions.

## Not Proven By This UAT

- Browser-side verification of 0 unrouted via WASM Worker — that's S03's E2E test responsibility
- Routing quality on boards other than Blink LED — only the Blink LED template is tested
- Via optimization (6 vias is acceptable but not necessarily optimal)

## Notes for Tester

- The `--release` flag is required for routing tests — debug builds are too slow (>5 minutes vs ~80 seconds)
- The `--nocapture` flag is needed to see diagnostic output — without it, `eprintln!` output is suppressed on passing tests
- The 2 ignored tests (`benchmark_500_component`, `benchmark_routing_time`) are performance benchmarks intentionally excluded from normal test runs
- The pre-existing `benchmark_regression` failure is unrelated to S02 and should be ignored during this UAT
