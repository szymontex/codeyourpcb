# S07: Benchmark Validation & Strategy Selection — Research

**Date:** 2026-03-14

## Summary

S07 is the terminal slice of M004 — it consumes all previous slices (S01–S06) to produce an automated benchmark suite, default strategy selection based on empirical data, visual comparison artifacts, and a regression test gate. The infrastructure is 90% built: `generate_variants()` already runs all strategies × multiple configs on any board, `score_board()` produces the 7-metric `RoutingScore`, `get_benchmarks()` provides fixture iteration, and `strategy_comparison.rs` has the comparison table pattern. What's missing is *orchestrating* these into a single automated pipeline that iterates all fixtures × all strategies, emits a structured comparison report (JSON + table), captures screenshot artifacts via Playwright, codifies the winner as default, and adds a regression gate test.

The main risks are: (1) test runtime — routing led_blink takes ~22s per strategy in release, stm32_breakout/multi_ic take minutes; the full benchmark suite across all fixtures × all strategies will take several minutes; (2) screenshot generation requires a running Vite dev server + Playwright, which is an E2E concern not a Rust test concern — the "screenshot artifacts" deliverable should be a Playwright E2E test, not a Rust integration test; (3) the 5 remaining DRC violations on led_blink (from S03) mean "zero DRC violations on all benchmark boards" from the milestone DoD may not be achievable without revisiting the router itself — S07 should document the gap honestly rather than hiding it.

## Recommendation

**Three-deliverable approach:**

1. **Rust benchmark suite** (`crates/cypcb-autoroute/tests/benchmark_validation.rs`) — A single `#[test]` function iterating all 3 fixtures × 2 strategies, collecting scores into a comparison table, asserting regression thresholds, and selecting the default strategy. Mark as `#[ignore]` for CI (runtime) with a dedicated quality-gate entry. Produces JSON report to stderr.

2. **Regression gate test** (`benchmark_regression` in same file) — A focused non-ignored test that routes led_blink with PathFinder only and asserts score thresholds (composite ≤ known_baseline × 1.1, DRC ≤ 5, smoothness ≥ 0.95). This runs in CI via `cargo test benchmark_regression`.

3. **Playwright screenshot E2E** (`viewer/e2e/benchmark-screenshots.spec.ts`) — Loads each benchmark fixture via `__loadBoard()`, triggers routing, captures canvas screenshots to `test-results/benchmark/`. This is the R115 deliverable.

Skip a CLI `benchmark` subcommand — the existing test infrastructure is cleaner for this purpose than a CLI tool that would need pkg-config/gio deps (excluded from CI per DECISIONS).

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| Iterating benchmark fixtures | `get_benchmarks()` from `cypcb-kicad::pcb_parser` | Returns `Vec<(KicadBenchmark, PathBuf)>` with all 3 fixtures, absolute paths |
| Routing with any strategy | `route_board()` with `AutorouteConfig { strategy: StrategyKind::X }` | Strategy dispatch already works, includes smoother integration |
| Scoring routed boards | `score_board()` from `cypcb-autoroute::scoring` | 7-metric `RoutingScore` with composite, JSON serializable |
| Generating multiple variants | `generate_variants()` from `cypcb-autoroute::variant` | Routes 4 configs sequentially, ranks by composite, auto-applies best |
| Comparison table formatting | `compare_fixture()` pattern in `strategy_comparison.rs` | Unicode box-drawing table format, strategy-vs-strategy comparison |
| E2E board loading | `window.__loadBoard(source)` | Established pattern — loads board, syncs viewport, renders |
| E2E screenshot capture | `page.screenshot({ path })` in Playwright | Used in `app-load.spec.ts` already |

## Existing Code and Patterns

- `crates/cypcb-autoroute/tests/strategy_comparison.rs` — **Primary reuse target.** `compare_fixture()` parses fresh, routes with both strategies, scores, prints table, asserts DRC baseline. S07 benchmark_validation.rs should follow this exact pattern but iterate all fixtures and produce aggregate results.
- `crates/cypcb-autoroute/tests/variant_generation.rs` — Shows how to call `generate_variants()` on led_blink. S07 could use this for the "all variants × all fixtures" matrix, but `route_board()` per-strategy is simpler and avoids the sequential clear→route→score loop overhead.
- `crates/cypcb-autoroute/tests/smoother_integration.rs` — `route_and_score()` helper pattern (parse → route → apply → rebuild spatial index → score). Clean, reusable pattern for S07.
- `crates/cypcb-kicad/src/pcb_parser.rs` lines 141–178 — `BENCHMARKS` const array and `get_benchmarks()`. Note: `get_benchmarks()` returns relative paths (`tests/fixtures/benchmark/X.kicad_pcb`). Integration tests resolve via `CARGO_MANIFEST_DIR/../../tests/fixtures/benchmark/`.
- `crates/cypcb-autoroute/src/variant.rs` — `default_variant_configs()` returns 4 configs. For S07, we want per-strategy comparison, so we should test individual strategies (PathFinder vs ImprovedAStar) directly, not variants.
- `scripts/quality-gate.sh` — Stage 7 already runs an `--ignored` benchmark test. S07 should add its regression test as a non-ignored test AND add the full benchmark suite as an `--ignored` test that the quality gate can invoke.
- `viewer/e2e/variant-panel.spec.ts` — Shows how to load a board via `__loadBoard()`, trigger routing, and inspect results. S07 E2E screenshot test follows this pattern but captures canvas screenshots.
- `crates/cypcb-autoroute/src/strategy.rs` — `StrategyKind` enum has `PathFinder` and `ImprovedAStar`. The "default strategy selection" deliverable means confirming `StrategyKind::default()` returns `PathFinder` (it already does) and documenting the empirical basis.

## Constraints

- **Test runtime:** led_blink routes in ~22s per strategy in `--release` mode. stm32_breakout and multi_ic are `#[ignore]` because they take 60s+ each. Full benchmark (3 fixtures × 2 strategies = 6 routing runs) will take ~5+ minutes in release. Must be `#[ignore]` for regular CI.
- **WASM timing unavailable:** `std::time::Instant` panics in WASM (D-M004-034). All benchmarking must run natively, not in WASM.
- **Desktop crates excluded from quality gates:** `cypcb-cli` and `cypcb-desktop` excluded from `cargo test` in CI due to missing pkg-config/gio-2.0 (DECISIONS). No CLI benchmark command for CI — use integration tests instead.
- **BoardWorld not Clone:** `bevy_ecs::World` doesn't implement Clone. Each fixture × strategy combination needs a fresh `parse_kicad_pcb()` call. Can't share a single parsed world across strategies.
- **DRC violations not yet zero:** PathFinder produces 5 DRC violations on led_blink (grid artifacts from S03). The milestone DoD says "zero DRC violations on all benchmark boards" — this is a known gap. S07 should document the gap and assert the current threshold (≤5) as the regression baseline.
- **Benchmark fixtures are synthetic:** D-M004-010 documents that fixtures are hand-crafted, not real KiCad projects. Strategy selection is based on these synthetic fixtures.
- **Score determinism:** Routing should be deterministic (same input → same score) per technical constraints. However, floating-point and HashMap iteration order may cause minor variations. Regression thresholds should have ±10% margin.

## Common Pitfalls

- **Forgetting `rebuild_spatial_index_with_traces()` before scoring** — Without this, crossing detection misses all trace-trace interactions, producing falsely low crossing counts. Every route→score sequence MUST call this (S02 forward intelligence explicitly warns about this).
- **Using `DesignRules::default()` instead of `DesignRules::jlcpcb_2layer()`** — Default rules have different clearances; scores won't match baselines. All S07 tests should use `jlcpcb_2layer()` consistently (matching S02/S03 patterns).
- **Running full benchmark in CI without `#[ignore]`** — The quality gate runs `cargo test --workspace` (non-ignored only). If the full benchmark matrix is non-ignored, it'll add 5+ minutes to every CI run. Keep the full matrix as `#[ignore]`; only the regression gate (led_blink-only PathFinder) should be non-ignored.
- **Screenshot tests depending on pixel-perfect rendering** — Existing DECISIONS note "headless WebGL rendering varies" and E2E tests use diagnostic surfaces, not pixel comparison. S07 screenshots should be captured as artifacts for human inspection, not pixel-diffed.
- **Tight regression thresholds** — If regression thresholds are too tight (e.g., composite must be exactly X), normal floating-point variation across platforms will cause flaky tests. Use ≤ (baseline × 1.1) or ≤ (baseline + margin).

## Open Risks

- **stm32_breakout and multi_ic routing time** — These fixtures may take too long for even the `#[ignore]` benchmark suite to complete in a reasonable time. May need to limit the full matrix to led_blink only, with larger fixtures as optional manual-only runs.
- **DRC zero-violation target** — The remaining 5 DRC violations on led_blink are grid-level artifacts from S03. Without changes to the core router (out of S07 scope), these cannot be eliminated. Milestone DoD may need to accept "≤5 violations" rather than "zero."
- **Playwright screenshot tests may be flaky in CI** — WASM loading, canvas rendering, and screenshot timing can be non-deterministic. Consider making screenshot capture best-effort (not assertion-gated).
- **Strategy selection may vary by board complexity** — PathFinder wins 3× on led_blink (composite 5001 vs 15544) but has not been proven on stm32_breakout/multi_ic. The "default strategy" decision is based primarily on led_blink evidence.

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Rust (performance benchmarks) | `terraphim/terraphim-skills@rust-performance` | available (19 installs — low, not recommended) |
| Playwright E2E | `bobmatnyc/claude-mpm-skills@playwright-e2e-testing` | available (1.2K installs) |
| Rust async patterns | already installed | installed |

No skills are directly relevant enough to warrant installation for S07's scope. The work is primarily orchestrating existing code patterns into a benchmark pipeline — domain-specific, not framework-specific.

## Sources

- Strategy comparison patterns and score baselines from `crates/cypcb-autoroute/tests/strategy_comparison.rs` (source: codebase)
- Benchmark fixture metadata from `crates/cypcb-kicad/src/pcb_parser.rs` lines 141-178 (source: codebase)
- Quality gate stage 7 pattern from `scripts/quality-gate.sh` (source: codebase)
- DRC violation baseline (5 on led_blink) from S03-SUMMARY.md forward intelligence (source: slice summaries)
- WASM timing limitation from D-M004-034 (source: DECISIONS.md)
- Playwright screenshot pattern from `viewer/e2e/app-load.spec.ts` line 51 (source: codebase)
- Variant generation sequential constraint from D-M004-033 (source: DECISIONS.md)
