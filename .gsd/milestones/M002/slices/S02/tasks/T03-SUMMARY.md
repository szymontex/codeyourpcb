---
id: T03
parent: S02
milestone: M002
provides:
  - Net ordering and rip-up/reroute orchestrator in orchestrator.rs
  - Working route_board() entry point that routes all nets end-to-end
  - Ratsnest extraction from BoardWorld with MST-based connection ordering
  - Basic grid-to-segment conversion for RoutingResult output
key_files:
  - crates/cypcb-autoroute/src/orchestrator.rs
  - crates/cypcb-autoroute/src/lib.rs
  - crates/cypcb-autoroute/src/pathfinder.rs
  - crates/cypcb-autoroute/src/grid.rs
  - crates/cypcb-autoroute/tests/integration.rs
key_decisions:
  - Pad zones introduced to pathfinder — pads are marked as obstacles on the grid for clearance enforcement, but the net's own pads must be reachable. PadZone circles override is_free() checks within pad radius + clearance margin.
  - Blocking net detection samples along the full direct path between start/end (not just endpoints) to find the most likely congestion blocker anywhere along the route.
  - ConnectionAttempt struct groups routing parameters to avoid clippy too_many_arguments while keeping the ripup function readable.
  - Basic segment conversion (direction-change splitting) included in T03 rather than deferring entirely to T04, so integration tests can verify Complete status now.
patterns_established:
  - extract_ratsnest() collects pad positions from BoardWorld by iterating components with (Position, Rotation, FootprintRef, NetConnections), matching pins to footprint PadDefs
  - order_nets() sorts by Manhattan span (short first), with is_power_net() heuristic pushing VCC/GND/etc to end
  - build_spanning_tree() uses greedy nearest-neighbor MST for multi-pin nets, producing n-1 point-to-point connections
  - Rip-up loop capped at config.max_ripup_iterations — finds victim via find_blocking_net(), clears, routes current, re-routes victim, restores on failure
observability_surfaces:
  - tracing::info_span!("route_board") wraps entire operation
  - Per-net tracing::info! with net_id, net_name, connection_count, success/failure, path_count
  - Rip-up events logged as tracing::warn! with victim and current net IDs
  - Final tracing::info! with routed/total counts and via count
  - RoutingResult.status exposes Complete vs Partial{unrouted_count}
  - Integration tests print metrics (segments, vias, total_length, quality_score) to stderr
duration: 1h
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T03: Build net ordering and rip-up/reroute orchestrator

**Implemented routing orchestrator with ratsnest extraction, priority-based net ordering, MST connection planning, and rip-up/reroute congestion resolution. Both reference boards route 100% Complete.**

## What Happened

Built the orchestration layer in `orchestrator.rs` that drives the full routing pipeline:

1. **Ratsnest extraction** (`extract_ratsnest`): Iterates all components in BoardWorld, matches pin connections to footprint pad definitions, computes absolute pad positions accounting for component rotation. Single-pad nets are filtered out. Multi-pin nets get a greedy nearest-neighbor MST to produce ordered point-to-point connections.

2. **Net ordering** (`order_nets`): Short nets first (by Manhattan bounding box span), power/ground nets (VCC, GND, 5V, 3V3, etc.) pushed to end. Power nets are more tolerant of longer paths.

3. **Routing loop** (`route_all_nets`): Routes each net's connections sequentially using the A* pathfinder. On failure, `attempt_ripup_reroute` searches along the direct path for blocking nets, rips up the most likely blocker, routes the current net, then re-routes the victim. Capped at `config.max_ripup_iterations`.

4. **Pad zones**: Discovered that pad clearance bloat was preventing routes from reaching their own pads. Added `PadZone` to the pathfinder — circles around pad endpoints that override `is_free()` checks, allowing routes to enter/exit their own net's pads while still respecting obstacles from other nets.

5. **Segment conversion** (`paths_to_segments`): Basic conversion from grid paths to `RouteSegment`/`ViaPlacement` — splits at direction changes and layer transitions. Collinear merging deferred to T04.

6. **Wired `route_board()`** in lib.rs: Builds grid, extracts ratsnest, orders nets, runs routing loop, converts to RoutingResult. Returns Complete when all nets route, Partial with unrouted count otherwise.

## Verification

- `cargo test -p cypcb-autoroute` — **33 tests pass** (30 unit + 3 integration)
- Unit tests: `order_nets_short_before_long`, `order_nets_power_last`, `spanning_tree_produces_n_minus_1_edges`, `extract_ratsnest_on_test_board`, `ripup_triggers_on_blocked_path`
- Integration: `routing-test.cypcb` → **Complete** (10 segments, 0 vias)
- Integration: `blink.cypcb` → **Complete** (46 segments, 8 vias, 182.5mm total length, quality score 222.5)
- `cargo clippy -p cypcb-autoroute` — zero warnings in our crate
- WASM compile: pre-existing `getrandom` issue in transitive dependency, not introduced by this task

### Slice-level verification status (intermediate task):
- ✅ `cargo test -p cypcb-autoroute` — all unit and integration tests pass
- ✅ `cargo test -p cypcb-autoroute -- --test integration` — routes reference boards end-to-end
- ⚠️ `cargo clippy -p cypcb-autoroute -- -D warnings` — zero warnings in our crate; pre-existing cypcb-core derive issue blocks -D warnings
- ⚠️ `cargo build -p cypcb-autoroute --target wasm32-unknown-unknown` — pre-existing getrandom WASM issue, not our code
- ✅ `blink.cypcb` routes 7/7 nets with `RoutingStatus::Complete`
- ✅ `routing-test.cypcb` routes 3/3 nets with `RoutingStatus::Complete`  
- ✅ All route segments have non-zero width
- ✅ Quality metrics logged (via count, total length within reasonable bounds)

## Diagnostics

- Run `RUST_LOG=cypcb_autoroute=info cargo test -p cypcb-autoroute -- --nocapture` to see per-net routing progress
- `RUST_LOG=cypcb_autoroute=debug` for pathfinder-level detail (path lengths, via counts, failed searches)
- `RoutingResult.status` programmatically exposes Complete vs Partial{unrouted_count}
- `calculate_metrics(&result)` returns `RoutingMetrics` with total_length, via_count, quality_score

## Deviations

- **Pad zones added to pathfinder** (not in original plan): Discovered that pad clearance bloat prevented the pathfinder from reaching net endpoints. Added `PadZone` and `find_path_with_zones()` as a clean extension rather than modifying the grid's obstacle logic.
- **Basic segment conversion done in T03**: Plan said "conversion to RouteSegment happens in T04", but integration tests needed Complete status with segments. Added `paths_to_segments()` with direction-change splitting. T04 can improve this with collinear merging.
- **`layer_to_index` made public** and `index_to_layer` added in grid.rs for use by orchestrator.

## Known Issues

- Segment conversion is basic (splits at every direction change) — T04 should add collinear merging for cleaner output
- WASM compile blocked by pre-existing `getrandom` transitive dependency issue — not introduced by this task

## Files Created/Modified

- `crates/cypcb-autoroute/src/orchestrator.rs` — **created**: ratsnest extraction, net ordering, routing loop, rip-up/reroute, segment conversion (~550 lines)
- `crates/cypcb-autoroute/src/lib.rs` — **modified**: added `pub mod orchestrator`, implemented working `route_board()` replacing stub
- `crates/cypcb-autoroute/src/pathfinder.rs` — **modified**: added `PadZone`, `find_path_with_zones()`, `in_pad_zone()` helper
- `crates/cypcb-autoroute/src/grid.rs` — **modified**: made `layer_to_index` public, added `index_to_layer()`, added `net_at()` method
- `crates/cypcb-autoroute/tests/integration.rs` — **modified**: updated integration tests with assertions for Complete status, non-zero segment widths, quality metrics output
