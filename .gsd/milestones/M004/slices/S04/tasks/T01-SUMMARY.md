---
id: T01
parent: S04
milestone: M004
provides:
  - smooth_routes() function that collapses staircases, chamfers corners, merges collinear segments
  - optimize_vias() function that eliminates redundant via pairs when single-layer path is DRC-clean
  - is_valid_angle() utility for 45°-multiple angle validation
key_files:
  - crates/cypcb-autoroute/src/smoother.rs
  - crates/cypcb-autoroute/src/via_optimizer.rs
  - crates/cypcb-autoroute/src/lib.rs
key_decisions:
  - segment_distance() from cypcb-drc used directly for per-move DRC checking (no full run_drc() during smoothing)
  - Staircase detection requires ≥3 alternating H/V connected segments to trigger collapse
  - Chamfer length = min(len_A, len_B) / 3 capped at 1mm, minimum 1µm threshold
  - Angle validation uses exact integer patterns (dx==0, dy==0, |dx|==|dy|) not floating-point atan2
patterns_established:
  - Smoother groups segments by (net_id, layer) and processes each group independently
  - DRC safety is checked per-move via segment_distance(); if a move fails clearance, the original segments are preserved
  - Via optimizer scans for complementary via pairs with a single between-segment on the alternate layer
observability_surfaces:
  - tracing::info! in smooth_routes() logs before/after segment count
  - tracing::debug! per-group in smooth_net_layer_group() logs per-(net_id, layer) segment reduction
  - tracing::debug! in is_drc_clean() logs each DRC rejection with segment coords and clearance
  - tracing::info! in optimize_vias() logs each eliminated via pair
duration: 15m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T01: Build smoother and via optimizer modules with unit tests

**Implemented trace smoother (staircase collapse, corner chamfering, collinear merge) and via optimizer with 22 unit tests, all passing with clean WASM build.**

## What Happened

Created `smoother.rs` (~370 LOC) with three-pass smoothing pipeline:
1. **Staircase-to-diagonal collapse** — detects ≥3 alternating H/V connected segments, replaces with single 45° diagonal + orthogonal tail. Each proposed move checked via `segment_distance()` against other-net segments.
2. **Corner chamfering** — for remaining 90° bends, inserts a 45° chamfer segment (length = min(len_A, len_B) / 3, capped at 1mm). Verified DRC-clean before committing.
3. **Collinear merge** — merges consecutive same-direction connected segments into single segments.

Created `via_optimizer.rs` (~150 LOC) that scans for complementary via pairs (L1→L2 + L2→L1) with a single between-segment on the alternate layer. If a direct segment on the original layer is DRC-clean, both vias are eliminated.

Both modules use `segment_distance()` from cypcb-drc for per-move clearance checking. All output segments are validated for 45°-multiple angles via `is_valid_angle()` using exact integer direction patterns.

## Verification

- `cargo test -p cypcb-autoroute --lib --release` — **110 tests pass** (22 new: 17 smoother + 5 via_optimizer)
- `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — **WASM compiles clean** (no std::time, no filesystem)
- 10-step staircase test produces ≤3 output segments (manual inspection requirement met)
- DRC rejection test confirms staircase preserved when diagonal violates clearance
- Via pair elimination test confirms 2 vias → 0 when direct path is clean
- Via pair preservation test confirms vias kept when obstacle blocks direct path

**Slice-level checks:**
- ✅ `cargo test -p cypcb-autoroute --lib --release` — all pass
- ⬜ `cargo test --test smoother_integration --release` — T02 (not yet created)
- ✅ `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — clean

## Diagnostics

- Run `RUST_LOG=cypcb_autoroute::smoother=debug cargo test -p cypcb-autoroute` to see per-move DRC rejection logs
- Run `RUST_LOG=cypcb_autoroute::via_optimizer=info cargo test -p cypcb-autoroute` to see via elimination logs
- `is_valid_angle()` is public — can be used by integration tests or downstream code to audit output geometry

## Deviations

None. Implementation followed the task plan directly.

## Known Issues

None.

## Files Created/Modified

- `crates/cypcb-autoroute/src/smoother.rs` — New: trace smoothing module with 3-pass pipeline + 17 unit tests
- `crates/cypcb-autoroute/src/via_optimizer.rs` — New: via pair elimination module + 5 unit tests
- `crates/cypcb-autoroute/src/lib.rs` — Added `pub mod smoother;` and `pub mod via_optimizer;`
- `.gsd/milestones/M004/slices/S04/S04-PLAN.md` — Added DRC rejection failure-path verification step
- `.gsd/milestones/M004/slices/S04/tasks/T01-PLAN.md` — Added Observability Impact section
