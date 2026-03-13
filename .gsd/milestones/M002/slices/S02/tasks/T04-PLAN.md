---
estimated_steps: 5
estimated_files: 4
---

# T04: Path post-processing, output conversion, and blink.cypcb validation

**Slice:** S02 — Custom Autorouter Core
**Milestone:** M002

## Description

Convert raw grid paths into clean `RouteSegment`/`ViaPlacement` output. Merge collinear segments to produce minimal trace geometry. Wire up proper trace widths from `RoutingRuleSet`. Validate the complete autorouter pipeline by routing `blink.cypcb` (8 components, 7 nets) end-to-end and asserting quality metrics.

## Steps

1. Create `crates/cypcb-autoroute/src/postprocess.rs`:
   - `fn simplify_path(path: &[GridNode], grid: &RoutingGrid) -> Vec<PathSegment>` — detect direction changes in the grid path and emit one `PathSegment` per straight run. Adjacent nodes moving in the same direction (dx, dy, same layer) collapse into a single segment.
   - `PathSegment` intermediate type: `{ start: GridNode, end: GridNode, layer: u8 }` for straight segments, plus `LayerTransition { position: GridNode, from: u8, to: u8 }` for vias.

2. Implement `fn convert_to_route_segments()`:
   - `fn convert_to_route_segments(simplified: &[PathSegment], grid: &RoutingGrid, net_id: NetId, rules: &dyn RoutingRuleSet) -> (Vec<RouteSegment>, Vec<ViaPlacement>)`
   - Convert `PathSegment` grid coordinates to Nm using `grid.grid_to_nm()`
   - Map layer index to `Layer` enum (0→TopCopper, N-1→BottomCopper, 1..N-2→Inner(n-1))
   - Set trace width from `rules.constraints_for_net(net_id).min_trace_width`
   - Create `ViaPlacement` for each `LayerTransition` with drill size from `rules.constraints_for_net(net_id).min_via_drill`

3. Wire post-processing into orchestrator:
   - After `route_all_nets()` produces raw grid paths, run `simplify_path()` then `convert_to_route_segments()` for each net
   - Collect all segments and vias into `RoutingResult::complete()` or `RoutingResult::partial()`
   - Calculate `RoutingMetrics` using `calculate_metrics()` from `cypcb-router`

4. Update integration tests for `blink.cypcb`:
   - Parse `blink.cypcb`, build `BoardWorld`, create `PresetRuleSet::new(RulesPreset::jlcpcb_2layer())` (or equivalent)
   - Call `route_board()`, assert `RoutingStatus::Complete` (7/7 nets)
   - Assert all `RouteSegment` widths match JLCPCB min_trace_width
   - Assert all `RouteSegment` layers are `TopCopper` or `BottomCopper`
   - Assert quality bounds: total_length < 500mm (generous upper bound), via_count < 20
   - Apply result via `apply_routes()` and assert no panic — proves output contract compatibility
   - Print metrics table to test output for future comparison

5. Write unit tests for post-processing:
   - Test: collinear merge — 5 horizontal steps → 1 segment
   - Test: L-shaped path → 2 segments
   - Test: path with via → correct ViaPlacement generated
   - Test: coordinate conversion accuracy — segment endpoints within 1 grid cell of expected Nm

## Must-Haves

- [ ] Collinear segments merged (raw grid path of N steps → minimal segment count)
- [ ] All RouteSegments have correct width from RoutingRuleSet
- [ ] All ViaPlaccements have correct drill size and layer pair
- [ ] Grid coordinates convert back to Nm accurately
- [ ] `blink.cypcb` routes 7/7 nets with RoutingStatus::Complete
- [ ] Output is compatible with `apply_routes()` (no panic, entities spawned correctly)
- [ ] Quality metrics within bounds (total_length < 500mm, via_count < 20)

## Verification

- `cargo test -p cypcb-autoroute` — all tests pass including `blink.cypcb` full route
- `cargo clippy -p cypcb-autoroute -- -D warnings` — zero warnings
- Post-processing unit tests confirm segment merging and coordinate accuracy
- Integration test prints metrics: total_length, via_count, segment_count, completion_rate

## Observability Impact

- Signals added/changed: `tracing::info!("post-processing: {} raw steps -> {} segments, {} vias", raw_count, segment_count, via_count)`. Metrics logged at info level.
- How a future agent inspects this: `RoutingMetrics` attached to `RoutingResult`; integration tests print comparison table
- Failure state exposed: assertion failures show actual vs expected metrics; segments with zero width trigger explicit error

## Inputs

- `crates/cypcb-autoroute/src/orchestrator.rs` — raw grid paths from T03
- `crates/cypcb-autoroute/src/grid.rs` — `grid_to_nm()` coordinate conversion
- `crates/cypcb-router/src/types.rs` — `RouteSegment`, `ViaPlacement`, `RoutingResult`, `calculate_metrics()`
- `crates/cypcb-router/src/lib.rs` — `apply_routes()` for compatibility test
- `crates/cypcb-rules/src/presets/` — `PresetRuleSet` for JLCPCB constraints

## Expected Output

- `crates/cypcb-autoroute/src/postprocess.rs` — path simplification and output conversion
- `crates/cypcb-autoroute/src/orchestrator.rs` — updated to use post-processing
- `crates/cypcb-autoroute/tests/integration.rs` — `blink.cypcb` test passing with quality assertions
- `crates/cypcb-autoroute/src/lib.rs` — updated module declarations
