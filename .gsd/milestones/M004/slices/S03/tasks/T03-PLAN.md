---
estimated_steps: 4
estimated_files: 3
---

# T03: WASM integration and benchmark strategy comparison

**Slice:** S03 — PathFinder Routing Engine
**Milestone:** M004

## Description

Wire WASM `auto_route()` to use the best strategy, then prove both strategies work on all 3 benchmark fixtures from S01 with quantitative score comparison. This is the integration test that validates the slice's demo claim: PathFinder competes with (or beats) improved A* on real boards, DRC violations are reduced from the baseline, and the WASM entry point works transparently.

## Steps

1. Update `auto_route()` in `crates/cypcb-render/src/lib.rs` — `AutorouteConfig::default()` already selects the best strategy (PathFinder from T02). Verify the existing JSON return contract is unchanged. No other changes needed in the WASM layer since `route_board()` handles dispatch internally.

2. Create `crates/cypcb-autoroute/tests/strategy_comparison.rs`. For each of the 3 benchmark fixtures (led_blink, stm32_breakout, multi_ic): parse with `parse_kicad_pcb()`, route with `ImprovedAStarStrategy`, score with `score_board()`, then parse again fresh, route with `PathFinderStrategy`, score again. Print a comparison table to stderr (strategy, fixture, composite, drc_violations, via_count, total_length). Assert both strategies produce `RoutingResult` with at least some routed segments.

3. Add score comparison assertions: for each fixture, assert PathFinder composite score ≤ ImprovedAStar composite (or if PathFinder loses on a fixture, document why with `eprintln!` — acceptable for this slice if the explanation is congestion non-convergence on complex boards). Assert DRC violations for PathFinder < baseline (baseline is 50 for blink from S02). Use range assertions, not exact values.

4. Add WASM compilation check and run full test suite. Verify `cargo check -p cypcb-render` compiles (auto_route uses strategy), `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` passes, `cargo test -p cypcb-autoroute` runs all tests including strategy_comparison. Update `route_board()` default to use the empirically better strategy based on test results.

## Must-Haves

- [ ] WASM `auto_route()` uses strategy-aware `route_board()` without JSON contract change
- [ ] Strategy comparison test covers all 3 benchmark fixtures
- [ ] Both strategies produce valid routes on all fixtures
- [ ] Score comparison table printed for CI inspection
- [ ] DRC violations reduced from S02 baseline
- [ ] WASM compilation passes

## Verification

- `cargo test -p cypcb-autoroute --test strategy_comparison -- --nocapture` — comparison table visible, all assertions pass
- `cargo check -p cypcb-render` — WASM auto_route compiles
- `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — full crate WASM-compatible
- `cargo test -p cypcb-autoroute` — all tests pass (existing + new)

## Inputs

- `crates/cypcb-autoroute/src/strategy.rs` — RoutingStrategy trait, StrategyKind from T01
- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — PathFinderStrategy from T02
- `crates/cypcb-autoroute/src/astar_improved.rs` — ImprovedAStarStrategy from T01
- `crates/cypcb-autoroute/src/scoring.rs` — score_board(), RoutingScore, ScoreWeights from S02
- `crates/cypcb-kicad/src/pcb_parser.rs` — parse_kicad_pcb() from S01
- `tests/fixtures/benchmark/*.kicad_pcb` — benchmark fixtures from S01
- `crates/cypcb-render/src/lib.rs` — auto_route() WASM entry point

## Observability Impact

- **Strategy comparison table**: `strategy_comparison` test prints a human-readable table to stderr showing composite score, DRC violations, via count, and total length for each strategy × fixture — visible in CI via `--nocapture`.
- **WASM entry point**: `auto_route()` uses `route_board()` which emits `tracing::info!(strategy = ...)` on dispatch — confirms strategy selection in production.
- **Failure visibility**: If PathFinder loses on a fixture, the test emits `eprintln!` explaining why (e.g., congestion non-convergence). Non-convergence is already logged via `tracing::warn!` in PathFinderStrategy.
- **Inspection**: Future agents can run `cargo test -p cypcb-autoroute --test strategy_comparison -- --nocapture 2>&1` and parse the table to detect score regressions.

## Expected Output

- `crates/cypcb-autoroute/tests/strategy_comparison.rs` — benchmark comparison integration test (~200 LOC)
- `crates/cypcb-render/src/lib.rs` — verified auto_route() uses strategy (minimal change)
- `crates/cypcb-autoroute/src/lib.rs` — default strategy set to empirical winner
