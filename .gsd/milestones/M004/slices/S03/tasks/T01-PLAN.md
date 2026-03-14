---
estimated_steps: 6
estimated_files: 4
---

# T01: RoutingStrategy trait, ImprovedAStarStrategy, and route_board() dispatch

**Slice:** S03 — PathFinder Routing Engine
**Milestone:** M004

## Description

Establish the multi-strategy routing abstraction and deliver an immediately-useful improved A* strategy. This creates the trait boundary that PathFinder (T02) and future strategies plug into, updates `route_board()` to dispatch via the selected strategy, and ships `ImprovedAStarStrategy` — which wraps the existing orchestrator with congestion-aware cost, better net ordering, more aggressive rip-up (20 iterations, 3 victims per failure). If PathFinder fails to converge in T02, this task alone already delivers a measurable improvement over the current router.

## Steps

1. Create `strategy.rs` with `RoutingStrategy` trait (methods: `name() -> &str`, `route(&self, &mut BoardWorld, &FootprintLibrary, &dyn RoutingRuleSet, &AutorouteConfig) -> RoutingResult`) and `StrategyKind` enum (`PathFinder`, `ImprovedAStar`). Add `pub mod strategy;` to lib.rs.

2. Add `strategy: StrategyKind` field to `AutorouteConfig` with default `StrategyKind::PathFinder`. Preserve backward compat — existing code using `AutorouteConfig::default()` gets PathFinder (will fall back to ImprovedAStar until T02 implements PathFinder).

3. Create `astar_improved.rs` implementing `ImprovedAStarStrategy`. Reuse `extract_ratsnest()`, `order_nets()`, and the existing `find_path_with_zones()`. Improvements over current `route_all_nets()`: (a) congestion-aware cost — check `grid.net_at()` in neighbor expansion and add penalty for routing near existing nets, (b) increased `max_ripup_iterations` to 20, (c) multi-victim rip-up — try up to 3 different blocking nets before giving up on a connection. The net ordering enhancement (considering fanout alongside span) goes here too.

4. Change `make_test_grid` in `grid.rs` from `pub(crate)` to `pub` and remove the `#[cfg(test)]` guard so strategy tests can use it. Add a `RoutingGrid::new_empty()` constructor if needed for clarity.

5. Update `route_board()` in `lib.rs` to dispatch: match on `config.strategy`, instantiate the appropriate strategy struct, call `strategy.route()`. For `StrategyKind::PathFinder`, temporarily fall back to `ImprovedAStarStrategy` until T02 lands.

6. Add unit tests in `strategy.rs` (trait dispatch works, strategy names correct) and `astar_improved.rs` (routes a simple test grid, produces valid RoutingResult, handles multi-net congestion).

## Must-Haves

- [ ] `RoutingStrategy` trait with `name()` and `route()` methods
- [ ] `StrategyKind` enum in `AutorouteConfig`
- [ ] `ImprovedAStarStrategy` producing valid `RoutingResult`
- [ ] `route_board()` dispatches to strategy
- [ ] `make_test_grid` accessible from integration tests
- [ ] All existing tests pass (no regression)
- [ ] WASM compilation passes

## Verification

- `cargo test -p cypcb-autoroute` — all 76+ existing tests pass plus new strategy tests
- `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — WASM OK
- New unit tests prove ImprovedAStarStrategy routes a multi-net grid and produces RoutingResult with segments

## Inputs

- `crates/cypcb-autoroute/src/orchestrator.rs` — `extract_ratsnest()`, `order_nets()`, `route_all_nets()`, `attempt_ripup_reroute()` — reuse directly
- `crates/cypcb-autoroute/src/pathfinder.rs` — `find_path_with_zones()` — inner search kernel
- `crates/cypcb-autoroute/src/postprocess.rs` — `paths_to_output()` — converts grid paths to segments/vias
- `crates/cypcb-autoroute/src/lib.rs` — current `route_board()` implementation to refactor

## Expected Output

- `crates/cypcb-autoroute/src/strategy.rs` — RoutingStrategy trait + StrategyKind enum (~60 LOC)
- `crates/cypcb-autoroute/src/astar_improved.rs` — ImprovedAStarStrategy implementation (~250 LOC)
- `crates/cypcb-autoroute/src/lib.rs` — updated route_board() with strategy dispatch
- `crates/cypcb-autoroute/src/grid.rs` — make_test_grid made public

## Observability Impact

- **New signal:** `tracing::info!` in `route_board()` now emits `strategy = <name>` field, showing which `RoutingStrategy` was selected for dispatch. This appears in all routing runs.
- **New signal:** `ImprovedAStarStrategy::route()` logs `routing_strategy = "improved-astar"` at entry, plus per-iteration rip-up stats (iteration count, victims tried, reroute success/failure).
- **Inspection:** `RoutingStrategy::name()` on any strategy instance returns a stable identifier (`"improved-astar"`, `"pathfinder"`) usable in structured logs and test assertions.
- **Failure visibility:** When multi-victim rip-up exhausts all candidates, a `tracing::warn!` with `net_id`, `victims_tried`, and `max_ripup_iterations` is emitted, making routing failures inspectable without debugger access.
