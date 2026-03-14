# S03: PathFinder Routing Engine

**Goal:** Replace the sequential A*-only autorouter with a multi-strategy routing engine where PathFinder negotiated congestion and improved A* compete on benchmark boards, with zero DRC violations as the target.
**Demo:** Both strategies route all 3 benchmark fixtures. `score_board()` compares them — PathFinder wins or we know why. DRC violations dramatically reduced from baseline (50 on blink → target 0). WASM `auto_route()` uses the best strategy transparently.

## Must-Haves

- `RoutingStrategy` trait with `name()` and `route()` methods, accepting `&mut BoardWorld`, `FootprintLibrary`, `&dyn RoutingRuleSet`, `&AutorouteConfig`
- `PathFinderStrategy` — negotiated congestion router with iterative rip-up/reroute, per-cell present+history cost tracking, convergence detection
- `ImprovedAStarStrategy` — wraps existing orchestrator with congestion-aware cost, better net ordering, increased rip-up iterations (10→20), multi-victim rip-up
- `CongestionMap` — separate struct for per-cell congestion cost tracking (not coupled to RoutingGrid)
- `AutorouteConfig` extended with strategy selection (defaults to PathFinder)
- `route_board()` dispatches to selected strategy
- WASM `auto_route()` uses best strategy without changing JSON return contract
- Strategy comparison tests on all 3 benchmark fixtures using `score_board()`
- Both strategies produce valid `RoutingResult` consumed by existing `apply_routes()` pipeline

## Proof Level

- This slice proves: contract + integration
- Real runtime required: yes (Rust tests run both strategies on benchmark fixtures with real grid construction)
- Human/UAT required: no (quantitative score comparison, not visual)

## Verification

- `cargo test -p cypcb-autoroute` — all existing tests pass (no regression) plus new strategy/congestion/pathfinder_v2 unit tests
- `cargo test -p cypcb-autoroute --test strategy_comparison` — both strategies route all 3 benchmark fixtures, scores compared, DRC violations measured
- `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — WASM compilation passes with new strategy code
- `cargo check -p cypcb-render` — auto_route() compiles with strategy update
- Diagnostic check: `RUST_LOG=cypcb_autoroute=info cargo test -p cypcb-autoroute -- --nocapture 2>&1 | grep -E 'strategy|Strategy|routing_strategy'` — verifies strategy name appears in tracing output when routing runs, confirming strategy dispatch and observability are wired

## Observability / Diagnostics

- Runtime signals: `tracing::info!` in PathFinder iteration loop (iteration count, overused cell count, nets re-routed per iteration, convergence status)
- Inspection surfaces: `RoutingStrategy::name()` in log output, score comparison in test stderr
- Failure visibility: PathFinder reports iteration cap hit vs convergence, unrouted net count, per-iteration congestion stats

## Integration Closure

- Upstream surfaces consumed: `score_board()` from S02 for strategy comparison, benchmark fixtures from S01 for validation, `RoutingGrid`/`RoutingCost`/`orchestrator`/`postprocess` from existing autoroute crate
- New wiring introduced: `RoutingStrategy` trait as the dispatch boundary, `AutorouteConfig.strategy` field, WASM `auto_route()` strategy selection
- What remains before milestone is truly usable end-to-end: S04 (trace smoothing), S05 (realtime tuning UI), S06 (variant UI), S07 (benchmark validation)

## Tasks

- [x] **T01: RoutingStrategy trait, ImprovedAStarStrategy, and route_board() dispatch** `est:35m`
  - Why: Establishes the multi-strategy abstraction and immediately delivers a better A* — if PathFinder fails to converge in T02, we still ship an improvement. Satisfies R104 structurally.
  - Files: `crates/cypcb-autoroute/src/strategy.rs`, `crates/cypcb-autoroute/src/astar_improved.rs`, `crates/cypcb-autoroute/src/lib.rs`, `crates/cypcb-autoroute/src/grid.rs`
  - Do: Define `RoutingStrategy` trait and `StrategyKind` enum. Implement `ImprovedAStarStrategy` wrapping existing orchestrator with congestion-aware cost via `net_map`, better net ordering, 20 rip-up iterations, multi-victim rip-up (3 victims). Add `strategy` field to `AutorouteConfig`. Update `route_board()` to dispatch via strategy. Make `make_test_grid` public for cross-module tests. Add unit tests.
  - Verify: `cargo test -p cypcb-autoroute` passes all existing + new tests, `cargo check --target wasm32-unknown-unknown -p cypcb-autoroute` passes
  - Done when: `route_board()` with `StrategyKind::ImprovedAStar` produces a valid `RoutingResult` on a test board, and the existing default behavior is preserved

- [x] **T02: PathFinder negotiated congestion router with CongestionMap** `est:45m`
  - Why: Core algorithmic work — implements R105 (negotiated congestion) and R106 (strategic via placement via congestion-driven layer transitions). This is the highest-risk task in the slice.
  - Files: `crates/cypcb-autoroute/src/congestion.rs`, `crates/cypcb-autoroute/src/pathfinder_v2.rs`, `crates/cypcb-autoroute/src/lib.rs`
  - Do: Implement `CongestionMap` with per-cell per-layer `present_cost` and `history_cost` vectors, overuse detection, and history accumulation. Implement `PathFinderStrategy` with iteration loop: route all nets with congestion-aware A* cost `(base * (1 + history) * (1 + alpha * present))`, update congestion after each iteration, re-route only nets through overused cells (VPR optimization), converge when zero overuse or iteration cap (50). Use existing `find_path_with_zones` as inner search kernel with augmented cost. Maintain per-net cell index for O(path_length) rip-up instead of O(grid_size) `clear_route()`. Keep heuristic admissible (base costs only, no congestion). Unit tests for CongestionMap operations and PathFinder convergence on test grids.
  - Verify: `cargo test -p cypcb-autoroute` — PathFinder converges on simple test grids with crossing nets, produces fewer congestion violations than sequential A*
  - Done when: `PathFinderStrategy` routes a multi-net test grid to convergence (zero overused cells), unit tests pass

- [x] **T03: WASM integration and benchmark strategy comparison** `est:30m`
  - Why: Proves the slice's demo claim — both strategies work on real benchmark fixtures, scores compared, DRC measured. Satisfies R107 (DRC target) and R116 (empirical comparison). Wires WASM for downstream slices.
  - Files: `crates/cypcb-render/src/lib.rs`, `crates/cypcb-autoroute/tests/strategy_comparison.rs`, `crates/cypcb-autoroute/src/lib.rs`
  - Do: Update WASM `auto_route()` to use `AutorouteConfig::default()` which now selects the best strategy. Create `strategy_comparison.rs` integration test that parses each benchmark fixture via `parse_kicad_pcb()`, routes with both strategies, scores with `score_board()`, asserts PathFinder composite ≤ ImprovedAStar composite (or documents why not), asserts DRC violations reduced from baseline. Print comparison table to stderr for CI inspection.
  - Verify: `cargo test -p cypcb-autoroute --test strategy_comparison -- --nocapture` shows score table, `cargo check -p cypcb-render` compiles
  - Done when: Strategy comparison test passes on all 3 benchmark fixtures, WASM compilation succeeds, score comparison table shows PathFinder improvement (or explains divergence)

## Files Likely Touched

- `crates/cypcb-autoroute/src/strategy.rs` — NEW: RoutingStrategy trait + StrategyKind enum
- `crates/cypcb-autoroute/src/congestion.rs` — NEW: CongestionMap for per-cell cost tracking
- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — NEW: PathFinder negotiated congestion implementation
- `crates/cypcb-autoroute/src/astar_improved.rs` — NEW: Improved A* strategy
- `crates/cypcb-autoroute/src/lib.rs` — Updated route_board() with strategy dispatch
- `crates/cypcb-autoroute/src/grid.rs` — make_test_grid public
- `crates/cypcb-autoroute/tests/strategy_comparison.rs` — NEW: benchmark comparison tests
- `crates/cypcb-render/src/lib.rs` — Updated auto_route() for strategy
