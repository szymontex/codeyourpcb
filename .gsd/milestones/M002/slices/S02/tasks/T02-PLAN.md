---
estimated_steps: 4
estimated_files: 4
---

# T02: Implement A* pathfinder with PCB-aware cost function

**Slice:** S02 — Custom Autorouter Core
**Milestone:** M002

## Description

Build the core routing algorithm that finds a path between two pad positions on the grid using A* search from the `pathfinding` crate. The cost function incorporates PCB-specific concerns: clearance from existing routes, via transition costs, layer preference, and 45° routing bias. The pathfinder operates on a single net at a time, finding a path from source pad to target pad, potentially crossing layers via vias.

## Steps

1. Create `crates/cypcb-autoroute/src/cost.rs`:
   - Define `RoutingCost` struct holding a reference to `RoutingRuleSet` and the current `NetId` being routed
   - `fn neighbor_cost(&self, from: GridNode, to: GridNode) -> f64` — base movement cost (1.0 for cardinal, √2 for diagonal) + layer change cost from `layer_change_cost()` + clearance proximity penalty (check spatial index for nearby obstacles)
   - `fn via_transition_cost(&self, from_layer: u8, to_layer: u8) -> f64` — delegates to `RoutingRuleSet::via_cost()`
   - `fn heuristic(&self, current: GridNode, goal: GridNode) -> f64` — 3D octile distance: max(|dx|, |dy|) + (√2-1)*min(|dx|, |dy|) + min_via_cost * |layer_diff|
   - Direction preference: penalize moves that aren't 0°, 45°, or 90° (grid is inherently 8-directional so this is mainly penalizing unnecessary zig-zagging via consecutive non-aligned moves)

2. Create `crates/cypcb-autoroute/src/pathfinder.rs`:
   - Define `GridNode` as `(u16, u16, u8)` — (grid_x, grid_y, layer_index)
   - `fn find_path(grid: &RoutingGrid, start: GridNode, end: GridNode, cost: &RoutingCost, config: &AutorouteConfig) -> Option<Vec<GridNode>>` using `pathfinding::directed::astar::astar()`
   - Successors function: 8 directional neighbors on same layer (if free) + layer transitions at current position (if via allowed and target layer free). Check `grid.is_free()` for each candidate. Vias only allowed if clearance from via outer diameter is satisfied.
   - Success function: current node matches goal (x, y, any layer if goal pad is through-hole; specific layer if SMD)
   - After path found, mark all path cells as occupied on the grid via `grid.mark_route()`

3. Wire pathfinder into `lib.rs` — add module declarations, make `find_path` and `GridNode` public. Update `route_board()` stub to be a real function signature (still delegates to orchestrator which doesn't exist yet, but the types are correct).

4. Write unit tests in `pathfinder.rs`:
   - Test: route on empty 20×20 single-layer grid — path found, length reasonable
   - Test: route around L-shaped obstacle — path detours correctly
   - Test: route with via between two layers — path includes layer change
   - Test: route impossible (completely blocked) — returns `None`
   - Test: cost function produces lower cost for straight paths vs zigzag

## Must-Haves

- [ ] `find_path()` uses `pathfinding::directed::astar::astar()` — no hand-rolled priority queue
- [ ] 8-directional movement on same layer with √2 cost for diagonals
- [ ] Via transitions use `RoutingRuleSet::via_cost()` for cost
- [ ] Layer preference uses `RoutingRuleSet::layer_change_cost()`
- [ ] Path cells marked as occupied after routing (for subsequent nets to route around)
- [ ] Returns `None` for impossible routes (no panic)

## Verification

- `cargo test -p cypcb-autoroute` — all pathfinder unit tests pass
- Path found on empty grid connects start to end with valid moves
- Path around obstacle avoids all obstacle cells
- Via test shows layer transition in path
- Blocked test returns `None`

## Observability Impact

- Signals added/changed: `tracing::debug_span!("find_path")` with net_id, start, end, path_length logged. Failed path searches log grid dimensions and obstacle count near the search area.
- How a future agent inspects this: path length and via count visible in test output; `find_path` returns `Option` making success/failure explicit
- Failure state exposed: `None` return with tracing warning including start/end coordinates and layer

## Inputs

- `crates/cypcb-autoroute/src/grid.rs` — `RoutingGrid` from T01 with `is_free()`, `mark_route()`, coordinate mapping
- `crates/cypcb-rules/src/routing_rules.rs` — `RoutingRuleSet` trait methods for cost function
- `pathfinding` crate — `astar()` function

## Expected Output

- `crates/cypcb-autoroute/src/pathfinder.rs` — `find_path()` function with A* search on grid
- `crates/cypcb-autoroute/src/cost.rs` — `RoutingCost` struct with PCB-aware cost and heuristic functions
- `crates/cypcb-autoroute/src/lib.rs` — updated module declarations
