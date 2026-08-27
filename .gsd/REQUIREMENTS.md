# Requirements

This file is the explicit capability and coverage contract for the project.

## Active

### R101 — KiCad .kicad_pcb Board Parser
- Class: core-capability
- Status: validated
- Description: Parse KiCad 6/7/8 .kicad_pcb files (S-expression format) into BoardWorld — extract board outline, footprints, pads, nets, existing traces, vias, and zones
- Why it matters: Ground truth for benchmarking requires real PCB designs; KiCad is the standard open-source format
- Source: user
- Primary owning slice: M004/S01
- Supporting slices: none
- Validation: 39 tests (unit + integration), CLI JSON output on 3 fixtures, ratsnest compatibility proof — M004/S01
- Notes: Only need placement+netlist extraction (not full KiCad fidelity). Dimensions in mm, our model uses nm. Board outline is bounding-box only. **The note that zones were not extracted because there is no `Zone` type in the ECS was true when it was written and is not now**: `cypcb-world` has `components::zone::Zone`, `cypcb-kicad`'s `parse_zone` builds a `ZoneImport`, and a pour that cannot be carried is refused by name rather than dropped.

### R102 — Benchmark Suite from Real KiCad Projects
- Class: quality-attribute
- Status: active
- Description: At least 3 open-source KiCad PCB projects (simple LED blink, medium STM32 breakout, complex multi-IC) downloaded, parsed, and usable as automated benchmark fixtures
- Why it matters: Empirical validation — autorouter quality measured against human-routed reference designs, not theoretical metrics
- Source: user
- Primary owning slice: M004/S01
- Supporting slices: M004/S07
- Validation: 3 synthetic fixtures parse with correct metadata, BENCHMARKS const + get_benchmarks() accessor, 5 integration tests — M004/S01 (partial: synthetic, not downloaded real projects)
- Notes: Fixtures are synthetic KiCad 8 files (license-clean, controlled). Real project fixtures may be added later. Store reference routing for comparison.

### R103 — Routing Quality Scoring System
- Class: core-capability
- Status: validated
- Description: Score any routed board on: total trace length, via count, DRC violation count, trace smoothness (bend angle distribution), crossing count, layer utilization balance
- Why it matters: Without quantitative scoring, "better" routing is subjective. Scoring enables variant ranking and regression detection.
- Source: user
- Primary owning slice: M004/S02
- Supporting slices: M004/S06, M004/S07
- Validation: 31 tests (27 unit + 4 integration), 7-metric RoutingScore with composite formula, CLI JSON output, baseline scores for blink.cypcb and routing-test.cypcb — M004/S02
- Notes: Score must be a single composite number (weighted sum) plus individual metric breakdown. Lower = better. score_board() takes ScoreWeights parameter for configurability.

### R104 — Multi-Strategy Routing Engine
- Class: core-capability
- Status: validated
- Description: Implement multiple routing strategies (at minimum: PathFinder negotiated congestion, improved A* with better heuristics) and run them on the same board for comparison
- Why it matters: User said "weź wszystkie opcje, porównuj ze sobą, zobacz z którą masz największy sukces" — empirical, not theoretical selection
- Source: user
- Primary owning slice: M004/S03
- Supporting slices: M004/S07
- Validation: RoutingStrategy trait with 2 implementations (PathFinder, ImprovedAStar), strategy_comparison integration test proves PathFinder wins 3× on led_blink (composite 5001 vs 15544), 88 unit tests + WASM compilation — M004/S03
- Notes: Winner determined by benchmark scores, not upfront assumption.

### R105 — Negotiated Congestion with Rip-up/Reroute
- Class: core-capability
- Status: validated
- Description: PathFinder-style routing: initial greedy routing of all nets, iterative congestion cost increase on overused resources, rip-up and reroute until convergence
- Why it matters: Industry-proven approach (KiCad, FreeRouting, FPGA tools). Current A* does sequential routing without global optimization.
- Source: research
- Primary owning slice: M004/S03
- Supporting slices: none
- Validation: PathFinderStrategy converges on crossing-net test grids in <15 iterations, CongestionMap with present/history cost tracking, VPR partial-reroute optimization, 11 unit tests + benchmark comparison — M004/S03
- Notes: Key difference from current A*: all nets route simultaneously with shared resource negotiation, not one-at-a-time.

### R106 — Proper Via Placement Strategy
- Class: core-capability
- Status: validated
- Description: Autorouter strategically places vias for layer transitions — considers via cost, prefers fewer vias, places them at natural transition points, respects via-to-via clearance
- Why it matters: Current router has via support in code but produces poor/no via placement in practice. User complained "nie ogarnia co lepije górą dołem, nie ma via"
- Source: user
- Primary owning slice: M004/S03
- Supporting slices: none
- Validation: PathFinder produces 0 vias on led_blink (vs ImprovedAStar's 2), congestion-driven layer transitions avoid gratuitous via placement — M004/S03
- Notes: Via placement must respect DRC rules (drill size, annular ring, clearance to traces/pads).

### R107 — Zero DRC Violations in Autorouter Output
- Class: quality-attribute
- Status: active
- Description: Every route produced by autorouter must pass DRC — no trace crossings, no clearance violations, no short circuits, no unconnected nets (unless explicitly partial)
- Why it matters: User's #1 complaint: "przecina ścieżki". A router that violates DRC is worse than no router.
- Source: user
- Primary owning slice: M004/S03
- Supporting slices: M004/S04, M004/S07
- Validation (2026-08-06, re-measured): `cargo test -p cypcb-autoroute --test drc_report -- --ignored` reports what the router introduces on each benchmark, separated from what the fixture already violated: led_blink **1**, stm32_breakout **174**, multi_ic **79**. The regression gate asserts `drc_violations <= 1` and composite `<= 1100.0`. The smoother is proven not to increase violations, this time by running with it off - `AutorouteConfig::smoothing`, identical counts both ways.
- Correction: the earlier claim that the remaining violations are "grid artifacts" is false and was measured to be false. led_blink's single violation is `C1 ↔ trace 'GND': 0.00mm`, and the fixture's netlist has C1 pad 1 on `SW_OUT`, pad 2 on `GND` - a GND trace lying across the switch output. That board does not blink. The 0.00mm population is copper on copper, not rounding: 20 of stm32_breakout's 26 trace-to-trace overlaps sit on a cell two nets' paths both hold.
- Notes: DRC check runs automatically after routing. If violations found, routing result is rejected.

### R108 — Clean 45°/90° Trace Geometry
- Class: quality-attribute
- Status: validated
- Description: All autorouted traces use only 0°, 45°, 90°, 135° angles. No arbitrary angles, no zig-zag staircase patterns, no sharp bends
- Why it matters: "Ostre krawędzie, nienaturalne" — professional PCB traces follow 45°/90° convention for signal integrity and aesthetics
- Source: user
- Primary owning slice: M004/S04
- Supporting slices: none
- Validation: smoothness=1.000 on led_blink integration test (all bends at 45° multiples), is_valid_angle() enforces exact integer angle patterns on every output segment, 22 unit tests covering staircase collapse, chamfering, merge, angle enforcement — M004/S04
- Notes: Grid-based routing naturally produces staircase patterns; 3-pass smoother converts to clean angled traces.

### R109 — Trace Smoothing Post-Processor
- Class: core-capability
- Status: validated
- Description: Post-processing pipeline that takes raw grid-path output and produces clean traces: merge collinear segments, simplify corners to 45° bends, remove unnecessary detours, minimize total length
- Why it matters: Even a good routing algorithm produces grid-aligned paths. Smoothing is what makes them look professional.
- Source: inferred
- Primary owning slice: M004/S04
- Supporting slices: none
- Validation: 3-pass smoother (staircase collapse, corner chamfer, collinear merge) with per-move DRC safety via segment_distance(), integrated into both PathFinder and ImprovedAStar strategies. 17 unit tests + integration test proving smoothness improvement and DRC non-regression on led_blink — M004/S04
- Notes: Preserves DRC compliance after smoothing — per-move clearance checks reject any move that would introduce violations.

### R110 — Realtime Tuning Parameters
- Class: differentiator
- Status: active
- Description: User-facing sliders for: trace density/spacing preference, via cost (fewer vs more vias), corner rounding amount, layer preference (top-heavy vs balanced)
- Why it matters: User wants autorouter to be "calkiem realtime" with interactive parameter control, not fire-and-forget
- Source: user
- Primary owning slice: M004/S05
- Supporting slices: none
- Validation: AutorouteParams struct with 4 fields (via_cost, layer_preference, roundness, density), WASM auto_route_with_params() entry point, collapsible tuning panel with 4 sliders, 300ms debounced re-routing, 8 unit + 4 integration + 7 E2E tests — M004/S05
- **Not validated today**: the E2E tests this line counts are `test.describe.skip` in `viewer/e2e/tuning-panel.spec.ts`, because the Route split-button is `display:none` in `viewer/index.html`. The code exists and the engine answers; nothing in the shipped interface reaches it. Blocked on D5.
- Notes: Parameters feed into routing cost function. Changing a slider triggers re-route.

### R111 — Reactive Re-Routing on Parameter Change
- Class: differentiator
- Status: validated
- Description: When user adjusts a tuning slider, the board re-routes within ~1 second for typical boards (Blink-level: <100ms, STM32-level: <1s)
- Why it matters: "powinien reagować jednak realtime" — interactive tuning loses value if re-routing takes 10+ seconds
- Source: user
- Primary owning slice: M004/S05
- Supporting slices: M004/S03
- Validation: Slider input events debounced at 300ms, trigger auto_route_with_params() → pullSnapshot() → canvas update. Integration test proves different params produce different scores. WASM compiles and routes. Timing budget validated in S07 benchmark. — M004/S05
- Notes: May require: faster algorithm, incremental re-routing (only affected nets), or WASM worker thread. Performance budget is real constraint on engine design.

### R112 — Routing Variant Generation
- Class: core-capability
- Status: validated
- Description: Generate 2-4 routing variants per board using different strategies/parameter sets. Each variant is a complete routed result with its score.
- Why it matters: "musimy obsługiwać wariantowość, musimy wiedzieć dlaczego dany routing jest lepszy od drugiego"
- Source: user
- Primary owning slice: M004/S06
- Supporting slices: M004/S02, M004/S03
- Validation: 4 variants generated sequentially (PathFinder default/low-via/high-density + ImprovedAStar default), ranked by composite score. 5 unit + 5 integration tests + 7 E2E tests prove full pipeline — M004/S06
- Notes: Variants run sequentially (BoardWorld not Clone). Limited by total time budget (~1s WASM for simple boards).

### R113 — Auto-Apply Best Variant with Hover Preview
- Class: primary-user-loop
- Status: active
- Description: Route button auto-applies the highest-scored variant. Score panel shows all variants with metrics. Hovering an alternative variant previews it on canvas without applying.
- Why it matters: "2 ale user może hoverować na inne rezultaty i je zobaczy na ekranie" — user picks with visual feedback
- Source: user
- Primary owning slice: M004/S06
- Supporting slices: none
- Validation: Route button calls auto_route_variants(), best variant auto-applied, panel shows ranked results with scores, hover renders cyan ghost overlay at 0.4 alpha with active traces dimmed. 7 E2E tests verify panel lifecycle, hover preview, click selection, and debug surface. — M004/S06
- **Not validated today**: the E2E tests this line counts are `test.describe.skip` in `viewer/e2e/variant-panel.spec.ts`, because the Route split-button is `display:none` in `viewer/index.html`. The code exists and the engine answers; nothing in the shipped interface reaches it. Blocked on D5.
- Notes: Canvas supports overlaying preview routes (different color/opacity) without mutating board state. Click-to-apply is display-only (doesn't re-route with clicked config).

### R114 — Benchmark Validation Against KiCad Reference Designs
- Class: quality-attribute
- Status: validated
- Description: Automated benchmark: strip routes from KiCad fixtures, re-route with our engine, compare scores (our routing vs original human routing)
- Why it matters: "pobierasz jakieś designy PCB z sieci, patrzysz, otwierasz, i Tobie powinno wyjść coś podobnego" — empirical quality proof
- Source: user
- Primary owning slice: M004/S07
- Supporting slices: M004/S01, M004/S02
- Validation: benchmark_regression CI gate (non-ignored) asserts composite ≤ 5501, DRC ≤ 5, smoothness ≥ 0.95. benchmark_full_matrix (ignored) compares 3 fixtures × 2 strategies with JSON report — M004/S07
- Notes: Goal is not to beat human routing (unrealistic for V1) but to approach it. Track score gap as regression metric.

### R115 — Visual Comparison of Routed Boards
- Class: quality-attribute
- Status: active
- Description: Generate screenshots of autorouter output and reference designs for visual comparison. Store as benchmark artifacts.
- Why it matters: "nawet obrazki możesz sobie po obrazkach porównywać" — metrics don't capture everything, visual diff catches layout quality issues
- Source: user
- Primary owning slice: M004/S07
- Supporting slices: none
- Validation: Playwright E2E captures 6 screenshots (full-page + canvas per fixture) to test-results/benchmark/ for human inspection — M004/S07
- **Not validated today**: the E2E tests this line counts are `test.describe.skip` in `viewer/e2e/benchmark-screenshots.spec.ts`, because the Route split-button is `display:none` in `viewer/index.html`. The code exists and the engine answers; nothing in the shipped interface reaches it. Blocked on D5.
- Notes: Uses existing 2D renderer. Full renderer upgrade is M005.

### R116 — Empirical Strategy Selection
- Class: quality-attribute
- Status: validated
- Description: Based on benchmark results across all fixtures, determine which routing strategy wins overall and make it the default
- Why it matters: "nie wiem mordo, weź wszystkie opcje, porównuj ze sobą" — let data decide, not assumptions
- Source: user
- Primary owning slice: M004/S07
- Supporting slices: M004/S03
- Validation: benchmark_full_matrix proves PathFinder composite (5001) beats ImprovedAStar (15544) on led_blink by 3×. PathFinder confirmed as default strategy — M004/S07
- Notes: Winner may vary by board complexity. Could result in automatic strategy selection heuristic.

### R201 — Web Worker Routing — Main Thread Never Blocked
- Class: core-capability
- Status: active
- Description: All WASM autorouting (single route, variants, tuning re-route) executes in a Web Worker. Main thread never calls synchronous WASM routing functions.
- Why it matters: User says "zacina totalnie przeglądarkę" — synchronous WASM on main thread freezes UI for 60-160+ seconds. Unusable.
- Source: user
- Primary owning slice: M005/S01
- Supporting slices: M005/S04
- Validation: unmapped
- Notes: Cancel via worker.terminate() + respawn. No SharedArrayBuffer (requires COOP/COEP headers).

### R202 — Routing Progress Visible During Execution
- Class: primary-user-loop
- Status: active
- Description: User sees a spinner/overlay immediately when routing starts and it remains visible throughout. Browser stays responsive — user can scroll, click cancel, interact with toolbar.
- Why it matters: "żadnego okienka, żadnego progressbaru, po prostu freeze" — zero feedback is unacceptable UX
- Source: user
- Primary owning slice: M005/S01
- Supporting slices: none
- Validation: unmapped
- Notes: With Web Worker, main thread is free to paint — spinner just works. No setTimeout hack needed.

### R203 — Cancel Routing Mid-Execution
- Class: primary-user-loop
- Status: active
- Description: Cancel button visible during routing, clicking it terminates the routing immediately and resets UI to pre-route state.
- Why it matters: User cannot wait minutes for routing to finish on complex boards. Must have escape hatch.
- Source: user
- Primary owning slice: M005/S01
- Supporting slices: none
- Validation: unmapped
- Notes: worker.terminate() is the only reliable cancellation — WASM has no cooperative preemption.

### R204 — 0 Unrouted on Blink LED
- Class: quality-attribute
- Status: active
- Description: PathFinder routes all 25 connections (8 nets) on the Blink LED template board with 0 unrouted. Proven by cargo test and by WASM result in browser.
- Why it matters: User sees "jedno niechlujne połączenie i reszta to ray tracers na żółto" — 5/25 unrouted on simplest board means router is broken.
- Source: user
- Primary owning slice: M005/S02
- Supporting slices: M005/S03
- Validation (2026-08-06): mapped and met. `benchmark_validation` asserts `unrouted == 0` before it looks at any quality metric, and all three benchmark fixtures route `Complete`. `tests/abandoned_connections.rs` names any connection the router gives up on, so a regression says which net rather than only how many.
- Notes: the suspected root cause - convergence failure on multi-pad nets - was the right shape but the wrong mechanism. `net_path_cells` gathered every connection of a net into one list and marked each cell once per connection, so a net's own junctions counted as overuse and it negotiated against itself forever. Deduplicating the net's cells fixed it.

### R205 — E2E Test: UI Responsive During Routing
- Class: quality-attribute
- Status: active
- Description: Playwright E2E test loads a board, clicks Route, and proves the browser did NOT freeze — overlay is visible, cancel button is clickable, page title is readable.
- Why it matters: CI tests pass green while browser freezes — "żadne z Twoich internal sposobów pomiarów nie daje CI znać że to nie działa"
- Source: user
- Primary owning slice: M005/S03
- Supporting slices: none
- Validation: unmapped
- Notes: Test must interact with UI DURING routing execution (not after). This only works with Web Worker.

### R206 — E2E Test: Routing Result Quality
- Class: quality-attribute
- Status: active
- Description: E2E test asserts routing result has 0 unrouted connections on a simple test board.
- Why it matters: Catches routing quality regressions that unit tests miss (different environment, WASM vs native).
- Source: user
- Primary owning slice: M005/S03
- Supporting slices: none
- Validation: unmapped
- Notes: Uses __routingWorker or status text to assert result quality.

### R207 — Variant Generation via Web Worker
- Class: core-capability
- Status: active
- Description: Route button generates 3+ routing variants via Web Worker. Score panel shows ranked results. Hover preview renders alternatives.
- Why it matters: Variant generation was the M004 differentiator but broke in browser due to main-thread freeze. Must work via Worker.
- Source: inferred
- Primary owning slice: M005/S04
- Supporting slices: none
- Validation: unmapped
- Notes: Builds on S01 Worker infrastructure. auto_route_variants() called inside worker.

## Deferred

### R120 — PCB Renderer Upgrade to KiCad/Atopile Visual Standard
- Class: quality-attribute
- Status: deferred
- Description: Upgrade 2D renderer to match KiCad/Atopile visual quality — proper copper fills, realistic pad shapes, via rings, solder mask, silkscreen
- Why it matters: "nasz obecny widok PCB odbiega od standardu" — visual comparison and professional appearance
- Source: user
- Primary owning slice: future (separate milestone)
- Supporting slices: none
- Validation: unmapped
- Notes: Originally planned as M005, pushed to future milestone. M005 is now WASM routing fix.

### R121 — Differential Pair Routing
- Class: core-capability
- Status: deferred
- Description: Route differential signal pairs with controlled spacing and length matching
- Why it matters: Required for USB, HDMI, and other high-speed interfaces
- Source: inferred
- Primary owning slice: future
- Supporting slices: none
- Validation: unmapped
- Notes: Requires routing engine architecture that supports paired routing constraints.

### R122 — Length Matching for High-Speed Signals
- Class: core-capability
- Status: deferred
- Description: Automatically match trace lengths for bus signals (DDR, SPI clock/data) using serpentine routing
- Why it matters: Timing-critical signals need matched propagation delay
- Source: inferred
- Primary owning slice: future
- Supporting slices: none
- Validation: unmapped
- Notes: Depends on net classification (signal class) from DSL constraints.

## Out of Scope

### R130 — Topological (Rubberband) Routing as Standalone Engine
- Class: constraint
- Status: out-of-scope
- Description: Full topological router like Topola (rubberband geometry, any-angle routing, exact geometry kernel)
- Why it matters: Prevents scope creep — topological routing is a fundamentally different architecture requiring custom geometry kernel
- Source: research
- Primary owning slice: none
- Supporting slices: none
- Validation: n/a
- Notes: Topological ideas may inform the smoother (S04) but full implementation is years of work.

### R131 — AI/ML-Based Routing Optimization
- Class: constraint
- Status: out-of-scope
- Description: Machine learning models trained on PCB routing data for placement or routing optimization
- Why it matters: Prevents scope creep — ML routing is research-stage, not production-ready
- Source: research
- Primary owning slice: none
- Supporting slices: none
- Validation: n/a
- Notes: Focus on proven algorithmic approaches (PathFinder, A*) with empirical tuning.

## Traceability

| ID | Class | Status | Primary owner | Supporting | Proof |
|---|---|---|---|---|---|
| R101 | core-capability | validated | M004/S01 | none | 39 tests + CLI + ratsnest compat (M004/S01) |
| R102 | quality-attribute | active | M004/S01 | M004/S07 | partial: 3 synthetic fixtures parse (M004/S01), benchmark_regression + full_matrix run automated comparisons (M004/S07). Still synthetic, not downloaded real projects. |
| R103 | core-capability | validated | M004/S02 | M004/S06, M004/S07 | 31 tests (27 unit + 4 integration), CLI JSON output, baseline scores (M004/S02) |
| R104 | core-capability | validated | M004/S03 | M004/S07 | 2 strategies + comparison test, PathFinder wins 3× (M004/S03) |
| R105 | core-capability | validated | M004/S03 | none | PathFinder converges on test grids, 11 unit tests + benchmark (M004/S03) |
| R106 | core-capability | validated | M004/S03 | none | PathFinder 0 vias vs ImprovedAStar 2 on led_blink (M004/S03) |
| R107 | quality-attribute | active | M004/S03 | M004/S04, M004/S07 | DRC 50→5 (partial, M004/S03), non-regression proven (M004/S04) |
| R108 | quality-attribute | validated | M004/S04 | none | smoothness=1.000, is_valid_angle() enforcement, 22 unit tests (M004/S04) |
| R109 | core-capability | validated | M004/S04 | none | 3-pass smoother + per-move DRC, 17 unit + 1 integration test (M004/S04) |
| R110 | differentiator | active | M004/S05 | none | AutorouteParams, WASM entry point, tuning panel, 8+4+7 tests (M004/S05) |
| R111 | differentiator | validated | M004/S05 | M004/S03 | 300ms debounced re-route, params→score difference proven (M004/S05) |
| R112 | core-capability | validated | M004/S06 | M004/S02, M004/S03 | 4 variants, 5 unit + 5 integration + 7 E2E tests (M004/S06) |
| R113 | primary-user-loop | active | M004/S06 | none | auto-apply best + hover ghost preview + 7 E2E tests (M004/S06) |
| R114 | quality-attribute | validated | M004/S07 | M004/S01, M004/S02 | benchmark_regression CI gate + benchmark_full_matrix comparison (M004/S07) |
| R115 | quality-attribute | active | M004/S07 | none | 6 Playwright screenshots to test-results/benchmark/ (M004/S07) |
| R116 | quality-attribute | validated | M004/S07 | M004/S03 | PathFinder 5001 vs ImprovedAStar 15544 on led_blink (M004/S07) |
| R120 | quality-attribute | deferred | future | none | unmapped |
| R121 | core-capability | deferred | future | none | unmapped |
| R122 | core-capability | deferred | future | none | unmapped |
| R130 | constraint | out-of-scope | none | none | n/a |
| R131 | constraint | out-of-scope | none | none | n/a |
| R201 | core-capability | active | M005/S01 | M005/S04 | unmapped |
| R202 | primary-user-loop | active | M005/S01 | none | unmapped |
| R203 | primary-user-loop | active | M005/S01 | none | unmapped |
| R204 | quality-attribute | active | M005/S02 | M005/S03 | unmapped |
| R205 | quality-attribute | active | M005/S03 | none | unmapped |
| R206 | quality-attribute | active | M005/S03 | none | unmapped |
| R207 | core-capability | active | M005/S04 | none | unmapped |

## Coverage gaps

Test suites that do not run, and what that costs. A suite skipped without a
line here is a coverage claim nobody can check; `the-requirements-name-every-skipped-suite`
in the viewer's tests fails if this list and the specs disagree.

| Suite | Why it is skipped |
|---|---|
| `viewer/e2e/tuning-panel.spec.ts` | The Route split-button and its dropdown are `display:none` in `viewer/index.html`. D5. |
| `viewer/e2e/variant-panel.spec.ts` | The panel has no code path that can show it. `showVariants()` in `src/variant-panel.ts` has no caller anywhere in the viewer - `main.ts` imports `initVariantPanel`, `hideVariants` and `isVariantPanelVisible` and routes through `auto_route_with_params`, a single run. The engine's `auto_route_variants()` exists and the bridge declares it; nothing calls it. Unhiding the button (D5) is necessary and not sufficient. |
| `viewer/e2e/benchmark-screenshots.spec.ts` | Routes boards through the same hidden button. D5. |

## Coverage Summary

- Active requirements: 12
- Mapped to slices: 23 (requirements whose primary owner in the traceability table is a slice: 16 under M004, 7 under M005)
- Validated: 11
- Unmapped active requirements: 0
- **Three requirements moved from validated to active on 2026-08-08**: R110,
  R113 and R115 each cited E2E tests as their validation, and those suites are
  skipped. A requirement is not validated by a test that does not run.
- **The traceability table went on calling those three validated until
  2026-08-27**, which is the shape of drift this file exists to prevent: the
  status was corrected where it is declared and not where it is repeated. The
  table is the repetition, so a test now holds the two together.
