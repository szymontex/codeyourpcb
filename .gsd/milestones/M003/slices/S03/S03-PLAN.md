# S03: Routing UX Upgrade

**Goal:** User can route a trace pad-to-pad with net-aware target highlighting, ratsnest guide to nearest unconnected pad, magnetic snap to destination, and angle constraint as toggleable option — all verified by E2E test.
**Demo:** Load routing-test.cypcb, click a pad to start routing, see target pads on same net glow, see ratsnest guide line to nearest target, move cursor near target pad and see it snap to center, press A to toggle angle constraint, complete the route, verify trace added.

## Must-Haves

- Net-aware target pad highlighting when routing starts (set `highlightedNet`, S01 glow/dim does the rest)
- Ratsnest guide line from current anchor to nearest unconnected pad on active net (brighter/thicker for active net, dimmed for others)
- Magnetic snap to destination pad center when cursor within threshold (1mm world OR 15px screen, whichever larger)
- Magnetic snap takes priority over angle snap when both active
- Angle constraint toggleable via `A` key (default OFF per roadmap)
- Keyboard handlers: Escape (cancel), F (flip layer), A (angle toggle)
- `highlightedNet` cleared on route completion and cancellation
- Pre-computed target pad list (filter by net at route start, scan only targets during mousemove)
- Snap visual indicator on target pad (pulsing circle in net color)
- Unit tests for routing state machine changes
- E2E tests: start route, complete route, cancel route, net highlight verification, angle toggle

## Proof Level

- This slice proves: integration (routing UX features work end-to-end in the browser)
- Real runtime required: yes (Canvas rendering, keyboard events, mouse interaction)
- Human/UAT required: no (E2E tests cover the flow via diagnostic surfaces)

## Verification

- `cd viewer && npx vitest run` — all tests pass including new routing unit tests
- `cd viewer && npx playwright test e2e/routing-ux.spec.ts` — all routing E2E tests pass
- `cd viewer && npx playwright test` — full suite passes (except known pre-existing flake in errors.spec.ts:102)
- `npx tsc --noEmit` — zero TypeScript errors
- `window.__routingState` exposes `angleSnapEnabled`, `magneticSnapEnabled`, `snappedToPad`
- `window.__renderDiag.highlightedNet` set during routing, cleared after

## Observability / Diagnostics

- Runtime signals: `__routingState` extended with `angleSnapEnabled`, `magneticSnapEnabled`, `snappedToPad` fields
- Inspection surfaces: `window.__routingState` (routing mode, net, snap state), `window.__renderDiag` (highlighted net, frame time)
- Failure visibility: routing state machine exposes full state — mode, anchor, segments, violations, snap target — for any debugging
- Redaction constraints: none

## Integration Closure

- Upstream surfaces consumed: `render-config.ts` (buildPadNetMap, RenderConfig, getLodTier), `renderer.ts` (drawPad highlight, drawRatsnest), `main.ts` (pullSnapshot, __loadBoard, __renderDiag)
- New wiring introduced: keyboard event listener (document-level with routing mode guard), highlightedNet set/clear in routing lifecycle, ratsnest filter by active net, magnetic snap in updatePreview pipeline
- What remains before the milestone is truly usable end-to-end: S04 (UI architecture), S05 (project manager), S06 (JLCPCB), S07 (polish + full verification)

## Tasks

- [x] **T01: Implement routing UX features with keyboard handlers and unit tests** `est:2h`
  - Why: All four routing features (net highlighting, ratsnest guide, magnetic snap, angle toggle) share state in the same routing state machine and render functions — implementing them together avoids partial states
  - Files: `viewer/src/routing.ts`, `viewer/src/interaction.ts`, `viewer/src/renderer.ts`, `viewer/src/main.ts`, `viewer/src/__tests__/routing.test.ts`
  - Do: Extend RoutingState with angleSnapEnabled/magneticSnapEnabled/snappedToPad/targetPads fields. Add findNearestTargetPad() using pre-computed target list. Update updatePreview() with magnetic snap before conditional angle snap. Add keyboard listener with routing mode guard. Set/clear highlightedNet in routing lifecycle. Update drawRoutingPreview() with snap indicator. Update drawRatsnest() to emphasize active net lines. Write unit tests for state transitions, snap logic, toggle behavior.
  - Verify: `cd viewer && npx vitest run` all pass, `npx tsc --noEmit` clean
  - Done when: routing.ts exposes all new state fields on `__routingState`, unit tests cover magnetic snap priority over angle snap, toggle state transitions, and target pad pre-computation

- [x] **T02: E2E routing tests with test fixture** `est:1.5h`
  - Why: Routing UX must be verified in-browser — keyboard events, Canvas click targeting, diagnostic surface reads, and the full routing lifecycle need a running app
  - Files: `viewer/e2e/routing-ux.spec.ts`, `viewer/e2e/fixtures/routing-test.cypcb`
  - Do: Create routing-test.cypcb fixture (3 components, 3 nets, known pad positions). Write E2E tests using `__loadBoard`, `__routingState`, `__renderDiag` diagnostic surfaces. Tests: start route on pad → verify routing mode + highlightedNet; complete route → verify trace added + highlight cleared; cancel with Escape → verify idle + highlight cleared; angle toggle with A → verify angleSnapEnabled flips; keyboard F → verify layer flip. Use page.evaluate for coordinate math, generous pad hit tolerance.
  - Verify: `cd viewer && npx playwright test e2e/routing-ux.spec.ts` all pass, `cd viewer && npx playwright test` full suite green (minus known flake)
  - Done when: ≥5 E2E tests pass covering start/complete/cancel/highlight/toggle flows

## Files Likely Touched

- `viewer/src/routing.ts`
- `viewer/src/interaction.ts`
- `viewer/src/renderer.ts`
- `viewer/src/main.ts`
- `viewer/src/__tests__/routing.test.ts`
- `viewer/e2e/routing-ux.spec.ts`
- `viewer/e2e/fixtures/routing-test.cypcb`
