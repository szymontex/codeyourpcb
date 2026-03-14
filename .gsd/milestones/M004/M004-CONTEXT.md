# M004: Production-Grade Autorouter — Context

**Gathered:** 2026-03-14
**Status:** Ready for planning

## Project Description

Replace the prototype A*-grid autorouter (~3200 LOC in `cypcb-autoroute`) with a production-grade routing engine validated against real KiCad PCB designs. The engine supports multiple routing strategies, generates variants with quality scoring, and provides realtime interactive tuning. Quality bar: zero DRC violations AND clean 45°/90° traces that look like a human routed them.

## Why This Milestone

The current autorouter is "żółw w przedszkolu" (user's words). It:
- Crosses traces (DRC violations in output)
- Doesn't strategically use top/bottom layers
- Produces no meaningful vias
- Creates sharp angles and zig-zag staircase patterns from grid quantization
- Results look nothing like professional PCB routing

This is the #1 barrier to the tool being taken seriously. A PCB tool with a bad autorouter is a PCB tool nobody uses.

## User-Visible Outcome

### When this milestone is complete, the user can:

- Click Route and get clean, DRC-compliant traces with proper via placement
- See routing score (trace length, via count, smoothness) and compare variants
- Hover alternative routing variants to preview them on canvas
- Adjust realtime parameters (density, via preference, roundness) and see routing update in ~1s
- Trust that the autorouter output is production-quality (no DRC violations, professional appearance)

### Entry point / environment

- Entry point: Route button in viewer toolbar + CLI `route` command
- Environment: Browser (WASM via `npx vite`), desktop (Tauri)
- Live dependencies involved: none (all in-browser WASM, no server needed)

## Completion Class

- Contract complete means: all benchmark boards route with zero DRC violations, scores improve over prototype
- Integration complete means: WASM `auto_route()` returns valid routing, viewer displays it correctly
- Operational complete means: realtime tuning works, variant preview works, user can meaningfully choose

## Final Integrated Acceptance

To call this milestone complete, we must prove:

- Route a KiCad-imported STM32 breakout board with zero DRC violations and clean 45°/90° traces
- Scoring shows quantitative improvement over prototype A* on all benchmark fixtures
- User can adjust density slider and see board re-route in <1 second
- Variant hover preview shows alternative routing on canvas

## Risks and Unknowns

- **PathFinder convergence** — negotiated congestion may not converge on complex boards within reasonable iterations. Mitigate: fallback to improved A* if PathFinder doesn't beat it.
- **WASM performance budget** — routing must be <1s for interactive tuning on typical boards. Mitigation: benchmark early (S01/S03), optimize or accept degraded interactivity for complex boards.
- **KiCad parser complexity** — .kicad_pcb S-expression format is extensive. Mitigation: parse only what we need (placement, netlist, pads, board outline), skip schematic/zone/fill data.
- **Smoother DRC safety** — post-processing traces to remove grid artifacts may introduce DRC violations. Mitigation: DRC check after every smoothing pass, reject if violations increase.

## Existing Codebase / Prior Art

- `crates/cypcb-autoroute/` — Current A* router (3228 LOC): grid.rs, pathfinder.rs, orchestrator.rs, cost.rs, postprocess.rs. Working but low quality output.
- `crates/cypcb-router/src/types.rs` — RouteSegment, ViaPlacement, RoutingResult, RoutingStatus types
- `crates/cypcb-rules/` — DesignConstraints, RoutingRuleSet, presets (JLCPCB, PCBWay, OSHPark, IPC)
- `crates/cypcb-world/src/components/trace.rs` — Trace, TraceSegment, Via ECS components
- `crates/cypcb-drc/` — DRC engine with clearance/width/drill/connectivity checks
- `crates/cypcb-kicad/` — KiCad footprint (.kicad_mod) import only — no .kicad_pcb parser yet
- `crates/cypcb-render/src/lib.rs` — PcbEngine WASM bridge with `auto_route()` method (just added)
- `viewer/src/main.ts` — triggerRouting() calls engine.auto_route(), displays result
- `viewer/src/wasm.ts` — PcbEngine interface with auto_route(): string

> See `.gsd/DECISIONS.md` for all architectural and pattern decisions.

## Relevant Requirements

- R101-R102 — KiCad parser and benchmark fixtures (S01)
- R103 — Scoring system (S02)
- R104-R107 — Routing engine core (S03)
- R108-R109 — Trace smoother (S04)
- R110-R111 — Realtime tuning (S05)
- R112-R113 — Variant UI (S06)
- R114-R116 — Benchmark validation (S07)

## Scope

### In Scope

- New routing engine (PathFinder + improved A*) in `crates/cypcb-autoroute/`
- KiCad .kicad_pcb parser (new crate or module in `cypcb-kicad`)
- Routing quality scoring system (new module)
- Trace smoothing post-processor (replace current simplify_path)
- Realtime parameter UI (sliders in viewer)
- Variant generation and preview UI
- Benchmark automation (CLI + test fixtures)
- WASM integration via existing `PcbEngine::auto_route()`

### Out of Scope / Non-Goals

- PCB renderer visual upgrade (M005)
- Differential pair routing (future)
- Length matching (future)
- Full topological (rubberband) routing engine
- AI/ML-based routing optimization
- KiCad schematic (.kicad_sch) import
- Editing imported KiCad boards in our DSL

## Technical Constraints

- All routing runs in WASM (no server dependency) — performance-critical
- Existing RoutingRuleSet trait must be respected (or extended, not replaced)
- RoutingResult types (RouteSegment, ViaPlacement) are the output contract
- DRC engine is the quality gate — routing that fails DRC is rejected
- Benchmark fixtures must be deterministic (same input → same score)

## Integration Points

- `PcbEngine::auto_route()` — WASM entry point, returns JSON result
- `viewer/src/main.ts` triggerRouting() — frontend routing trigger
- `cypcb-router::types::RoutingResult` — output data structure
- `cypcb-drc` — validation gate after routing
- `cypcb-rules::RoutingRuleSet` — design rules input

## Key User Quotes (Preserved Verbatim)

- "autorouter który jest teraz w użyciu jest fatalny, przecina ścieżki, nie ogarnia co lepiej górą dołem, nie ma via"
- "pobierasz jakieś designy PCB z sieci, patrzysz je otwierasz jakie są elementy, jak są poroutane ścieżki, i Tobie powinno wyjść coś podobnego"
- "musimy obsługiwać wariantowość, musimy wiedzieć dlaczego dany routing jest lepszy od drugiego"
- "to ma być super hitech — a na razie jest żółw w przedszkolu"
- "Musisz najpierw wymyślić najlepszą metodę na świecie"
- "weź wszystkie opcje jakie masz, i porównuj ze sobą, zobacz z którą będziesz miał największy sukces"
- "ten autorouter powinien być kurcze całkiem realtime"
- "nawet obrazki możesz sobie po obrazkach porównywać"

## Open Questions

- **KiCad version targeting** — KiCad 6/7/8 have slightly different S-expression schemas. Target KiCad 7+ (most common current version), handle 6 if minimal effort.
- **Benchmark board selection** — Need to find and validate 3-5 good reference projects. Criteria: 2-layer, reasonable complexity, well-routed by human, open source license.
- **PathFinder iteration limit** — How many negotiation iterations before declaring convergence? Start with 50, tune empirically.
- **Score weights** — How to weight trace length vs via count vs smoothness in composite score? Start equal, tune based on benchmark results.
