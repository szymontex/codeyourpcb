---
estimated_steps: 5
estimated_files: 3
---

# T03: Build net ordering and rip-up/reroute orchestrator

**Slice:** S02 — Custom Autorouter Core
**Milestone:** M002

## Description

Implement the orchestration layer that routes all nets on a board. This is the "brain" of the autorouter: it extracts the ratsnest (which pin pairs need connecting) from `BoardWorld`, orders nets by routing priority, routes each net sequentially using the pathfinder from T02, and handles congestion via rip-up/reroute when a net fails to route.

## Steps

1. Create `crates/cypcb-autoroute/src/orchestrator.rs`:
   - `fn extract_ratsnest(world: &BoardWorld) -> Vec<NetRoute>` — for each net in `BoardWorld`, collect all pin pad positions that belong to that net (from `NetConnections` + component `Position` + `Pad` offsets). Produce `NetRoute { net_id, pads: Vec<PadTarget> }` where `PadTarget` has position (Nm), layer mask, and pad size.
   - Handle multi-pin nets: use minimum spanning tree of pad positions (greedy nearest-neighbor) to produce ordered point-to-point connections for each net. This avoids routing from pad A to pad Z across the entire board when pad B is nearby.

2. Implement net ordering:
   - `fn order_nets(ratsnest: &[NetRoute]) -> Vec<usize>` — returns indices sorted by priority
   - Priority: shortest total Manhattan span first (short nets are easier and less likely to block others)
   - Exception: nets classified as power/ground (by name heuristic: "VCC", "GND", "5V", "3V3", etc.) routed last (they're more tolerant of longer paths)
   - Critical nets (if signal class is `HighSpeed` from rules) go first regardless of length

3. Implement the routing loop in `fn route_all_nets()`:
   - For each net in priority order: extract pad targets, convert to grid coordinates, call `find_path()` for each connection pair
   - On routing failure for a net: identify which existing net's route is blocking (find occupied cells near the failed path endpoints), rip up that net (clear its grid cells via `grid.clear_route(net_id)`), re-route the current net, then re-route the ripped-up net
   - Cap rip-up iterations at `config.max_rip_up_iterations` (default 3)
   - After all iterations, collect routed paths (as `Vec<GridNode>` per net) and unrouted net list

4. Wire into `route_board()` in `lib.rs`:
   - `pub fn route_board(world: &BoardWorld, rules: &dyn RoutingRuleSet, config: &AutorouteConfig) -> RoutingResult`
   - Build grid, extract ratsnest, order nets, run routing loop, return raw grid paths + unrouted list
   - Output is still raw grid paths at this point — conversion to `RouteSegment` happens in T04
   - For now, produce a minimal `RoutingResult` with grid paths converted to segments (basic, no merging) so integration tests can check completion status

5. Write unit tests and update integration tests:
   - Unit test: `extract_ratsnest` on a manually-built `BoardWorld` with known nets
   - Unit test: `order_nets` puts short nets before long, power nets last
   - Unit test: rip-up triggers when path is blocked by prior net
   - Integration test: `routing-test.cypcb` (3 components, 3 nets) should now route with `RoutingStatus::Complete`

## Must-Haves

- [ ] Ratsnest extraction correctly identifies all pin-pair connections per net
- [ ] Net ordering prioritizes short nets over long, power/ground last
- [ ] Rip-up/reroute fires on congestion and is capped at configured iterations
- [ ] `route_board()` returns `RoutingStatus::Complete` when all nets route, `Partial` otherwise
- [ ] `routing-test.cypcb` integration test passes with Complete status
- [ ] `tracing` instrumentation on per-net routing: net_id, success/failure, path length

## Verification

- `cargo test -p cypcb-autoroute` — orchestrator unit tests pass
- Integration test `route_routing_test_board` passes with `RoutingStatus::Complete`
- Rip-up unit test demonstrates re-routing on congestion

## Observability Impact

- Signals added/changed: `tracing::info_span!("route_board")` wrapping the entire operation. Per-net: `tracing::info!("routing net {} ({} connections)", net_id, connection_count)`. Rip-up events: `tracing::warn!("rip-up: removing net {} to make way for net {}", victim, current)`. Final: `tracing::info!("routing complete: {}/{} nets routed, {} vias", routed, total, via_count)`.
- How a future agent inspects this: `RoutingResult` status + metrics; tracing output in test logs (`RUST_LOG=cypcb_autoroute=debug cargo test`)
- Failure state exposed: `RoutingStatus::Partial` includes unrouted count; tracing logs list unrouted net IDs

## Inputs

- `crates/cypcb-autoroute/src/grid.rs` — `RoutingGrid` with `mark_route()`, `clear_route()`, `is_free()`
- `crates/cypcb-autoroute/src/pathfinder.rs` — `find_path()` from T02
- `crates/cypcb-world/src/world.rs` — `BoardWorld` for net/component/pad queries
- `crates/cypcb-world/src/components/electrical.rs` — `NetConnections`, `PinConnection` for ratsnest extraction

## Expected Output

- `crates/cypcb-autoroute/src/orchestrator.rs` — `extract_ratsnest()`, `order_nets()`, `route_all_nets()`, full routing orchestration
- `crates/cypcb-autoroute/src/lib.rs` — working `route_board()` function
- `crates/cypcb-autoroute/tests/integration.rs` — `routing-test.cypcb` test passing
