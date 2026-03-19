---
id: T01
parent: S02
milestone: M005
provides:
  - Ghost-cell-free rip-up in PathFinder — grid.clear_route(net_id) now works correctly
key_files:
  - crates/cypcb-autoroute/src/pathfinder_v2.rs
key_decisions:
  - Remove mark_route(u32::MAX) loop entirely rather than reorder; clear_route() is self-sufficient
patterns_established:
  - Rip-up must only call congestion_map.unmark_net then grid.clear_route — never re-mark cells with a sentinel value first
observability_surfaces:
  - route_blink_board test prints per-run metrics table with RoutingStatus, segments, vias, completion %
duration: 10m
verification_result: passed
completed_at: 2026-03-18T23:15:00+01:00
blocker_discovered: false
---

# T01: Fix rip-up ghost cell bug in PathFinder

**Removed mark_route(u32::MAX) poisoning loop from PathFinder rip-up — all 7 Blink LED nets now route to 100% completion**

## What Happened

The rip-up block in `pathfinder_v2.rs` (line ~255) was calling `grid.mark_route(x, y, layer, u32::MAX)` on every cell before `grid.clear_route(net_id)`. This overwrote each cell's `net_map` entry from `net_id` to `u32::MAX`, so when `clear_route()` scanned for `net_map == net_id`, it found nothing — leaving `CELL_TRACE` flags permanently set as ghost obstacles.

The fix was surgical: removed the 3-line `for` loop and its comment. The corrected rip-up block is now `congestion_map.unmark_net(&cells)` followed by `grid.clear_route(net_id)`, which correctly clears both `CELL_TRACE` and `net_map` for all cells belonging to the net.

## Verification

Ran `cargo test --release -p cypcb-autoroute -- route_blink_board --nocapture`. Test passed with `RoutingStatus::Complete`, 45 segments, 6 vias, 100% completion. Previously this test failed with 5/25 unrouted connections.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test --release -p cypcb-autoroute -- route_blink_board --nocapture` | 0 | ✅ pass | 81s |
| 2 | `cargo test --release -p cypcb-autoroute -- test_blink_led_zero_unrouted --nocapture` | — | ⏭️ skip (T02) | — |
| 3 | `cargo test --release -p cypcb-autoroute` (full suite) | — | ⏭️ skip (T02) | — |
| 4 | `viewer/pkg/cypcb_render_bg.wasm` freshness | — | ⏭️ skip (T02) | — |

## Diagnostics

Run `cargo test --release -p cypcb-autoroute -- route_blink_board --nocapture` and check the metrics table for `RoutingStatus::Complete` and `Completion: 100%`. If ghost cells regress, the status will show `Partial` with non-zero unrouted count.

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — Removed 3-line mark_route(u32::MAX) loop from rip-up block (~line 255)
- `.gsd/milestones/M005/slices/S02/S02-PLAN.md` — Added Observability / Diagnostics section
- `.gsd/milestones/M005/slices/S02/tasks/T01-PLAN.md` — Added Observability Impact section
- `.gsd/KNOWLEDGE.md` — Added rip-up ghost cell pattern entry
