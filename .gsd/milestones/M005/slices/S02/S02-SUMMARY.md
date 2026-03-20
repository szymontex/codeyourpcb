---
id: S02
parent: M005
milestone: M005
provides:
  - Ghost-cell-free PathFinder rip-up — 0 unrouted on Blink LED (all 25 connections, 7 nets)
  - test_blink_led_zero_unrouted integration test as hard contract for routing quality
  - Rebuilt WASM binary (viewer/pkg/cypcb_render_bg.wasm) containing the fix
requires:
  - slice: none
    provides: none
affects:
  - S03
key_files:
  - crates/cypcb-autoroute/src/pathfinder_v2.rs
  - crates/cypcb-autoroute/tests/integration.rs
  - viewer/pkg/cypcb_render_bg.wasm
key_decisions:
  - Remove mark_route(u32::MAX) loop entirely rather than reorder — clear_route(net_id) is self-sufficient and the sentinel value was poisoning the grid
patterns_established:
  - Rip-up must only call congestion_map.unmark_net then grid.clear_route — never re-mark cells with a sentinel value first
  - Zero-unrouted proof tests print a diagnostic block to stderr and assert both metrics.unrouted_nets == 0 and RoutingStatus::Complete
observability_surfaces:
  - "cargo test --release -p cypcb-autoroute -- test_blink_led_zero_unrouted --nocapture: prints Status/Segments/Vias/Length/Unrouted diagnostics"
  - "cargo test --release -p cypcb-autoroute -- route_blink_board --nocapture: prints per-run metrics table with RoutingStatus and Completion %"
  - "ls -la viewer/pkg/cypcb_render_bg.wasm: WASM artifact freshness check"
drill_down_paths:
  - .gsd/milestones/M005/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M005/slices/S02/tasks/T02-SUMMARY.md
duration: 25m
verification_result: passed
completed_at: 2026-03-19
---

# S02: Routing Quality — 0 Unrouted on Blink LED

**Removed ghost-cell bug from PathFinder rip-up loop — Blink LED now routes all 25 connections (7 nets) to 100% completion with 0 unrouted, proven by cargo test and rebuilt into WASM binary**

## What Happened

The Blink LED board had 5/25 unrouted connections because PathFinder's rip-up block was poisoning the routing grid. Before calling `grid.clear_route(net_id)`, it ran `grid.mark_route(x, y, layer, u32::MAX)` on every cell of the ripped-up net. This overwrote each cell's `net_map` entry from the real `net_id` to `u32::MAX`, so when `clear_route()` scanned for cells matching `net_id`, it found nothing — leaving `CELL_TRACE` flags permanently set as invisible ghost obstacles. Each rip-up iteration accumulated more phantom blocked cells.

**T01** removed the 3-line poisoning loop (surgical fix). The corrected rip-up block now does only `congestion_map.unmark_net(&cells)` followed by `grid.clear_route(net_id)`, which correctly clears both `CELL_TRACE` and `net_map`. After the fix, `route_blink_board` immediately passed with `RoutingStatus::Complete` — 45 segments, 6 vias, 100% completion.

**T02** added the `test_blink_led_zero_unrouted` proof test to `integration.rs` with explicit assertions: `unrouted_nets == 0`, `RoutingStatus::Complete`, `routes.len() > 0`, `via_count < 20`. The test prints a diagnostic block for visual confirmation. Then rebuilt the WASM binary via `build-wasm.sh` so the fix is available for S03 E2E tests and S01's Web Worker routing.

## Verification

| # | Command | Result | Details |
|---|---------|--------|---------|
| 1 | `cargo test --release -p cypcb-autoroute -- route_blink_board --nocapture` | ✅ pass | Status: Complete, 45 segments, 6 vias, 100% completion |
| 2 | `cargo test --release -p cypcb-autoroute -- test_blink_led_zero_unrouted --nocapture` | ✅ pass | Unrouted: 0, 45 segments, 6 vias, 182.5mm length |
| 3 | `cargo test --release -p cypcb-autoroute --test integration` | ✅ pass | 6 passed, 2 ignored, 0 failed |
| 4 | `ls -la viewer/pkg/cypcb_render_bg.wasm` | ✅ exists | 637,460 bytes, rebuilt with PathFinder fix |

Note: `cargo test --release -p cypcb-autoroute` (full suite including `benchmark_validation.rs`) has a pre-existing `benchmark_regression` failure (composite score 15543.6 > threshold 5501.0) unrelated to S02 work. All 124 unit tests + 6 integration tests pass.

## Requirements Advanced

- R204 (0 Unrouted on Blink LED) — PathFinder now routes all 25 connections with 0 unrouted. Proven by `test_blink_led_zero_unrouted` integration test and `route_blink_board` metrics table. WASM binary rebuilt with the fix.

## Requirements Validated

- R204 — `test_blink_led_zero_unrouted` asserts `unrouted_nets == 0` and `RoutingStatus::Complete` in cargo test. WASM binary contains the fix for browser verification by S03.

## New Requirements Surfaced

- none

## Requirements Invalidated or Re-scoped

- none

## Deviations

- Tree-sitter parser source (`grammar/src/parser.c`) was not pre-generated in the worktree — required `npm install && npx tree-sitter generate` before cargo builds. This is a worktree setup issue, not a plan deviation.

## Known Limitations

- `benchmark_regression` test in `benchmark_validation.rs` fails pre-existingly (composite score 15543.6 vs 5501.0 threshold). This is unrelated to S02 — the KiCad-based scoring benchmark baseline has drifted. Not addressed in this slice.
- Browser-side verification of 0 unrouted (WASM in Worker) is not proven in this slice — that's S03's job via E2E tests.

## Follow-ups

- The pre-existing `benchmark_regression` threshold (5501.0) appears to be stale — the composite score of 15543.6 suggests the benchmark fixtures or scoring formula changed without updating the threshold. Should be investigated in a separate slice or as S03 setup.

## Files Created/Modified

- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — Removed 3-line mark_route(u32::MAX) poisoning loop from rip-up block
- `crates/cypcb-autoroute/tests/integration.rs` — Added `test_blink_led_zero_unrouted` proof test
- `viewer/pkg/cypcb_render_bg.wasm` — Rebuilt WASM binary containing PathFinder fix

## Forward Intelligence

### What the next slice should know
- S02's boundary artifact for S03 is the `test_blink_led_zero_unrouted` test (asserts 0 unrouted in native cargo test) and the rebuilt `cypcb_render_bg.wasm` in `viewer/pkg/`. S03 E2E tests should verify the WASM result matches — check `__routingWorker.lastResult` or status text for 0 unrouted after routing Blink LED in browser.
- The routing produces 45 segments, 6 vias, 182.5mm total length on Blink LED — these are stable baseline numbers S03 can optionally assert.

### What's fragile
- The `benchmark_regression` test threshold is stale at 5501.0 while actual composite is 15543.6. Running `cargo test --release -p cypcb-autoroute` without `--test integration` filter will fail on this unrelated test. S03 should be aware when running full test suites.

### Authoritative diagnostics
- `cargo test --release -p cypcb-autoroute -- test_blink_led_zero_unrouted --nocapture` is the primary proof — it prints `Unrouted: 0` and asserts it. This is the fastest way to check if the routing quality guarantee holds.
- `route_blink_board` test prints a full metrics table (status, segments, vias, length, completion %) — useful for quick health check.

### What assumptions changed
- The plan assumed the root cause was "suspected: PathFinder convergence failure on multi-pad nets" — the actual root cause was a ghost-cell bug in the rip-up loop that poisoned the grid. The fix was 3 lines removed, not a convergence algorithm change.
