# S02: Custom Autorouter Core

**Goal:** Replace FreeRouting JAR dependency with a custom A*-based autorouter that routes multi-layer PCBs with constraint awareness, producing `RoutingResult` output compatible with existing `apply_routes()` pipeline.
**Demo:** `cargo test -p cypcb-autoroute` passes all tests including routing `blink.cypcb` (8 components, 7 nets) and `routing-test.cypcb` (3 components, 3 nets) end-to-end with 100% completion, DRC-clean output, and quality metrics logged.

## Must-Haves

- New `cypcb-autoroute` crate with grid-based A* routing engine
- Grid model that maps board geometry to discrete cells with per-layer occupancy
- A* pathfinder using `pathfinding` crate with PCB-aware cost function (clearance, via cost, layer preference, 45° routing)
- Net ordering by criticality with rip-up/reroute loop for congestion resolution
- Output as `RoutingResult` (`Vec<RouteSegment>` + `Vec<ViaPlacement>`) compatible with `apply_routes()`
- Collinear segment simplification (merge grid steps into clean trace segments)
- WASM-compatible (no `std::thread`, no `std::fs`, pure Rust dependencies)
- Routes `blink.cypcb` reference board with 100% net completion
- Routes `routing-test.cypcb` with 100% net completion
- Quality metrics (total length, via count, completion rate) logged and asserted

## Proof Level

- This slice proves: contract + integration
- Real runtime required: yes (Rust test runner executes autorouter on reference boards)
- Human/UAT required: no (quality is measured by metrics; visual inspection deferred to S03 renderer)

## Verification

- `cargo test -p cypcb-autoroute` — all unit and integration tests pass
- `cargo test -p cypcb-autoroute -- --test integration` — routes reference boards end-to-end
- `cargo clippy -p cypcb-autoroute -- -D warnings` — zero clippy warnings
- `cargo build -p cypcb-autoroute --target wasm32-unknown-unknown` — WASM target compiles
- Integration test asserts: `blink.cypcb` routes 7/7 nets with `RoutingStatus::Complete`
- Integration test asserts: `routing-test.cypcb` routes 3/3 nets with `RoutingStatus::Complete`
- Integration test asserts: all route segments have non-zero width matching rule constraints
- Integration test asserts: quality metrics (via count, total length) are within reasonable bounds

## Observability / Diagnostics

- Runtime signals: `tracing` spans for grid construction, per-net routing, rip-up iterations, path conversion. Each net logs: net_id, pad count, route success/failure, segment count, via count, path length.
- Inspection surfaces: `RoutingMetrics` returned with every `RoutingResult`; `AutorouteStats` struct with per-net timing and completion status. Tests print metrics to stdout on failure.
- Failure visibility: `RoutingStatus::Partial { unrouted_count }` with unrouted net IDs logged. Failed A* searches log source/target positions, grid dimensions, and obstacle count in the search area.
- Redaction constraints: none (no secrets or PII in routing data)

## Integration Closure

- Upstream surfaces consumed: `cypcb-rules::RoutingRuleSet` (constraint interface), `cypcb-world::BoardWorld` (board model, spatial index, net registry, pad/component/zone queries), `cypcb-router::types` (`RoutingResult`, `RouteSegment`, `ViaPlacement`, `RoutingMetrics`, `calculate_metrics`)
- New wiring introduced in this slice: `cypcb-autoroute::route_board()` function that takes `&BoardWorld` + `&dyn RoutingRuleSet` and returns `RoutingResult`. Integration tests verify the output is compatible with `apply_routes()`.
- What remains before the milestone is truly usable end-to-end: S03 (renderer shows traces), S05 (DSL constraints drive router), S06 (UI integration for triggering autoroute), S08 (performance optimization for large boards)

## Tasks

- [x] **T01: Scaffold cypcb-autoroute crate with grid model and integration test harness** `est:2h`
  - Why: Foundation for everything — defines the grid data structure, coordinate mapping, and creates the integration tests that will be failing until subsequent tasks make them pass
  - Files: `crates/cypcb-autoroute/Cargo.toml`, `crates/cypcb-autoroute/src/lib.rs`, `crates/cypcb-autoroute/src/grid.rs`, `crates/cypcb-autoroute/tests/integration.rs`, `Cargo.toml`
  - Do: Create new crate in workspace. Implement `RoutingGrid` struct with configurable resolution, per-layer cell occupancy (`Vec<u8>` bitset per layer), Nm↔grid coordinate conversion. Populate grid from `BoardWorld` (pads, zones, existing traces as obstacles). Write integration tests that parse `blink.cypcb` and `routing-test.cypcb`, build grid, and assert routing completion (these tests call a stub `route_board()` that returns `RoutingResult::failed()` — they'll fail until T03/T04). Add workspace member to root `Cargo.toml`.
  - Verify: `cargo test -p cypcb-autoroute` compiles; grid unit tests pass; integration tests compile but routing assertions fail (expected)
  - Done when: Grid model correctly maps board geometry to cells, pads and zones appear as obstacles, coordinate round-trip tests pass

- [x] **T02: Implement A* pathfinder with PCB-aware cost function** `est:2h`
  - Why: The core routing algorithm — finds a path between two pads on the grid with clearance-aware, multi-layer cost function using the `pathfinding` crate
  - Files: `crates/cypcb-autoroute/src/pathfinder.rs`, `crates/cypcb-autoroute/src/cost.rs`, `crates/cypcb-autoroute/src/lib.rs`
  - Do: Implement `find_path(grid, start, end, rules)` using `pathfinding::directed::astar::astar()`. Node type: `(u16, u16, u8)` for `(grid_x, grid_y, layer)`. 8-directional neighbors on same layer + via transitions. Cost function: base distance (1.0 cardinal, √2 diagonal), clearance penalty from `RoutingRuleSet`, via cost from `via_cost()`, layer preference from `layer_change_cost()`, 45° preference (penalize non-45° non-orthogonal). Heuristic: 3D Manhattan distance with minimum via cost. Mark path cells as occupied after routing. Unit tests: route single net on empty grid, route around obstacle, route with via between layers.
  - Verify: `cargo test -p cypcb-autoroute` — pathfinder unit tests pass, paths found are valid (no collisions, correct start/end)
  - Done when: A* finds valid paths on multi-layer grids respecting clearance and via constraints

- [x] **T03: Build net ordering and rip-up/reroute orchestrator** `est:2h`
  - Why: Routes all nets on a board, not just one — handles ordering by criticality and resolves congestion via rip-up/reroute
  - Files: `crates/cypcb-autoroute/src/orchestrator.rs`, `crates/cypcb-autoroute/src/lib.rs`
  - Do: Implement `route_board(world, rules) -> RoutingResult`. Extract ratsnest from `BoardWorld` (pin pairs per net from `NetConnections` + pad positions). Order nets: shortest Manhattan distance first, power/ground last. Route each net using pathfinder from T02. On failure: identify blocking net, rip up (clear its grid cells), reroute both. Cap rip-up at 3 iterations. Collect raw grid paths per net. Add `tracing` instrumentation for per-net routing progress. Handle partial results via `RoutingResult::partial()`.
  - Verify: `cargo test -p cypcb-autoroute` — orchestrator unit tests pass; routing-test.cypcb (simple 3-component board) routes all nets in integration test
  - Done when: `route_board()` returns `RoutingStatus::Complete` for `routing-test.cypcb`

- [x] **T04: Path post-processing, output conversion, and blink.cypcb validation** `est:2h`
  - Why: Converts raw grid paths to clean `RouteSegment`/`ViaPlacement` output, merges collinear segments, and proves the autorouter on the primary reference board
  - Files: `crates/cypcb-autoroute/src/postprocess.rs`, `crates/cypcb-autoroute/src/orchestrator.rs`, `crates/cypcb-autoroute/tests/integration.rs`
  - Do: Implement collinear segment merging (consecutive grid steps along same direction → single `RouteSegment`). Convert grid coordinates back to Nm. Create `ViaPlacement` for each layer transition. Set trace widths from `RoutingRuleSet::constraints_for_net()`. Calculate `RoutingMetrics` via `calculate_metrics()`. Integration test: parse `blink.cypcb`, build `BoardWorld`, route with `PresetRuleSet::new(RulesPreset::jlcpcb_2layer())`, assert 7/7 nets complete, assert all segments have valid width/layer, assert metrics within bounds (total length < 500mm, via count < 20), assert output is compatible with `apply_routes()`.
  - Verify: `cargo test -p cypcb-autoroute` — all tests pass including `blink.cypcb` routing; `cargo clippy -p cypcb-autoroute -- -D warnings` clean
  - Done when: `blink.cypcb` routes 7/7 nets, output passes through `apply_routes()` without panic, quality metrics asserted

- [x] **T05: WASM compilation verification and quality benchmarks** `est:1h`
  - Why: Proves WASM compatibility (hard requirement for web deployment) and establishes quality baseline benchmarks for future optimization
  - Files: `crates/cypcb-autoroute/Cargo.toml`, `crates/cypcb-autoroute/tests/integration.rs`, `crates/cypcb-autoroute/benches/routing_bench.rs`
  - Do: Verify `cargo build -p cypcb-autoroute --target wasm32-unknown-unknown` compiles. Fix any WASM-incompatible dependencies (should be none if plan was followed). Add benchmark using `#[bench]` or criterion (if available) measuring route time for both reference boards. Add integration test that runs DRC on autorouter output (parse board, route, apply_routes, run DRC, assert zero violations). Log quality metrics comparison table in test output. Ensure all `tracing` spans are present for diagnostic visibility.
  - Verify: `cargo build -p cypcb-autoroute --target wasm32-unknown-unknown` succeeds; `cargo test -p cypcb-autoroute` all green; DRC integration test passes
  - Done when: WASM compiles, all tests pass, DRC produces zero violations on routed output, benchmark baseline recorded

## Files Likely Touched

- `Cargo.toml` (workspace members)
- `crates/cypcb-autoroute/Cargo.toml`
- `crates/cypcb-autoroute/src/lib.rs`
- `crates/cypcb-autoroute/src/grid.rs`
- `crates/cypcb-autoroute/src/pathfinder.rs`
- `crates/cypcb-autoroute/src/cost.rs`
- `crates/cypcb-autoroute/src/orchestrator.rs`
- `crates/cypcb-autoroute/src/postprocess.rs`
- `crates/cypcb-autoroute/tests/integration.rs`
- `crates/cypcb-autoroute/benches/routing_bench.rs`
