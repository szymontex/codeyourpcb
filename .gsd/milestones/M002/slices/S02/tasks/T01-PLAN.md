---
estimated_steps: 5
estimated_files: 6
---

# T01: Scaffold cypcb-autoroute crate with grid model and integration test harness

**Slice:** S02 — Custom Autorouter Core
**Milestone:** M002

## Description

Create the `cypcb-autoroute` crate as a new workspace member. Implement the grid data structure that discretizes the board into cells with per-layer occupancy tracking. Populate the grid from `BoardWorld` data (pads, zones, existing traces become obstacles). Set up integration tests that will exercise the full routing pipeline on reference boards — these tests will initially fail on the routing assertion (calling a stub `route_board()`) but compile and run successfully.

## Steps

1. Create `crates/cypcb-autoroute/Cargo.toml` with dependencies: `cypcb-core`, `cypcb-world`, `cypcb-rules`, `cypcb-router` (for `RoutingResult` types), `pathfinding`, `rstar`, `tracing`. Add `cypcb-parser` and `cypcb-rules` as dev-dependencies for integration tests. Add crate to workspace `Cargo.toml` members.

2. Create `crates/cypcb-autoroute/src/lib.rs` with module declarations and a public `route_board()` stub that returns `RoutingResult::failed("not yet implemented")`. Define `AutorouteConfig` struct with grid resolution, max rip-up iterations, and via preferences.

3. Create `crates/cypcb-autoroute/src/grid.rs` implementing `RoutingGrid`:
   - Grid resolution configurable (default: `min_clearance / 2` from rules, typically ~63µm for JLCPCB)
   - Coordinate conversion: `nm_to_grid(Nm) -> u16` and `grid_to_nm(u16) -> Nm` with board origin offset
   - Per-layer occupancy: `Vec<Vec<u8>>` where each cell is a bitfield (0=free, bits for: pad, trace, zone, via)
   - `fn from_board(world: &BoardWorld, rules: &dyn RoutingRuleSet) -> RoutingGrid` — iterates pads, zones, locked traces and marks grid cells as occupied with appropriate clearance bloat
   - `fn mark_obstacle(&mut self, x, y, layer, radius_cells)` — marks cells within radius as occupied
   - `fn is_free(&self, x, y, layer) -> bool` — checks cell availability
   - `fn mark_route(&mut self, x, y, layer, net_id)` and `fn clear_route(net_id)` for dynamic route tracking

4. Write unit tests for grid: coordinate round-trip accuracy, obstacle marking, clearance bloat, layer isolation, `is_free` correctness.

5. Create `crates/cypcb-autoroute/tests/integration.rs`:
   - Helper function to parse a `.cypcb` file into `BoardWorld` using `cypcb-parser`
   - Test `route_routing_test_board`: parse `routing-test.cypcb`, call `route_board()`, assert `RoutingStatus::Complete` — this test will fail (expected, `route_board` is a stub)
   - Test `route_blink_board`: parse `blink.cypcb`, call `route_board()`, assert `RoutingStatus::Complete` and 7/7 nets — this test will also fail
   - Test `grid_from_blink`: parse `blink.cypcb`, build `RoutingGrid`, assert grid dimensions match board size, assert pad positions are marked as occupied

## Must-Haves

- [ ] `cypcb-autoroute` crate compiles as workspace member
- [ ] `RoutingGrid` converts between Nm and grid coordinates accurately (round-trip within 1 grid cell)
- [ ] Grid populated from `BoardWorld` correctly marks pads and zones as obstacles
- [ ] Clearance bloating applies `min_clearance` from rules around each obstacle
- [ ] Integration test file exists and compiles (routing tests expected to fail, grid test passes)
- [ ] All dependencies are WASM-compatible (no std::thread, no std::fs in main crate)

## Verification

- `cargo test -p cypcb-autoroute` — grid unit tests pass, integration grid test passes
- `cargo test -p cypcb-autoroute -- route_routing_test` should compile but fail (stub returns Failed)
- `cargo build -p cypcb-autoroute` compiles without errors

## Observability Impact

- Signals added/changed: `tracing::info_span!("grid_construction")` with board dimensions and cell count logged
- How a future agent inspects this: grid dimensions and obstacle count printed in test output; `RoutingGrid` has `pub fn stats() -> GridStats` returning width, height, layers, obstacle_cell_count
- Failure state exposed: grid construction logs warnings for boards too large for chosen resolution (>10M cells)

## Inputs

- `crates/cypcb-world/src/world.rs` — `BoardWorld` API for board_info, component queries, pad/zone/trace iteration
- `crates/cypcb-rules/src/routing_rules.rs` — `RoutingRuleSet::constraints_for_net()` for min_clearance
- `crates/cypcb-router/src/types.rs` — `RoutingResult`, `RouteSegment`, `ViaPlacement` output types
- `examples/blink.cypcb`, `examples/routing-test.cypcb` — reference boards for integration tests

## Expected Output

- `crates/cypcb-autoroute/Cargo.toml` — new crate manifest with correct dependencies
- `crates/cypcb-autoroute/src/lib.rs` — module structure, `AutorouteConfig`, stub `route_board()`
- `crates/cypcb-autoroute/src/grid.rs` — `RoutingGrid` with coordinate mapping, occupancy tracking, obstacle population
- `crates/cypcb-autoroute/tests/integration.rs` — integration test harness with reference board tests
- `Cargo.toml` — updated workspace members list
