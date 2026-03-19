# S04: Trace Smoother & Via Optimizer — Research

**Date:** 2026-03-14

## Summary

S04 must convert raw grid-aligned autorouter output into clean 45°/90° traces and minimize unnecessary vias, while preserving DRC compliance. The current pipeline is: `pathfinder_loop()` → `paths_to_output()` (merges collinear grid steps + converts to Nm RouteSegments) → `apply_routes()` (spawns ECS Trace/Via entities). The smoother inserts between `paths_to_output()` and `apply_routes()`, operating on `Vec<RouteSegment>` (Nm coordinates, not grid cells).

The existing `postprocess.rs` already does collinear merge (staircase → single diagonal if truly collinear). But grid A* paths produce staircase patterns (e.g., right-right-down-right-right-down) that alternate direction each step — these are NOT collinear so they pass through as many tiny segments. The smoother must collapse these staircases into clean 45°/90° segments. Additionally, S03 left 5 DRC violations on led_blink — the research question is whether these are grid-boundary artifacts that smoothing resolves.

Three algorithms needed: (1) **Corner chamfering** — replace 90° grid corners with 45° bends (two segments replace one corner), (2) **Redundant segment removal** — Douglas-Peucker style simplification that removes waypoints where a direct path is possible without DRC violation, (3) **Via optimization** — identify via pairs that can be eliminated by rerouting the sandwiched segment on the original layer.

## Requirements Targeted

| ID | Title | Role | Key Risk |
|----|-------|------|----------|
| R108 | Clean 45°/90° Trace Geometry | **Primary owner** | Smoothing must restrict to exactly 0°/45°/90°/135° angles — no arbitrary angles |
| R109 | Trace Smoothing Post-Processor | **Primary owner** | Must preserve DRC compliance after every smoothing pass |
| R107 | Zero DRC Violations | **Supporting** | Check if smoothing resolves remaining 5 violations from S03 |

## Recommendation

Build the smoother as a new module `crates/cypcb-autoroute/src/smoother.rs` that operates on `Vec<RouteSegment>` (post grid-to-Nm conversion). Build the via optimizer as `crates/cypcb-autoroute/src/via_optimizer.rs` that operates on `RoutingResult` (segments + vias together). Both are invoked from inside `PathFinderStrategy::route()` and `ImprovedAStarStrategy::route()` after `paths_to_output()` but before returning the `RoutingResult`. DRC check runs after smoothing to validate safety.

**Approach for smoothing:**

1. **Staircase-to-diagonal collapse**: Detect alternating H/V grid steps (right-down-right-down pattern) and replace with a single diagonal + orthogonal segment. This is the highest-impact transformation.

2. **Corner chamfering**: For remaining 90° bends, insert a 45° chamfer segment. A→corner→B becomes A→chamfer_start→chamfer_end→B where the chamfer is a short 45° segment.

3. **Segment merging**: After chamfering, re-merge any collinear consecutive segments.

4. **DRC safety check**: After each pass, verify no new clearance violations introduced. If a smoothing move introduces a violation, reject that individual move.

**Approach for via optimization:**

Scan for via pairs (down-via at point A, up-via at point B) with a single segment between them on the alternate layer. If the equivalent segment on the original layer is DRC-clean (no obstacles), eliminate both vias and replace with a direct segment. This is conservative but handles the most common unnecessary-via pattern.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| Segment-to-segment distance | `cypcb_drc::rules::clearance::segment_distance()` | Already proven, handles edge cases, i64 precision |
| DRC checking | `cypcb_drc::run_drc()` | Full rule suite — use as validation gate after smoothing |
| Grid↔Nm conversion | `RoutingGrid::grid_to_nm()`, `nm_to_grid()` | Already used by postprocess.rs |
| Collinear merge | `postprocess::simplify_path()` | Already handles direction-change detection |
| Composite scoring | `scoring::score_board()` | Use for before/after comparison in tests |

## Existing Code and Patterns

- `crates/cypcb-autoroute/src/postprocess.rs` — Current pipeline: `simplify_path()` merges collinear grid steps, `convert_to_route_segments()` converts to Nm+layer, `paths_to_output()` combines both. **The smoother hooks AFTER this, not replacing it.** Grid-level simplification is still useful as a first pass.

- `crates/cypcb-router/src/types.rs` — `RouteSegment { net_id, layer, width, start: Point, end: Point }` is the data structure the smoother operates on. Immutable structs; smoother produces a new `Vec<RouteSegment>`.

- `crates/cypcb-router/src/lib.rs` — `apply_routes()` groups segments by (net_id, layer) into Trace entities. Smoother doesn't need to worry about ECS — it just produces cleaner RouteSegments.

- `crates/cypcb-drc/src/rules/clearance.rs` — `segment_distance()` computes exact minimum distance between two line segments (i64 coordinates). Essential for DRC-safe smoothing — every smoothed segment must maintain min_clearance from all other net segments.

- `crates/cypcb-autoroute/src/scoring.rs` — `compute_smoothness()` already measures angle penalty on bends. The smoother's goal is to make this metric approach 1.0 (all bends at 45° multiples).

- `crates/cypcb-autoroute/src/pathfinder_v2.rs:96-107` — PathFinder calls `paths_to_output()` per net, then collects into `all_segments` / `all_vias`. The smoother call goes after this collection, before the `RoutingResult::complete()` return.

- `crates/cypcb-autoroute/tests/strategy_comparison.rs` — `compare_fixture()` pattern: parse fresh → route → apply_routes → rebuild spatial index → score_board → assert. Smoother tests should follow the same pattern, comparing scores before/after smoothing.

## Constraints

- **RouteSegment is the boundary**: Smoother operates on `Vec<RouteSegment>` (Nm coordinates). It does NOT access the grid. This keeps it decoupled from the routing algorithm.
- **DRC must pass after smoothing**: The boundary map says "DRC still passes after smoothing." This is a hard gate — if smoothing introduces violations, reject those specific moves.
- **45°/90° angles only**: Output segments must have angles that are exact multiples of 45° (0°, 45°, 90°, 135°, 180°, etc.). No arbitrary angles from line simplification algorithms.
- **WASM compatible**: No std::time::Instant, no filesystem. All computation must work in wasm32-unknown-unknown.
- **Performance budget**: Smoothing is post-processing, not in the hot loop. But it still runs in WASM, so should be fast. Target: <100ms for led_blink-scale boards.
- **Segments carry net_id and layer**: Smoother must preserve net_id and layer on all output segments. Width is also preserved.
- **Via optimizer must preserve connectivity**: Every pad-to-pad connection that was routed must remain connected after via removal.
- **Point uses i64 Nm coordinates**: All geometry math uses integer nanometers. No floating point for positions (only for angle calculations).

## Common Pitfalls

- **Staircase detection is not collinear merge** — The existing `simplify_path()` only merges steps in the SAME direction. A staircase (right-down-right-down) alternates directions, so it passes through as many 2-cell segments. The smoother must detect the overall staircase pattern and replace it with diagonal + orthogonal.

- **Smoothing can introduce clearance violations** — Moving a trace segment to a new position (e.g., diagonal shortcut) may bring it closer to another net's trace or pad. Must check clearance against ALL other-net segments/pads in the neighborhood before committing a move.

- **Via removal can break connectivity on complex nets** — A net with 3+ pads may have branching paths. Simply removing a via pair could disconnect a branch. Must verify connectivity is preserved (all pads still reachable through remaining segments).

- **45° angle enforcement** — Douglas-Peucker or Ramer simplification produces arbitrary angles. Must use a constrained version that only generates 45°-multiple segments, or post-filter results.

- **Integer overflow in angle calculations** — `atan2(dy, dx)` requires f64 for the angle computation, but input coordinates are i64 Nm. Cast carefully — intermediate products can overflow i64 (use i128 for cross products).

- **Order of operations matters** — Must smooth per-net (not globally) to avoid accidentally merging segments from different nets. Via optimization is per-net too.

## Open Risks

- **DRC violation source ambiguity** — The 5 remaining DRC violations on led_blink from S03 may be grid-boundary artifacts (fixable by smoothing) OR fundamental routing conflicts (not fixable without re-routing). Need to diagnose which before claiming smoothing resolves them.

- **Staircase pattern detection heuristic** — Real grid paths may not be clean alternating H/V steps. Paths through congested areas may have irregular step patterns that don't fit simple staircase templates. May need iterative local optimization rather than pattern matching.

- **Via optimizer scope** — The boundary map says "minimize via count while maintaining connectivity." On led_blink, PathFinder produces 0 vias (so nothing to optimize). The via optimizer may only matter for larger boards or ImprovedAStar output (which produces 2 vias on led_blink). Testing may be limited.

- **Smoothing + DRC interaction** — Running full `run_drc()` after every individual smoothing move is too expensive. Need a lightweight local clearance check (segment_distance against nearby segments) for per-move validation, with full DRC as final gate.

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| PCB routing / EDA | `l3wi/claude-eda@eda-pcb` | Available (57 installs) — general EDA knowledge, not algorithm-specific |
| tscircuit | `tscircuit/skill@tscircuit` | Available (176 installs) — different PCB framework, not directly applicable |
| KiCad file format | `o2scale/electronics-agent-kit@kicad-file-format` | Available (28 installs) — already have parser, not needed |

No directly applicable skills for trace smoothing algorithms. This is custom algorithmic work using existing project geometry primitives.

## Architecture: Where Smoother Fits in Pipeline

```
PathFinder/ImprovedAStar route()
  └── pathfinder_loop() / orchestrator
        └── per-net: find_path() → Vec<GridNode>
  └── per-net: paths_to_output() → (Vec<RouteSegment>, Vec<ViaPlacement>)
  └── collect all_segments, all_vias
  └── ★ NEW: smooth_routes(all_segments, rules) → Vec<RouteSegment>     ← smoother.rs
  └── ★ NEW: optimize_vias(segments, vias, rules) → (Vec<RouteSegment>, Vec<ViaPlacement>)  ← via_optimizer.rs
  └── RoutingResult::complete(smoothed_segments, optimized_vias)
```

Both strategies call smoother/via_optimizer after collecting their output. This avoids duplicating the smoothing logic.

## Key Data Structures

```rust
// Input to smoother (from postprocess.rs):
RouteSegment { net_id: NetId, layer: Layer, width: Nm, start: Point, end: Point }
ViaPlacement { net_id: NetId, position: Point, drill: Nm, start_layer: Layer, end_layer: Layer }

// Smoother operates per-net, per-layer:
// Group segments by (net_id, layer), smooth each group independently,
// reassemble into flat Vec<RouteSegment>.

// Via optimizer operates per-net:
// Group vias by net_id, find eliminable via pairs, remove them
// and replace intermediate segment with same-layer segment.
```

## Algorithm Details

### 1. Staircase-to-Diagonal Collapse

Detect sequences of short segments that form a staircase pattern:
- Segments alternate between two directions (e.g., horizontal and vertical)
- Each segment is roughly the same length (within 2× tolerance)
- Overall trajectory is diagonal

Replace with: one diagonal segment + one orthogonal segment (or just diagonal if it reaches the endpoint).

Example: (0,0)→(1,0)→(1,1)→(2,1)→(2,2) becomes (0,0)→(2,2) — a single 45° diagonal.

### 2. Corner Chamfering

For each 90° corner (segment A ends at point P, segment B starts at P, A⊥B):
- Create a 45° chamfer by cutting the corner
- Chamfer length = min(len_A, len_B, configurable_max) / 2
- Result: A shortened → chamfer_start, 45° segment → chamfer_end, B shortened from chamfer_end

### 3. Segment Merging

After smoothing passes, merge any consecutive segments that are collinear (same direction vector).

### 4. DRC Safety

Per-move: check `segment_distance()` between new segment and all same-layer, different-net segments within bounding box + clearance margin.
Final: full `run_drc()` on the smoothed result.

## Sources

- PCB trace smoothing removes grid artifacts via angle optimization and redundant segment removal (source: [Google Search on PCB smoothing algorithms])
- Via minimization uses layer reassignment and via-pair elimination while preserving connectivity (source: [Google Search on via optimization])
- S03 Summary: PathFinder produces 5 DRC violations on led_blink, 0 vias; grid-aligned paths need smoothing (source: S03-SUMMARY.md forward intelligence)
- Existing `compute_smoothness()` measures bend angle deviation from 45° multiples — smoothing target is to maximize this metric (source: scoring.rs)
