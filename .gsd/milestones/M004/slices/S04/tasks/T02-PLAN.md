---
estimated_steps: 5
estimated_files: 3
---

# T02: Wire smoother into strategies and validate with integration test

**Slice:** S04 — Trace Smoother & Via Optimizer
**Milestone:** M004

## Description

Integrate the smoother and via optimizer into both routing strategies (PathFinder and ImprovedAStar) so all autorouted output is automatically smoothed. Create an integration test that routes led_blink, compares before/after smoothness and DRC scores, and proves the smoother delivers measurable improvement without introducing DRC violations. This is the slice's primary proof — the unit tests from T01 prove algorithmic correctness, but this test proves the smoother works on real routed output.

## Steps

1. Modify `PathFinderStrategy::route()` in `pathfinder_v2.rs`: after collecting `all_segments` and `all_vias` from per-net `paths_to_output()` calls, call `smooth_routes()` per-net (group segments by net_id, build other-net context, smooth each group, reassemble). Then call `optimize_vias()`. Use `rules.constraints_for_net(0).min_clearance` for the clearance parameter. Add tracing for before/after segment counts.

2. Apply the same integration to `ImprovedAStarStrategy::route()` in `astar_improved.rs` — identical pattern (shared code would be ideal but inline is acceptable given 2 call sites).

3. Create `crates/cypcb-autoroute/tests/smoother_integration.rs`: parse led_blink.kicad_pcb, route with PathFinder, apply routes, score. The test must demonstrate improvement. Approach: route the board (smoother is now always active), score it, and assert that smoothness ≥ 0.5 (real improvement from grid paths which score ~0.2-0.3). Also assert DRC violations ≤ 5 (S03 baseline — must not regress). Print a score summary table to stderr following the strategy_comparison.rs pattern.

4. Verify the existing `strategy_comparison` test still passes — the smoother should improve or maintain scores, never worsen them.

5. Check WASM compilation: `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown`.

## Must-Haves

- [ ] PathFinderStrategy::route() calls smooth_routes() + optimize_vias() on its output
- [ ] ImprovedAStarStrategy::route() calls smooth_routes() + optimize_vias() on its output
- [ ] Integration test on led_blink shows smoothness improvement (≥ 0.5)
- [ ] DRC violations don't increase vs S03 baseline (≤ 5 for PathFinder on led_blink)
- [ ] Existing strategy_comparison test still passes
- [ ] WASM compiles

## Verification

- `cargo test --test smoother_integration --release` — passes, prints before/after metrics showing improvement
- `cargo test --test strategy_comparison --release -- led_blink` — still passes (scores equal or better)
- `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — compiles
- `cargo test -p cypcb-autoroute --lib --release` — all 88+ existing tests still pass

## Observability Impact

- Signals added: `tracing::info!` in both strategies showing pre-smooth vs post-smooth segment counts
- How a future agent inspects: `RUST_LOG=cypcb_autoroute=info cargo test --test smoother_integration --release -- --nocapture` shows smoothing statistics

## Inputs

- `crates/cypcb-autoroute/src/smoother.rs` — `smooth_routes()` from T01
- `crates/cypcb-autoroute/src/via_optimizer.rs` — `optimize_vias()` from T01
- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — PathFinderStrategy::route() insertion point (lines 96-117)
- `crates/cypcb-autoroute/src/astar_improved.rs` — ImprovedAStarStrategy::route() insertion point (lines 96-116)
- `crates/cypcb-autoroute/tests/strategy_comparison.rs` — pattern for fixture parsing, scoring, and table output
- `tests/fixtures/benchmark/led_blink.kicad_pcb` — benchmark fixture for integration test

## Expected Output

- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — modified: smoother/via_optimizer calls added after paths_to_output collection
- `crates/cypcb-autoroute/src/astar_improved.rs` — modified: same smoother/via_optimizer integration
- `crates/cypcb-autoroute/tests/smoother_integration.rs` — new integration test (~120 LOC) with score comparison and assertions
