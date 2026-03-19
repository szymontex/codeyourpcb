# S02: Routing Quality Score System — Research

**Date:** 2026-03-14

## Summary

The scoring system builds atop strong existing primitives. `RoutingMetrics` in `cypcb-router::types` already computes `total_length`, `via_count`, `layer_changes`, and `unrouted_nets` with a rudimentary `quality_score()` (length_mm + via_count×5 + unrouted×1000). The DRC engine (`cypcb-drc::run_drc`) already counts violations with segment-to-segment distance math in `clearance.rs`. **What's missing**: trace smoothness (bend angle distribution), crossing count, layer balance, and a proper composite score struct with configurable weights.

The boundary map specifies `cypcb-autoroute::scoring` module exposing `RoutingScore` and `score_board(world: &BoardWorld) -> RoutingScore`. Scoring from `BoardWorld` (post-`apply_routes()`) is the right level because: (a) DRC needs a world, (b) it can score any board including imported KiCad ones, and (c) future slices (S06 variant ranking, S07 benchmarks) need to score arbitrary world states.

The main technical concern is the crossing detection algorithm being O(n²) on segment count. For the benchmark fixtures (94 nets max, ~100-200 segments), this is fine. For the 500-component synthetic bench, a spatial pre-filter using the existing R*-tree (`rstar`) will keep it tractable. The smoothness metric requires a precise definition: we should compute bend angles between consecutive segments of the same trace and penalize deviations from 45° multiples, matching the PCB industry convention (R108).

## Recommendation

Create a new `scoring` module in `cypcb-autoroute` with:

1. **`RoutingScore` struct** — all 7 metrics individually + weighted composite. Implement `Serialize`/`Deserialize` for JSON output (CLI, WASM).
2. **`score_board(world: &mut BoardWorld, rules: &DesignRules) -> RoutingScore`** — queries Trace/Via ECS entities, runs DRC, computes all metrics.
3. **`ScoreWeights` struct** — configurable weights for composite score. Default weights start equal, tune in S07.
4. **CLI `score` command** — reads .cypcb file, builds world, routes, scores, prints breakdown.
5. **Tests against existing blink.cypcb routing** — route the board, score it, verify baseline values.

Do NOT create a separate crate — scoring is tightly coupled with the autorouter's output semantics and the boundary map places it in `cypcb-autoroute`.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| Segment-to-segment distance | `cypcb_drc::rules::clearance::segment_distance()` | Already handles all edge cases (parallel, crossing, endpoint), uses i128 overflow protection |
| DRC violation counting | `cypcb_drc::run_drc()` | 7 rule checkers, spatial index acceleration, same-net exemption — don't reimplement |
| Trace length computation | `RouteSegment::length()` and `Trace::total_length()` | i128 overflow-safe Euclidean distance |
| Basic metrics | `cypcb_router::types::calculate_metrics()` | Already computes total_length, via_count, layer_changes, unrouted_nets |
| Spatial queries for crossing detection | `cypcb_world::SpatialIndex` (R*-tree via `rstar`) | Already indexes traces with `rebuild_spatial_index_with_traces()` |

## Existing Code and Patterns

- `crates/cypcb-router/src/types.rs` — `RoutingMetrics` struct and `calculate_metrics()` are the seed. `quality_score()` is the crude ancestor of our composite score. Pattern: compute from `RoutingResult` or query ECS. Our `RoutingScore` supersedes this for quality evaluation but does NOT replace it (existing code uses `RoutingMetrics` for simple checks).
- `crates/cypcb-drc/src/lib.rs` — `run_drc(&mut BoardWorld, &DesignRules) -> DrcResult` — takes mutable world (ECS queries need it), returns violation count. Scoring will call this. Note: DRC requires spatial index rebuild after `apply_routes()`.
- `crates/cypcb-drc/src/rules/clearance.rs` — `segment_distance()` function (pub) computes exact min distance between two line segments. Key building block for crossing detection (crossing = distance 0 on same layer, different nets).
- `crates/cypcb-world/src/components/trace.rs` — `Trace` component has `segments: Vec<TraceSegment>`, `layer: Layer`, `net_id: NetId`, `width: Nm`. `Via` component has `position`, `drill`, `start_layer`, `end_layer`, `net_id`. Both are ECS components queryable via `bevy_ecs`.
- `crates/cypcb-world/src/world.rs` — `rebuild_spatial_index_with_traces()` indexes trace segments as AABBs expanded by half-width. Use this before crossing detection to avoid O(n²) full scan.
- `crates/cypcb-autoroute/tests/integration.rs` — Pattern for integration tests: `parse_board()` helper, `test_rules()` for JLCPCB preset, `route_board()` then validate. Follow this pattern for scoring tests.
- `crates/cypcb-cli/src/main.rs` — CLI uses `clap::Subcommand`. Add `Score(commands::ScoreCommand)` variant. Follow `CheckCommand` pattern (simplest existing command).
- `crates/cypcb-autoroute/Cargo.toml` — `cypcb-drc` is currently a **dev-dependency** only. Must promote to a regular dependency for scoring to use `run_drc()`.

## Constraints

- **`cypcb-drc` dependency upgrade**: Currently `cypcb-drc` is dev-only in `cypcb-autoroute/Cargo.toml`. Scoring needs `run_drc()` at runtime → must add `cypcb-drc` as a regular dependency. No dependency cycle exists (`drc` depends on `world`, `autoroute` already depends on `world`). However, `cypcb-autoroute` is compiled for WASM (via `cypcb-render`) — verify `cypcb-drc` compiles for `wasm32-unknown-unknown` target. Current evidence: `PcbEngine::auto_route()` in `cypcb-render` already calls `self.run_drc_internal()` which uses `cypcb-drc`, so DRC is already WASM-compatible.
- **Mutable world for DRC**: `run_drc()` takes `&mut BoardWorld` (ECS query API requirement). Scoring function must also take `&mut BoardWorld`. This matches existing patterns.
- **Spatial index rebuild**: After `apply_routes()`, traces are ECS entities but NOT in the spatial index unless `rebuild_spatial_index_with_traces()` is called. Scoring must ensure traces are indexed before crossing detection. The existing `routed_output_passes_drc` integration test already calls `rebuild_spatial_index()` — follow same pattern.
- **CLI dependency chain**: `cypcb-cli` depends on `cypcb-autoroute` and `cypcb-drc` already. Adding a `score` command should not introduce new deps. However, `cypcb-cli` is excluded from quality gates (needs pkg-config/gio-2.0). Score command tests should be in `cypcb-autoroute` integration tests, not CLI tests.
- **All dimensions in Nm (nanometers)**: Project convention — no raw floats for physical measurements. Score output can use f64 for display (mm conversion) but internal storage uses Nm/integer types.
- **serde needed on RoutingScore**: CLI and WASM need JSON serialization. `cypcb-autoroute` doesn't currently depend on serde. Must add `serde` with `derive` feature, or use manual JSON formatting (simpler, matches `auto_route()` pattern which uses `format!` for JSON).

## Common Pitfalls

- **Forgetting spatial index rebuild** — If traces are applied to BoardWorld but the spatial index isn't rebuilt, crossing detection via spatial queries will miss traces entirely. Always call `rebuild_spatial_index_with_traces()` before scoring.
- **O(n²) crossing detection on large boards** — Naively checking every segment pair is O(n²). For 200 segments this is 40,000 comparisons (fine). For 2000+ segments (500-component synthetic board), use the R*-tree spatial index to pre-filter candidates within clearance distance, similar to how `ClearanceRule` works.
- **Same-net "crossings" are not crossings** — Segments of the same net that overlap at junctions are intentional connections, not violations. Must filter by `net_id` — only count inter-net crossings as the metric.
- **Angle calculation edge cases** — `atan2(0, 0)` returns 0 on most platforms but is technically undefined. Zero-length segments (start == end) should be skipped in smoothness calculation.
- **Layer balance for single-layer boards** — A 1-layer board (or a board with all traces on one layer) should get a layer_balance score of 1.0, not a penalty. Balance metric should be `min(top_count, bottom_count) / max(top_count, bottom_count)` with 1.0 = perfectly balanced, 0.0 = all on one layer. A 1-layer board is "balanced" by definition.
- **Composite score weight sensitivity** — Equal weights may not produce meaningful rankings. Start with equal weights but expose `ScoreWeights` so S07 can tune empirically. Lower composite = better (R103).

## Open Risks

- **Crossing detection accuracy vs. DRC crossings** — DRC clearance violations and "crossings" are related but distinct. A clearance violation of 0nm between segments of different nets IS a crossing, but DRC also catches near-misses. The crossing count should be a separate metric (exact intersection count), not derived from DRC violation count.
- **Smoothness metric definition needs validation** — The PCB industry doesn't have a universal "smoothness score." Our definition (penalty for non-45° angles) is reasonable but may not capture all aesthetic issues (e.g., unnecessary detours, zigzag patterns). May need refinement in S07 based on visual comparison.
- **Score stability across routing runs** — If the autorouter is non-deterministic (net ordering ties, floating-point variations), scores may vary between runs. The existing `order_nets()` sorts by Manhattan span with power-net tiebreaking, which should be deterministic. Verify in tests.
- **WASM `score_board()` exposure** — S06 needs scoring from the viewer (variant ranking). `PcbEngine` in `cypcb-render` will need a `score()` method. This slice should design `RoutingScore` with WASM-friendly serialization but does NOT need to add the WASM binding (that's S06's scope per boundary map).

## Metric Definitions (Proposed)

| Metric | Type | Formula | Lower = Better? |
|--------|------|---------|-----------------|
| `total_length` | `Nm` (i64) | Sum of all trace segment lengths | Yes |
| `via_count` | `u32` | Count of Via entities in world | Yes |
| `drc_violations` | `u32` | `run_drc().violation_count()` | Yes (0 = target) |
| `smoothness` | `f64` (0.0–1.0) | 1.0 - (penalty for non-45° angles) / (total_bends) | No (1.0 = perfect) |
| `crossings` | `u32` | Count of same-layer inter-net segment intersections | Yes (0 = target) |
| `layer_balance` | `f64` (0.0–1.0) | min(layer_counts) / max(layer_counts) | No (1.0 = perfect) |
| `composite` | `f64` | Weighted sum (lower = better) | Yes |

Composite formula (proposed):
```
composite = w_length * (total_length_mm / reference_length) 
          + w_via * via_count 
          + w_drc * drc_violations * 1000  
          + w_smoothness * (1.0 - smoothness) * 100
          + w_crossings * crossings * 500
          + w_balance * (1.0 - layer_balance) * 50
```
Where `reference_length` = board diagonal (normalizes across board sizes).

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Rust (core language) | coding-guidelines | installed (available in system prompt) |
| Rust async | rust-async-patterns | installed (not needed — scoring is synchronous) |
| PCB/EDA tools | — | none found (domain-specific, no generic skill applies) |

No skills need to be installed for this slice. The work is pure Rust algorithmic implementation within existing crate boundaries.

## Sources

- Existing `RoutingMetrics` and `quality_score()` (source: `crates/cypcb-router/src/types.rs:209-265`)
- DRC engine architecture (source: `crates/cypcb-drc/src/lib.rs`)
- Segment distance algorithm (source: `crates/cypcb-drc/src/rules/clearance.rs:segment_distance()`)
- Trace/Via ECS components (source: `crates/cypcb-world/src/components/trace.rs`)
- Autorouter integration tests pattern (source: `crates/cypcb-autoroute/tests/integration.rs`)
- CLI command pattern (source: `crates/cypcb-cli/src/main.rs`, `crates/cypcb-cli/src/commands/check.rs`)
- Benchmark fixtures metadata (source: `crates/cypcb-kicad/src/pcb_parser.rs:123-178`)
- Boundary map S02 contract (source: `.gsd/milestones/M004/M004-ROADMAP.md`)

## Requirements Coverage

This slice owns:
- **R103 — Routing Quality Scoring System** (primary owner): Score any routed board on total trace length, via count, DRC violations, smoothness, crossings, layer balance. Single composite number + breakdown.

This slice supports (consumed by downstream):
- **R112 — Routing Variant Generation** (S06): `score_board()` used to rank variants
- **R114 — Benchmark Validation** (S07): Scoring produces the quantitative comparison data
- **R116 — Empirical Strategy Selection** (S07): Composite scores determine which strategy wins

Key R103 clause: "Score must be a single composite number (weighted sum) plus individual metric breakdown. Lower = better." — all proposed metrics and composite formula satisfy this.
