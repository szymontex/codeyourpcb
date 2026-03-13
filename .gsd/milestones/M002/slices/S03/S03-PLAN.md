# S03: Renderer Upgrade & Manual Trace Editing

**Goal:** User can see autorouter output rendered with proper trace widths/colors/clearances, and can click-drag to manually route or edit individual traces in the 2D viewer with live DRC feedback.
**Demo:** Load a .cypcb file with autorouted traces. Traces render with net-specific colors and proper widths. Click a pad to start routing — a preview trace follows the cursor with 45°/90° snapping. DRC violations highlight in real-time during routing. Click another pad to complete the route. Click an existing trace to select it (highlighted). Press Delete to remove it.

## Must-Haves

- Traces in the spatial index (alongside components) so DRC checks trace-to-pad and trace-to-trace clearance
- Trace entity IDs exposed in `BoardSnapshot`/`TraceInfo` so the JS side can reference specific traces
- Net-colored trace rendering (deterministic color per net name)
- Trace/via hit-testing (click to select a specific trace)
- WASM `PcbEngine` mutation API: add trace segment, remove trace, query trace at point, incremental DRC
- MockPcbEngine supports same mutation API for dev mode
- Manual routing state machine: click pad → drag with 45°/90° snap → click pad to finish
- Live DRC violation overlay during manual routing
- Escape to cancel in-progress route, Delete to remove selected trace

## Proof Level

- This slice proves: integration (Rust trace mutation → WASM bridge → JS renderer → user interaction → DRC feedback loop)
- Real runtime required: yes (dev server with WASM or mock engine)
- Human/UAT required: yes (manual routing interaction requires visual confirmation)

## Verification

- `cargo test -p cypcb-world -- spatial` — spatial index indexes traces/vias, queries return them
- `cargo test -p cypcb-drc -- clearance` — clearance rule detects trace-to-pad and trace-to-trace violations
- `cargo test -p cypcb-render -- trace` — snapshot includes trace IDs, mutation methods work correctly
- Dev server visual verification: load blink.cypcb, autoroute, verify net-colored traces render, click a pad to start manual routing, verify snap preview and DRC overlay, complete route, select and delete a trace

## Observability / Diagnostics

- Runtime signals: console.log on interaction state transitions (`[Route] idle → routing_start → routing_drag → routing_complete`), DRC violation count logged after each incremental check
- Inspection surfaces: `window.__routingState` debug object exposed in dev mode (current state, anchor point, snap angle, DRC violation count); Rust `PcbEngine::trace_count()` and `PcbEngine::violation_count()` for programmatic inspection
- Failure visibility: DRC violations rendered as red markers on canvas; trace mutation errors returned as strings from WASM methods; interaction state machine resets to idle on any unhandled error
- Redaction constraints: none (no secrets in PCB design data)

## Integration Closure

- Upstream surfaces consumed: `crates/cypcb-autoroute/` (autorouter output as `Trace`/`Via` entities in `BoardWorld`), `crates/cypcb-drc/` (clearance rule, `DesignRules`), `crates/cypcb-world/` (spatial index, `Trace`/`Via` components), `crates/cypcb-render/` (WASM bridge, `BoardSnapshot`)
- New wiring introduced in this slice: JS interaction state machine → WASM mutation API → Rust ECS mutation → spatial index rebuild → DRC recheck → snapshot rebuild → JS re-render loop
- What remains before the milestone is truly usable end-to-end: S04 (3D viewer), S05 (DSL v2 modules/constraints), S06 (UI polish — undo/redo, grid snap, net highlighting), S07 (E2E tests), S08 (performance)

## Tasks

- [x] **T01: Extend spatial index to include traces/vias and add trace-level DRC clearance** `est:2h`
  - Why: The spatial index currently only indexes components. Without trace/via entries, DRC can't check trace-to-pad or trace-to-trace clearance, and hit-testing can't find traces. This is the foundation for both live DRC feedback and trace selection.
  - Files: `crates/cypcb-world/src/world.rs`, `crates/cypcb-world/src/spatial.rs`, `crates/cypcb-drc/src/rules/clearance.rs`, `crates/cypcb-render/src/lib.rs`, `crates/cypcb-render/src/snapshot.rs`
  - Do: 1) Add `rebuild_spatial_index_with_traces()` method to `BoardWorld` that indexes both components (via footprint bounds) AND trace segments (per-segment bounding box expanded by half trace width) AND vias (circular bounding box). 2) Add `id` field (u32 entity index) to `TraceInfo` and `ViaInfo` in the snapshot types (both Rust and TS). 3) Populate the `id` field in `collect_traces()`/`collect_vias()`. 4) Call the new spatial index rebuild after `load_routes()` and `populate_from_snapshot()`. 5) Add clearance tests for trace-to-pad and trace-to-trace scenarios. 6) Add segment-to-segment distance calculation for more precise trace clearance checking.
  - Verify: `cargo test -p cypcb-world -- spatial` passes; `cargo test -p cypcb-drc -- clearance` includes new trace-level tests passing; `cargo test -p cypcb-render` passes with trace ID fields
  - Done when: Spatial index includes trace/via entities, DRC detects trace-to-pad clearance violations, snapshot TraceInfo/ViaInfo include entity IDs

- [x] **T02: Net-colored trace rendering, trace selection, and hit-testing** `est:1.5h`
  - Why: The current renderer uses fixed layer colors for traces. For manual editing, users need to visually distinguish nets and click-select individual traces. This task adds the visual foundation for trace interaction.
  - Files: `viewer/src/renderer.ts`, `viewer/src/layers.ts`, `viewer/src/types.ts`, `viewer/src/interaction.ts`, `viewer/src/wasm.ts`
  - Do: 1) Add `id` field to `TraceInfo` and `ViaInfo` TS types. 2) Add `netColor(netName: string): string` function that hashes net name → HSL color (deterministic, visually distinct). 3) Update `drawTrace()` to optionally use net color instead of layer color (add render state toggle `colorByNet`). 4) Add `selectedTraceId` to `RenderState`. 5) Draw selected trace with highlight (thicker outline, brighter color). 6) Add `hitTestTrace(snapshot, viewport, screenX, screenY, tolerance): TraceInfo | null` function that converts screen coords to world, expands by tolerance, and finds nearest trace segment. 7) Update interaction `onSelect` to call `hitTestTrace` and set `selectedTraceId`. 8) Add net name tooltip on trace hover.
  - Verify: Dev server shows net-colored traces when `colorByNet` enabled; clicking a trace selects and highlights it; hover shows net name
  - Done when: Traces render with per-net colors, clicking a trace selects it with visual feedback, trace hit-testing works at reasonable tolerance

- [x] **T03: WASM bridge mutation API and MockPcbEngine parity** `est:2h`
  - Why: Manual trace editing requires mutating the board state from JavaScript — adding new traces, removing existing ones, and getting live DRC feedback. The WASM PcbEngine and MockPcbEngine both need these methods.
  - Files: `crates/cypcb-render/src/lib.rs`, `crates/cypcb-render/src/snapshot.rs`, `viewer/src/wasm.ts`, `viewer/src/types.ts`
  - Do: 1) Add `add_trace(net_name, layer, width, segments_json) → trace_id` to PcbEngine (Rust). Creates a Manual Trace entity, updates spatial index, returns entity ID. 2) Add `remove_trace(trace_id) → bool` to PcbEngine. Removes trace entity, updates spatial index. 3) Add `get_trace_at_point(x_nm, y_nm, tolerance_nm) → Option<trace_id>` to PcbEngine. Uses spatial index query. 4) Add `run_drc_incremental() → violations_json` to PcbEngine. Runs DRC and returns violations (same as full DRC for now — incremental optimization deferred). 5) Add `trace_count() → usize` diagnostic method. 6) Expose all methods via `#[wasm_bindgen]` with JsValue serialization. 7) Update `PcbEngine` TS interface with matching method signatures. 8) Implement same methods in `MockPcbEngine` (pure JS trace storage + simple clearance mock). 9) Add Rust tests for add/remove/query cycle.
  - Verify: `cargo test -p cypcb-render -- trace` passes for add/remove/query cycle; MockPcbEngine tests pass in dev mode
  - Done when: Both WASM and Mock engines support trace add/remove/query/DRC, TS interface matches Rust API

- [x] **T04: Manual routing interaction state machine with DRC preview** `est:2.5h`
  - Why: This is the core user-facing feature — click a pad to start routing, drag with angle snapping to draw trace segments, see live DRC violations, click a pad to finish. Without this, manual trace editing doesn't exist.
  - Files: `viewer/src/interaction.ts`, `viewer/src/renderer.ts`, `viewer/src/types.ts`, `viewer/src/main.ts`
  - Do: 1) Define `RoutingMode` enum: `idle | routing` and `RoutingState` type: `{ mode, startPad, currentLayer, anchorPoint, previewSegments, snapAngle, drcViolations }`. 2) Add routing state to `InteractionState`. 3) On left-click when idle: hit-test pads — if a pad is clicked, enter `routing` mode with that pad as anchor, detect layer from pad. 4) On mousemove when routing: compute snapped endpoint (45°/90° from anchor), build preview segment, call `run_drc_incremental()` on the preview (debounced to 10Hz), store DRC violations. 5) On left-click when routing: if clicking a pad (finish route), call `add_trace()` with all segments, exit to idle. If clicking empty space, anchor current segment and start next segment from endpoint. 6) On Escape when routing: cancel route, clear preview, return to idle. 7) On Delete when idle with selected trace: call `remove_trace(selectedTraceId)`, deselect. 8) Update renderer to draw preview trace (dashed line) and DRC violation markers during routing. 9) Add `computeSnappedPoint(anchor, cursor, angles)` utility for 45°/90° snapping. 10) Wire keyboard events in `main.ts` for Escape/Delete. 11) Add `window.__routingState` debug surface in dev mode.
  - Verify: Dev server: click pad → see preview trace following cursor → snap to 45° angles → DRC violations show during drag → click another pad → trace appears → press Delete → trace removed
  - Done when: Full manual routing flow works with snapping and DRC feedback; Escape cancels; Delete removes selected trace

- [x] **T05: Integration verification and polish** `est:1h`
  - Why: Wire everything together end-to-end: load a real .cypcb file, autoroute, then manually edit. Verify the full loop: render → select → route → DRC → delete. Fix any integration issues between T01-T04.
  - Files: `viewer/src/main.ts`, `viewer/src/renderer.ts`, `viewer/src/interaction.ts`
  - Do: 1) Verify WASM build compiles with new PcbEngine methods (`cargo build -p cypcb-render --target wasm32-unknown-unknown`). 2) Run full Rust test suite (`cargo test`). 3) Start dev server, load blink.cypcb, autoroute it, verify traces render with net colors. 4) Click a pad, verify routing preview with 45° snap. 5) Complete a manual route, verify trace appears. 6) Select and delete a trace. 7) Verify DRC violations appear during routing near existing traces. 8) Add layer switching via keyboard (press 'F' to flip between Top/Bottom during routing). 9) Fix any integration issues discovered. 10) Verify MockPcbEngine dev workflow works without WASM.
  - Verify: Full Rust test suite passes; WASM builds; dev server shows working manual routing flow
  - Done when: End-to-end manual routing flow works in dev server with both WASM and mock engines

## Files Likely Touched

- `crates/cypcb-world/src/world.rs`
- `crates/cypcb-world/src/spatial.rs`
- `crates/cypcb-drc/src/rules/clearance.rs`
- `crates/cypcb-render/src/lib.rs`
- `crates/cypcb-render/src/snapshot.rs`
- `viewer/src/types.ts`
- `viewer/src/renderer.ts`
- `viewer/src/interaction.ts`
- `viewer/src/layers.ts`
- `viewer/src/wasm.ts`
- `viewer/src/main.ts`
