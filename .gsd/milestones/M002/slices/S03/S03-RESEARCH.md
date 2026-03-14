# S03: Renderer Upgrade & Manual Trace Editing — Research

**Date:** 2026-03-13

## Summary

S03 bridges the autorouter (S02) output to the visual frontend and adds the most complex user-facing interaction in the project: KiCad-style click-to-route manual trace editing with live DRC feedback. The existing code has solid foundations — Canvas 2D renderer already draws traces/vias/pads with proper layer ordering, the viewport/coordinate system is clean (nanometers ↔ screen pixels), and the interaction module handles zoom/pan/select. But there are significant gaps: no trace hit-testing, no drag state machine, no WASM bridge for manual trace creation/deletion, no per-segment trace identity for editing, no live DRC feedback loop, and the DRC engine itself doesn't yet check trace-to-trace or trace-to-pad clearances (spatial index only indexes components, not traces — documented in S02 T05).

The primary risk is the interaction complexity: building a responsive click-drag routing UI in Canvas 2D with real-time DRC requires a carefully designed state machine, efficient hit-testing (spatial queries against traces/vias), and a low-latency DRC feedback loop. KiCad's interactive router (PNS — Push 'N' Shove) is ~20k lines of C++. We don't need that sophistication — a simple click-pad-to-start, drag-to-route, click-to-anchor pattern is sufficient for v2.0 — but even the simplified version requires coordinated changes across 4 layers: interaction state machine (TS), renderer overlays (TS), WASM bridge API additions (Rust), and DRC engine extensions (Rust).

## Recommendation

Split the work into three phases:

1. **Renderer upgrade** — Render autorouter output with proper trace widths, net-colored traces, via rendering, trace selection/highlighting, and net highlighting. Add hit-testing for traces and vias so clicks can identify them. This is purely visual and doesn't change the WASM API.

2. **WASM bridge extensions** — Add methods to `PcbEngine` for manual trace operations: `add_trace_segment()`, `remove_trace()`, `get_trace_at_point()`, `run_drc_incremental()`. Extend the snapshot to include per-trace IDs so the frontend can reference specific traces for editing.

3. **Manual routing interaction** — Build the state machine: idle → pad-clicked (start routing) → dragging (preview trace) → click-to-anchor (place segment) → click-pad-to-finish. Include: 45°/90° angle snapping, layer switching via keyboard, live DRC violation overlay during drag, undo via Escape.

DRC extension (adding trace entities to spatial index) is a prerequisite for live feedback. Without it, the user gets no clearance feedback during manual routing — which defeats the purpose.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| Trace/via hit-testing | Extend `SpatialIndex` with trace bounding boxes | R*-tree already handles O(log n) spatial queries; just need to index trace segments too |
| Angle snapping | Standard 45°/90° snap math | Two-line formula: round angle to nearest multiple of 45°, project to snapped endpoint |
| Net-coloring | Hash net name → HSL color | Deterministic, visually distinct colors per net without a fixed palette |
| State machine | Simple enum + match in interaction handler | No library needed — this is <200 lines of TypeScript |
| Incremental DRC | Run DRC on changed region only | The existing `run_drc()` is fast enough for small boards; for live feedback, limit check to clearance rule only on the edited trace's bounding box |

## Existing Code and Patterns

- `viewer/src/renderer.ts` — Canvas 2D renderer already draws traces with `drawTrace()` (polyline with `lineWidth`, `lineCap: 'round'`, `lineJoin: 'round'`), vias with `drawVia()`, and violations with `drawViolation()`. Layer ordering is correct (bottom→top→inner). Locked trace indicator (dashed overlay) already exists.
- `viewer/src/interaction.ts` — Clean interaction handler with zoom (wheel), pan (middle/ctrl+click), and click-to-select. The `onSelect(x_nm, y_nm)` callback is the natural hook point for "start routing" mode.
- `viewer/src/viewport.ts` — `worldToScreen()`/`screenToWorld()` coordinate conversion, `zoomAtPoint()`, `fitBoard()`. All working correctly.
- `viewer/src/types.ts` — `TraceInfo` has `segments`, `width`, `layer`, `net_name`, `locked`. Missing: `id` (trace entity ID for editing). `TraceSegmentInfo` has `start_x/y`, `end_x/y`. No `net_id` numeric field.
- `viewer/src/layers.ts` — Layer colors use fixed palette (`top_copper: '#C83434'`, `bottom_copper: '#3434C8'`). `getTraceColor()` maps layer string to color.
- `viewer/src/wasm.ts` — `PcbEngine` interface exposes `load_source()`, `load_routes()`, `get_snapshot()`, `query_point()`. No mutation methods for individual traces. The `WasmPcbEngineAdapter` wraps raw WASM engine with JS-side parsing. The `MockPcbEngine` also implements the same interface.
- `crates/cypcb-render/src/lib.rs` — `PcbEngine` WASM bridge builds `BoardSnapshot` from ECS world. `collect_traces()` iterates `Trace` components. `load_routes()` parses `.routes` file format and spawns `Trace`/`Via` entities. `clear_autorouted_traces()` removes non-locked autorouted traces.
- `crates/cypcb-world/src/components/trace.rs` — `Trace` component with `segments: Vec<TraceSegment>`, `width: Nm`, `layer: Layer`, `net_id: NetId`, `locked: bool`, `source: TraceSource`. `TraceSource::Manual` vs `TraceSource::Autorouted`.
- `crates/cypcb-router/src/lib.rs` — `apply_routes()` groups `RouteSegment`s by `(net_id, layer)` into `Trace` entities and spawns `Via` entities. This is the Rust-side equivalent of what we need for manual trace creation.
- `crates/cypcb-drc/src/rules/clearance.rs` — Clearance rule checks spatial index entries, but the spatial index only contains component entities (not traces/vias). S02 T05 explicitly notes: "DRC trace-level clearance checking (trace-to-pad, trace-to-trace) not yet supported."
- `crates/cypcb-world/src/spatial.rs` — R*-tree based `SpatialIndex` with `SpatialEntry` (entity, AABB, layer_mask). `rebuild_spatial_index()` only indexes components via footprint bounds. Needs extension to index traces/vias.

## Constraints

- **Canvas 2D only** — No WebGL/WebGPU for this slice. The existing renderer is Canvas 2D and S04 (3D viewer) will add Three.js later. All rendering work must stay in Canvas 2D.
- **WASM bridge is the bottleneck** — The WASM `PcbEngine` is the single source of truth. Any mutation (adding/removing traces) must go through it. The JS-side `MockPcbEngine` must also support the same operations for dev mode.
- **Coordinate system** — World coords are nanometers (i64 in Rust, number in TS). Y-up in world, Y-down on screen. All trace editing math must respect this.
- **No tree-sitter in WASM** — Parsing happens in JS. But trace editing doesn't go through the parser — it's direct ECS mutation.
- **`BoardWorld.ecs_mut()` requires `&mut self`** — Any query that iterates entities needs a mutable reference. This affects API design for concurrent read+write.
- **Trace identity** — Currently traces have no stable ID exposed to JS. The `TraceInfo` snapshot type doesn't include an entity ID. We need to add one for trace selection/deletion.
- **DRC performance** — Full DRC run on a small board takes <10ms. But on larger boards, running DRC on every mouse move would lag. Need incremental/regional DRC for live feedback.
- **Existing test structure** — S02 has 40 unit + 5 integration tests for cypcb-autoroute. Renderer has no tests (it's pure rendering). Interaction has no tests. We should add at minimum: trace hit-test unit tests, DRC trace-clearance tests.

## Common Pitfalls

- **Canvas hit-testing without spatial index** — Iterating all traces on every mouse move is O(n). Even for moderate boards (100 traces), this adds latency. Use spatial index queries on the world-coordinate bounding box around the cursor.
- **Coordinate precision loss** — Traces use f64 in the snapshot but i64 in the world. Rounding errors at the f64→i64 boundary can cause mismatches. Use integer coordinates consistently for hit-testing; only convert to float for rendering.
- **State machine edge cases** — The routing interaction has many abort paths: Escape during routing, right-click cancel, clicking outside board, switching layers mid-route, undo last segment. Each needs explicit handling or the UI will freeze in an intermediate state.
- **Snapshot rebuild cost** — `build_snapshot()` iterates all entities every time. If we call it on every mouse move for live DRC, it'll be expensive on large boards. Instead, maintain a partial update path: only re-run DRC and only rebuild the changed trace data.
- **Trace-to-trace clearance geometry** — Checking clearance between two line segments is non-trivial (segment-to-segment distance requires perpendicular distance, endpoint distances, and projection math). Get the math right or use a tested library. The `rstar` crate's AABB queries give fast candidate selection, but actual clearance requires precise segment geometry.
- **45° angle snapping during drag** — The snap point must update on every mouse move, but the "anchor" point (last placed vertex) stays fixed. Snap the cursor position relative to the anchor, not the start pad.
- **Layer mismatch between pad and trace** — When the user starts routing from a through-hole pad, routing can be on any layer. For SMD pads, routing must start on the pad's layer. Need to auto-detect and enforce this.

## Open Risks

- **DRC trace-level checking complexity** — Adding traces to the spatial index requires extending `rebuild_spatial_index()` to also index `Trace` entities. Each trace segment needs its own bounding box entry. For a trace with N segments, that's N spatial entries. This is a meaningful change to the spatial index architecture.
- **Live DRC latency on larger boards** — Even with incremental DRC, checking clearance for a preview trace against all nearby entities involves segment-to-segment distance calculations. For dense boards with many traces, this could exceed the 16ms frame budget. May need to debounce DRC to run at 10Hz instead of every frame.
- **WASM↔JS round-trip cost** — Each trace mutation requires: JS calls WASM method → Rust mutates ECS → Rust runs DRC → Rust serializes result → JS receives and re-renders. If this takes >16ms, the routing preview will feel laggy. May need to batch operations or skip full snapshot rebuild.
- **Undo/redo scope** — Manual trace editing implies undo/redo. Full undo/redo is S06 scope, but even basic "Escape to cancel current route" and "Ctrl+Z to undo last segment" need some state tracking. Need to decide: implement minimal undo for this slice, or defer entirely.
- **MockPcbEngine parity** — The mock engine (used in dev without WASM) needs to support the same trace mutation API. Otherwise dev workflow breaks. This means implementing trace add/remove/query in pure JS as well.

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Canvas 2D interactive drawing | none found | No relevant skill — the "canvas" skills are for Obsidian Canvas, not HTML5 Canvas |
| PCB EDA design | `tscircuit/skill@tscircuit` (156 installs), `l3wi/claude-eda@eda-pcb` (56 installs) | Available but not relevant — these are for using external EDA tools, not building one |
| Rust WASM | none searched | Using existing patterns from cypcb-render; no skill needed |

## Sources

- `crates/cypcb-autoroute/` — S02 autorouter output format (`RoutingResult` with `RouteSegment` and `ViaPlacement`)
- `crates/cypcb-router/src/lib.rs` — `apply_routes()` shows how routing results become ECS `Trace`/`Via` entities
- `crates/cypcb-drc/src/rules/clearance.rs` — Current clearance rule uses spatial index AABB queries (component-only)
- `.gsd/milestones/M002/slices/S02/tasks/T05-SUMMARY.md` — Explicitly documents that trace-level DRC is not yet implemented
- `viewer/src/interaction.ts` — Current interaction handlers (zoom/pan/select) to be extended
- `viewer/src/renderer.ts` — Current trace/via rendering to be enhanced
- KiCad PNS (Push 'N' Shove) router architecture — reference for interactive routing patterns (not cloned, studied from documentation)
