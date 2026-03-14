# S02: Custom Autorouter Core — Research

**Date:** 2026-03-13

## Summary

S02 replaces the FreeRouting JAR wrapper (`cypcb-router`) with a custom A*-based autorouter that runs natively and compiles to WASM. The codebase is well-prepared for this: `cypcb-rules` already provides the `RoutingRuleSet` trait (object-safe, with per-net constraints, via costs, layer costs, and inter-net clearance), `cypcb-world` has an R*-tree spatial index with layer-aware queries, and the existing `RoutingResult`/`RouteSegment`/`ViaPlacement` types in `cypcb-router` define the exact output contract the renderer and DRC already consume.

The architecture should be a new `cypcb-autoroute` crate (not modifying `cypcb-router`, which keeps the FreeRouting fallback). The autorouter needs three major subsystems: (1) a grid/graph representation of the routing space, (2) an A* pathfinder with PCB-aware cost function and multi-layer support, and (3) a net ordering + rip-up/reroute loop to achieve high completion rates. The `pathfinding` crate (v4.14, WASM-compatible, pure Rust) provides a production-quality A* implementation that avoids hand-rolling priority queue logic.

Key risk: achieving routing quality comparable to FreeRouting on real boards. This is retired by routing the `blink.cypcb` reference board (8 components, 7 nets) and comparing output quality metrics (total length, via count, completion rate). The 500-component benchmark target is S08 scope — S02 proves correctness and quality on small/medium boards.

## Recommendation

**Build a grid-based A* autorouter in a new `cypcb-autoroute` crate** using the `pathfinding` crate for the A* search and `rstar` (already a workspace dependency) for obstacle queries. Architecture:

1. **Grid model**: Uniform grid at configurable resolution (default: min_clearance/2, typically ~63µm for JLCPCB). Each cell stores occupancy per layer. Grid is built from board size, component pads, keepout zones, and existing locked traces.

2. **A* pathfinder**: Each node is `(grid_x, grid_y, layer_index)`. Neighbors are 8-directional on same layer + layer transitions at via-legal positions. Cost function incorporates: Manhattan distance heuristic, trace width clearance from `RoutingRuleSet`, via cost from `RoutingRuleSet::via_cost()`, layer preference from `RoutingRuleSet::layer_change_cost()`, 45° routing preference (penalize non-45° moves), and clearance from existing routes via spatial index.

3. **Net ordering + rip-up/reroute**: Route nets in order of decreasing criticality (shortest ratsnest first, power/ground last). On failure, rip up the blocking net with lowest priority, reroute both. Limit rip-up iterations (configurable, default 3 passes).

4. **Output**: Convert grid paths to `RouteSegment`/`ViaPlacement` sequences, simplify collinear segments, produce `RoutingResult` compatible with existing `apply_routes()`.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| A* priority queue + search | `pathfinding` crate v4.14 | Production-quality, WASM-compatible, handles the fiddly open/closed set logic. `pathfinding::directed::astar::astar()` takes successors + heuristic + success closures — maps directly to our grid model. |
| Spatial obstacle queries | `rstar` crate (already in workspace) | R*-tree with `locate_in_envelope_intersecting` is O(log n) for clearance checks during routing. Already used by `SpatialIndex`. |
| Routing rule constraints | `cypcb-rules::RoutingRuleSet` trait | Object-safe trait already defines `constraints_for_net()`, `via_cost()`, `layer_change_cost()`, `clearance_between()`. S01 built this specifically for autorouter integration. |
| Output types | `cypcb-router::types` | `RoutingResult`, `RouteSegment`, `ViaPlacement`, `RoutingMetrics` already defined and consumed by renderer + `apply_routes()`. |
| ECS board model | `cypcb-world::BoardWorld` | Component queries, net registry, spatial index all ready. |

## Existing Code and Patterns

- `crates/cypcb-router/src/lib.rs` — `apply_routes()` function: clears old autorouted traces, spawns new `Trace`/`Via` entities from `RoutingResult`. Our autorouter output feeds directly into this. The `preserve_locked_traces()` helper identifies traces the router must avoid.
- `crates/cypcb-router/src/types.rs` — `RouteSegment`, `ViaPlacement`, `RoutingResult`, `RoutingMetrics`, `calculate_metrics()`. These are our output contract. The autorouter must produce `Vec<RouteSegment>` and `Vec<ViaPlacement>`.
- `crates/cypcb-rules/src/routing_rules.rs` — `RoutingRuleSet` trait with 5 methods. The A* cost function calls these. `PresetRuleSet` in `presets/mod.rs` is the default implementation with per-net overrides.
- `crates/cypcb-rules/src/constraints.rs` — `DesignConstraints` with 35 fields. Key fields for routing: `min_clearance`, `min_trace_width`, `min_via_drill`, `min_via_annular_ring`, `blind_vias_allowed`, `buried_vias_allowed`.
- `crates/cypcb-world/src/spatial.rs` — `SpatialIndex` wrapping `RTree<SpatialEntry>`. Supports `query_region_on_layers()` and `query_region_entries()` with layer masks. Rebuild from entity positions.
- `crates/cypcb-world/src/components/trace.rs` — `Trace`, `TraceSegment`, `Via`, `TraceSource` types. The autorouter creates traces with `TraceSource::Autorouted`.
- `crates/cypcb-world/src/components/electrical.rs` — `NetId(u32)`, `NetConnections`, `PinConnection`. Net connections map pins to nets — the autorouter needs this to build the ratsnest (which pin pairs to route).
- `crates/cypcb-world/src/components/physical.rs` — `Layer` enum with `to_copper_mask()` for spatial index queries. `Pad` type with `is_smd()` / `is_through_hole()` and layer mask.
- `crates/cypcb-world/src/components/zone.rs` — `Zone` with `ZoneKind::Keepout` — these are obstacles the router must respect.
- `crates/cypcb-world/src/world.rs` — `BoardWorld` with `board_info()` returning `(BoardSize, LayerStack)`. `intern_net()` / `net_name()` for net registry. `query_region_on_layers()` for spatial queries.
- `crates/cypcb-router/src/dsn.rs` — DSN export handles `Layer` → DSN name mapping. Shows the established pattern for layer name conversion.
- `crates/cypcb-drc/` — DRC engine with clearance, edge clearance, annular ring, trace width, drill size, connectivity rules. The autorouter should produce DRC-clean output; we can run DRC post-routing as verification.
- `examples/blink.cypcb` — Primary reference board: NE555 blink circuit, 8 components, 7 nets, 60×40mm 2-layer board. Good complexity for initial validation.
- `examples/routing-test.cypcb` — Minimal test: 3 components, 3 nets, 40×25mm. Useful for unit tests.

## Constraints

- **WASM compatibility required**: The autorouter must compile to `wasm32-unknown-unknown`. No `std::thread`, no `std::fs`, no system-dependent dependencies. The `pathfinding` crate is pure Rust and WASM-compatible. Must use `getrandom` with `js` feature if any randomization needed.
- **Integer nanometer coordinates**: All geometry in `Nm(i64)`. Grid coordinates must map cleanly to/from Nm. Grid resolution chosen to divide cleanly into common trace widths.
- **`RoutingRuleSet` is the constraint interface**: The autorouter takes `&dyn RoutingRuleSet`, not raw `DesignConstraints`. This keeps the interface abstract and testable.
- **Output must be `RoutingResult`**: Existing `apply_routes()` in `cypcb-router` consumes this type. The renderer and export pipeline already handle `Trace`/`Via` entities spawned from it.
- **`cypcb-autoroute` is a new crate**: It depends on `cypcb-core`, `cypcb-world`, `cypcb-rules`, and `pathfinding`. It does NOT depend on `cypcb-router` (avoiding circular deps). The `RoutingResult` type should be re-exported or moved to a shared location — or `cypcb-autoroute` can depend on `cypcb-router` since `cypcb-router` is a leaf crate.
- **Layer index convention**: `cypcb-rules` uses `u8` layer indices (0=top, N-1=bottom). `cypcb-world` uses `Layer` enum. The autorouter's internal grid uses `u8` indices and maps to/from `Layer` at the boundary.
- **No `cypcb-world` modification needed**: The autorouter reads from `BoardWorld` (components, pads, nets, zones, board size, existing traces) and produces `RoutingResult`. No new ECS components required.
- **Performance target**: Route a 500-component board in <30s is the M002 goal, but that's S08 optimization scope. S02 target: route `blink.cypcb` (8 components) correctly with quality comparable to FreeRouting. Grid resolution and search space must be manageable — adaptive grid or coarse-to-fine might be needed for large boards.

## Common Pitfalls

- **Grid resolution too fine → memory explosion** — A 100×100mm board at 1µm resolution = 10^10 cells × layers = hundreds of GB. Must use a sensible resolution. At ~63µm (half of JLCPCB 5mil clearance), a 100×100mm board = ~1600×1600 grid = ~2.5M cells per layer. For 2 layers that's 5M cells — manageable. For 6 layers, 15M — still fine but watch memory.
- **Diagonal routing without 45° preference** — PCB traces should follow 0°, 45°, 90° angles. The A* cost function must penalize other angles. Use 8-directional neighbors (N, NE, E, SE, S, SW, W, NW) on the grid with √2 cost for diagonals.
- **Ignoring pad entry/exit angles** — Traces should enter pads along the pad's axis, not at arbitrary angles. The grid start/end nodes should be positioned at pad centers, and the first/last segment should align with pad orientation.
- **Net ordering matters enormously** — Routing order determines whether congested areas get routed successfully. Short nets first, then longer nets. Power/ground nets last (they're forgiving about path length). Critical nets (high-speed, differential) first within each length tier.
- **Collinear segment simplification** — Raw grid paths produce many short segments along straight lines. Must merge collinear segments into single `TraceSegment` instances for clean output.
- **Via placement at grid intersections only** — Vias should only be placed where the grid allows, respecting annular ring and drill clearances. The via's footprint occupies multiple grid cells.
- **Rip-up loop not terminating** — Must cap rip-up iterations and gracefully report partial results. The `RoutingResult::partial()` constructor exists for this.
- **Coordinate system mismatch** — Grid (0,0) maps to board (0,0). Verify the Y-axis convention matches (board uses bottom-left origin, Y-up).

## Open Risks

- **Quality vs. FreeRouting**: FreeRouting uses a sophisticated shape-based router with negotiated congestion. Our grid-based A* may produce longer traces, more vias, or worse routing aesthetics. Mitigation: path smoothing pass after A*, via minimization pass, and comparison benchmarks. If quality is unacceptable, we can add negotiated congestion (PathFinder algorithm) in a follow-up.
- **WASM autorouter performance**: A* on a 1600×1600×2 grid for 50+ nets could be slow in WASM. The `pathfinding` crate is pure Rust and should compile efficiently, but WASM is ~2-3x slower than native. Mitigation: use Web Worker to avoid blocking UI (integration point for S03/S08), consider coarse-to-fine routing, benchmark early.
- **Multi-layer routing complexity**: Inner layer routing, blind/buried vias, and layer assignment are significantly harder than 2-layer routing. Start with 2-layer, then extend to 4+ layers. The grid model supports arbitrary layer count, but the cost function and via rules need careful tuning per layer pair.
- **Large board memory**: For boards >100mm with fine grid resolution, memory could be an issue in WASM (limited to ~4GB, practically ~2GB). Adaptive grid resolution or sparse representation may be needed for S08 performance work.
- **Copper pour interaction**: Copper pour zones interact with routing — traces through pour zones need thermal relief. The autorouter should treat copper pour zones as obstacles with special clearance rules. This can be deferred if the current DSL doesn't define pour zones in practice.

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| PCB EDA / autorouting | `l3wi/claude-eda@eda-pcb` | available (56 installs) — PCB design guidance |
| tscircuit | `tscircuit/skill@tscircuit` | available (156 installs) — different EDA framework, not directly relevant |
| Rust backend patterns | `windmill-labs/windmill@rust-backend` | available (83 installs) — general Rust, not PCB-specific |
| KiCad file format | `o2scale/electronics-agent-kit@kicad-file-format` | available (26 installs) — file format only, not routing |

None of these skills are directly relevant to implementing an A* PCB autorouter. The `eda-pcb` skill is closest but focuses on PCB design guidance rather than routing algorithm implementation. No skill installation recommended.

## Sources

- Grid-based A* with PCB-specific cost functions is the standard approach for custom autorouters (source: IEEE papers on PCB routing, datacamp A* overview)
- `pathfinding` crate v4.14.0 provides `astar()` function with successors/heuristic/success API, pure Rust, WASM-compatible (source: [crates.io](https://crates.io/crates/pathfinding), [docs.rs](https://docs.rs/pathfinding))
- Rip-up/reroute with 3-5 iterations achieves 95%+ completion on typical boards (source: PCB autorouter literature, FreeRouting documentation)
- `rstar` R*-tree for O(log n) spatial queries during clearance checking — already in workspace (source: existing `cypcb-world/spatial.rs`)
- S01 task summaries confirm: `RoutingRuleSet` trait (5 methods), 10 manufacturer presets, IPC clearance tables, signal class constraints — all ready for autorouter consumption
- `blink.cypcb` (NE555 circuit, 8 components, 7 nets) is the primary reference board for quality comparison
- FreeRouting JAR wrapper in `cypcb-router` provides baseline quality metrics for comparison
