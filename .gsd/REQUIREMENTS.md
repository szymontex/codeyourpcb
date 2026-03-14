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
- Notes: Only need placement+netlist extraction (not full KiCad fidelity). Dimensions in mm, our model uses nm. Zones not yet extracted (no Zone type in ECS). Board outline is bounding-box only.

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
- Status: active
- Description: Implement multiple routing strategies (at minimum: PathFinder negotiated congestion, improved A* with better heuristics) and run them on the same board for comparison
- Why it matters: User said "weź wszystkie opcje, porównuj ze sobą, zobacz z którą masz największy sukces" — empirical, not theoretical selection
- Source: user
- Primary owning slice: M004/S03
- Supporting slices: M004/S07
- Validation: unmapped
- Notes: Winner determined by benchmark scores, not upfront assumption.

### R105 — Negotiated Congestion with Rip-up/Reroute
- Class: core-capability
- Status: active
- Description: PathFinder-style routing: initial greedy routing of all nets, iterative congestion cost increase on overused resources, rip-up and reroute until convergence
- Why it matters: Industry-proven approach (KiCad, FreeRouting, FPGA tools). Current A* does sequential routing without global optimization.
- Source: research
- Primary owning slice: M004/S03
- Supporting slices: none
- Validation: unmapped
- Notes: Key difference from current A*: all nets route simultaneously with shared resource negotiation, not one-at-a-time.

### R106 — Proper Via Placement Strategy
- Class: core-capability
- Status: active
- Description: Autorouter strategically places vias for layer transitions — considers via cost, prefers fewer vias, places them at natural transition points, respects via-to-via clearance
- Why it matters: Current router has via support in code but produces poor/no via placement in practice. User complained "nie ogarnia co lepije górą dołem, nie ma via"
- Source: user
- Primary owning slice: M004/S03
- Supporting slices: none
- Validation: unmapped
- Notes: Via placement must respect DRC rules (drill size, annular ring, clearance to traces/pads).

### R107 — Zero DRC Violations in Autorouter Output
- Class: quality-attribute
- Status: active
- Description: Every route produced by autorouter must pass DRC — no trace crossings, no clearance violations, no short circuits, no unconnected nets (unless explicitly partial)
- Why it matters: User's #1 complaint: "przecina ścieżki". A router that violates DRC is worse than no router.
- Source: user
- Primary owning slice: M004/S03
- Supporting slices: M004/S04, M004/S07
- Validation: unmapped
- Notes: DRC check runs automatically after routing. If violations found, routing result is rejected.

### R108 — Clean 45°/90° Trace Geometry
- Class: quality-attribute
- Status: active
- Description: All autorouted traces use only 0°, 45°, 90°, 135° angles. No arbitrary angles, no zig-zag staircase patterns, no sharp bends
- Why it matters: "Ostre krawędzie, nienaturalne" — professional PCB traces follow 45°/90° convention for signal integrity and aesthetics
- Source: user
- Primary owning slice: M004/S04
- Supporting slices: none
- Validation: unmapped
- Notes: Grid-based routing naturally produces staircase patterns; post-processing must convert to clean angled traces.

### R109 — Trace Smoothing Post-Processor
- Class: core-capability
- Status: active
- Description: Post-processing pipeline that takes raw grid-path output and produces clean traces: merge collinear segments, simplify corners to 45° bends, remove unnecessary detours, minimize total length
- Why it matters: Even a good routing algorithm produces grid-aligned paths. Smoothing is what makes them look professional.
- Source: inferred
- Primary owning slice: M004/S04
- Supporting slices: none
- Validation: unmapped
- Notes: Must preserve DRC compliance after smoothing — no introducing violations during optimization.

### R110 — Realtime Tuning Parameters
- Class: differentiator
- Status: active
- Description: User-facing sliders for: trace density/spacing preference, via cost (fewer vs more vias), corner rounding amount, layer preference (top-heavy vs balanced)
- Why it matters: User wants autorouter to be "calkiem realtime" with interactive parameter control, not fire-and-forget
- Source: user
- Primary owning slice: M004/S05
- Supporting slices: none
- Validation: unmapped
- Notes: Parameters feed into routing cost function. Changing a slider triggers re-route.

### R111 — Reactive Re-Routing on Parameter Change
- Class: differentiator
- Status: active
- Description: When user adjusts a tuning slider, the board re-routes within ~1 second for typical boards (Blink-level: <100ms, STM32-level: <1s)
- Why it matters: "powinien reagować jednak realtime" — interactive tuning loses value if re-routing takes 10+ seconds
- Source: user
- Primary owning slice: M004/S05
- Supporting slices: M004/S03
- Validation: unmapped
- Notes: May require: faster algorithm, incremental re-routing (only affected nets), or WASM worker thread. Performance budget is real constraint on engine design.

### R112 — Routing Variant Generation
- Class: core-capability
- Status: active
- Description: Generate 2-4 routing variants per board using different strategies/parameter sets. Each variant is a complete routed result with its score.
- Why it matters: "musimy obsługiwać wariantowość, musimy wiedzieć dlaczego dany routing jest lepszy od drugiego"
- Source: user
- Primary owning slice: M004/S06
- Supporting slices: M004/S02, M004/S03
- Validation: unmapped
- Notes: Variants run in parallel (web workers or sequential with different configs). Limited by total time budget.

### R113 — Auto-Apply Best Variant with Hover Preview
- Class: primary-user-loop
- Status: active
- Description: Route button auto-applies the highest-scored variant. Score panel shows all variants with metrics. Hovering an alternative variant previews it on canvas without applying.
- Why it matters: "2 ale user może hoverować na inne rezultaty i je zobaczy na ekranie" — user picks with visual feedback
- Source: user
- Primary owning slice: M004/S06
- Supporting slices: none
- Validation: unmapped
- Notes: Canvas must support overlaying preview routes (different color/opacity) without mutating board state.

### R114 — Benchmark Validation Against KiCad Reference Designs
- Class: quality-attribute
- Status: active
- Description: Automated benchmark: strip routes from KiCad fixtures, re-route with our engine, compare scores (our routing vs original human routing)
- Why it matters: "pobierasz jakieś designy PCB z sieci, patrzysz, otwierasz, i Tobie powinno wyjść coś podobnego" — empirical quality proof
- Source: user
- Primary owning slice: M004/S07
- Supporting slices: M004/S01, M004/S02
- Validation: unmapped
- Notes: Goal is not to beat human routing (unrealistic for V1) but to approach it. Track score gap as regression metric.

### R115 — Visual Comparison of Routed Boards
- Class: quality-attribute
- Status: active
- Description: Generate screenshots of autorouter output and reference designs for visual comparison. Store as benchmark artifacts.
- Why it matters: "nawet obrazki możesz sobie po obrazkach porównywać" — metrics don't capture everything, visual diff catches layout quality issues
- Source: user
- Primary owning slice: M004/S07
- Supporting slices: none
- Validation: unmapped
- Notes: Uses existing 2D renderer. Full renderer upgrade is M005.

### R116 — Empirical Strategy Selection
- Class: quality-attribute
- Status: active
- Description: Based on benchmark results across all fixtures, determine which routing strategy wins overall and make it the default
- Why it matters: "nie wiem mordo, weź wszystkie opcje, porównuj ze sobą" — let data decide, not assumptions
- Source: user
- Primary owning slice: M004/S07
- Supporting slices: M004/S03
- Validation: unmapped
- Notes: Winner may vary by board complexity. Could result in automatic strategy selection heuristic.

## Deferred

### R120 — PCB Renderer Upgrade to KiCad/Atopile Visual Standard
- Class: quality-attribute
- Status: deferred
- Description: Upgrade 2D renderer to match KiCad/Atopile visual quality — proper copper fills, realistic pad shapes, via rings, solder mask, silkscreen
- Why it matters: "nasz obecny widok PCB odbiega od standardu" — visual comparison and professional appearance
- Source: user
- Primary owning slice: M005 (separate milestone)
- Supporting slices: none
- Validation: unmapped
- Notes: User explicitly chose M005 for this. M004 uses current renderer for visual comparison.

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
| R102 | quality-attribute | active | M004/S01 | M004/S07 | partial: 3 synthetic fixtures parse (M004/S01) |
| R103 | core-capability | validated | M004/S02 | M004/S06, M004/S07 | 31 tests (27 unit + 4 integration), CLI JSON output, baseline scores (M004/S02) |
| R104 | core-capability | active | M004/S03 | M004/S07 | unmapped |
| R105 | core-capability | active | M004/S03 | none | unmapped |
| R106 | core-capability | active | M004/S03 | none | unmapped |
| R107 | quality-attribute | active | M004/S03 | M004/S04, M004/S07 | unmapped |
| R108 | quality-attribute | active | M004/S04 | none | unmapped |
| R109 | core-capability | active | M004/S04 | none | unmapped |
| R110 | differentiator | active | M004/S05 | none | unmapped |
| R111 | differentiator | active | M004/S05 | M004/S03 | unmapped |
| R112 | core-capability | active | M004/S06 | M004/S02, M004/S03 | unmapped |
| R113 | primary-user-loop | active | M004/S06 | none | unmapped |
| R114 | quality-attribute | active | M004/S07 | M004/S01, M004/S02 | unmapped |
| R115 | quality-attribute | active | M004/S07 | none | unmapped |
| R116 | quality-attribute | active | M004/S07 | M004/S03 | unmapped |
| R120 | quality-attribute | deferred | M005 | none | unmapped |
| R121 | core-capability | deferred | future | none | unmapped |
| R122 | core-capability | deferred | future | none | unmapped |
| R130 | constraint | out-of-scope | none | none | n/a |
| R131 | constraint | out-of-scope | none | none | n/a |

## Coverage Summary

- Active requirements: 14
- Mapped to slices: 14
- Validated: 2
- Unmapped active requirements: 0
