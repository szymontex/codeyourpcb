---
estimated_steps: 8
estimated_files: 5
---

# T01: Implement routing UX features with keyboard handlers and unit tests

**Slice:** S03 — Routing UX Upgrade
**Milestone:** M003

## Description

Extend the routing state machine with four UX features: net-aware target pad highlighting, ratsnest guide emphasis, magnetic snap to destination pads, and toggleable angle constraint. Add keyboard handlers for Escape/F/A. Write unit tests covering new state transitions and snap logic.

All four features are tightly coupled — they share RoutingState fields, fire in the same updatePreview() pipeline, and render through the same drawRoutingPreview()/drawRatsnest() paths. Implementing together avoids partial states where one feature exists without the others it depends on.

## Steps

1. **Extend RoutingState in routing.ts** — Add fields: `angleSnapEnabled: boolean` (default `false`), `magneticSnapEnabled: boolean` (default `true`), `magneticSnapRadius: number` (default `1_000_000` nm = 1mm), `snappedToPad: PadHit | null`, `targetPads: PadHit[]` (pre-computed on route start). Add `toggleAngleSnap(state)` function. Add `findNearestTargetPad(worldX, worldY, state, viewportScale)` that scans targetPads with dual threshold (world radius OR 15px/scale, whichever larger).

2. **Update startRoute() in routing.ts** — When routing starts, pre-compute `targetPads`: iterate all components+pads in snapshot, filter by same netName as start pad, exclude start pad itself, compute world positions. Return the net name so main.ts can set `highlightedNet`.

3. **Update updatePreview() in routing.ts** — After grid snap, before angle snap: call `findNearestTargetPad()`. If found within radius, set `snappedToPad` and use pad center as preview endpoint (magnetic snap wins). Only then apply angle snap if `angleSnapEnabled` AND `snappedToPad` is null. Update `completeRoute()` and `cancelRoute()` to clear `snappedToPad` and `targetPads`.

4. **Add keyboard handler in interaction.ts** — Add `document.addEventListener('keydown', ...)` inside `setupInteraction()`. Guard: only intercept when `routingState.mode === 'routing'` OR canvas is focused. Keys: `Escape` → cancelRoute + clear highlight callback, `f`/`F` → flipLayer(), `a`/`A` → toggleAngleSnap(). Return cleanup function. Check `document.activeElement` to avoid intercepting when Monaco editor is focused (not a TEXTAREA/INPUT with class containing 'monaco').

5. **Wire highlighting lifecycle in main.ts** — In the interaction callbacks: when startRoute returns net name, set `renderState.highlightedNet = netName`. When completeRoute or cancelRoute fires, set `renderState.highlightedNet = null`. Expose extended `__routingState` with new fields (`angleSnapEnabled`, `magneticSnapEnabled`, `snappedToPad`).

6. **Update renderer.ts drawRoutingPreview()** — When `routingState.snappedToPad` is set, draw a snap indicator: pulsing circle (radius 0.3mm) at target pad center in net-color with 0.6 alpha, plus a crosshair. Update **drawRatsnest()**: when `highlightedNet` is set (during routing), draw matching net lines at full alpha and 2x width, dim non-matching lines to 0.15 alpha.

7. **Write unit tests in routing.test.ts** — Test cases: (a) startRoute pre-computes targetPads for correct net, (b) findNearestTargetPad returns closest pad within radius, (c) findNearestTargetPad returns null when no pad in range, (d) magnetic snap overrides angle snap in updatePreview, (e) toggleAngleSnap flips state, (f) completeRoute clears snappedToPad and targetPads, (g) cancelRoute clears snappedToPad and targetPads, (h) angle snap disabled by default.

8. **TypeScript check** — Run `npx tsc --noEmit` and fix any type errors. Run `cd viewer && npx vitest run` to verify all tests pass.

## Must-Haves

- [ ] `angleSnapEnabled` defaults to `false` (roadmap: "optional toggle, not forced")
- [ ] `targetPads` pre-computed at route start (not scanned per frame)
- [ ] Magnetic snap uses dual threshold: world radius (1mm) OR screen-pixel radius (15px), whichever larger
- [ ] Magnetic snap takes priority over angle snap when target pad is in range
- [ ] Keyboard handler guarded by routing mode — doesn't fire when Monaco editor focused
- [ ] `highlightedNet` set on route start, cleared on complete/cancel
- [ ] Ratsnest draws active-net lines brighter/thicker, dims others during routing
- [ ] Snap visual indicator rendered at target pad center
- [ ] Unit tests cover snap priority, toggle, target pad computation, cleanup on complete/cancel

## Verification

- `npx tsc --noEmit` — zero errors
- `cd viewer && npx vitest run` — all tests pass including new routing.test.ts
- `window.__routingState` in browser console shows `angleSnapEnabled`, `magneticSnapEnabled`, `snappedToPad` fields

## Observability Impact

- Signals added: `__routingState` extended with `angleSnapEnabled`, `magneticSnapEnabled`, `snappedToPad`, `targetPadsCount`
- How a future agent inspects this: `window.__routingState` in browser devtools or via page.evaluate in E2E
- Failure state exposed: routing state machine fully observable — snap target identity, angle mode, target pad count all visible

## Inputs

- `viewer/src/routing.ts` — existing state machine with startRoute/updatePreview/completeRoute/cancelRoute
- `viewer/src/interaction.ts` — existing mouse handler setup, needs keyboard addition
- `viewer/src/renderer.ts` — existing drawRoutingPreview and drawRatsnest
- `viewer/src/main.ts` — existing render loop, pullSnapshot, __routingState exposure
- `viewer/src/render-config.ts` — buildPadNetMap, RenderConfig (consumed, not modified)
- S01 Summary — pad highlighting works via highlightedNet on RenderState, padNetMap available

## Expected Output

- `viewer/src/routing.ts` — extended RoutingState, findNearestTargetPad(), toggleAngleSnap(), updated updatePreview/startRoute/completeRoute/cancelRoute
- `viewer/src/interaction.ts` — keyboard event listener with routing mode guard
- `viewer/src/renderer.ts` — snap indicator in drawRoutingPreview, ratsnest emphasis in drawRatsnest
- `viewer/src/main.ts` — highlightedNet lifecycle wiring, extended __routingState diagnostic
- `viewer/src/__tests__/routing.test.ts` — ≥8 unit tests for routing state machine changes
