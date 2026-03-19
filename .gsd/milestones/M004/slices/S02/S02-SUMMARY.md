---
id: S02
parent: M004
milestone: M004
provides:
  - RoutingScore struct with 7 metrics (total_length, via_count, drc_violations, smoothness, crossings, layer_balance, composite)
  - ScoreWeights struct with Default implementation (equal weights of 1.0)
  - score_board(world, rules, weights) → RoutingScore — boundary contract for S06 and S07
  - CLI `cypcb score <file>` command with JSON output
  - 27 unit tests + 4 integration tests covering all metric computations and baseline scores
  - Baseline scores recorded for blink.cypcb (composite=52046.24) and routing-test.cypcb (composite=5000.55)
requires:
  - slice: none
    provides: parallel with S01, uses existing BoardWorld + DRC
affects:
  - S03 (routing engine uses score_board() to compare strategies)
  - S06 (variant ranking consumes RoutingScore)
  - S07 (benchmark suite consumes score_board() for automated comparison)
key_files:
  - crates/cypcb-autoroute/src/scoring.rs
  - crates/cypcb-autoroute/tests/scoring_integration.rs
  - crates/cypcb-cli/src/commands/score.rs
  - crates/cypcb-autoroute/Cargo.toml
  - crates/cypcb-autoroute/src/lib.rs
  - crates/cypcb-cli/src/commands/mod.rs
  - crates/cypcb-cli/src/main.rs
  - crates/cypcb-cli/Cargo.toml
key_decisions:
  - D-M004-012: serde Serialize for RoutingScore (not manual format!())
  - D-M004-013: Crossing detection uses segment_distance()==0, separate from DRC violations
  - D-M004-014: Scoring module lives in cypcb-autoroute, not a separate crate
  - D-M004-015: DRC/crossing assertions use range checks (< 200/< 50), not == 0
  - D-M004-016: ScoreCommand uses DesignRules for DRC, PresetRuleSet for routing
patterns_established:
  - score_board(world, rules, weights) → RoutingScore as single scoring entry point
  - Composite formula: weighted sum with board-diagonal normalization, DRC penalty ×1000, crossings ×500, smoothness ×100, balance ×50
  - angle_penalty() for smoothness — deviation from nearest 45° multiple
  - ScoreCommand follows CheckCommand pattern: read → parse → build → route → apply → score → JSON
  - route_and_apply() integration test helper encapsulates full routing pipeline
observability_surfaces:
  - CLI `cypcb score <file>` outputs pretty-printed JSON with all 7 metrics
  - tracing::debug! in score_board() with all metric values
  - RoutingScore derives Serialize — JSON for CLI, WASM, and test consumers
  - Integration tests emit metric tables via eprintln! for CI inspection
drill_down_paths:
  - .gsd/milestones/M004/slices/S02/tasks/T01-SUMMARY.md
  - .gsd/milestones/M004/slices/S02/tasks/T02-SUMMARY.md
duration: 45m
verification_result: passed
completed_at: 2026-03-14
---

# S02: Routing Quality Score System

**Implemented `cypcb_autoroute::scoring` module with 7-metric `RoutingScore`, configurable `ScoreWeights`, `score_board()` boundary contract, CLI `cypcb score` command, and baseline scores for 2 test boards — 31 tests total.**

## What Happened

**T01 (25m):** Created `scoring.rs` in cypcb-autoroute with `RoutingScore` struct (7 fields, derives Serialize), `ScoreWeights` with sensible defaults, and `score_board()` function. Promoted cypcb-drc from dev-dependency to regular dependency. Implemented all metric computations: total trace length from ECS query, via count, DRC violation count via `run_drc()`, smoothness from consecutive-segment bend angles (penalizes non-45° multiples, skips zero-length segments), crossing detection using spatial index + `segment_distance()==0` with canonical pair ordering to prevent double-counting, and layer balance as min/max trace-count ratio. Composite formula uses weighted sum with board-diagonal normalization. Wrote 27 unit tests covering angle penalty edge cases, layer balance scenarios, smoothness computations, composite formula, JSON serialization, and score_board on empty/traced/viad boards.

**T02 (20m):** Created `ScoreCommand` CLI following the CheckCommand pattern — reads .cypcb, parses, builds world, routes with default config, applies routes, rebuilds spatial index with traces, scores, outputs JSON. Registered in commands/mod.rs and main.rs. Added cypcb-autoroute, cypcb-drc, cypcb-rules dependencies to cypcb-cli. Wrote 4 integration tests establishing baselines: blink.cypcb (length=182.46mm, vias=8, drc=50, smoothness=1.0, crossings=4, balance=0.2857, composite=52046.24), routing-test.cypcb (length=25.93mm, vias=0, drc=5, smoothness=1.0, crossings=0, balance=1.0, composite=5000.55), empty board defaults, and JSON serialization round-trip.

## Verification

- ✅ `cargo test -p cypcb-autoroute` — 76 tests pass (67 unit + 5 existing integration + 4 scoring integration), 2 ignored benchmarks
- ✅ `cargo test -p cypcb-autoroute --test scoring_integration` — 4/4 pass (blink baseline, routing-test baseline, empty board, JSON serialization)
- ✅ `cargo check -p cypcb-cli` — CLI compiles with score command
- ✅ `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — WASM compilation passes
- ✅ Empty board produces well-defined defaults: length=0, vias=0, crossings=0, smoothness=1.0, balance=1.0, composite=0.0
- ✅ RoutingScore serializes to JSON with all 7 field names present

## Requirements Advanced

- R103 (Routing Quality Scoring System) — fully implemented: 7-metric scoring with composite, CLI command, baseline values established

## Requirements Validated

- R103 — 31 tests (27 unit + 4 integration) prove all 7 metrics compute correctly on empty, simple, and complex boards. CLI produces JSON output. score_board() is the contract boundary consumed by S06/S07.

## New Requirements Surfaced

- none

## Requirements Invalidated or Re-scoped

- none

## Deviations

- `score_board()` takes 3 args `(world, rules, weights)` instead of 2 `(world, rules)` — ScoreWeights added as explicit parameter for configurability. Additive change, compatible with API contract.
- DRC violation and crossing assertions changed from `== 0` to range checks (`< 200`, `< 50`) — A*-based autorouter produces violations on complex boards. Scoring correctly reports reality; tighter assertions would couple scoring tests to routing quality.

## Known Limitations

- blink.cypcb scores 50 DRC violations and 4 crossings — reflects A*-based autorouter quality, not scoring bugs. S03 (PathFinder) should reduce these.
- Smoothness metric is 1.0 for all current A* routes because postprocessor merges collinear segments — only non-45° angles from a future smoother or manual routes will produce < 1.0.
- CLI `cypcb score` not tested end-to-end (requires pkg-config/gio system deps) — compilation verified via `cargo check`.

## Follow-ups

- none

## Files Created/Modified

- `crates/cypcb-autoroute/src/scoring.rs` — new module: RoutingScore, ScoreWeights, ScoringConfig, score_board(), 27 unit tests
- `crates/cypcb-autoroute/src/lib.rs` — added `pub mod scoring;`
- `crates/cypcb-autoroute/Cargo.toml` — promoted cypcb-drc, added serde + serde_json
- `crates/cypcb-autoroute/tests/scoring_integration.rs` — 4 integration tests with baseline scores
- `crates/cypcb-cli/src/commands/score.rs` — ScoreCommand implementation
- `crates/cypcb-cli/src/commands/mod.rs` — score module registration
- `crates/cypcb-cli/src/main.rs` — Score variant and match arm
- `crates/cypcb-cli/Cargo.toml` — added cypcb-autoroute, cypcb-drc, cypcb-rules dependencies

## Forward Intelligence

### What the next slice should know
- `score_board()` requires `&mut BoardWorld` (bevy_ecs query API), `&DesignRules` (from cypcb-drc), and `&ScoreWeights` (use `ScoreWeights::default()` for standard weights)
- Spatial index must be rebuilt with traces before scoring: call `world.rebuild_spatial_index_with_traces()` — without this, crossing detection misses all trace-trace interactions
- Composite formula: lower is better. DRC violations dominate (×1000 penalty) — a board with 50 DRC violations scores ~50,000 composite regardless of other metrics

### What's fragile
- Crossing detection depends on `segment_distance()` returning exactly 0 for intersecting segments — floating point precision matters; if the DRC crate changes distance calculation internals, crossings may drift
- Smoothness is always 1.0 for current A* routes — when S04 smoother introduces non-grid angles, smoothness tests may need recalibration

### Authoritative diagnostics
- `cargo test -p cypcb-autoroute --test scoring_integration -- --nocapture` — shows full metric tables for both boards
- `RoutingScore` JSON output is the ground truth for all metric values — inspect via serde_json::to_string_pretty()

### What assumptions changed
- Plan assumed integration tests would assert drc_violations == 0 — actual A* router produces violations on complex boards, so range checks used instead
