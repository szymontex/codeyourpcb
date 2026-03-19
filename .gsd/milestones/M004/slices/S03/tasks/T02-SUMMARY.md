---
id: T02
parent: S03
milestone: M004
provides:
  - CongestionMap with present/history cost tracking and convergence detection
  - PathFinderStrategy implementing RoutingStrategy trait with VPR-style negotiated congestion
  - Congestion-augmented A* search with admissible heuristic
  - Per-net cell index for O(path_length) rip-up
  - VPR partial-reroute optimization (only re-route nets through overused cells)
key_files:
  - crates/cypcb-autoroute/src/congestion.rs
  - crates/cypcb-autoroute/src/pathfinder_v2.rs
  - crates/cypcb-autoroute/src/lib.rs
  - crates/cypcb-autoroute/src/orchestrator.rs
key_decisions:
  - "D-M004-023: Made orchestrator helpers public instead of duplicating (reversed D-M004-022 pattern)"
patterns_established:
  - PathFinderStrategy owns the iteration loop; CongestionMap is a separate struct shared across iterations
  - Congestion cost added to A* neighbor cost only; heuristic stays unadulterated for admissibility
  - History cost escalation (beta starts 0.5, +0.1 per iteration) prevents oscillation
  - VPR partial-reroute — only nets through overused cells re-routed after iteration 1
observability_surfaces:
  - "tracing::info! per PathFinder iteration: iteration number, overused cell count, nets re-routed, beta value"
  - "tracing::info! on convergence: iteration where zero overuse achieved"
  - "tracing::warn! on non-convergence: iteration cap hit with remaining overuse count"
  - "PathFinderStrategy::name() returns 'pathfinder' for log/test correlation"
duration: 25m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T02: PathFinder negotiated congestion router with CongestionMap

**Implemented VPR-style PathFinder negotiated congestion router with CongestionMap, per-net cell index for O(path_length) rip-up, and convergence on crossing-net test grids**

## What Happened

Created `congestion.rs` with `CongestionMap` struct tracking per-cell occupancy and history costs across all routing layers. Cost model: `(1.0 + history) * (1.0 + max(0, occupancy - capacity)) - 1.0` — zero cost when not overused, escalating with both current overuse and accumulated history. History updates only affect overused cells, preventing oscillation.

Created `pathfinder_v2.rs` with `PathFinderStrategy` implementing the `RoutingStrategy` trait. The core algorithm:
1. Initializes CongestionMap and per-net cell index (`HashMap<u32, Vec<(u32, u32, u8)>>`)
2. Iteration 1: routes all nets with congestion-augmented A* cost
3. Subsequent iterations: only re-routes nets passing through overused cells (VPR optimization)
4. After each iteration: updates history costs with escalating beta (0.5 + 0.1 * iteration)
5. Converges when zero overused cells or iteration cap (50) reached

The congestion-augmented A* adds `congestion_map.congestion_cost(x, y, layer)` to `RoutingCost::neighbor_cost()` for each neighbor. The heuristic remains unadulterated (admissible). The inner search also allows routing through cells owned by the same net (for rip-up/reroute).

Made orchestrator helper functions public (`pad_to_grid_node`, `pad_to_zone`, `build_spanning_tree`, `is_multi_layer`, `Connection`) instead of duplicating them — this reverses the T01 duplication pattern (D-M004-022) in favor of shared code.

Updated `route_board()` to dispatch `StrategyKind::PathFinder` to the real `PathFinderStrategy` instead of falling back to ImprovedAStar.

## Verification

- `cargo test -p cypcb-autoroute`: **88 tests passed**, 0 failed, 0 ignored
  - 8 new CongestionMap unit tests: mark/unmark symmetry, congestion cost, history accumulation, multi-layer, out-of-bounds, overuse detection
  - 3 new PathFinder tests: strategy name, convergence on crossing nets, impossible routing
  - All 77 existing tests pass (no regression)
- PathFinder converges on 30×20 grid with 4 crossing nets in <15 iterations ✅
- PathFinder handles impossible routing (thick wall) gracefully — reports unrouted ✅
- `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown`: **WASM OK** (no std::time::Instant)
- `cargo check -p cypcb-render`: **compiles OK**

### Slice-level verification status (T02 — intermediate task):
- ✅ `cargo test -p cypcb-autoroute` — all existing + new tests pass (88 total)
- ⬜ `cargo test -p cypcb-autoroute --test strategy_comparison` — not yet created (T03)
- ✅ `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — passes
- ✅ `cargo check -p cypcb-render` — passes
- ✅ Diagnostic check — strategy names `pathfinder` and `improved-astar` appear in test filtering

## Diagnostics

- `PathFinderStrategy::name()` returns `"pathfinder"` — used in tracing spans and test assertions
- Per-iteration tracing: `tracing::info!(iteration, overused_cells, nets_rerouted, total_nets, beta)` after each PathFinder iteration
- Convergence logged with final iteration number; non-convergence emits `tracing::warn!` with remaining overuse count
- CongestionMap methods are testable independently: `overuse_count()`, `overused_cells()`, `is_converged()`, `congestion_cost()`

## Deviations

- Made orchestrator helpers (`pad_to_grid_node`, `pad_to_zone`, `build_spanning_tree`, `is_multi_layer`, `Connection`) public instead of duplicating them. D-M004-022 pattern was "duplicate for self-contained strategies" but duplicating ~100 LOC of identical logic into a second strategy was worse than making them shared. Decision documented as D-M004-023.
- PathFinder inner search implemented directly in `pathfinder_v2.rs` rather than wrapping `find_path_with_zones()` — the congestion cost closure needs access to CongestionMap state that can't be injected through the existing API. Same algorithm (8-directional + via transitions + pad zones), augmented with congestion cost.
- Impossible routing test uses thick wall (6 cells wide) instead of enclosing a target — pad zones (~4 cell radius) made single-cell walls passable. Test correctly validates graceful degradation.

## Known Issues

None.

## Files Created/Modified

- `crates/cypcb-autoroute/src/congestion.rs` — NEW: CongestionMap struct with present/history cost tracking (~160 LOC + ~120 LOC tests)
- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — NEW: PathFinderStrategy with iteration loop, congestion-augmented A*, VPR partial-reroute (~380 LOC + ~230 LOC tests)
- `crates/cypcb-autoroute/src/lib.rs` — Updated: added `pub mod congestion; pub mod pathfinder_v2;`, changed PathFinder dispatch to real PathFinderStrategy
- `crates/cypcb-autoroute/src/orchestrator.rs` — Modified: made `Connection`, `build_spanning_tree`, `pad_to_grid_node`, `pad_to_zone`, `is_multi_layer` public
- `crates/cypcb-autoroute/src/strategy.rs` — Updated: removed "falls back to ImprovedAStar" doc comment from PathFinder variant
