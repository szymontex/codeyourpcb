---
estimated_steps: 4
estimated_files: 1
---

# T01: Fix rip-up ghost cell bug in PathFinder

**Slice:** S02 — Routing Quality — 0 Unrouted on Blink LED
**Milestone:** M005

## Description

The PathFinder rip-up code in `pathfinder_v2.rs` has a confirmed bug: before calling `grid.clear_route(net_id)`, it loops through the net's cells calling `grid.mark_route(x, y, layer, u32::MAX)`. This overwrites the `net_map` entry from `net_id` to `u32::MAX`, so the subsequent `clear_route(net_id)` finds nothing to clear — those cells permanently retain the `CELL_TRACE` flag with no owning net. Over 50 iterations, these ghost cells accumulate as invisible obstacles, causing 5/25 unrouted connections on the Blink LED board.

The fix is surgical: remove the `mark_route(u32::MAX)` loop. `grid.clear_route(net_id)` correctly clears both `CELL_TRACE` and `net_map` for all cells with `net_map == net_id`.

## Steps

1. Open `crates/cypcb-autoroute/src/pathfinder_v2.rs` and locate the rip-up block inside `pathfinder_loop()` (around lines 253-258). The current code looks like:
   ```rust
   if let Some(cells) = net_cells.remove(&net_id) {
       congestion_map.unmark_net(&cells);
       // Clear from grid
       for &(x, y, layer) in &cells {
           grid.mark_route(x, y, layer as usize, u32::MAX);
       }
       grid.clear_route(net_id);
   }
   ```

2. Remove the entire `for &(x, y, layer) in &cells` loop that calls `grid.mark_route(x, y, layer as usize, u32::MAX)` and the `// Clear from grid` comment above it. The fixed code should be:
   ```rust
   if let Some(cells) = net_cells.remove(&net_id) {
       congestion_map.unmark_net(&cells);
       grid.clear_route(net_id);
   }
   ```

3. **Do NOT** remove `congestion_map.unmark_net(&cells)` — that line is correct and necessary for congestion bookkeeping.

4. Run the existing integration test:
   ```bash
   cargo test --release -p cypcb-autoroute -- route_blink_board --nocapture
   ```
   This test already asserts `RoutingStatus::Complete` (all 7 nets routed). It was failing before the fix.

## Must-Haves

- [ ] The `for &(x, y, layer) in &cells { grid.mark_route(x, y, layer as usize, u32::MAX); }` loop is removed
- [ ] `congestion_map.unmark_net(&cells)` is preserved (do NOT remove)
- [ ] `grid.clear_route(net_id)` is preserved (do NOT remove)
- [ ] `route_blink_board` test passes with `RoutingStatus::Complete`

## Verification

- `cargo test --release -p cypcb-autoroute -- route_blink_board --nocapture` — must pass, output should show `RoutingStatus::Complete`, segments > 0
- If the test still fails after the fix, print the full metrics table and report the failure — do NOT proceed to workarounds. Stop and report.

## Observability Impact

- **Signal changed:** Removing the `mark_route(u32::MAX)` loop eliminates ghost cell accumulation. The per-iteration metrics printed by `route_blink_board --nocapture` will now converge to 0 unrouted instead of plateauing at 5/25.
- **How to inspect:** Run `cargo test --release -p cypcb-autoroute -- route_blink_board --nocapture` and check that output shows `RoutingStatus::Complete`. Grep for `Unrouted` to confirm 0.
- **Failure visibility:** If the bug regresses, `route_blink_board` fails with `RoutingStatus::Partial` and the metrics table shows which nets remain stuck. No new logging needed — the existing test output is sufficient.

## Inputs

- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — the buggy rip-up code at ~line 253-258
- `crates/cypcb-autoroute/src/grid.rs` — `clear_route()` (lines 467-481) and `mark_route()` (lines 457-465) for reference only — do NOT modify grid.rs

## Expected Output

- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — rip-up block fixed (3 lines removed), ghost cell bug eliminated
- `route_blink_board` integration test passes with `RoutingStatus::Complete`
