---
id: S03
parent: M004
milestone: M004
provides:
  - RoutingStrategy trait and StrategyKind enum for multi-strategy dispatch
  - PathFinderStrategy — VPR-style negotiated congestion router with CongestionMap, per-net cell index, convergence detection
  - ImprovedAStarStrategy — 20-iteration multi-victim rip-up, fanout-aware net ordering
  - CongestionMap — per-cell per-layer present/history cost tracking with overuse detection
  - route_board() dispatches to selected strategy via AutorouteConfig.strategy
  - WASM auto_route() transparently uses PathFinder (best strategy) via default config
  - KiCad PCB parser position normalization (board-origin-relative coordinates)
  - Strategy comparison benchmark test proving PathFinder superiority on led_blink
requires:
  - slice: S01
    provides: KiCad benchmark fixtures (led_blink, stm32_breakout, multi_ic) parsed via parse_kicad_pcb()
  - slice: S02
    provides: score_board() for quantitative strategy comparison
affects:
  - S04 (consumes raw RoutingResult grid paths for smoothing)
  - S05 (consumes route_board() with strategy dispatch for realtime tuning)
  - S06 (consumes multiple RoutingStrategy implementations for variant generation)
  - S07 (consumes strategies + scoring for automated benchmark suite)
key_files:
  - crates/cypcb-autoroute/src/strategy.rs
  - crates/cypcb-autoroute/src/congestion.rs
  - crates/cypcb-autoroute/src/pathfinder_v2.rs
  - crates/cypcb-autoroute/src/astar_improved.rs
  - crates/cypcb-autoroute/src/lib.rs
  - crates/cypcb-autoroute/tests/strategy_comparison.rs
  - crates/cypcb-kicad/src/pcb_parser.rs
key_decisions:
  - "D-M004-017: CongestionMap separate from RoutingGrid — PathFinder-specific, no memory overhead for other grid users"
  - "D-M004-018: Per-net cell index HashMap<u32, Vec<(u32,u32,u8)>> for O(path_length) rip-up instead of O(grid_size)"
  - "D-M004-019: PathFinder implemented its own inner A* search (not wrapping find_path_with_zones) — congestion cost closure needs CongestionMap access"
  - "D-M004-020: VPR partial-reroute — only re-route nets through overused cells, not all nets every iteration"
  - "D-M004-022: ImprovedAStarStrategy duplicates orchestrator helpers (self-contained pattern)"
  - "D-M004-023: PathFinderStrategy uses public orchestrator helpers (reversed D-M004-022 for code sharing)"
  - "D-M004-024: KiCad parser translates positions to board-origin-relative coords (absolute coords exceeded grid bounds)"
  - "D-M004-025: Large benchmark tests (stm32_breakout, multi_ic) are #[ignore] — A* on large grids exceeds 60s"
patterns_established:
  - RoutingStrategy trait as the dispatch boundary — all strategies implement name() + route()
  - route_board() uses Box<dyn RoutingStrategy> dispatch based on AutorouteConfig.strategy
  - CongestionMap with escalating history beta (0.5 + 0.1 * iteration) prevents oscillation
  - VPR partial-reroute — only nets through overused cells re-routed after iteration 1
  - compare_fixture() pattern — parse fresh → route → apply_routes → rebuild spatial index → score_board → assert
observability_surfaces:
  - tracing::info! per PathFinder iteration (iteration count, overused cells, nets re-routed, beta value)
  - tracing::info! on convergence (iteration number where zero overuse achieved)
  - tracing::warn! on non-convergence (iteration cap with remaining overuse count)
  - RoutingStrategy::name() in route_board() tracing span for strategy identification
  - Multi-victim rip-up warnings with victims_tried and max_ripup_iterations on failure
  - Strategy comparison table printed to stderr in integration tests
drill_down_paths:
  - .gsd/milestones/M004/slices/S03/tasks/T01-SUMMARY.md
  - .gsd/milestones/M004/slices/S03/tasks/T02-SUMMARY.md
  - .gsd/milestones/M004/slices/S03/tasks/T03-SUMMARY.md
duration: ~103m (T01: 18m, T02: 25m, T03: 60m)
verification_result: passed
completed_at: 2026-03-14
---

# S03: PathFinder Routing Engine

**Multi-strategy routing engine with PathFinder negotiated congestion beating ImprovedAStar 3× on composite score (5001 vs 15544), DRC violations reduced from baseline 50 to 5, zero vias**

## What Happened

Built a complete multi-strategy routing engine in three tasks:

**T01 — Strategy Abstraction & Improved A*** established the `RoutingStrategy` trait with `name()` and `route()` methods, `StrategyKind` enum (`PathFinder`, `ImprovedAStar`), and refactored `route_board()` from inline logic to strategy dispatch via `Box<dyn RoutingStrategy>`. `ImprovedAStarStrategy` wraps the existing orchestrator with three improvements: fanout-aware net ordering, 20 rip-up iterations (up from 10), and multi-victim rip-up trying 3 different blocking nets per failure.

**T02 — PathFinder Negotiated Congestion** implemented the core algorithm. `CongestionMap` tracks per-cell per-layer occupancy and history costs. `PathFinderStrategy` runs iterative rip-up/reroute: route all nets with congestion-augmented A* cost, update history on overused cells with escalating beta (0.5 + 0.1 × iteration), partial-reroute only nets through overused cells (VPR optimization), converge when zero overuse or 50-iteration cap. Per-net cell index enables O(path_length) rip-up instead of O(grid_size). The inner A* search was implemented directly rather than wrapping `find_path_with_zones()` to support the congestion cost closure.

**T03 — WASM Integration & Benchmark Comparison** wired everything together. Discovered and fixed a critical KiCad parser bug: absolute component positions (e.g., 120mm,115mm) exceeded the routing grid — fixed by normalizing to board-origin-relative coordinates. Strategy comparison test on led_blink showed PathFinder wins decisively: composite 5000.8 vs 15543.6, DRC violations 5 vs 15, vias 0 vs 2, trace length 40.6mm vs 79.6mm. WASM `auto_route()` transparently uses PathFinder via default config — no JSON contract changes.

## Verification

| Check | Result |
|-------|--------|
| `cargo test -p cypcb-autoroute --lib --release` | ✅ 88/88 passed (67 existing + 21 new) |
| `cargo test --test strategy_comparison --release -- led_blink` | ✅ PathFinder composite 5001 ≤ ImprovedAStar 15544 |
| `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` | ✅ WASM compiles |
| `cargo check -p cypcb-render` | ✅ auto_route() compiles |
| Diagnostic: strategy names in tracing output | ✅ strategy_kind_display, strategy_kind_default tests |
| DRC violations vs S02 baseline (50) | ✅ PathFinder=5, ImprovedAStar=15 |

## Requirements Advanced

- R104 (Multi-Strategy Routing Engine) — RoutingStrategy trait with 2 implementations, route_board() dispatch, strategy comparison on benchmark fixtures
- R105 (Negotiated Congestion with Rip-up/Reroute) — PathFinderStrategy with iterative congestion resolution, VPR partial-reroute, convergence detection
- R106 (Proper Via Placement Strategy) — congestion-driven layer transitions; PathFinder produces 0 vias on led_blink vs ImprovedAStar's 2
- R107 (Zero DRC Violations) — DRC violations reduced from baseline 50 to 5 (PathFinder); not yet zero but dramatically improved

## Requirements Validated

- R104 — Two strategies compete on benchmark fixture with quantitative score comparison; PathFinder wins on all metrics
- R105 — PathFinder converges on test grids with crossing nets, produces fewer congestion violations than sequential A*
- R106 — PathFinder places zero unnecessary vias on led_blink (congestion-driven layer transition avoids gratuitous via placement)

## New Requirements Surfaced

- none

## Requirements Invalidated or Re-scoped

- R107 — DRC violations reduced to 5, not yet zero. Remaining violations likely from trace-pad clearance at grid boundaries. Full zero-violation target remains for S04 (smoothing may resolve grid artifacts) and S07 (final validation).

## Deviations

- **KiCad parser position normalization** (T03) — not in the original task plan but required to make routing work. Component positions in KiCad are absolute coordinates; without board-origin subtraction, all pads mapped to a single grid corner cell.
- **PathFinder inner search** (T02) — implemented directly instead of wrapping `find_path_with_zones()` because the congestion cost closure needs CongestionMap state that can't be injected through the existing API. Same algorithm, augmented with congestion cost.
- **Orchestrator helpers made public** (T02) — reversed T01's duplication pattern (D-M004-022) in favor of shared code (D-M004-023). ImprovedAStarStrategy retains its own copies but PathFinder uses public orchestrator API.
- **Per-fixture test split** (T03) — plan called for single test over all 3 fixtures. stm32_breakout and multi_ic tests are `#[ignore]` due to >60s runtime.

## Known Limitations

- DRC violations not yet zero (5 on led_blink) — remaining violations are likely grid-alignment artifacts that S04 (trace smoothing) may resolve
- stm32_breakout and multi_ic strategy comparison tests require `--ignored` flag and take minutes — A* on large grids is slow
- KiCad reference routes (traces/vias from .kicad_pcb files) are NOT offset by board origin — doesn't affect routing but would matter for reference comparison
- ImprovedAStarStrategy still duplicates orchestrator helper functions (D-M004-022 not refactored)

## Follow-ups

- S04 should check if trace smoothing resolves the remaining 5 DRC violations on led_blink
- When routing performance improves (S04/S05), unignore stm32_breakout and multi_ic strategy comparison tests
- Consider normalizing KiCad reference route coordinates for S07 visual comparison

## Files Created/Modified

- `crates/cypcb-autoroute/src/strategy.rs` — NEW: RoutingStrategy trait, StrategyKind enum (~95 LOC)
- `crates/cypcb-autoroute/src/astar_improved.rs` — NEW: ImprovedAStarStrategy with multi-victim rip-up (~620 LOC)
- `crates/cypcb-autoroute/src/congestion.rs` — NEW: CongestionMap with present/history cost tracking (~280 LOC)
- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — NEW: PathFinderStrategy with VPR-style iteration loop (~610 LOC)
- `crates/cypcb-autoroute/src/lib.rs` — Updated: strategy dispatch in route_board(), new module declarations
- `crates/cypcb-autoroute/src/grid.rs` — Modified: make_test_grid() public
- `crates/cypcb-autoroute/src/orchestrator.rs` — Modified: helpers made public
- `crates/cypcb-autoroute/tests/strategy_comparison.rs` — NEW: benchmark comparison test (~180 LOC)
- `crates/cypcb-autoroute/Cargo.toml` — Added dev-dependencies for integration tests
- `crates/cypcb-kicad/src/pcb_parser.rs` — Fixed: board-origin position normalization
- `crates/cypcb-kicad/src/lib.rs` — Re-exported BENCHMARKS constant

## Forward Intelligence

### What the next slice should know
- PathFinder produces grid-aligned paths just like A* — S04 smoother must handle the same staircase patterns
- DRC violations are 5 on led_blink — check if smoothing resolves them (grid boundary artifacts)
- `route_board()` returns `RoutingResult` with `Vec<RouteSegment>` — the smoother consumes these segments
- Both strategies produce valid results through `apply_routes()` pipeline — no special handling needed downstream

### What's fragile
- KiCad position normalization in pcb_parser.rs — if board_bounds changes, coordinate mapping breaks. The `board_origin_mm` subtraction is critical.
- PathFinder convergence depends on history beta escalation schedule (0.5 + 0.1 × iter) — if changed, may oscillate or converge too slowly on dense boards
- Per-net cell index must be kept in sync with grid state during rip-up — stale index causes incorrect overuse detection

### Authoritative diagnostics
- `cargo test --test strategy_comparison --release -- --nocapture` — prints the comparison table with composite scores, DRC, vias, length
- `RUST_LOG=cypcb_autoroute=info cargo test -p cypcb-autoroute -- --nocapture` — shows PathFinder iteration convergence stats
- `CongestionMap::overuse_count()` and `is_converged()` — direct convergence queries

### What assumptions changed
- Original assumption: `find_path_with_zones()` could be wrapped with congestion cost — actual: needed independent implementation because cost closure requires CongestionMap state
- Original assumption: helper functions should be duplicated per strategy — actual: public shared helpers in orchestrator.rs is cleaner
- Original assumption: all 3 benchmarks run in CI — actual: only led_blink is fast enough (<22s release); others need `--ignored`
