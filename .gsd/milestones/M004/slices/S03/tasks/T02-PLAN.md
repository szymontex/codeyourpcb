---
estimated_steps: 5
estimated_files: 3
---

# T02: PathFinder negotiated congestion router with CongestionMap

**Slice:** S03 — PathFinder Routing Engine
**Milestone:** M004

## Description

Implement the PathFinder negotiated congestion routing algorithm — the core algorithmic contribution of this slice. PathFinder routes all nets simultaneously on a shared grid, tracks per-cell congestion (present occupancy and historical contention), and iteratively re-routes nets through overused cells with increasing cost penalties until convergence (zero overused cells) or iteration cap. This naturally produces strategic via placement because layer transitions that resolve congestion are rewarded. Uses existing `find_path_with_zones()` as the inner A* search kernel with augmented congestion cost.

## Steps

1. Create `congestion.rs` with `CongestionMap` struct. Fields: `present_cost: Vec<Vec<f64>>` (per-layer, indexed by `y*width+x`), `history_cost: Vec<Vec<f64>>` (same layout), `occupancy: Vec<Vec<u16>>` (count of nets using each cell), `capacity: u16` (typically 1 for PCB routing). Methods: `new(width, height, layers)`, `mark_net(cells)` (increment occupancy), `unmark_net(cells)` (decrement), `update_history(alpha)` (for each overused cell: `history += alpha`), `congestion_cost(x, y, layer) -> f64` returning `(1.0 + history) * (1.0 + present_factor * max(0, occupancy - capacity))`, `overused_cells() -> Vec<(u32, u32, u8)>`, `is_converged() -> bool` (no overused cells), `overuse_count() -> usize`.

2. Create `pathfinder_v2.rs` with `PathFinderStrategy` implementing `RoutingStrategy`. The `route()` method: (a) Build grid from board via `RoutingGrid::from_board()`. (b) Extract ratsnest via `extract_ratsnest()`. (c) Initialize CongestionMap. (d) Initialize per-net cell index `HashMap<u32, Vec<(u32, u32, u8)>>` for O(path_length) rip-up. (e) Run iteration loop (max 50 iterations).

3. Implement the PathFinder iteration loop. Each iteration: (a) Determine which nets to re-route — iteration 1 routes all nets; subsequent iterations only re-route nets passing through overused cells (VPR optimization). (b) For each net to route: rip up previous route using per-net cell index (clear from grid + unmark from CongestionMap), run A* with congestion-augmented cost closure, record new route, update per-net cell index and CongestionMap occupancy. (c) After all nets: call `congestion_map.update_history(beta)` where `beta` starts at 0.5 and increases by 0.1 per iteration. (d) Check convergence. (e) Log iteration stats. The congestion cost augmentation wraps `RoutingCost::neighbor_cost()`: `base_cost + congestion_map.congestion_cost(x, y, layer)`. The heuristic remains unadulterated (admissible).

4. Convert final routes to `RoutingResult` via existing `postprocess::paths_to_output()`. Handle non-convergence: if iteration cap reached with remaining overuse, still output the best routes found and log warning. Return `RoutingResult::partial()` if any nets remain unrouted.

5. Add unit tests: CongestionMap operations (mark/unmark/history/convergence), PathFinder convergence on a small test grid with 3-4 crossing nets (should converge in <10 iterations), PathFinder handles impossible routing (blocked grid returns partial result gracefully).

## Must-Haves

- [ ] `CongestionMap` with present/history cost tracking and convergence detection
- [ ] `PathFinderStrategy` implementing `RoutingStrategy` trait
- [ ] Iteration loop with VPR partial-reroute optimization
- [ ] Per-net cell index for O(path_length) rip-up
- [ ] Congestion cost added to A* neighbor cost, heuristic stays admissible
- [ ] History cost accumulation prevents oscillation
- [ ] Convergence on simple test grids with crossing nets
- [ ] WASM compilation passes (no `std::time::Instant`, use iteration count)

## Verification

- `cargo test -p cypcb-autoroute` — all existing + new tests pass
- CongestionMap unit tests verify mark/unmark/history/convergence semantics
- PathFinder converges on a 30×20 test grid with 4 crossing nets in <15 iterations
- `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — WASM OK

## Observability Impact

- Signals added: `tracing::info!` per PathFinder iteration (iteration number, overused cell count, nets re-routed, convergence status)
- How a future agent inspects: `-- --nocapture` on tests shows iteration-by-iteration convergence progress
- Failure state exposed: iteration cap hit logged as warning with final overuse count

## Inputs

- `crates/cypcb-autoroute/src/strategy.rs` — RoutingStrategy trait from T01
- `crates/cypcb-autoroute/src/grid.rs` — RoutingGrid, make_test_grid, mark_route/clear_route, net_at, is_free
- `crates/cypcb-autoroute/src/pathfinder.rs` — find_path_with_zones() as inner search kernel
- `crates/cypcb-autoroute/src/orchestrator.rs` — extract_ratsnest(), order_nets(), PadTarget, NetRoute, pad_to_grid_node, pad_to_zone
- `crates/cypcb-autoroute/src/cost.rs` — RoutingCost::neighbor_cost() and heuristic()
- `crates/cypcb-autoroute/src/postprocess.rs` — paths_to_output() for final conversion

## Expected Output

- `crates/cypcb-autoroute/src/congestion.rs` — CongestionMap struct (~120 LOC)
- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — PathFinderStrategy implementation (~450 LOC)
- `crates/cypcb-autoroute/src/lib.rs` — updated to register new modules, PathFinder as default strategy
