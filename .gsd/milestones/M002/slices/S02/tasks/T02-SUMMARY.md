---
id: T02
parent: S02
milestone: M002
provides:
  - A* pathfinder with find_path() using pathfinding crate's astar()
  - PCB-aware RoutingCost with via cost, layer preference, diagonal movement, 3D octile heuristic
  - GridNode type (u16, u16, u8) for grid-based pathfinding
key_files:
  - crates/cypcb-autoroute/src/pathfinder.rs
  - crates/cypcb-autoroute/src/cost.rs
  - crates/cypcb-autoroute/src/lib.rs
  - crates/cypcb-autoroute/src/grid.rs
  - crates/cypcb-autoroute/Cargo.toml
key_decisions:
  - Cost function uses integer-scaled u64 (f64 * 1000) for pathfinding crate compatibility — preserves sub-unit precision for √2 diagonal costs
  - Via transitions check all layers (not just adjacent) allowing multi-layer jumps if rules permit
  - make_test_grid() extracted as pub(crate) for cross-module test reuse
patterns_established:
  - TestRules struct pattern for RoutingRuleSet in unit tests — via_cost = span * 2.0, layer_change_cost(0) = 0.1 (top preferred)
  - find_path() marks route cells after pathfinding — callers don't need to do it
  - any_end_layer flag for through-hole pad routing (path can arrive on any layer)
observability_surfaces:
  - tracing::debug_span!("find_path") with net_id, start/end coordinates, path_length, via_count
  - Failed path searches log grid dimensions and obstacle_count via tracing::warn
  - find_path() returns Option<Vec<GridNode>> — None is explicit failure
duration: 25min
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T02: Implement A* pathfinder with PCB-aware cost function

**Built find_path() using pathfinding::astar with PCB-aware RoutingCost (8-directional movement, via transitions, layer preference, √2 diagonal cost, 3D octile heuristic).**

## What Happened

Created two modules — `cost.rs` with `RoutingCost` struct and `pathfinder.rs` with `find_path()`.

`RoutingCost` wraps `RoutingRuleSet` to provide:
- Cardinal movement cost 1.0, diagonal √2
- Via transition cost from `rules.via_cost()` with configurable multiplier
- Layer preference bias from `rules.layer_change_cost()` scaled to 10% to avoid dominating
- 3D octile heuristic: `max(dx,dy) + (√2-1)*min(dx,dy) + min_via_cost * layer_diff`
- Precomputed `min_via_cost` for admissible heuristic

`find_path()` uses `pathfinding::directed::astar::astar()` with:
- 8-directional neighbors on same layer + via transitions to all other layers
- Success condition supports `any_end_layer` for through-hole pads
- Path cells marked as occupied after successful routing
- Tracing spans with net_id, start/end, path_length, via_count

Cost values are scaled to u64 (* 1000) for pathfinding crate's integer requirement while preserving sub-unit precision.

Added `pathfinding = "4"` dependency. Extracted `make_test_grid()` as `pub(crate)` for cross-module test use.

## Verification

- `cargo test -p cypcb-autoroute --lib` — 25 tests pass (13 grid + 7 cost + 6 pathfinder, including `any_end_layer_mode`)
- `cargo clippy -p cypcb-autoroute` — zero warnings from cypcb-autoroute
- Path on empty 20×20 grid: found, length 19-30 steps, all moves adjacent
- Path around L-shaped obstacle: found, no path cell overlaps obstacle
- Via between layers: path includes ≥2 layer transitions (down and back)
- Blocked target: returns None
- Path cells marked as occupied after routing
- Straight path costs less than zigzag path (cost function test)

### Slice-level verification (partial — intermediate task):
- ✅ `cargo test -p cypcb-autoroute --lib` — all unit tests pass
- ✅ `cargo clippy -p cypcb-autoroute` — zero warnings from this crate
- ❌ `cargo test -p cypcb-autoroute -- --test integration` — 2 failures expected (route_board() still stub)
- ❌ `cargo build -p cypcb-autoroute --target wasm32-unknown-unknown` — pre-existing `getrandom` WASM issue in upstream deps (not introduced by T02)
- ⏳ Integration test asserts for blink/routing-test — blocked on T03 orchestrator
- ⏳ Quality metrics assertions — blocked on T04 post-processing

## Diagnostics

- Run `cargo test -p cypcb-autoroute -- pathfinder --nocapture` to see tracing output from find_path
- `find_path()` returns `Option<Vec<GridNode>>` — None is explicit failure with tracing::warn
- Failed searches log: start/end coordinates, grid dimensions, obstacle count
- Successful searches log: path_length, via_count

## Deviations

- Heuristic uses 3D octile distance (plan said "3D Manhattan with min via cost") — octile is tighter and still admissible for 8-directional grids
- Cost function does not include "direction preference penalty for zig-zagging" as a separate component — instead, the natural cost geometry (√2 for diagonal > 1.0 for cardinal) inherently favors aligned paths, validated by `straight_path_cheaper_than_zigzag` test
- Cost module TestRules uses `via_cost = span * 2.0` instead of `span * 0.5` — the lower value made vias cheaper than cardinal moves, which is unrealistic for PCB routing

## Known Issues

- WASM compilation fails due to pre-existing `getrandom` crate issue in upstream dependencies — not introduced by T02, will need resolution in T05

## Files Created/Modified

- `crates/cypcb-autoroute/src/pathfinder.rs` — A* pathfinder with find_path(), GridNode type, 8-dir + via successors
- `crates/cypcb-autoroute/src/cost.rs` — RoutingCost with neighbor_cost(), heuristic(), via transition costs
- `crates/cypcb-autoroute/src/lib.rs` — added cost and pathfinder module declarations
- `crates/cypcb-autoroute/src/grid.rs` — extracted make_test_grid() as pub(crate)
- `crates/cypcb-autoroute/Cargo.toml` — added pathfinding = "4" dependency
