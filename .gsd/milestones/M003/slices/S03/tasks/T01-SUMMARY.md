---
id: T01
parent: S03
milestone: M003
provides:
  - RoutingState extended with angleSnap, magneticSnap, snappedToPad, targetPads
  - computeTargetPads() for pre-computing same-net pads at route start
  - findNearestTargetPad() with dual threshold (world + screen-px)
  - toggleAngleSnap() and resetToIdle() state helpers
  - Keyboard handler (Escape/F/A) in interaction.ts with editor guard
  - Snap visual indicator and ratsnest emphasis in renderer
  - 14 unit tests for routing state machine UX features
key_files:
  - viewer/src/routing.ts
  - viewer/src/interaction.ts
  - viewer/src/renderer.ts
  - viewer/src/main.ts
  - viewer/src/__tests__/routing.test.ts
key_decisions:
  - Magnetic snap uses dual threshold (1mm world OR 15px/scale) — ensures usability at any zoom
  - Angle snap defaults to OFF per roadmap ("optional toggle, not forced")
  - interaction.ts now owns routing keyboard shortcuts (Escape/F/A) — main.ts delegates routing-mode keys
  - resetToIdle() preserves all user preferences (grid snap, angle snap, magnetic snap settings)
patterns_established:
  - Target pads pre-computed once at startRoute(), not scanned per frame
  - onRouteStart/onRouteEnd callbacks on InteractionState for highlight lifecycle
observability_surfaces:
  - window.__routingState.angleSnapEnabled
  - window.__routingState.magneticSnapEnabled
  - window.__routingState.snappedToPad
  - window.__routingState.targetPadsCount
duration: 1 task
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T01: Implement routing UX features with keyboard handlers and unit tests

**Extended routing state machine with magnetic snap, angle constraint toggle, target pad pre-computation, ratsnest emphasis, and 14 unit tests.**

## What Happened

Added five new fields to RoutingState: `angleSnapEnabled`, `magneticSnapEnabled`, `magneticSnapRadius`, `snappedToPad`, `targetPads`. On route start, `computeTargetPads()` scans the snapshot once for all pads on the same net (excluding start pad) and stores them. During `updatePreview()`, `findNearestTargetPad()` checks if the cursor is within a dual threshold (world radius OR 15px converted to world coords, whichever larger). When a target pad is in range, the preview endpoint snaps to its center and `snappedToPad` is set. Angle snap (45°/90°) only applies when enabled AND no magnetic snap is active.

Keyboard handler added to `setupInteraction()` in interaction.ts: Escape cancels route, F flips layer, A toggles angle snap — all guarded by routing mode check and Monaco editor focus detection. The main.ts keyboard handler was updated to defer routing-specific keys to interaction.ts.

Renderer updated: `drawRoutingPreview()` draws a pulsing circle + crosshair at the snapped pad center when `snappedToPad` is set. `drawRatsnest()` now takes `highlightedNet` — matching-net lines draw at full alpha and 2x width, non-matching dim to 0.15 alpha. The `__routingState` diagnostic surface was extended with `angleSnapEnabled`, `magneticSnapEnabled`, `snappedToPad`, `targetPadsCount`.

## Verification

- `npx tsc --noEmit` — zero errors
- `cd viewer && npx vitest run` — 77 tests pass (14 new in routing.test.ts)
- `cd viewer && npx playwright test` — 52 E2E tests pass
- New routing tests cover: targetPads computation, snap within/outside radius, magnetic-over-angle priority, toggle, cleanup on complete/cancel, defaults

## Diagnostics

- `window.__routingState` in browser console shows all new fields
- `window.__renderDiag.highlightedNet` shows active net during routing
- Console logs: `[Route] idle → routing: ... targets=N`, `[Route] Angle snap: ON/OFF`

## Deviations

- Added `resetToIdle()` helper instead of inline spread in multiple places — cleaner pattern for preserving user preferences across route complete/cancel
- Moved routing-mode keyboard shortcuts (Escape/F/A) from main.ts to interaction.ts — keeps routing logic co-located with the interaction layer that owns the state

## Known Issues

None.

## Files Created/Modified

- `viewer/src/routing.ts` — Extended RoutingState, added toggleAngleSnap, computeTargetPads, findNearestTargetPad, resetToIdle, updated startRoute/updatePreview/cancelRoute
- `viewer/src/interaction.ts` — Added keyboard handler with editor guard, onRouteStart/onRouteEnd callbacks, pass viewport scale to updatePreview
- `viewer/src/renderer.ts` — Snap indicator in drawRoutingPreview, ratsnest emphasis in drawRatsnest
- `viewer/src/main.ts` — Wired onRouteStart/onRouteEnd for highlightedNet lifecycle, extended __routingState diagnostic, deferred routing keys to interaction.ts
- `viewer/src/__tests__/routing.test.ts` — 14 unit tests covering routing UX features
