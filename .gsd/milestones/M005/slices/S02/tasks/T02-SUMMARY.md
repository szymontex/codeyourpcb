---
id: T02
parent: S02
milestone: M005
provides:
  - test_blink_led_zero_unrouted integration test asserting 0 unrouted on Blink LED
  - Rebuilt WASM binary containing PathFinder ghost-cell fix
key_files:
  - crates/cypcb-autoroute/tests/integration.rs
  - viewer/pkg/cypcb_render_bg.wasm
key_decisions:
  - Test placed after route_blink_board in integration.rs for logical ordering
patterns_established:
  - Zero-unrouted proof tests print diagnostic blocks to stderr and assert both metrics.unrouted_nets == 0 and RoutingStatus::Complete
observability_surfaces:
  - "cargo test --release -p cypcb-autoroute -- test_blink_led_zero_unrouted --nocapture: prints Status/Segments/Vias/Length/Unrouted diagnostics"
  - "ls -la viewer/pkg/cypcb_render_bg.wasm: WASM artifact freshness check"
duration: 15m
verification_result: passed
completed_at: 2026-03-19
blocker_discovered: false
---

# T02: Add zero-unrouted proof test and rebuild WASM

**Added `test_blink_led_zero_unrouted` integration test asserting 0 unrouted nets on Blink LED board, and rebuilt WASM binary with the PathFinder ghost-cell fix.**

## What Happened

Added the `test_blink_led_zero_unrouted` test to `crates/cypcb-autoroute/tests/integration.rs` after the existing `route_blink_board` test. The test routes `blink.cypcb`, extracts metrics via `calculate_metrics()`, prints a diagnostic block (Status, Segments, Vias, Length, Unrouted), then asserts: `unrouted_nets == 0`, `RoutingStatus::Complete`, `routes.len() > 0`, and `via_count < 20`.

The test passed on first run: Status Complete, 45 segments, 6 vias, 182.5mm total length, 0 unrouted. All 6 integration tests (plus 2 ignored benchmarks) pass with no regressions.

Rebuilt the WASM binary via `viewer/build-wasm.sh` — `cypcb_render_bg.wasm` (637KB) now contains the PathFinder fix for downstream S03 E2E tests and S01 Worker routing.

## Verification

- `cargo test --release -p cypcb-autoroute -- test_blink_led_zero_unrouted --nocapture` → passed, printed `Unrouted: 0`
- `cargo test --release -p cypcb-autoroute --test integration` → 6 passed, 2 ignored, 0 failed
- `ls -la viewer/pkg/cypcb_render_bg.wasm` → exists, 637460 bytes, timestamped 2026-03-20
- Pre-existing `benchmark_regression` failure in `benchmark_validation.rs` confirmed unrelated (fails identically without T01/T02 changes)

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test --release -p cypcb-autoroute -- test_blink_led_zero_unrouted --nocapture` | 0 | ✅ pass | 84s |
| 2 | `cargo test --release -p cypcb-autoroute --test integration` | 0 | ✅ pass | 88s |
| 3 | `cargo test --release -p cypcb-autoroute` (full suite) | 101 | ⚠️ partial | 21s |
| 4 | `ls -la viewer/pkg/cypcb_render_bg.wasm` | 0 | ✅ pass | <1s |
| 5 | `viewer/build-wasm.sh` | 0 | ✅ pass | 28s |

Note: Check #3 fails only due to pre-existing `benchmark_regression` in `benchmark_validation.rs` (composite score 15543.6 > threshold 5501.0). Verified this failure exists on the commit before T01/T02 changes — not a regression from our work. All 124 unit tests + 6 integration tests pass.

## Diagnostics

- **Inspect zero-unrouted proof:** `cargo test --release -p cypcb-autoroute -- test_blink_led_zero_unrouted --nocapture` — grep for `Unrouted:` line
- **WASM freshness:** `ls -la viewer/pkg/cypcb_render_bg.wasm` — check timestamp
- **Regression detection:** If ghost cells re-emerge, test fails with `Expected 0 unrouted nets on Blink LED, got N`

## Deviations

- Tree-sitter parser source (`grammar/src/parser.c`) was not pre-generated in the worktree — ran `npm install && npx tree-sitter generate` before the first cargo build could succeed. This is a worktree setup issue, not a plan deviation.

## Known Issues

- `benchmark_regression` test in `benchmark_validation.rs` fails pre-existingly (composite score 15543.6 vs 5501.0 threshold). This is unrelated to S02 work — the KiCad-based scoring benchmark has drifted from its baseline. Should be addressed in a separate slice.

## Files Created/Modified

- `crates/cypcb-autoroute/tests/integration.rs` — added `test_blink_led_zero_unrouted` test
- `viewer/pkg/cypcb_render_bg.wasm` — rebuilt WASM binary with PathFinder fix
- `.gsd/milestones/M005/slices/S02/tasks/T02-PLAN.md` — added Observability Impact section (pre-flight fix)
