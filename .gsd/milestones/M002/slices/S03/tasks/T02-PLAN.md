---
estimated_steps: 6
estimated_files: 5
---

# T02: Net-colored trace rendering, trace selection, and hit-testing

**Slice:** S03 — Renderer Upgrade & Manual Trace Editing
**Milestone:** M002

## Description

The current renderer uses fixed layer colors (red for Top, blue for Bottom) for all traces. For manual trace editing, users need to visually distinguish which traces belong to which net and click to select individual traces. This task adds: (1) deterministic net-based color assignment, (2) trace selection with visual highlighting, and (3) client-side hit-testing that identifies which trace the user clicked on. These are all purely visual/interaction changes — no WASM API mutations needed.

## Steps

1. Add `netColor(netName: string): string` function to `viewer/src/layers.ts`. Hash the net name to a hue value (0-360), use fixed saturation (70%) and lightness (50%) for HSL color. This produces deterministic, visually distinct colors per net without a fixed palette. Include a small set of overrides for common nets: VCC → red, GND → dark blue, 3V3 → orange.

2. Add render state fields to `viewer/src/renderer.ts`: `colorByNet: boolean` (toggle between layer color and net color), `selectedTraceId: number | null` (currently selected trace entity ID), `hoveredTraceId: number | null` (trace under cursor). Update `drawTrace()` to use `netColor(trace.net_name)` when `colorByNet` is true, falling back to `getTraceColor()` when false.

3. Draw selected trace with highlight: when `trace.id === selectedTraceId`, draw the trace with a wider stroke (1.5x width) in a brighter version of its color, plus a subtle glow effect (outer stroke with transparency). Draw hovered trace with a lighter overlay to indicate clickability.

4. Add `hitTestTrace(snapshot, viewport, screenX, screenY, tolerancePx): { trace: TraceInfo, segmentIndex: number } | null` function to a new `viewer/src/hit-test.ts` module. Convert screen coords to world coords. For each trace in the snapshot, for each segment, compute the perpendicular distance from the world point to the segment line. Return the trace whose closest segment is within `tolerance / viewport.scale` nanometers of the click point. Use `trace.width / 2` as additional tolerance (clicking on the trace copper counts).

5. Update `interaction.ts` `onSelect` handler to call `hitTestTrace` and update `selectedTraceId` in render state. If a trace is hit, select it (set `selectedTraceId`). If nothing is hit, deselect (set `selectedTraceId` to null). Add `onMouseMove` handler for hover state — update `hoveredTraceId` but debounce to avoid excessive re-renders (use requestAnimationFrame guard).

6. Add net name display for selected trace: when a trace is selected, draw a small label near the cursor showing the net name and trace width (e.g., "VCC — 0.2mm"). Style it as a semi-transparent badge.

## Must-Haves

- [ ] Net color function produces deterministic, distinct colors from net names
- [ ] Traces render with per-net colors when colorByNet is enabled
- [ ] Clicking a trace selects it with visible highlight
- [ ] Clicking empty space deselects
- [ ] Hover state shows subtle highlight on trace under cursor
- [ ] Hit-testing works correctly at various zoom levels

## Verification

- Start dev server, load a .cypcb file with autorouted traces (blink.cypcb)
- Verify traces display in net-specific colors (different nets have different colors)
- Click a trace — verify it highlights
- Click empty space — verify highlight disappears
- Hover over traces — verify subtle hover effect
- Zoom in/out — verify hit-testing works at different scales

## Observability Impact

- Signals added/changed: None (purely visual rendering changes)
- How a future agent inspects this: `console.log` of selected trace info when clicking; `window.__renderState?.selectedTraceId` for programmatic inspection
- Failure state exposed: If hit-testing fails, clicking traces will select nothing — visible immediately in the UI

## Inputs

- `viewer/src/types.ts` — `TraceInfo` with `id` field (from T01)
- `viewer/src/renderer.ts` — existing `drawTrace()`, `RenderState`
- `viewer/src/layers.ts` — existing `getTraceColor()`, `LAYER_COLORS`
- `viewer/src/interaction.ts` — existing `onSelect` callback
- `viewer/src/viewport.ts` — `screenToWorld()`, `worldToScreen()`

## Expected Output

- `viewer/src/layers.ts` — `netColor()` function added
- `viewer/src/hit-test.ts` — new module with `hitTestTrace()` function
- `viewer/src/renderer.ts` — net-colored rendering, selection highlight, hover highlight
- `viewer/src/interaction.ts` — trace selection on click, hover tracking on mousemove
- `viewer/src/types.ts` — no changes (id field already added in T01)
