# S02: Routing Quality Score System

**Goal:** Any routed board gets a composite quality score covering 7 metrics (trace length, via count, DRC violations, smoothness, crossings, layer balance, composite). The `score_board()` function is the contract consumed by S06 (variant ranking) and S07 (benchmarks).
**Demo:** `cargo test -p cypcb-autoroute --test scoring_integration` passes — routes blink.cypcb, scores it, all 7 metrics produce validated baseline values. CLI `cypcb score <file>` prints score breakdown as JSON.

## Must-Haves

- `RoutingScore` struct with all 7 metrics: total_length (Nm), via_count (u32), drc_violations (u32), smoothness (f64 0–1), crossings (u32), layer_balance (f64 0–1), composite (f64)
- `ScoreWeights` struct with configurable weights for composite calculation, sensible defaults
- `score_board(world: &mut BoardWorld, rules: &DesignRules) -> RoutingScore` function in `cypcb-autoroute::scoring`
- Smoothness metric: penalizes bend angles that aren't multiples of 45°, skips zero-length segments
- Crossing detection: counts same-layer inter-net segment intersections, uses spatial index for boards with many segments, filters same-net junctions
- Layer balance: min/max ratio, single-layer boards score 1.0
- Composite formula: weighted sum, lower = better, normalizes trace length by board diagonal
- `RoutingScore` implements `Serialize` for JSON output
- CLI `score` command reads .cypcb file, routes, scores, prints breakdown
- `cypcb-drc` promoted from dev-dependency to regular dependency in `cypcb-autoroute`
- Integration tests establish baseline scores for blink.cypcb

## Proof Level

- This slice proves: contract (scoring boundary contract for S06/S07)
- Real runtime required: no (unit + integration tests sufficient)
- Human/UAT required: no

## Verification

- `cargo test -p cypcb-autoroute` — all scoring unit tests pass (angle calculations, layer balance edge cases, crossing detection, composite formula)
- `cargo test -p cypcb-autoroute --test scoring_integration` — routes blink.cypcb and routing-test.cypcb, scores them, asserts all 7 metrics are within expected ranges
- `cargo build -p cypcb-cli` — CLI compiles with score command (full CLI test skipped due to pkg-config constraint)

## Integration Closure

- Upstream surfaces consumed: `cypcb_drc::run_drc()`, `cypcb_world::components::trace::{Trace, Via}`, `cypcb_world::BoardWorld::rebuild_spatial_index_with_traces()`, `cypcb_drc::rules::clearance::segment_distance()`
- New wiring introduced in this slice: `cypcb-autoroute::scoring` module, CLI `Score` subcommand
- What remains before the milestone is truly usable end-to-end: S03 (routing engine), S06 (WASM `score()` binding for variant ranking), S07 (benchmark runner consuming scores)

## Observability / Diagnostics

- **Structured JSON output**: `RoutingScore` derives `Serialize` — all 7 metrics are inspectable via CLI `cypcb score <file>` or WASM `score()` binding. Each metric is individually readable, not just the composite.
- **Composite score breakdown**: Weights are transparent in `ScoreWeights` struct — agents and users can see how each metric contributes.
- **DRC violation count**: Surfaces `run_drc()` result count as a first-class metric — any board with violations has non-zero `drc_violations`, immediately visible.
- **Failure visibility**: `score_board()` logs via `tracing` when spatial index is rebuilt, when DRC runs, and metric computation times. Zero traces/vias → metrics gracefully return 0/1.0 defaults, not panics.
- **Redaction**: No secrets involved — all score data is diagnostic and safe to log/serialize.

## Verification

- `cargo test -p cypcb-autoroute` — all scoring unit tests pass (angle calculations, layer balance edge cases, crossing detection, composite formula)
- `cargo test -p cypcb-autoroute --test scoring_integration` — routes blink.cypcb and routing-test.cypcb, scores them, asserts all 7 metrics are within expected ranges
- `cargo build -p cypcb-cli` — CLI compiles with score command (full CLI test skipped due to pkg-config constraint)
- **Failure-path check**: Unit test verifies `score_board()` on empty board returns zero-length, zero vias, zero crossings, smoothness=1.0, layer_balance=1.0, composite=0.0 — confirming graceful empty-board handling
- **Diagnostic output check**: Integration test `score_json_serialization` verifies `RoutingScore` serializes to JSON containing all 7 field names, confirming structured output is inspectable by downstream consumers and agents

## Tasks

- [x] **T01: Implement scoring module with all 7 metrics and unit tests** `est:2h`
  - Why: Core algorithmic work — defines RoutingScore struct, ScoreWeights, and score_board() function. This is the boundary contract S06/S07 consume.
  - Files: `crates/cypcb-autoroute/Cargo.toml`, `crates/cypcb-autoroute/src/scoring.rs`, `crates/cypcb-autoroute/src/lib.rs`
  - Do: Promote cypcb-drc to regular dep. Add serde with derive feature. Create scoring.rs with RoutingScore (Serialize), ScoreWeights (defaults), score_board(). Implement: total_length from Trace ECS query, via_count from Via query, drc_violations via run_drc(), smoothness from consecutive-segment bend angles (penalty for non-45° multiples), crossings via spatial-index-accelerated inter-net same-layer intersection counting using segment_distance()==0, layer_balance as min/max trace-count ratio. Composite formula with board-diagonal normalization. Unit tests for each metric computation.
  - Verify: `cargo test -p cypcb-autoroute` — all scoring unit tests pass
  - Done when: `score_board()` compiles, all unit tests pass, RoutingScore serializes to JSON

- [x] **T02: CLI score command and integration tests with baseline scores** `est:1h`
  - Why: Proves scoring works end-to-end on real boards and establishes the baseline scores that S07 benchmarks will compare against. CLI command fulfills R103's "score any routed board" requirement.
  - Files: `crates/cypcb-cli/src/commands/score.rs`, `crates/cypcb-cli/src/commands/mod.rs`, `crates/cypcb-cli/src/main.rs`, `crates/cypcb-autoroute/tests/scoring_integration.rs`
  - Do: Add ScoreCommand following CheckCommand pattern (takes .cypcb file, parses, builds world, routes with default config, scores, prints JSON). Register in commands/mod.rs and main.rs. Write integration tests: route blink.cypcb → score → assert total_length > 0, via_count >= 0, drc_violations == 0, smoothness in [0,1], layer_balance in [0,1], composite > 0. Route routing-test.cypcb → score → verify. Test unrouted board → verify drc_violations or unrouted reflected in score.
  - Verify: `cargo test -p cypcb-autoroute --test scoring_integration` passes, `cargo build -p cypcb-cli` compiles
  - Done when: Integration tests pass with stable baseline scores, CLI builds with score command

## Files Likely Touched

- `crates/cypcb-autoroute/Cargo.toml`
- `crates/cypcb-autoroute/src/lib.rs`
- `crates/cypcb-autoroute/src/scoring.rs`
- `crates/cypcb-autoroute/tests/scoring_integration.rs`
- `crates/cypcb-cli/src/commands/score.rs`
- `crates/cypcb-cli/src/commands/mod.rs`
- `crates/cypcb-cli/src/main.rs`
