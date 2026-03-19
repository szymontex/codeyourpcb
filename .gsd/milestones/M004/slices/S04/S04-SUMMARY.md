---
id: S04
parent: M004
milestone: M004
provides:
  - smooth_routes() — 3-pass trace smoother (staircase collapse, corner chamfer, collinear merge) producing clean 45°/90° geometry
  - optimize_vias() — eliminates redundant via pairs when single-layer path is DRC-clean
  - is_valid_angle() — utility for 45°-multiple angle validation
  - Smoother integrated into both PathFinderStrategy and ImprovedAStarStrategy post-routing pipelines
requires:
  - slice: S03
    provides: PathFinderStrategy and ImprovedAStarStrategy with raw grid-aligned RouteSegment output
affects:
  - S05
  - S06
key_files:
  - crates/cypcb-autoroute/src/smoother.rs
  - crates/cypcb-autoroute/src/via_optimizer.rs
  - crates/cypcb-autoroute/src/pathfinder_v2.rs
  - crates/cypcb-autoroute/src/astar_improved.rs
  - crates/cypcb-autoroute/tests/smoother_integration.rs
  - crates/cypcb-autoroute/src/lib.rs
key_decisions:
  - Smoother operates on Vec<RouteSegment> in Nm coordinates, not grid cells — decoupled from routing internals
  - Angle validation uses exact integer patterns (dx==0, dy==0, |dx|==|dy|) not floating-point atan2
  - Per-move DRC uses segment_distance() against other-net segments; full run_drc() only as final gate
  - Chamfer length = min(len_A, len_B) / 3 capped at 1mm, minimum 1µm threshold
  - Staircase detection requires ≥3 alternating H/V connected segments to trigger collapse
  - Per-net smoothing with other-net segments as DRC context
patterns_established:
  - Smoother groups segments by (net_id, layer) and processes each group independently
  - DRC safety checked per-move via segment_distance(); failed moves preserve original segments
  - Via optimizer scans for complementary via pairs with single between-segment on alternate layer
  - Identical smoother integration pattern in both strategies (~25 lines each)
observability_surfaces:
  - tracing::info! in smooth_routes() logs before/after segment count
  - tracing::debug! in smooth_net_layer_group() logs per-(net_id, layer) segment reduction
  - tracing::debug! in is_drc_clean() logs each DRC rejection with segment coords and clearance
  - tracing::info! in optimize_vias() logs each eliminated via pair
  - Integration test prints formatted score table to stderr (smoothness, DRC, vias, length, composite)
drill_down_paths:
  - .gsd/milestones/M004/slices/S04/tasks/T01-SUMMARY.md
  - .gsd/milestones/M004/slices/S04/tasks/T02-SUMMARY.md
duration: 30m
verification_result: passed
completed_at: 2026-03-14
---

# S04: Trace Smoother & Via Optimizer

**Three-pass trace smoother producing smoothness=1.000 on led_blink with zero DRC regression, integrated into both routing strategies.**

## What Happened

Built a trace smoothing pipeline (`smoother.rs`, ~370 LOC) and via optimizer (`via_optimizer.rs`, ~150 LOC) that post-process raw grid-aligned autorouter output into clean 45°/90° traces.

**T01** implemented the core algorithms:
1. **Staircase-to-diagonal collapse** — detects ≥3 alternating H/V connected segments, replaces with a single 45° diagonal + orthogonal tail. Each proposed move DRC-checked via `segment_distance()` against other-net segments.
2. **Corner chamfering** — for remaining 90° bends, inserts a 45° chamfer segment (length = min(len_A, len_B) / 3, capped at 1mm). Only committed when DRC-clean.
3. **Collinear merge** — merges consecutive same-direction connected segments into single segments.

The via optimizer scans for complementary via pairs (L1→L2 + L2→L1) with a single between-segment on the alternate layer. If a direct segment on the original layer is DRC-clean, both vias are eliminated.

**T02** wired the smoother into both `PathFinderStrategy::route()` and `ImprovedAStarStrategy::route()`, positioned after `paths_to_output()` collection and before `RoutingResult` construction. Created an integration test routing `led_blink.kicad_pcb` that achieves smoothness=1.000 (all bends at 45° multiples) with DRC violations holding at 5 (unchanged from S03 baseline).

## Verification

All slice-level checks passed:

- ✅ `cargo test -p cypcb-autoroute --lib --release` — 110 tests pass (22 new: 17 smoother + 5 via_optimizer + 88 existing)
- ✅ `cargo test --test smoother_integration --release` — smoothness=1.000, DRC=5, vias=0, composite=5000.8
- ✅ `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — WASM compiles clean
- ✅ DRC rejection test (`drc_rejection_staircase_blocked`) — confirms smoothing move rejected when clearance violated, failure path exercised
- ✅ Observability: `RUST_LOG=cypcb_autoroute=info` shows pre/post segment counts; integration test prints formatted score table to stderr
- ✅ `cargo test --test strategy_comparison --release -- led_blink` — PathFinder still wins (5000.8 vs 15543.6)

## Requirements Advanced

- R107 (Zero DRC Violations) — DRC stays at 5 after smoothing (non-regression proven). Smoother does not introduce new violations. Still not zero — remaining 5 are grid artifacts from S03, target S07.
- R108 (Clean 45°/90° Trace Geometry) — smoothness=1.000 proves all output segments are at valid 45°-multiple angles. `is_valid_angle()` enforces this on every output segment.
- R109 (Trace Smoothing Post-Processor) — full 3-pass pipeline implemented: staircase collapse, corner chamfer, collinear merge. DRC safety preserved via per-move clearance checks.

## Requirements Validated

- R108 — smoothness=1.000 on led_blink integration test proves all traces are clean 45°/90° geometry. 22 unit tests cover staircase collapse, chamfering, merge, angle enforcement, DRC rejection, edge cases. `is_valid_angle()` validates every output segment.
- R109 — 3-pass smoother pipeline with per-move DRC safety, integrated into both strategies, proven by integration test (smoothness improvement + DRC non-regression). 17 unit tests + 1 integration test.

## New Requirements Surfaced

None.

## Requirements Invalidated or Re-scoped

None.

## Deviations

None — implementation followed both task plans directly.

## Known Limitations

- DRC violations remain at 5 on led_blink (unchanged from S03). These are grid-level artifacts, not smoother regressions. Target zero in S07.
- Via optimizer has limited impact on current benchmarks (PathFinder already produces 0 vias on led_blink). Will prove more useful on complex multi-layer boards.
- Smoother cannot fix fundamental routing quality issues (wrong net ordering, poor congestion resolution) — it only polishes the output geometry.

## Follow-ups

None — all planned work completed as specified.

## Files Created/Modified

- `crates/cypcb-autoroute/src/smoother.rs` — New: 3-pass trace smoothing module (~370 LOC) + 17 unit tests
- `crates/cypcb-autoroute/src/via_optimizer.rs` — New: via pair elimination module (~150 LOC) + 5 unit tests
- `crates/cypcb-autoroute/src/lib.rs` — Added `pub mod smoother;` and `pub mod via_optimizer;`
- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — Added smoother/via_optimizer integration after paths_to_output()
- `crates/cypcb-autoroute/src/astar_improved.rs` — Identical smoother/via_optimizer integration
- `crates/cypcb-autoroute/tests/smoother_integration.rs` — New: integration test (~115 LOC) with score comparison

## Forward Intelligence

### What the next slice should know
- Smoother is always active — there is no toggle to disable it. S05 (realtime tuning) may want a "roundness" parameter that controls chamfer aggressiveness.
- `smooth_routes()` signature: `(segments: &[RouteSegment], other_net_segments: &[RouteSegment], min_clearance: Nm) -> Vec<RouteSegment>` — S05/S06 call sites are already in both strategies.
- `is_valid_angle()` is public and can be used by downstream code to audit output geometry.

### What's fragile
- Per-move DRC uses `segment_distance()` which checks against a flat list of other-net segments — O(n*k) where k is other-net count. Fine for current board sizes but could become a bottleneck on very complex boards with thousands of segments.
- Staircase detection requires strict alternating H/V pattern — mixed-angle grid artifacts may not trigger the optimization.

### Authoritative diagnostics
- `RUST_LOG=cypcb_autoroute::smoother=debug cargo test --test smoother_integration --release -- --nocapture` — shows per-move DRC rejection logs with segment coordinates and clearance distances
- Integration test score table (printed to stderr) — single glance confirms smoothness, DRC, and composite are within budget

### What assumptions changed
- Expected smoother to improve smoothness from some baseline to a higher value — actual result is smoothness=1.000 (perfect), meaning the 3-pass pipeline fully eliminates all non-45° angles on led_blink. This is better than anticipated.
