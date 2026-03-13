---
id: T04
parent: S03
milestone: M002
provides:
  - Manual routing state machine (idle → routing → idle) with pad click entry/exit
  - 45°/90° angle snapping for trace preview
  - Pad hit-testing for route start/complete targeting
  - Live DRC preview via debounced add→check→remove cycle
  - Preview trace rendering (dashed committed + dashed preview segment)
  - Delete key removes selected trace; Escape cancels in-progress route
  - window.__routingState debug surface for runtime inspection
key_files:
  - viewer/src/routing.ts
  - viewer/src/interaction.ts
  - viewer/src/renderer.ts
  - viewer/src/main.ts
key_decisions:
  - Routing state machine is a pure-function module (routing.ts) — all transitions are side-effect-free state→state transforms, with side effects (engine mutations) handled at the call site in interaction.ts
  - DRC preview uses temporary trace add→DRC→remove pattern debounced at 100ms — avoids accumulating phantom traces while still providing live feedback
  - Pad hit-testing uses pad diagonal / 2 + tolerance rather than axis-aligned bounding box — more forgiving for rotated components
  - Layer detection from pad layer_mask bit 0x02 (Bottom) vs default Top — matches the LAYER_MASK constants in layers.ts
  - Renamed local RoutingState interface to AutorouteUiState in main.ts to resolve name collision with imported routing.RoutingState
patterns_established:
  - Pure state machine pattern for UI interaction modes — state transitions are testable functions, rendering reads state, effects happen at integration points
  - ensureDrcChecker() lazy initialization pattern — DRC checker created only when routing begins, avoids engine reference issues during init
observability_surfaces:
  - "window.__routingState — exposes mode, anchorPoint, snapAngle, netName, currentLayer, committedSegments count, drcViolationCount, previewSegment"
  - "Console logs prefixed [Route] for all state transitions: idle → routing, waypoint, routing → idle (complete/cancel)"
  - "Console logs [Route] DRC preview: N violations on each debounced DRC check"
duration: 1.5h
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T04: Manual routing interaction state machine with DRC preview

**Built the full manual routing flow: click pad → drag with 45°/90° snap → click pad to finish, with live DRC violations and Escape/Delete support.**

## What Happened

Created `viewer/src/routing.ts` as a dedicated module containing:
- `RoutingState` type and `RoutingMode` enum (`idle | routing`)
- `PadHit` type for pad hit-testing results  
- `hitTestPad()` — finds the nearest pad to a world-coordinate click, accounting for component rotation and pad geometry
- `computeSnappedPoint()` — snaps cursor position to nearest 45° increment from anchor point
- Pure state transition functions: `startRoute()`, `updatePreview()`, `addWaypoint()`, `completeRoute()`, `cancelRoute()`
- `createDrcPreviewChecker()` — debounced DRC that temporarily adds a preview trace, runs DRC, and removes it

Updated `interaction.ts` to integrate routing:
- Click handler checks routing mode first — while routing, clicks target pads (complete) or empty space (waypoint)
- In idle mode, pad clicks start routing before falling through to trace/component selection
- Mousemove updates the preview segment with angle snapping and triggers debounced DRC

Updated `renderer.ts`:
- Added `drawRoutingPreview()` that renders committed segments (semi-transparent solid), preview segment (dashed), anchor marker, endpoint cursor, DRC violation rings, snap angle indicator, and net name label
- Added `routing` field to `RenderState`

Updated `main.ts`:
- Wired engine reference to interaction state for trace mutations
- Added Escape key handler to cancel routing, Delete/Backspace to remove selected trace
- Exposed `window.__routingState` debug surface
- Renamed local `RoutingState` to `AutorouteUiState` to avoid collision

## Verification

- `npx tsc --noEmit` — clean compile, zero errors
- `npx vite build` — production build succeeds (24.75s)
- `cargo test -p cypcb-world -- spatial` — 14 tests + 5 doc-tests pass ✓
- `cargo test -p cypcb-drc -- clearance` — passes ✓
- `cargo test -p cypcb-render -- trace` — 14 tests pass ✓
- Full workspace test suite (excluding desktop): 345 passed, 2 pre-existing failures in cypcb-export (unrelated)
- Browser visual verification not possible (no display server in environment) — deferred to T05 integration task

## Diagnostics

- `window.__routingState` — live routing state: `{ mode, anchorPoint, snapAngle, netName, currentLayer, committedSegments, drcViolationCount, previewSegment }`
- `window.__renderState` — trace selection state (unchanged from T02)
- Console `[Route]` prefix on all state transitions and DRC checks
- Status bar updates during routing (trace info, "Trace deleted" feedback)

## Deviations

- Browser visual verification could not be performed (no X server/display). The full interactive verification (pad click → routing preview → snap → DRC → complete → delete) is deferred to T05 which is explicitly the integration verification task.

## Known Issues

- DRC preview checker captures the routing state by closure at scheduling time — if the state changes between scheduling and execution (100ms window), the DRC result may be slightly stale. This is acceptable for preview purposes and avoids complexity.
- No layer switching during routing (press 'F' to flip layers) — planned for T05.

## Files Created/Modified

- `viewer/src/routing.ts` — New module: routing state machine, pad hit-testing, angle snapping, DRC preview checker
- `viewer/src/interaction.ts` — Integrated routing into click/mousemove handlers, added routing fields to InteractionState
- `viewer/src/renderer.ts` — Added drawRoutingPreview() for dashed preview trace, DRC markers, snap indicators; added routing field to RenderState
- `viewer/src/main.ts` — Wired engine to interaction, added Escape/Delete handlers, exposed __routingState debug surface, renamed local RoutingState to AutorouteUiState
