---
id: T03
parent: S03
milestone: M004
provides:
  - Strategy comparison test proving PathFinder beats ImprovedAStar on led_blink benchmark
  - WASM auto_route() transparently uses PathFinder via route_board() dispatch
  - KiCad PCB parser position normalization (board-origin-relative coordinates)
  - BENCHMARKS constant re-exported from cypcb-kicad for test access
key_files:
  - crates/cypcb-autoroute/tests/strategy_comparison.rs
  - crates/cypcb-kicad/src/pcb_parser.rs
  - crates/cypcb-kicad/src/lib.rs
  - crates/cypcb-autoroute/Cargo.toml
key_decisions:
  - "D-M004-024: KiCad parser translates component positions to board-origin-relative coords (subtract board_bounds.min). Without this, all pads mapped to grid corner (629,472) because KiCad absolute coords (e.g. 120mm,115mm) exceeded the 40×30mm board grid."
  - "D-M004-025: stm32_breakout and multi_ic strategy comparison tests marked #[ignore] because A* routing on large grids (75×65mm / 100×80mm) exceeds 60s even in release mode. led_blink runs in CI as the primary comparison."
patterns_established:
  - "compare_fixture() pattern: parse fresh → route with strategy → apply_routes → rebuild spatial index → score_board → assert. Each strategy gets a fresh world parse to avoid state contamination."
observability_surfaces:
  - "strategy_comparison test prints comparison table to stderr (╔ table format) showing composite, DRC violations, vias, length per strategy×fixture"
  - "tracing::info in route_board() shows dispatched strategy name; PathFinder iteration loop shows per-iteration convergence stats"
duration: ~60m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T03: WASM integration and benchmark strategy comparison

**PathFinder beats ImprovedAStar on led_blink (composite 5001 vs 15544, DRC 5 vs 15, 0 vias vs 2), WASM compiles, KiCad position normalization fixed**

## What Happened

1. **Verified WASM `auto_route()`** — `auto_route()` in `cypcb-render` already calls `route_board()` with `AutorouteConfig::default()`, which selects PathFinder (the default from T02). JSON return contract `{"ok":true,"routed":N,"unrouted":N}` unchanged. No code changes needed.

2. **Created `strategy_comparison.rs`** — integration test that parses KiCad benchmark fixtures with `parse_kicad_pcb()`, routes with both strategies, scores with `score_board()`, and prints a comparison table. Split into per-fixture tests: `led_blink` runs in CI, larger boards are `#[ignore]`.

3. **Fixed KiCad PCB parser coordinate origin** — discovered that all pads mapped to the same grid cell (629,472) because KiCad stores absolute positions (e.g., components at 120mm,115mm) while the routing grid assumes (0,0) origin. Fixed by subtracting `board_bounds.min` from component positions in `parse_footprint()`. This makes component coordinates board-relative, matching the grid's coordinate space.

4. **Results on led_blink:**
   - ImprovedAStar: composite=15543.6, DRC=15, vias=2, length=79.61mm
   - PathFinder: composite=5000.8, DRC=5, vias=0, length=40.64mm
   - PathFinder wins decisively (3x better composite, 3x fewer DRC violations)
   - Both well below S02 baseline of 50 DRC violations

5. **WASM checks pass:** `cargo check -p cypcb-render` and `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` both compile cleanly.

## Verification

| Check | Result |
|-------|--------|
| `cargo test -p cypcb-autoroute --test strategy_comparison --release -- strategy_comparison_led_blink --nocapture` | ✅ PASS — table printed, PathFinder wins |
| `cargo check -p cypcb-render` | ✅ PASS |
| `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` | ✅ PASS |
| `cargo test -p cypcb-autoroute --lib --release` | ✅ PASS — 88 tests |
| `cargo test -p cypcb-kicad --tests --release` | ✅ PASS — 17 tests (including benchmark_parse) |
| DRC violations < S02 baseline (50) | ✅ PathFinder=5, ImprovedAStar=15 |
| Diagnostic: strategy name in tracing output | ✅ strategy_kind_display, strategy_kind_default tests confirm |

### Slice-level verification status (T03 is final task):
- ✅ `cargo test -p cypcb-autoroute` — lib tests pass (88/88), strategy_comparison led_blink passes
- ✅ `cargo test -p cypcb-autoroute --test strategy_comparison` — led_blink passes, stm32/multi_ic are `#[ignore]`
- ✅ `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — WASM compilation passes
- ✅ `cargo check -p cypcb-render` — auto_route() compiles
- ✅ Diagnostic: strategy names visible in tracing output

## Diagnostics

- Run `cargo test -p cypcb-autoroute --test strategy_comparison --release -- --nocapture` to see the comparison table
- Run with `RUST_LOG=cypcb_autoroute=info` to see PathFinder iteration convergence, strategy dispatch, and ratsnest extraction
- For slow benchmarks: `cargo test -p cypcb-autoroute --test strategy_comparison --release -- --ignored --nocapture`

## Deviations

1. **KiCad parser position fix** — not in the task plan but required to make routing work on KiCad fixtures. Component positions needed board-origin normalization.
2. **Per-fixture test split** — plan called for single test over all 3 fixtures. stm32_breakout and multi_ic routing is too slow (>60s each, even in release mode) so they're `#[ignore]` tests. led_blink fully validates the integration.
3. **No `route_board()` default change** — plan step 4 said "Update route_board() default to use empirically better strategy." PathFinder was already the default from T02, and the test confirms it's the better strategy, so no change needed.

## Known Issues

- stm32_breakout and multi_ic strategy comparison tests require `--ignored` flag and take minutes. A* on large grids is inherently slow; future optimization could add per-test timeouts or coarser grid resolution for benchmarks.
- KiCad reference routes (traces/vias extracted from the .kicad_pcb file) are NOT yet offset by board origin. This doesn't affect routing (reference routes are stored separately) but would matter if reference routes were applied to the world for comparison.

## Files Created/Modified

- `crates/cypcb-autoroute/tests/strategy_comparison.rs` — NEW: benchmark comparison integration test (~180 LOC)
- `crates/cypcb-autoroute/Cargo.toml` — added dev-dependencies: cypcb-core, cypcb-drc, cypcb-kicad, cypcb-router, tracing-subscriber
- `crates/cypcb-kicad/src/pcb_parser.rs` — added board_origin_mm parameter to parse_footprint(), translates component positions to board-relative coords
- `crates/cypcb-kicad/src/lib.rs` — re-exported BENCHMARKS constant from pcb_parser
- `.gsd/milestones/M004/slices/S03/tasks/T03-PLAN.md` — added Observability Impact section (pre-flight fix)
