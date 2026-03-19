# S02 — Routing Quality — 0 Unrouted on Blink LED — Research

**Date:** 2026-03-18
**Slice:** M005/S02
**Requirement:** R204 — 0 Unrouted on Blink LED

## Summary

The PathFinder strategy currently produces 5 unrouted connections (out of 25 total) on the Blink LED board (60×40mm, 8 components, 7 nets). The routing takes ~120s in release mode and results in only 23 trace segments — a severe quality failure on the project's simplest template.

Root cause analysis identified **one confirmed bug and two contributing factors**:

1. **Ghost trace cells from rip-up (confirmed bug):** In `pathfinder_v2.rs` lines 253-258, the rip-up code calls `grid.mark_route(x, y, layer, u32::MAX)` on each cell before `grid.clear_route(net_id)`. The `mark_route` call sets the `CELL_TRACE` flag AND overwrites `net_map` to `u32::MAX`. The subsequent `clear_route(net_id)` searches for `net_map == net_id` — but those cells now have `net_map == u32::MAX`, so nothing gets cleared. Result: **ripped-up cells permanently retain `CELL_TRACE` flag** with no owner. They become invisible obstacles that no net can traverse (not free, not owned by any net, not in any pad zone). Over 50 PathFinder iterations, these ghost cells accumulate and poison the grid.

2. **Large grid with many iterations:** The blink board produces a 945×630 grid (~595K cells per layer). With 50 max iterations and 7 nets, the ghost cell accumulation becomes severe — each iteration that rips up and reroutes creates more ghost obstacles.

3. **Power nets routed last with maximum congestion:** VCC (5 pads, 4 connections) and GND (6 pads, 5 connections) are routed last because `order_nets()` puts power nets at the end. By the time they route, the grid is maximally congested with both real traces and ghost obstacles.

## Recommendation

**Fix the rip-up ghost cell bug first** — this is the root cause. Replace the broken two-step rip-up (mark_route + clear_route) with a correct single-step approach: just call `grid.clear_route(net_id)` which properly clears both the `CELL_TRACE` flag and the `net_map` entry. The manual `mark_route(x, y, layer, u32::MAX)` loop must be removed entirely.

After fixing the bug, verify with `cargo test --release -p cypcb-autoroute -- route_blink_board`. If routing still doesn't reach 0 unrouted, investigate secondary issues:
- Increase `MAX_PATHFINDER_ITERATIONS` from 50 to 100
- Consider grid resolution adjustments for better pad reachability
- Check if pad zone radii are adequate for SOIC-8 pad clearance navigation

Add a new `test_blink_led_zero_unrouted` test that asserts `unrouted == 0` as the primary proof artifact for R204.

## Implementation Landscape

### Key Files

- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — **Primary fix target.** Contains `pathfinder_loop()` with the broken rip-up code at lines 252-258. The `find_path_congestion_augmented()` function is correct (it checks `grid.net_at() == Some(net_id)` for same-net traversal). Fix is surgical: ~5 lines changed.
- `crates/cypcb-autoroute/src/grid.rs` — Contains `RoutingGrid` with `mark_route()`, `clear_route()`, `is_free()`. No changes needed — the APIs are correct, the bug is in how pathfinder_v2 calls them.
- `crates/cypcb-autoroute/src/orchestrator.rs` — Contains `extract_ratsnest()`, `build_spanning_tree()`, `pad_to_grid_node()`, `pad_to_zone()`, `order_nets()`. No changes expected.
- `crates/cypcb-autoroute/tests/integration.rs` — Contains `route_blink_board` test that already asserts `RoutingStatus::Complete`. Currently failing. Will pass after fix.
- `crates/cypcb-autoroute/src/congestion.rs` — CongestionMap tracking. No changes needed — `unmark_net()` correctly decrements occupancy.

### The Bug (exact code)

In `pathfinder_v2.rs`, `pathfinder_loop()`, the rip-up block:

```rust
// BROKEN: mark_route sets CELL_TRACE + net_map=u32::MAX
// Then clear_route(net_id) finds nothing because net_map != net_id anymore
if let Some(cells) = net_cells.remove(&net_id) {
    congestion_map.unmark_net(&cells);
    for &(x, y, layer) in &cells {
        grid.mark_route(x, y, layer as usize, u32::MAX);  // BUG: leaves CELL_TRACE
    }
    grid.clear_route(net_id);  // NO-OP: cells already have net_map=u32::MAX
}
```

**Fix:** Remove the manual `mark_route(u32::MAX)` loop. Just call `grid.clear_route(net_id)` which properly clears CELL_TRACE and net_map for all cells with `net_map == net_id`:

```rust
if let Some(cells) = net_cells.remove(&net_id) {
    congestion_map.unmark_net(&cells);
    grid.clear_route(net_id);  // Correctly clears CELL_TRACE + net_map
}
```

### Build Order

1. **Fix the rip-up bug** in `pathfinder_v2.rs` — single surgical edit
2. **Run existing test** `cargo test --release -p cypcb-autoroute -- route_blink_board` — should now pass with `RoutingStatus::Complete`
3. **Add `test_blink_led_zero_unrouted`** — explicit test asserting `unrouted == 0` with detailed metrics (the S02→S03 boundary artifact)
4. **Rebuild WASM** — `wasm-pack build` so the fix is available in browser too
5. **If needed:** tune iteration count or grid resolution as secondary fixes

### Verification Approach

- `cargo test --release -p cypcb-autoroute -- route_blink_board --nocapture` — must show `RoutingStatus::Complete` with 0 unrouted
- New test `test_blink_led_zero_unrouted` asserting: `result.status == Complete`, `unrouted_count == 0`, segments > 0, vias < 20
- `cargo test --release -p cypcb-autoroute` — full suite must pass (no regressions)
- Timing: routing should complete in <30s (was ~120s with ghost cells causing pathfinding to explore dead ends)

## Constraints

- `grid.clear_route()` is O(width × height × layers) — it scans the full grid. This is acceptable for the rip-up use case since it runs at most 50 × 7 = 350 times. The per-net cell index (`net_cells`) is used for congestion map bookkeeping, not for grid clearing.
- The `mark_route` / `clear_route` APIs in `grid.rs` are correct and used correctly elsewhere (e.g., in `orchestrator.rs` rip-up). Only `pathfinder_v2.rs` has this bug.
- WASM binary must be rebuilt after the fix for browser verification (S01 already set up the Worker pipeline).

## Common Pitfalls

- **Don't remove `congestion_map.unmark_net(&cells)`** — this is correct and necessary. Only the `mark_route(u32::MAX)` loop is wrong.
- **Don't change `grid.clear_route()` behavior** — it correctly clears CELL_TRACE and net_map. The bug is in the caller.
- **Don't confuse `net_cells` (congestion bookkeeping) with grid state** — `net_cells` tracks cells for O(path_length) congestion unmark. Grid's `clear_route()` is the authoritative state clearer.
- **Test in release mode** — debug mode routing takes 5-10× longer. The 50-iteration cap may time out in debug.

## Open Risks

- After fixing the ghost cell bug, if some connections still fail, secondary investigation needed: pad zone radius might be too small for SOIC-8 pads at 63.5µm grid resolution (pad zone radius is ~15 cells ≈ 0.95mm, SOIC-8 pad is 0.65×1.55mm — should be adequate, but worth verifying).
- The `route_blink_board` integration test already asserts `RoutingStatus::Complete` — if the fix doesn't achieve 0 unrouted, the test will still fail and we need deeper investigation.
