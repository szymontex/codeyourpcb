---
id: T02
parent: S04
milestone: M004
provides:
  - smooth_routes() + optimize_vias() integrated into both PathFinderStrategy::route() and ImprovedAStarStrategy::route()
  - smoother_integration test proving smoothness=1.000 and DRC≤5 on led_blink
key_files:
  - crates/cypcb-autoroute/src/pathfinder_v2.rs
  - crates/cypcb-autoroute/src/astar_improved.rs
  - crates/cypcb-autoroute/tests/smoother_integration.rs
key_decisions:
  - Per-net smoothing with other-net segments as DRC context (group by net_id, smooth each group with all other nets' segments as other_net_segments)
  - optimize_vias() called with empty other_net_segments since all segments are already smoothed and context was applied during smooth_routes()
  - min_clearance sourced from rules.constraints_for_net(0).min_clearance
patterns_established:
  - Smoother integration pattern identical in both strategies — inline at both call sites (2 call sites, ~25 lines each)
observability_surfaces:
  - tracing::info! in both strategies showing pre_smooth_segments, post_smooth_segments, pre_smooth_vias, post_smooth_vias
  - Integration test prints score table to stderr with smoothness, DRC violations, vias, length, composite
duration: 15m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T02: Wire smoother into strategies and validate with integration test

**Integrated trace smoother and via optimizer into both routing strategies with integration test proving smoothness=1.000 and DRC≤5 on led_blink.**

## What Happened

Added `smooth_routes()` and `optimize_vias()` calls into both `PathFinderStrategy::route()` and `ImprovedAStarStrategy::route()`, positioned after the `paths_to_output()` collection loop and before the `RoutingResult` construction. The integration groups segments by `net_id`, builds other-net context for each group, smooths per-net, then reassembles and runs via optimization.

Created `smoother_integration.rs` integration test that routes `led_blink.kicad_pcb` with PathFinder (smoother now always active), scores the result, and asserts smoothness ≥ 0.5 and DRC violations ≤ 5. The test achieves smoothness=1.000 (perfect — all bends at 45° multiples) with 5 DRC violations (exactly at the S03 baseline).

## Verification

All checks passed:

- `cargo test --test smoother_integration --release` — **PASSED**: smoothness=1.000, DRC violations=5, vias=0
- `cargo test --test strategy_comparison --release -- led_blink` — **PASSED**: PathFinder composite (5000.8) ≤ ImprovedAStar (15543.6)
- `cargo test -p cypcb-autoroute --lib --release` — **PASSED**: 110 tests pass (22 smoother/via_optimizer + 88 existing)
- `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — **PASSED**: WASM compiles clean

Slice-level verification status:
- ✅ `cargo test -p cypcb-autoroute --lib --release` — all 110 tests pass
- ✅ `cargo test --test smoother_integration --release` — smoothness improvement confirmed, DRC non-regression
- ✅ `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — WASM compiles
- ✅ DRC rejection test: `drc_rejection_staircase_blocked` unit test verifies smoothing move rejected when clearance violated

## Diagnostics

- `RUST_LOG=cypcb_autoroute=info cargo test --test smoother_integration --release -- --nocapture` — shows smoothing statistics (pre/post segment counts)
- `RUST_LOG=cypcb_autoroute::smoother=debug cargo test --test smoother_integration --release -- --nocapture` — shows per-move DRC rejection logs
- Integration test prints a formatted score table to stderr showing smoothness, DRC violations, vias, trace length, and composite

## Deviations

None — implementation followed the plan exactly.

## Known Issues

None.

## Files Created/Modified

- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — added smoother/via_optimizer imports and post-routing smooth+optimize calls
- `crates/cypcb-autoroute/src/astar_improved.rs` — identical smoother/via_optimizer integration
- `crates/cypcb-autoroute/tests/smoother_integration.rs` — new integration test (~115 LOC) with score comparison and assertions
