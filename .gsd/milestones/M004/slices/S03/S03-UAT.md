# S03: PathFinder Routing Engine — UAT

**Milestone:** M004
**Written:** 2026-03-14

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: S03 is a backend routing engine — all outputs are quantitative (scores, DRC counts, convergence) and verified via Rust tests. No UI surfaces to visually inspect. The slice plan explicitly states "Human/UAT required: no (quantitative score comparison, not visual)."

## Preconditions

- Rust toolchain installed with `wasm32-unknown-unknown` target
- Working directory is the project root (`/workspace/codeyourpcb`)
- KiCad benchmark fixtures exist in `tests/fixtures/benchmark/` (created by S01)
- Scoring module exists in `cypcb-autoroute::scoring` (created by S02)
- Release profile recommended for strategy comparison tests (debug builds are 5-10× slower)

## Smoke Test

Run `cargo test -p cypcb-autoroute --lib --release` — expect 88 tests passing, 0 failed. This confirms strategy trait, CongestionMap, PathFinder, ImprovedAStar, and all existing autoroute code compile and pass.

## Test Cases

### 1. RoutingStrategy trait dispatch works correctly

1. Run `cargo test -p cypcb-autoroute --lib --release -- strategy_kind`
2. **Expected:** 3 tests pass: `strategy_kind_default_is_pathfinder`, `strategy_kind_display`, `strategy_kind_equality`
3. Verify default strategy is `PathFinder` (not `ImprovedAStar`)
4. Verify display formats: `"pathfinder"`, `"improved-astar"`

### 2. ImprovedAStarStrategy routes with multi-victim rip-up

1. Run `cargo test -p cypcb-autoroute --lib --release -- astar_improved`
2. **Expected:** 5 tests pass including `multi_victim_ripup_finds_alternative`, `route_simple_grid_produces_valid_result`, `route_congested_grid_handles_conflicts`
3. Verify multi-victim rip-up tries up to 3 blocking nets per failed connection
4. Verify fanout-aware net ordering (lower fanout routes first among same-span nets)

### 3. CongestionMap tracks occupancy and history correctly

1. Run `cargo test -p cypcb-autoroute --lib --release -- congestion`
2. **Expected:** 8 tests pass: mark/unmark symmetry, congestion cost computation, history accumulation, multi-layer tracking, out-of-bounds safety, overuse detection
3. Verify `congestion_cost()` returns 0.0 when not overused
4. Verify `congestion_cost()` > 0 when cell is overused (occupancy > capacity)
5. Verify `update_history()` only increments cost on overused cells

### 4. PathFinder converges on crossing-net test grids

1. Run `cargo test -p cypcb-autoroute --lib --release -- pathfinder`
2. **Expected:** 3 tests pass: `pathfinder_strategy_name`, `pathfinder_converges_crossing_nets`, `pathfinder_impossible_routing`
3. Verify PathFinder converges in <15 iterations on a 30×20 grid with 4 crossing nets
4. Verify impossible routing (thick wall) produces graceful degradation (routes what it can, reports unrouted)

### 5. PathFinder beats ImprovedAStar on led_blink benchmark

1. Run `cargo test -p cypcb-autoroute --test strategy_comparison --release -- strategy_comparison_led_blink --nocapture`
2. **Expected:** Comparison table printed to stderr showing both strategies' scores
3. Verify PathFinder composite score ≤ ImprovedAStar composite score
4. Verify PathFinder DRC violations ≤ ImprovedAStar DRC violations
5. Verify both strategies' DRC violations are below S02 baseline of 50
6. **Expected values (approximate):** PathFinder composite ~5001, DRC ~5, vias 0, length ~40mm; ImprovedAStar composite ~15544, DRC ~15, vias 2, length ~80mm

### 6. WASM compilation passes with all new strategy code

1. Run `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown`
2. **Expected:** Compilation succeeds with no errors
3. Verify no `std::time::Instant` or other WASM-incompatible APIs were introduced

### 7. WASM auto_route() compiles with strategy update

1. Run `cargo check -p cypcb-render`
2. **Expected:** Compilation succeeds — `auto_route()` uses `AutorouteConfig::default()` which selects PathFinder
3. Verify JSON return contract `{"ok":true,"routed":N,"unrouted":N}` is unchanged (no structural changes to auto_route output)

### 8. KiCad parser position normalization works

1. Run `cargo test -p cypcb-kicad --tests --release`
2. **Expected:** 17+ tests pass, including benchmark_parse tests
3. Verify component positions are board-origin-relative (not KiCad absolute coordinates)
4. This was the critical fix that made routing on KiCad fixtures actually work — without it all pads map to grid corner

## Edge Cases

### PathFinder with impossible routing

1. Run `cargo test -p cypcb-autoroute --lib --release -- pathfinder_impossible_routing`
2. **Expected:** PathFinder hits iteration cap (50), reports non-convergence via tracing::warn!, routes what it can, leaves truly blocked nets unrouted
3. Should NOT panic or hang

### CongestionMap out-of-bounds access

1. Run `cargo test -p cypcb-autoroute --lib --release -- out_of_bounds`
2. **Expected:** Operations on coordinates outside grid dimensions are safe (no panic), return zero cost

### Large benchmark fixtures (optional, slow)

1. Run `cargo test -p cypcb-autoroute --test strategy_comparison --release -- --ignored --nocapture`
2. **Expected:** stm32_breakout and multi_ic tests run (may take several minutes)
3. Note: These are `#[ignore]` tests — only run when explicitly requested

## Failure Signals

- Any test in `cargo test -p cypcb-autoroute` fails → regression in strategy dispatch or routing logic
- WASM check fails → WASM-incompatible code introduced (likely `std::time`, `std::fs`, etc.)
- Strategy comparison shows ImprovedAStar beating PathFinder → PathFinder convergence broken or config wrong
- DRC violations above S02 baseline (50) → routing quality regression
- PathFinder test shows >50 iterations → convergence problem with history beta schedule
- `cargo check -p cypcb-render` fails → auto_route() API contract broken

## Requirements Proved By This UAT

- R104 (Multi-Strategy Routing Engine) — test cases 1, 2, 4, 5 prove two strategies compete on same board with quantitative comparison
- R105 (Negotiated Congestion) — test cases 3, 4 prove PathFinder iteratively resolves congestion with convergence
- R106 (Via Placement Strategy) — test case 5 proves PathFinder places 0 vias vs ImprovedAStar's 2 (congestion-driven layer decisions)
- R107 (Zero DRC Violations) — test case 5 proves DRC reduced from baseline 50 to 5 (partial progress; zero target continues in S04/S07)

## Not Proven By This UAT

- R107 zero DRC violations — PathFinder achieves 5, not 0. Remaining violations may be grid artifacts resolvable by S04 smoothing
- R108 clean 45°/90° traces — grid-aligned paths still staircase; S04 post-processing required
- Performance under realtime budget (<1s) — led_blink routes in ~10s debug / ~2s release; optimization needed for S05
- stm32_breakout and multi_ic strategy comparison — too slow for CI, only verified via `--ignored`
- Visual quality of routed output — S03 is quantitative only; visual verification deferred to S07

## Notes for Tester

- Always use `--release` for strategy comparison tests — debug builds are ~5-10× slower and may time out
- The strategy comparison table is printed to stderr, not stdout — use `--nocapture` to see it
- PathFinder's composite score improvement (3× better) is significant but DRC is still not zero — this is expected and documented
- `RUST_LOG=cypcb_autoroute=info` adds detailed per-iteration PathFinder convergence stats to output
- The led_blink fixture has 7 components and 7 nets — it's the simplest benchmark. Real-world improvement may vary on complex boards.
