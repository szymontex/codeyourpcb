# S04: Trace Smoother & Via Optimizer

**Goal:** Raw grid-aligned autorouter paths post-processed into clean 45°/90° traces with minimized vias, DRC still passing after smoothing.
**Demo:** Route led_blink with PathFinder → smooth → score. Smoothness metric improves (closer to 1.0), trace length decreases, DRC violations don't increase. Before/after comparison in integration test output.

## Must-Haves

- `smoother.rs` module: staircase-to-diagonal collapse, corner chamfering, collinear segment merge — output contains only 0°/45°/90°/135° angle segments
- `via_optimizer.rs` module: eliminate redundant via pairs where single-layer routing is DRC-clean
- Both modules operate on `Vec<RouteSegment>` / `Vec<ViaPlacement>` (Nm coordinates, not grid)
- Per-move DRC safety: smoothed segments checked against nearby other-net segments via `segment_distance()`
- Smoother preserves net_id, layer, and width on all output segments
- Both strategies (PathFinder + ImprovedAStar) call smoother/via_optimizer after `paths_to_output()`
- Integration test proves smoothness improves and DRC doesn't regress on led_blink

## Proof Level

- This slice proves: contract (algorithmic correctness + DRC safety of post-processing)
- Real runtime required: no (Rust tests only)
- Human/UAT required: no

## Verification

- `cargo test -p cypcb-autoroute --lib --release` — all existing + new smoother/via_optimizer unit tests pass
- `cargo test --test smoother_integration --release` — before/after scores show smoothness improvement, DRC non-regression
- `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — WASM compiles (no std::time, no filesystem)
- DRC rejection test: at least one unit test verifies a smoothing move is rejected when it violates clearance, confirming the failure path is exercised and `tracing::debug!` rejection log fires

## Observability / Diagnostics

- Runtime signals: `tracing::info!` in `smooth_routes()` with before/after segment count and smoothness delta
- Inspection surfaces: integration test prints before/after score table to stderr (same pattern as strategy_comparison.rs)
- Failure visibility: per-move DRC rejection logged at `tracing::debug!` level with segment coordinates and clearance distance

## Integration Closure

- Upstream surfaces consumed: `RouteSegment`/`ViaPlacement` from `cypcb_router::types`, `segment_distance()` from `cypcb_drc::rules::clearance`, `run_drc()` for final gate
- New wiring introduced: smoother/via_optimizer calls added inside `PathFinderStrategy::route()` and `ImprovedAStarStrategy::route()` between `paths_to_output()` collection and `RoutingResult::complete()`
- What remains before the milestone is truly usable end-to-end: S05 (realtime tuning), S06 (variant preview UI), S07 (benchmark validation)

## Tasks

- [x] **T01: Build smoother and via optimizer modules with unit tests** `est:60m`
  - Why: Core algorithmic work — staircase detection, corner chamfering, segment merge, via pair elimination need comprehensive unit tests before integration
  - Files: `crates/cypcb-autoroute/src/smoother.rs`, `crates/cypcb-autoroute/src/via_optimizer.rs`, `crates/cypcb-autoroute/src/lib.rs`
  - Do: Implement `smooth_routes(segments: &[RouteSegment], other_net_segments: &[RouteSegment], min_clearance: Nm) -> Vec<RouteSegment>` with three passes (staircase→diagonal, corner chamfer, collinear merge). Implement `optimize_vias(segments: Vec<RouteSegment>, vias: Vec<ViaPlacement>, other_net_segments: &[RouteSegment], min_clearance: Nm) -> (Vec<RouteSegment>, Vec<ViaPlacement>)`. Use `segment_distance()` for per-move DRC safety. Enforce 45°-multiple angles on all output. Group segments by (net_id, layer) and smooth per-group. Add mod declarations in lib.rs.
  - Verify: `cargo test -p cypcb-autoroute --lib --release` — all new unit tests pass; `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — WASM compiles
  - Done when: ≥15 unit tests covering staircase collapse, chamfering, merge, angle enforcement, DRC rejection, via elimination, and edge cases (empty input, single segment, zero-length)

- [x] **T02: Wire smoother into strategies and validate with integration test** `est:45m`
  - Why: Proves end-to-end effect — smoother must improve real routed output while maintaining DRC safety
  - Files: `crates/cypcb-autoroute/src/pathfinder_v2.rs`, `crates/cypcb-autoroute/src/astar_improved.rs`, `crates/cypcb-autoroute/tests/smoother_integration.rs`
  - Do: In both strategies' `route()`, after collecting all_segments/all_vias, call `smooth_routes()` per-net with other-net segments as context, then `optimize_vias()`. Create integration test that routes led_blink with PathFinder, compares scores before/after smoothing (separate routing runs with smoother enabled/disabled, or manual smooth call). Assert: smoothness_after > smoothness_before, drc_violations_after <= drc_violations_before, total_length_after <= total_length_before. Print comparison table to stderr.
  - Verify: `cargo test --test smoother_integration --release` passes with visible improvement metrics
  - Done when: Integration test passes — smoothness improved, DRC non-regression proven, WASM still compiles

## Files Likely Touched

- `crates/cypcb-autoroute/src/smoother.rs` (new)
- `crates/cypcb-autoroute/src/via_optimizer.rs` (new)
- `crates/cypcb-autoroute/src/lib.rs`
- `crates/cypcb-autoroute/src/pathfinder_v2.rs`
- `crates/cypcb-autoroute/src/astar_improved.rs`
- `crates/cypcb-autoroute/tests/smoother_integration.rs` (new)
