---
id: T01
parent: S02
milestone: M004
provides:
  - RoutingScore struct with all 7 metrics (total_length, via_count, drc_violations, smoothness, crossings, layer_balance, composite)
  - ScoreWeights struct with Default implementation (equal weights)
  - score_board() public function — boundary contract for S06 and S07
  - 27 unit tests covering all metric computations and edge cases
key_files:
  - crates/cypcb-autoroute/src/scoring.rs
  - crates/cypcb-autoroute/Cargo.toml
  - crates/cypcb-autoroute/src/lib.rs
key_decisions:
  - D-M004-012: serde Serialize for RoutingScore (not manual format!())
  - D-M004-013: Crossing detection uses segment_distance()==0 separately from DRC violations
  - D-M004-014: Scoring module lives in cypcb-autoroute, not a separate crate
  - Used std::collections::{HashMap,HashSet} instead of hashbrown (not a direct dependency of cypcb-autoroute)
patterns_established:
  - score_board(world, rules, weights) → RoutingScore as the single scoring entry point
  - Composite formula: weighted sum with board-diagonal normalization for length, penalty multipliers for DRC (×1000), crossings (×500), smoothness (×100), balance (×50)
  - angle_penalty() helper for smoothness metric — deviation from nearest 45° multiple
  - TraceData internal struct for collecting ECS trace data before metric computation
observability_surfaces:
  - tracing::debug! in score_board() with all 7 metric values
  - RoutingScore derives Serialize — JSON output for CLI and WASM consumers
duration: 25m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T01: Implement scoring module with all 7 metrics and unit tests

**Created `cypcb_autoroute::scoring` module with `RoutingScore`, `ScoreWeights`, `score_board()`, and 27 unit tests covering all 7 routing quality metrics.**

## What Happened

1. **Updated Cargo.toml**: Promoted `cypcb-drc` from dev-dependency to regular dependency. Added `serde = { workspace = true }` and `serde_json = { workspace = true }` (dev-dep for serialization test). No dependency cycles.

2. **Created `scoring.rs` with structs**: `RoutingScore` with 7 fields (all derive `Serialize`), `ScoreWeights` with `Default` (equal weights of 1.0), `ScoringConfig` holding weights.

3. **Implemented `score_board()`**: Queries ECS for `Trace` and `Via` entities, runs `run_drc()`, calls all metric helpers, computes composite. Takes `&mut BoardWorld`, `&DesignRules`, `&ScoreWeights`, returns `RoutingScore`.

4. **Smoothness metric**: Iterates consecutive segment pairs per trace, computes bend angle via `atan2`, penalizes deviations from 45° multiples. Zero-length segments skipped. No bends = 1.0 (perfect).

5. **Crossing detection**: Rebuilds spatial index with `rebuild_spatial_index_with_traces()`, queries nearby segments on same layer, filters same-net pairs, uses `segment_distance()==0` for intersection detection. Canonical pair ordering prevents double-counting.

6. **Layer balance + composite + tests**: Layer balance as min/max trace-count ratio (1.0 for single-layer). Composite: weighted sum with length normalized by board diagonal. 27 unit tests covering angle penalty (0°/45°/90°/22.5°/23°/negative), layer balance (empty/single/balanced/imbalanced), smoothness (empty/no-bends/90°/45°/zero-length), composite formula (zero/length/via/drc/lower-is-better), JSON serialization, score_board on empty/traced/viad boards, crossing detection (different-net/same-net/different-layer).

7. **Added `pub mod scoring;`** to `lib.rs`.

## Verification

- ✅ `cargo test -p cypcb-autoroute` — all 67 tests pass (27 new scoring + 40 existing)
- ✅ `cargo check -p cypcb-autoroute` — compiles clean, cypcb-drc linked as regular dep
- ✅ `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — WASM compilation passes
- ✅ Integration tests still pass (5 pass, 2 ignored benchmarks)

### Slice-level verification (partial — T01 is intermediate):
- ✅ `cargo test -p cypcb-autoroute` — all scoring unit tests pass
- ❌ `cargo test -p cypcb-autoroute --test scoring_integration` — test file not yet created (T02 scope)
- ❌ `cargo build -p cypcb-cli` — score command not yet added (T02 scope)

## Diagnostics

- `score_board()` emits `tracing::debug!` with all 7 metric values after computation
- `RoutingScore` derives `Serialize` — inspect via `serde_json::to_string()` or CLI JSON output (T02)
- Empty board produces well-defined defaults: length=0, vias=0, crossings=0, smoothness=1.0, balance=1.0, composite=0.0

## Deviations

- `score_board()` signature takes 3 args `(world, rules, weights)` instead of 2 `(world, rules)` as in the task plan — `ScoreWeights` was added as explicit parameter for configurability rather than using a hardcoded default. This is additive and compatible with the API contract.
- Used `std::collections::{HashMap,HashSet}` instead of `hashbrown` — `hashbrown` is not a direct dependency of `cypcb-autoroute`. `std::collections` is sufficient and avoids adding a dependency.

## Known Issues

- None

## Files Created/Modified

- `crates/cypcb-autoroute/Cargo.toml` — promoted cypcb-drc to regular dep, added serde + serde_json
- `crates/cypcb-autoroute/src/scoring.rs` — new module: RoutingScore, ScoreWeights, ScoringConfig, score_board(), 6 metric helpers, 27 unit tests
- `crates/cypcb-autoroute/src/lib.rs` — added `pub mod scoring;`
- `.gsd/milestones/M004/slices/S02/S02-PLAN.md` — added Observability/Diagnostics section, failure-path verification check
- `.gsd/milestones/M004/slices/S02/tasks/T01-PLAN.md` — added Observability Impact section
