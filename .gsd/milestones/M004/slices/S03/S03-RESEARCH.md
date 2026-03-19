# S03: PathFinder Routing Engine — Research

**Date:** 2026-03-14

## Summary

S03 replaces the sequential A*-only autorouter with a multi-strategy routing engine: a PathFinder negotiated congestion router and an improved A* with better heuristics. The existing codebase provides solid infrastructure — `RoutingGrid`, `RoutingCost`, `postprocess.rs`, and `scoring.rs` are all reusable. The key work is: (1) adding a `RoutingStrategy` trait abstraction, (2) implementing PathFinder's iterative negotiated congestion on top of the existing grid, (3) enhancing the A* router with congestion-aware cost and better net ordering, and (4) wiring both through scoring for comparison.

The current A* router (~4300 LOC) produces DRC violations (50 on blink.cypcb) and poor via placement. PathFinder's core insight — route all nets with shared resources, iteratively increase costs on congested cells — maps cleanly onto the existing `RoutingGrid` with per-cell congestion tracking. The `pathfinding` crate's `astar()` can be reused as the inner path-finder since cost functions are closures evaluated at search time, meaning dynamic congestion costs work without library changes.

Main risk is PathFinder convergence within WASM performance budget. The benchmark boards create 300K-2M cells per layer at standard resolution. PathFinder needs 20-50 iterations, each re-routing some/all nets via A*. For led_blink (7 nets, 300K cells), this is ~7 A* searches × 30 iterations = 210 path searches — tractable. For stm32_breakout (40 nets, 1.2M cells), ~40 × 50 = 2000 searches — may need the partial-reroute optimization (only re-route nets passing through congested cells). multi_ic (94 nets, 500K cells with adaptive grid) will be the stress test.

## Recommendation

Implement a `RoutingStrategy` trait with two concrete strategies:

1. **PathFinderStrategy** — Negotiated congestion: route all nets, track per-cell present/history congestion costs, iteratively re-route nets through overused resources until convergence or iteration cap (50). Use existing `pathfinding::astar()` with augmented cost closures. Only re-route nets touching congested cells (VPR optimization, not full re-route every iteration).

2. **ImprovedAStarStrategy** — Wraps the existing A* orchestrator with: (a) congestion-aware cost that penalizes routing through areas used by other nets, (b) better net ordering (considering fanout and net length together), (c) increased rip-up iterations (10→20), (d) multi-victim rip-up (try up to 3 victims per failed connection).

`route_board()` gets an optional `strategy` parameter (defaults to PathFinder). WASM `auto_route()` uses the best strategy. Both produce `RoutingResult` consumed by the existing `apply_routes()` pipeline.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| A* search | `pathfinding` crate v4 `astar()` | Already proven in codebase, integer costs with float conversion, optimal with admissible heuristic |
| Grid infrastructure | `RoutingGrid` in grid.rs | 820 LOC of well-tested cell management, coordinate conversion, obstacle marking, route tracking |
| Cost function | `RoutingCost` in cost.rs | Octile distance heuristic, via costs, layer preferences — extend don't replace |
| Path simplification | `simplify_path()` + `convert_to_route_segments()` | Collinear merge, via detection, coordinate conversion all working |
| Quality scoring | `score_board()` in scoring.rs | 7-metric composite with DRC integration — use as-is for strategy comparison |
| Design rules | `RoutingRuleSet` trait + `PresetRuleSet` | JLCPCB presets, via costs, clearance values all correct |
| Net extraction | `extract_ratsnest()` in orchestrator.rs | Pad target computation, spanning tree, net ordering all reusable |

## Existing Code and Patterns

- `crates/cypcb-autoroute/src/grid.rs` (819 LOC) — `RoutingGrid` with per-layer flat `Vec<u8>` occupancy + `Vec<u32>` net ownership. Has `mark_route()`, `clear_route()`, `is_free()`, `net_at()`. **Reuse entirely.** Must add `present_cost` and `history_cost` parallel arrays for PathFinder congestion tracking. Grid also has `make_test_grid()` for unit tests.

- `crates/cypcb-autoroute/src/pathfinder.rs` (457 LOC) — `find_path()` / `find_path_with_zones()` using `pathfinding::astar()`. Key pattern: successors closure captures grid reference, checks `is_free()` / `in_pad_zone()`, calls `cost.neighbor_cost()`. **This is the inner search kernel** — PathFinder calls it repeatedly with different congestion cost overlays. `PadZone` mechanism already handles net-own-pad reachability.

- `crates/cypcb-autoroute/src/orchestrator.rs` (1015 LOC) — `extract_ratsnest()`, `order_nets()`, `route_all_nets()`, `attempt_ripup_reroute()`. **Reuse `extract_ratsnest()` and `order_nets()` directly.** Replace `route_all_nets()` with strategy-specific orchestration. The rip-up mechanism in `attempt_ripup_reroute()` finds blocking nets via `find_blocking_net()` (line sampling along path) — useful for the improved A* strategy.

- `crates/cypcb-autoroute/src/cost.rs` (247 LOC) — `RoutingCost` with `neighbor_cost()` and `heuristic()`. The cost structure uses `rules.via_cost()` and `rules.layer_change_cost()`. **For PathFinder, extend this** to accept congestion cost maps and include `(base_cost + history) * (1 + present_overuse)` in `neighbor_cost()`. The `heuristic()` must remain admissible (only use base costs, not congestion).

- `crates/cypcb-autoroute/src/postprocess.rs` (496 LOC) — `simplify_path()` + `convert_to_route_segments()` + `paths_to_output()`. **Reuse unchanged** — both strategies produce `Vec<Vec<GridNode>>` paths per net, which feed into the same postprocessing pipeline.

- `crates/cypcb-autoroute/src/scoring.rs` (1049 LOC) — `score_board()` with `RoutingScore` (7 metrics + composite). **Reuse as the arbiter** — after both strategies route, score both results and compare. `DRC violations × 1000` penalty means PathFinder's zero-violation target directly rewards it in scoring.

- `crates/cypcb-autoroute/src/lib.rs` (195 LOC) — `route_board()` entry point and `AutorouteConfig`. **Extend `route_board()` to accept an optional strategy parameter.** Preserve backward compatibility by defaulting to the best strategy.

- `crates/cypcb-render/src/lib.rs:333` — WASM `auto_route()` calls `route_board()` with default config. **Update to use best strategy** without changing the JSON return contract.

- `crates/cypcb-router/src/types.rs` — `RoutingResult`, `RouteSegment`, `ViaPlacement` types. **Untouched** — both strategies produce the same output type.

## Constraints

- **WASM compilation required**: `cypcb-autoroute` compiles for `wasm32-unknown-unknown` — any new dependency must support WASM. The `pathfinding` crate already works. No `std::time::Instant` in WASM (iteration count instead of wall-clock for convergence).
- **`pathfinding` crate uses integer costs (u64)**: Costs are converted via `float_to_int_cost(f * 1000.0)`. Congestion costs must be scaled to match this convention.
- **`route_board()` takes `&mut BoardWorld`**: bevy_ecs query API requires `&mut`. Strategy trait methods must take `&mut BoardWorld`.
- **Grid uses `u16` for GridNode coordinates**: Max 65535 cells per axis. Current boards fit within this (largest grid: ~1574 cells wide). No change needed.
- **`RoutingRuleSet` trait is in `cypcb-rules` (leaf crate)**: Cannot add strategy-related methods. Strategy must be in `cypcb-autoroute`.
- **`AutorouteConfig` is the public config struct**: Strategy selection should be added here, not as a separate parameter, to minimize API surface changes.
- **Workspace-level `opt-level = 'z'` for WASM, but `cypcb-autoroute` has per-crate `opt-level = 3`**: Autorouter performance benefits from optimization. Confirmed not a WASM dependency (autorouter runs in WASM but crate-level opt applies only to native; WASM profile uses workspace level).
- **`RoutingGrid::from_board()` returns `Option<Self>`**: Grid construction can fail if no board entity exists. Strategy must handle this.
- **Net ownership tracking via `net_map: Vec<Vec<u32>>`**: PathFinder needs to know which cells are used by which nets to detect congestion. This already exists.
- **Adaptive grid resolution doubles cell size for boards >80mm**: multi_ic (100×80mm) triggers this. Grid becomes ~787×629 = ~500K cells/layer instead of ~2M.

## Common Pitfalls

- **Heuristic admissibility with congestion costs** — If the A* heuristic includes congestion costs, it may overestimate and break optimality. The heuristic must use only base costs (octile distance + min via cost). Congestion is in `neighbor_cost()` only.
- **PathFinder oscillation** — Two nets can oscillate, each pushing the other off a resource forever. Prevent this with history cost accumulation: `history[cell] += alpha` every iteration a cell is overused. History never decreases, so repeatedly contested cells get exponentially expensive, forcing one net to find an alternative.
- **Grid cell coordinate overflow in congestion maps** — `present_cost` and `history_cost` are `Vec<f64>` per layer. For stm32_breakout at 1.2M cells × 2 layers × 8 bytes = ~19MB. For multi_ic (adaptive) at 500K × 4 layers × 8 = ~16MB. Acceptable for WASM (typical 256MB heap).
- **Re-routing ALL nets every iteration is slow** — VPR optimization: only re-route nets that pass through overused cells. After each iteration, collect the set of congested cells, then the set of nets using those cells. Only those nets get ripped up and re-routed. This can reduce per-iteration work by 5-10x on boards where congestion is localized.
- **`clear_route()` is O(width × height × layers)** — It scans the entire `net_map` for the net_id. For frequent rip-ups in PathFinder iterations, this is expensive. Consider maintaining a per-net cell index (`HashMap<u32, Vec<(u32, u32, u8)>>`) for O(path_length) rip-up.
- **DRC check after every PathFinder iteration** — Too expensive. Only run DRC on the final converged result. During iteration, use congestion (cell overuse count) as a proxy for violations.
- **Via cost multiplier interaction with congestion** — If via cost is too high relative to congestion cost, PathFinder may never place vias even when layer switching would resolve congestion. Tune: congestion penalty should dominate via cost after ~10 iterations.
- **`make_test_grid()` is `pub(crate)` and `#[cfg(test)]`** — Need a public constructor for strategy unit tests. Either make it `pub` or add a `RoutingGrid::new_empty()` constructor.

## Open Risks

- **PathFinder convergence on stm32_breakout (40 nets, 1.2M cells)**: May not converge in 50 iterations if nets are tightly packed. Mitigation: cap iterations and return partial result with score. The improved A* serves as fallback.
- **WASM routing time for multi_ic (94 nets, 4-layer, ~500K cells)**: Each PathFinder iteration routes ~94 nets × A* search. If each search takes 10ms, one iteration = ~1s. 50 iterations = ~50s. This violates the <3s target for complex boards. Mitigation: partial re-route optimization, coarser grid for initial iterations, or accept >3s for complex boards in this slice (S05 adds incremental re-routing for interactivity).
- **Via placement quality**: PathFinder naturally places vias where layer transitions reduce congestion. However, it may produce redundant vias (transit to another layer and back within a few cells). Post-processing via optimization is deferred to S04 but we need to verify PathFinder doesn't produce pathological via patterns.
- **Zero DRC violations target (R107)**: The current A* produces 50 DRC violations on blink.cypcb. PathFinder's congestion mechanism should reduce crossings (the main DRC issue) but zero violations depends on grid resolution vs clearance. If grid cells are too coarse, two traces may pass on adjacent cells that violate clearance in real coordinates. May need a DRC validation pass after converting from grid to real coordinates.
- **Strategy comparison fairness**: PathFinder runs for many iterations while improved A* is a single pass. To compare fairly, both must be measured on routing quality (score_board) and time. Time comparison informs S05 (realtime tuning) constraints.
- **Integration with S04 smoother**: Both strategies produce grid-aligned paths. S04 must smooth them regardless of strategy. PathFinder's grid paths may have different characteristics (more layer changes, longer detours around congestion) than A* paths — smoother must handle both.

## Architecture Notes

### RoutingStrategy Trait

```rust
pub trait RoutingStrategy {
    fn name(&self) -> &str;
    fn route(
        &self,
        world: &mut BoardWorld,
        library: &FootprintLibrary,
        rules: &dyn RoutingRuleSet,
        config: &AutorouteConfig,
    ) -> RoutingResult;
}
```

### CongestionGrid (PathFinder extension to RoutingGrid)

PathFinder needs per-cell congestion tracking beyond what `RoutingGrid` provides. Two options:

1. **Extend `RoutingGrid`** with `present_cost: Vec<Vec<f64>>` and `history_cost: Vec<Vec<f64>>` — simpler but couples congestion to all grid users.
2. **Separate `CongestionMap`** wrapping a reference to `RoutingGrid` — cleaner separation, PathFinder owns its congestion data. **Recommended.**

### PathFinder Iteration Loop

```
1. Build grid from board (reuse RoutingGrid::from_board)
2. Extract ratsnest (reuse extract_ratsnest)
3. Initialize CongestionMap (all zeros)
4. For iteration 1..max_iterations:
   a. For each net (ordered by priority):
      - Rip up previous iteration's route for this net
      - Route with congestion-aware cost: cost = base * (1 + history[cell]) * (1 + alpha * present[cell])
      - Record route on grid + mark present occupancy
   b. Compute overuse per cell (count of nets using each cell - capacity)
   c. Update history: for each overused cell, history[cell] += beta
   d. If no overused cells: converged → break
   e. (Optimization) Identify nets through overused cells for next iteration
5. Convert final routes to RoutingResult
```

### File Plan

- `crates/cypcb-autoroute/src/strategy.rs` — `RoutingStrategy` trait + enum for selection
- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — PathFinder implementation (~400-600 LOC)
- `crates/cypcb-autoroute/src/congestion.rs` — `CongestionMap` for per-cell cost tracking (~100 LOC)
- `crates/cypcb-autoroute/src/astar_improved.rs` — Improved A* strategy wrapping existing orchestrator (~200 LOC)
- `crates/cypcb-autoroute/src/lib.rs` — Updated `route_board()` with strategy parameter
- `crates/cypcb-autoroute/tests/strategy_comparison.rs` — Benchmark comparison tests
- `crates/cypcb-render/src/lib.rs` — Updated `auto_route()` to use best strategy

### Grid Dimension Estimates

| Board | Dimensions | Standard Grid | Adaptive Grid | Cells/Layer |
|-------|-----------|--------------|---------------|-------------|
| led_blink | 40×30mm | 629×472 | N/A (<80mm) | 297K |
| stm32_breakout | 75×65mm | 1181×1023 | N/A (<80mm) | 1.2M |
| multi_ic | 100×80mm | 1574×1259 | 787×629 (2x) | 495K |

## Requirements Targeted

| ID | Role | Risk | Key Concern |
|----|------|------|-------------|
| R104 | primary | medium | RoutingStrategy trait + 2 implementations |
| R105 | primary | high | PathFinder convergence on complex boards |
| R106 | primary | medium | Via placement governed by layer-switch cost in congestion negotiation |
| R107 | primary | high | Zero DRC violations requires congestion → zero overuse + grid-to-real clearance safety |
| R111 | supporting | medium | Performance budget constrains PathFinder iteration count |
| R116 | supporting | low | Strategy comparison via score_board() enables empirical selection in S07 |

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| PCB routing / EDA | `l3wi/claude-eda@eda-pcb` (57 installs) | available — PCB design guidance, not algorithmic routing. Low relevance. |
| tscircuit | `tscircuit/skill@tscircuit` (176 installs) | available — different tool (tscircuit), not relevant to our Rust autorouter. |
| KiCad file format | `o2scale/electronics-agent-kit@kicad-file-format` (28 installs) | available — already handled in S01, not needed for S03. |
| Rust pathfinding | none found | N/A — `pathfinding` crate docs are straightforward, no skill needed. |

No skills recommended for installation — this is pure algorithmic work on existing Rust infrastructure.

## Sources

- PathFinder algorithm: negotiated congestion with history+present cost, iterative rip-up/reroute of all nets (source: [UFL/UToronto academic papers on FPGA routing](https://vertexaisearch.cloud.google.com))
- VPR optimization: only re-route nets through congested cells, not all nets every iteration (source: [VPR documentation / Versatile Place and Route](https://vertexaisearch.cloud.google.com))
- FreeRouting uses DSN/SES format with adjustable costs and post-routing via optimization passes (source: [autorouting.com](https://autorouting.com))
- `pathfinding` crate v4: `astar()` with integer costs, successor closures — already working in the codebase
- Existing codebase: `cypcb-autoroute` 4278 LOC, all tests passing (76 unit + 9 integration), scoring produces 7-metric composite with DRC integration
