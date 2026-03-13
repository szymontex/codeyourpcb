---
id: T02
parent: S03
milestone: M002
provides:
  - Deterministic net-based color assignment for traces
  - Trace selection with glow highlight on click
  - Trace hover with subtle overlay highlight
  - Client-side hit-testing (point-to-segment distance)
  - Net name + width label tooltip on selected trace
  - Debug surface window.__renderState for programmatic inspection
key_files:
  - viewer/src/hit-test.ts
  - viewer/src/layers.ts
  - viewer/src/renderer.ts
  - viewer/src/interaction.ts
  - viewer/src/main.ts
key_decisions:
  - Net color uses string hash → HSL hue with fixed S=70% L=50%; power/ground nets get explicit overrides (VCC=red, GND=dark blue, 3V3=orange)
  - Hit-testing tolerance is pixel-based (5px default) converted to world units + half trace copper width
  - Trace selection takes priority over component selection in click handler; miss falls through to component query_point
  - colorByNet defaults to true — layer colors are still available as fallback when disabled
  - Label follows cursor position via mousemove listener, drawn as semi-transparent badge
patterns_established:
  - tracePolyline() helper builds path once, reused by main stroke, glow, and lock overlay
  - rAF-guarded hover tracking — mousemove schedules at most one rAF per frame for hit-testing
  - InteractionState carries snapshot reference for hit-testing without extra plumbing
observability_surfaces:
  - "window.__renderState.selectedTraceId / hoveredTraceId / colorByNet for programmatic inspection"
  - "console.log on trace selection with net name, id, and segment index"
duration: 1 session
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T02: Net-colored trace rendering, trace selection, and hit-testing

**Added per-net HSL coloring, click-to-select with glow highlight, hover overlay, point-to-segment hit-testing, and cursor-following net label for traces.**

## What Happened

Implemented all six steps from the task plan:

1. **netColor()** in layers.ts — hashes net name to HSL hue, with explicit overrides for VCC/GND/3V3 variants. Added `brightenColor()` and `colorWithAlpha()` helpers for highlight effects.

2. **Renderer state** — extended `RenderState` with `colorByNet`, `selectedTraceId`, `hoveredTraceId`, and `labelPosition`. `drawTrace()` now accepts these and selects between net color and layer color. Default is `colorByNet: true`.

3. **Selection/hover visuals** — selected trace gets a 2.5x-width semi-transparent glow behind it plus a 1.5x-width brighter main stroke. Hovered (non-selected) trace gets a white 25% opacity overlay. Both compose cleanly with the locked indicator.

4. **hit-test.ts** — new module with `hitTestTrace()`. Converts screen coords to world coords, iterates all trace segments, uses clamped projection for point-to-segment distance. Tolerance = `5px / scale + trace.width/2` (pixel tolerance + copper width).

5. **Interaction wiring** — click handler tries trace hit-test first; on hit, sets `selectedTraceId` and shows trace info in status bar. On miss, clears trace selection and falls through to component `query_point`. Hover uses rAF guard to avoid per-pixel re-renders. Cursor changes to `pointer` over traces.

6. **Net label** — when a trace is selected, a semi-transparent badge follows the cursor showing "NET_NAME — 0.20mm". Drawn last (on top of everything).

Also fixed mock data in wasm.ts that was missing the `id` field added by T01.

## Verification

- `cargo test -p cypcb-world -- spatial` — 14 tests + 5 doc-tests pass ✓
- `cargo test -p cypcb-drc -- clearance` — 36 tests + 2 doc-tests pass ✓
- `cargo test -p cypcb-render -- trace` — 2 tests pass ✓
- TypeScript `tsc --noEmit` — clean, no errors ✓
- `vite build` — successful ✓
- Hit-test math verified with isolated point-to-segment distance tests (on-segment, perpendicular, beyond-end, degenerate cases)
- Net color determinism and override correctness verified with unit assertions
- Browser visual verification not possible (no X server in CI) — deferred to manual testing

## Diagnostics

- `window.__renderState.selectedTraceId` — currently selected trace entity ID (or null)
- `window.__renderState.hoveredTraceId` — trace under cursor (or null)
- `window.__renderState.colorByNet` — whether net coloring is active
- Console logs `[Trace] Selected: <net> id: <id> seg: <idx>` on each trace click

## Deviations

- Added `id` field to mock trace/via data in `wasm.ts` — this was a gap from T01 that didn't have mock data for the new field. Minor fix, not a plan deviation.

## Known Issues

- `roundRect` canvas method used in `drawNetLabel` may not be available in very old browsers (pre-2023). All modern browsers support it.
- No UI toggle for `colorByNet` yet — defaults to true. A toolbar checkbox can be added in a future task if needed.

## Files Created/Modified

- `viewer/src/hit-test.ts` — new module: hitTestTrace() with point-to-segment distance math
- `viewer/src/layers.ts` — added netColor(), brightenColor(), colorWithAlpha(), NET_COLOR_OVERRIDES
- `viewer/src/renderer.ts` — extended RenderState, updated drawTrace() for net colors/selection/hover, added tracePolyline() helper, added drawNetLabel()
- `viewer/src/interaction.ts` — added trace hit-test on click, rAF-guarded hover tracking, extended InteractionState
- `viewer/src/main.ts` — wired new state fields, trace select/hover callbacks, label position tracking, exposed window.__renderState
- `viewer/src/wasm.ts` — added missing id field to mock trace/via data
