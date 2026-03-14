# M004: Production-Grade Autorouter

**Vision:** Replace the prototype A*-grid autorouter with a production-grade, empirically-validated routing engine. Multiple strategies compete on real KiCad benchmark boards. Quality scoring, realtime interactive tuning, variant preview. Zero DRC violations and clean 45°/90° traces that look like a human routed them.

## Success Criteria

- Autorouter output has zero DRC violations on all benchmark boards
- All traces are clean 45°/90° geometry (no grid staircase artifacts)
- Vias are strategically placed for multi-layer routing
- Scoring proves quantitative improvement over prototype A* on all benchmark fixtures
- At least 3 KiCad reference designs parsed and benchmarked
- Realtime re-routing responds to parameter changes in <1s (typical boards)
- User can hover alternative routing variants and see them on canvas

## Key Risks / Unknowns

- **PathFinder convergence on complex boards** — negotiated congestion may fail to converge within performance budget
- **WASM performance for realtime** — sub-second re-routing in browser constrains algorithm choice
- **KiCad .kicad_pcb parser scope** — format is extensive, must scope to placement+netlist only
- **Post-processing DRC safety** — smoothing traces may introduce clearance violations

## Proof Strategy

- PathFinder convergence → retire in S03 by routing all benchmark fixtures and measuring iteration count + time
- WASM performance → retire in S03 by benchmarking routing time per board, target <1s for LED blink, <3s for STM32
- KiCad parser scope → retire in S01 by successfully parsing 3+ real .kicad_pcb files into BoardWorld
- Post-processing safety → retire in S04 by DRC-checking before and after smoothing on all fixtures

## Verification Classes

- Contract verification: Rust unit tests, benchmark score comparisons, DRC pass/fail
- Integration verification: WASM auto_route() returns valid JSON, viewer renders routed board
- Operational verification: realtime tuning responds within budget, variant preview works in browser
- UAT / human verification: visual comparison of routed boards vs KiCad reference screenshots

## Milestone Definition of Done

This milestone is complete only when all are true:

- All benchmark boards route with zero DRC violations
- Routing scores improve over prototype A* on every benchmark fixture
- Traces are clean 45°/90° — no raw grid paths in output
- Vias are placed with strategic layer transitions
- Realtime parameter tuning triggers re-route in <1s on typical boards
- Variant UI auto-applies best, hover shows alternatives with scores
- Automated benchmark suite runs and produces comparison report
- WASM integration works without dev server (npx vite only)

## Requirement Coverage

- Covers: R101, R102, R103, R104, R105, R106, R107, R108, R109, R110, R111, R112, R113, R114, R115, R116
- Partially covers: none
- Leaves for later: R120 (renderer upgrade → M005), R121 (diff pairs), R122 (length matching)
- Orphan risks: none

## Slices

- [x] **S01: KiCad PCB Parser & Benchmark Fixtures** `risk:high` `depends:[]`
  > After this: 3+ real .kicad_pcb files parse into BoardWorld with correct components, pads, nets, and board outline. CLI command `parse-kicad` produces valid .cypcb-compatible data. Benchmark fixture files exist with reference routing scores.

- [x] **S02: Routing Quality Score System** `risk:medium` `depends:[]`
  > After this: Any routed board gets a composite quality score (trace length, via count, DRC violations, smoothness, crossings, layer balance). CLI `score` command works. Existing prototype A* output has a baseline score on all fixtures.

- [x] **S03: PathFinder Routing Engine** `risk:high` `depends:[S01,S02]`
  > After this: Negotiated congestion router routes all benchmark boards. Scores compared to prototype A* — PathFinder wins or we know why. Via placement is strategic. DRC violations = 0.

- [ ] **S04: Trace Smoother & Via Optimizer** `risk:medium` `depends:[S03]`
  > After this: Raw grid paths post-processed into clean 45°/90° traces. Before/after screenshots show dramatic improvement. DRC still passes after smoothing.

- [ ] **S05: Realtime Tuning Parameters** `risk:medium` `depends:[S03,S04]`
  > After this: User adjusts density/via-preference/roundness sliders, board re-routes within ~1s. Visible on canvas. Parameters stored in settings.

- [ ] **S06: Variant Generation & Preview UI** `risk:low` `depends:[S03,S04]`
  > After this: Route button generates 2-4 variants with different strategies/configs. Score panel shows rankings. Hovering an alternative shows preview overlay on canvas. Auto-applies best.

- [ ] **S07: Benchmark Validation & Strategy Selection** `risk:medium` `depends:[S01,S02,S03,S04,S05,S06]`
  > After this: Automated benchmark suite runs all strategies on all fixtures, produces comparison report (scores + screenshots). Default strategy selected by data. Regression test ensures future changes don't degrade routing quality.

## Boundary Map

### S01 → S03, S07

Produces:
- `cypcb-kicad::pcb_parser` module — `parse_kicad_pcb(path) -> Result<BoardWorld>` extracting footprints, pads, nets, board outline
- `tests/fixtures/benchmark/` — 3+ .kicad_pcb files with metadata (component count, net count, reference score)
- `KicadBenchmark` struct — fixture metadata for automated benchmark runs

Consumes:
- nothing (first slice, parallel with S02)

### S02 → S03, S06, S07

Produces:
- `cypcb-autoroute::scoring` module — `RoutingScore { total_length, via_count, drc_violations, smoothness, crossings, layer_balance, composite }` 
- `score_board(world: &BoardWorld) -> RoutingScore` function
- CLI integration: `cypcb score <file>` prints score breakdown

Consumes:
- nothing (parallel with S01, uses existing BoardWorld + DRC)

### S03 → S04, S05, S06, S07

Produces:
- `cypcb-autoroute::pathfinder_v2` module — PathFinder negotiated congestion router
- `cypcb-autoroute::strategy` trait — `RoutingStrategy { fn route(&self, world, rules, config) -> RoutingResult }` 
- At least 2 strategy implementations (PathFinder, improved A*)
- Updated `route_board()` accepting strategy parameter
- WASM `auto_route()` updated to use best strategy

Consumes from S01:
- Benchmark fixtures for validation during development
Consumes from S02:
- `score_board()` for comparing strategy outputs

### S04 → S05, S06

Produces:
- `cypcb-autoroute::smoother` module — `smooth_routes(segments, rules) -> Vec<RouteSegment>` converting grid paths to clean 45°/90° traces
- `cypcb-autoroute::via_optimizer` — minimize via count while maintaining connectivity

Consumes from S03:
- Raw RoutingResult with grid-aligned segments

### S05 → S06

Produces:
- `AutorouteParams` struct — user-tunable parameters (density, via_preference, roundness, layer_preference)
- Viewer UI: slider panel for tuning parameters  
- Reactive re-routing: parameter change → auto_route() with new params → canvas update

Consumes from S03:
- Strategy `route_board()` that accepts AutorouteParams
Consumes from S04:
- Smoother pipeline for clean output

### S06 → S07

Produces:
- `generate_variants(world, rules, strategies) -> Vec<(RoutingResult, RoutingScore)>` — run multiple strategies
- Viewer UI: score panel with variant list, hover preview overlay on canvas
- Auto-apply best variant, manual selection available

Consumes from S02:
- `score_board()` for ranking variants
Consumes from S03:
- Multiple RoutingStrategy implementations
Consumes from S04:
- Smoother applied to each variant

### S07 (terminal)

Produces:
- Automated benchmark script: route all fixtures × all strategies, output comparison table
- Screenshot comparison artifacts
- Default strategy selection based on empirical results
- Regression test: `cargo test benchmark_regression` ensures scores don't degrade

Consumes from all previous slices.
