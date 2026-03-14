---
estimated_steps: 6
estimated_files: 3
---

# T01: Implement scoring module with all 7 metrics and unit tests

**Slice:** S02 — Routing Quality Score System
**Milestone:** M004

## Description

Create the `scoring` module in `cypcb-autoroute` containing `RoutingScore`, `ScoreWeights`, and `score_board()`. This is the core boundary contract: S06 uses it for variant ranking, S07 for benchmark comparison. All 7 metrics must be individually computed and combined into a weighted composite score (lower = better, per R103).

## Steps

1. **Update Cargo.toml** — Promote `cypcb-drc` from dev-dependency to regular dependency. Add `serde = { version = "1", features = ["derive"] }`. Verify no dependency cycles exist (drc→world, autoroute→world — no cycle).

2. **Create `scoring.rs` with structs** — Define `RoutingScore` with fields: `total_length: Nm`, `via_count: u32`, `drc_violations: u32`, `smoothness: f64`, `crossings: u32`, `layer_balance: f64`, `composite: f64`. Derive `Serialize`. Define `ScoreWeights` with f64 fields for each metric, implement `Default` with equal weights. Define `ScoringConfig` holding weights.

3. **Implement `score_board()`** — Takes `&mut BoardWorld` and `&DesignRules`, returns `RoutingScore`. Steps inside:
   - Query all `Trace` entities via bevy_ecs → sum `total_length()` for total_length
   - Query all `Via` entities → count for via_count
   - Call `run_drc(world, rules)` → `violation_count()` for drc_violations
   - Call helper `compute_smoothness()` on collected traces → smoothness
   - Call helper `compute_crossings()` using spatial index → crossings
   - Call helper `compute_layer_balance()` from trace layer distribution → layer_balance
   - Call `compute_composite()` with all metrics + weights + board diagonal → composite

4. **Implement smoothness metric** — For each trace, iterate consecutive segment pairs, compute bend angle via `atan2`. Angle penalty = how far the angle deviates from nearest 45° multiple. Smoothness = 1.0 - (total_penalty / total_bends). Skip zero-length segments (start == end). Empty board or no bends = 1.0 (perfect).

5. **Implement crossing detection** — Ensure spatial index includes traces (`rebuild_spatial_index_with_traces()` must have been called). For each trace segment, query spatial index for nearby segments on same layer. For candidate pairs with different net_ids, use `segment_distance()` == 0 to detect intersection. Count unique crossing pairs (avoid double-counting via ordered entity ID comparison).

6. **Implement layer balance + composite + unit tests** — Layer balance: count traces per layer, `min(counts) / max(counts)`, 1.0 for single-layer. Composite: weighted sum with length normalized by board diagonal. Write unit tests for: angle penalty calculation (0°, 45°, 90°, 23° arbitrary), layer balance (1-layer, balanced, imbalanced), smoothness on known geometries, composite formula with known inputs, RoutingScore JSON serialization.

## Must-Haves

- [ ] `cypcb-drc` is a regular (not dev) dependency of `cypcb-autoroute`
- [ ] `RoutingScore` struct with all 7 fields, derives `Serialize`
- [ ] `ScoreWeights` struct with `Default` implementation
- [ ] `score_board(&mut BoardWorld, &DesignRules) -> RoutingScore` is public
- [ ] Smoothness handles zero-length segments and no-bend cases without panic
- [ ] Crossing detection filters same-net pairs (junctions are not crossings)
- [ ] Layer balance returns 1.0 for single-layer boards
- [ ] Composite score is lower = better
- [ ] All unit tests pass: `cargo test -p cypcb-autoroute`

## Verification

- `cargo test -p cypcb-autoroute` — all new unit tests in scoring.rs pass
- `cargo check -p cypcb-autoroute` — no compilation errors, cypcb-drc linked as regular dep
- `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — WASM compilation still works (if wasm target installed)

## Inputs

- `crates/cypcb-router/src/types.rs` — RoutingMetrics pattern, quality_score() as predecessor
- `crates/cypcb-drc/src/lib.rs` — run_drc() signature: `run_drc(world: &mut BoardWorld, rules: &DesignRules) -> DrcResult`
- `crates/cypcb-drc/src/rules/clearance.rs` — `segment_distance(p1, p2, p3, p4) -> i64` for crossing detection
- `crates/cypcb-world/src/components/trace.rs` — `Trace { segments, layer, net_id, width }`, `Via { position, net_id, ... }` ECS components
- `crates/cypcb-world/src/world.rs` — `rebuild_spatial_index_with_traces()` for spatial queries
- S02-RESEARCH.md — metric definitions table, composite formula, pitfall warnings

## Observability Impact

- **New signal**: `RoutingScore` JSON output with 7 individually-named fields. Any agent or human can inspect each metric independently.
- **Inspection surface**: `score_board()` is the single entry point. Returns a fully-populated struct; no partial results or silent failures.
- **Failure visibility**: Empty boards produce well-defined defaults (smoothness=1.0, layer_balance=1.0, composite=0.0). Zero-length segments are skipped without panic. These edge cases are tested.
- **Tracing**: `score_board()` emits `tracing::debug!` with metric values for pipeline observability.

## Expected Output

- `crates/cypcb-autoroute/Cargo.toml` — updated with cypcb-drc regular dep and serde
- `crates/cypcb-autoroute/src/scoring.rs` — new module with RoutingScore, ScoreWeights, score_board(), all metric helpers
- `crates/cypcb-autoroute/src/lib.rs` — `pub mod scoring;` added
