---
estimated_steps: 6
estimated_files: 2
---

# T01: Rust benchmark validation suite with regression gate

**Slice:** S07 — Benchmark Validation & Strategy Selection
**Milestone:** M004

## Description

Create `benchmark_validation.rs` integration test with two test functions: a fast `benchmark_regression` gate for CI and a comprehensive `benchmark_full_matrix` for full strategy comparison. The regression gate routes led_blink with PathFinder and asserts score thresholds. The full matrix routes all 3 fixtures × 2 strategies, produces a comparison table and JSON report, and confirms PathFinder as the empirically-selected default strategy. Update quality-gate.sh to include the regression test.

## Steps

1. Create `crates/cypcb-autoroute/tests/benchmark_validation.rs` with imports matching `strategy_comparison.rs` pattern (cypcb_autoroute, cypcb_kicad, cypcb_drc, cypcb_router, cypcb_rules, cypcb_world, cypcb_core). Add shared helpers: `fixture_path()`, `test_rules()`, `route_and_score()` returning `(RoutingScore, usize)` (score + route_count).

2. Implement `benchmark_regression` test (non-`#[ignore]`):
   - Route `led_blink.kicad_pcb` with `PathFinderStrategy`
   - Print score table to stderr (composite, DRC, smoothness, vias, length)
   - Assert: `composite <= 5501.0` (baseline 5001 × 1.1), `drc_violations <= 5`, `smoothness >= 0.95`, `route_count > 0`
   - Print pass/fail summary with actual values vs thresholds

3. Implement `BenchmarkResult` struct and `benchmark_full_matrix` test (`#[ignore]`):
   - Define `BenchmarkResult { fixture, strategy, score, route_count }` 
   - Iterate all 3 `BENCHMARKS` × 2 strategies (PathFinder, ImprovedAStar)
   - For each: parse fresh → route → apply_routes → rebuild_spatial_index_with_traces → score_board
   - Collect all results into `Vec<BenchmarkResult>`
   - Print aggregate comparison table (all rows with fixture × strategy scores)
   - Emit JSON report to stderr: `eprintln!("BENCHMARK_JSON: {}", serde_json::to_string(&results))`
   - Assert PathFinder composite ≤ ImprovedAStar composite on led_blink
   - Print strategy selection conclusion: "Default strategy: PathFinder (empirically validated)"

4. Add `BenchmarkResult` with `Serialize` derive for JSON output. Include fixture name, strategy name, composite, drc_violations, smoothness, via_count, total_length_mm, route_count.

5. Update `scripts/quality-gate.sh` stage 7: change from `benchmark_500 --ignored` to running both the existing ignored benchmark AND `benchmark_regression` (non-ignored). The non-ignored `benchmark_regression` already runs in stage 3 (`cargo test --workspace`), but stage 7 can run it explicitly in `--release` for the performance benefit.

6. Verify: `cargo test -p cypcb-autoroute benchmark_regression --release` passes, `cargo test -p cypcb-autoroute --release --ignored -- benchmark_full_matrix` passes.

## Must-Haves

- [ ] `benchmark_regression` test is non-ignored and asserts composite ≤ 5501, DRC ≤ 5, smoothness ≥ 0.95
- [ ] `benchmark_full_matrix` test is `#[ignore]` and iterates all 3 fixtures × 2 strategies
- [ ] Full matrix asserts PathFinder ≤ ImprovedAStar on led_blink composite
- [ ] Comparison table printed to stderr with Unicode box-drawing format
- [ ] JSON report emitted to stderr in full matrix test
- [ ] `rebuild_spatial_index_with_traces()` called before every `score_board()`
- [ ] `DesignRules::jlcpcb_2layer()` used for all DRC scoring
- [ ] Quality gate stage 7 updated

## Verification

- `cargo test -p cypcb-autoroute benchmark_regression --release` — passes with score table in output
- `cargo test -p cypcb-autoroute --release --ignored -- benchmark_full_matrix` — passes with full comparison matrix
- `grep 'benchmark_regression\|benchmark_full' scripts/quality-gate.sh` — shows stage 7 references

## Observability Impact

- Signals added: comparison table to stderr (human-readable), JSON report line prefixed with `BENCHMARK_JSON:` (machine-readable)
- How a future agent inspects: `cargo test benchmark_regression --release -- --nocapture` shows all scores
- Failure state exposed: assertion messages include "got X, threshold Y" with context

## Inputs

- `crates/cypcb-autoroute/tests/strategy_comparison.rs` — pattern for `fixture_path()`, `test_rules()`, `route_and_score()`, `compare_fixture()` helpers, and table formatting
- `crates/cypcb-autoroute/tests/smoother_integration.rs` — pattern for `rebuild_spatial_index_with_traces()` before scoring
- `crates/cypcb-kicad/src/pcb_parser.rs` — `BENCHMARKS` const, `get_benchmarks()`, `KicadBenchmark` for fixture iteration
- S03 forward intelligence: led_blink PathFinder baseline composite ~5001, DRC violations = 5
- S04 forward intelligence: smoothness = 1.000 on led_blink after smoothing

## Expected Output

- `crates/cypcb-autoroute/tests/benchmark_validation.rs` — NEW: ~200 LOC integration test with regression gate + full matrix
- `scripts/quality-gate.sh` — MODIFIED: stage 7 updated to include benchmark_regression
