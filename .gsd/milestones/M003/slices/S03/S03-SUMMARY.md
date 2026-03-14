---
id: S03
parent: M003
milestone: M003
provides:
  - Net-aware target pad highlighting during routing (set highlightedNet, S01 glow/dim renders it)
  - Ratsnest emphasis — active net at full alpha + 2x width, others dimmed to 0.15
  - Magnetic snap to destination pad center (dual threshold: 1mm world OR 15px screen)
  - Angle constraint toggle via A key (defaults OFF per roadmap)
  - Keyboard handlers (Escape cancel, F flip layer, A angle toggle) with editor guard
  - Pre-computed target pad list at route start for efficient per-frame lookup
  - Snap visual indicator (pulsing circle + crosshair) on target pad
  - routing-test.cypcb fixture (3 components, 3 nets, known positions)
  - 6 E2E tests covering start/complete/cancel/highlight/angle-toggle/layer-flip
  - __viewport diagnostic surface for E2E coordinate computation
  - WasmPcbEngineAdapter JS fallback for trace mutations
requires:
  - slice: S01
    provides: Professional 2D renderer with pad highlighting, net labels, RenderConfig, buildPadNetMap
affects:
  - S07
key_files:
  - viewer/src/routing.ts
  - viewer/src/interaction.ts
  - viewer/src/renderer.ts
  - viewer/src/main.ts
  - viewer/src/wasm.ts
  - viewer/src/__tests__/routing.test.ts
  - viewer/e2e/routing-ux.spec.ts
  - viewer/e2e/fixtures/routing-test.cypcb
key_decisions:
  - Magnetic snap uses dual threshold (1mm world OR 15px/scale) — usable at any zoom level
  - Angle snap defaults OFF — roadmap says "optional toggle, not forced"
  - Magnetic snap takes priority over angle snap when both active
  - interaction.ts owns routing keyboard shortcuts (Escape/F/A) — co-located with routing state
  - resetToIdle() preserves user preferences (grid snap, angle snap, magnetic snap) across route lifecycle
  - Target pads pre-computed once at startRoute(), not scanned per frame
  - WasmPcbEngineAdapter JS fallback for add_trace/remove_trace — WASM module lacks these exports
  - __viewport diagnostic surface exposed with live getters for E2E coordinate computation
  - __loadBoard syncs interactionState.viewport + snapshot — critical for pad hit-testing
patterns_established:
  - onRouteStart/onRouteEnd callbacks on InteractionState for highlight lifecycle management
  - getPadScreenCoords E2E helper reads __viewport + __pcbEngine snapshot for reliable Playwright clicks
  - loadFixture E2E helper loads .cypcb via __loadBoard then waits for render settle
observability_surfaces:
  - window.__routingState.angleSnapEnabled
  - window.__routingState.magneticSnapEnabled
  - window.__routingState.snappedToPad
  - window.__routingState.targetPadsCount
  - window.__viewport (centerX, centerY, scale, width, height)
  - window.__renderDiag.highlightedNet
drill_down_paths:
  - .gsd/milestones/M003/slices/S03/tasks/T01-SUMMARY.md
  - .gsd/milestones/M003/slices/S03/tasks/T02-SUMMARY.md
duration: 2 tasks
verification_result: passed
completed_at: 2026-03-13
---

# S03: Routing UX Upgrade

**Pad-to-pad routing with net-aware target highlighting, ratsnest guide, magnetic snap, and angle constraint toggle — verified by 6 E2E tests and 14 unit tests.**

## What Happened

T01 extended the routing state machine with five new fields: `angleSnapEnabled`, `magneticSnapEnabled`, `magneticSnapRadius`, `snappedToPad`, `targetPads`. On route start, `computeTargetPads()` scans the snapshot once for same-net pads (excluding start pad). During `updatePreview()`, `findNearestTargetPad()` checks dual threshold (1mm world OR 15px/scale, whichever larger). When a target is in range, the preview endpoint snaps to pad center and `snappedToPad` is set. Angle snap (45°/90°) only applies when enabled AND no magnetic snap is active — magnetic snap takes priority.

Keyboard handler added to `setupInteraction()` in interaction.ts: Escape cancels route, F flips layer, A toggles angle snap. All guarded by routing mode check and Monaco editor focus detection. Renderer updated with pulsing snap indicator on target pad and ratsnest emphasis (active net at full alpha + 2x width, others dimmed).

T02 built a deterministic `routing-test.cypcb` fixture (R1, R2, LED1 with POWER/SIGNAL/GROUND nets) and 6 Playwright E2E tests. First run exposed a latent bug: `__loadBoard` wasn't syncing `interactionState.viewport`, causing all pad hit-tests to miss (screenToWorld used stale defaults). Fixed by syncing viewport+snapshot in `__loadBoard`. Route completion then crashed — WASM module lacks `add_trace_json` export. Added JS-side fallback in `WasmPcbEngineAdapter` that mutates the cached snapshot directly (same logic as MockPcbEngine). Both fixes were necessary for routing to work in any real scenario.

## Verification

- `npx tsc --noEmit` — zero TypeScript errors
- `cd viewer && npx vitest run` — 77/77 tests pass (14 new routing unit tests)
- `cd viewer && npx playwright test e2e/routing-ux.spec.ts` — 6/6 pass
- `cd viewer && npx playwright test` — 58/58 pass (0 failures)
- `window.__routingState` exposes all new fields (verified in E2E)
- `window.__renderDiag.highlightedNet` set during routing, cleared after (verified in E2E)
- `window.__viewport` provides live viewport state (verified in E2E)

## Requirements Advanced

- None moved to new status — routing UX is a quality improvement on existing manual trace editing capability

## Requirements Validated

- None newly validated — existing "Manual trace editing" requirement was already validated in M002

## New Requirements Surfaced

- None

## Requirements Invalidated or Re-scoped

- None

## Deviations

- Fixed `__loadBoard` interactionState viewport/snapshot sync bug — not in the plan, but required for any routing E2E test (and any real file loading) to work
- Added JS-side fallback for trace mutations in WasmPcbEngineAdapter — WASM module doesn't export `add_trace_json`/`remove_trace`; JS fallback was necessary for route completion
- Fixed `renderer-quality.spec.ts` net highlight test — viewport sync fix changed hit-test behavior, existing test needed adjustment

## Known Limitations

- WASM module lacks `add_trace_json`, `remove_trace`, `run_drc_incremental`, `trace_count`, `get_trace_at_point`, `rotate_component` — all trace/mutation operations run via JS fallback. Functionally equivalent but not native WASM performance.
- Magnetic snap visual indicator is a simple pulsing circle — no directional approach animation
- No snap sound/haptic feedback (web platform limitation)

## Follow-ups

- S07 will consume the testable routing state machine for extended E2E verification
- WASM trace mutation exports should be added when Rust engine is next modified — removes JS fallback layer

## Files Created/Modified

- `viewer/src/routing.ts` — Extended RoutingState with magnetic snap, angle toggle, target pads, resetToIdle
- `viewer/src/interaction.ts` — Keyboard handler (Escape/F/A) with editor guard, onRouteStart/onRouteEnd callbacks, viewport scale pass-through
- `viewer/src/renderer.ts` — Snap indicator in drawRoutingPreview, ratsnest emphasis in drawRatsnest
- `viewer/src/main.ts` — __viewport diagnostic surface, __loadBoard viewport/snapshot sync, extended __routingState, onRouteStart/onRouteEnd wiring for highlightedNet lifecycle
- `viewer/src/wasm.ts` — JS fallback for add_trace/remove_trace/run_drc_incremental/trace_count in WasmPcbEngineAdapter
- `viewer/src/__tests__/routing.test.ts` — 14 unit tests for routing UX features
- `viewer/e2e/routing-ux.spec.ts` — 6 E2E tests for routing flow
- `viewer/e2e/fixtures/routing-test.cypcb` — Test fixture with 3 components and 3 nets
- `viewer/e2e/renderer-quality.spec.ts` — Fixed net highlight test to avoid accidental pad click

## Forward Intelligence

### What the next slice should know
- Routing keyboard shortcuts live in interaction.ts, not main.ts — any new keyboard handlers should follow the same pattern (document-level listener with routing mode guard + editor focus check)
- `__loadBoard` now properly syncs interactionState — new E2E tests that load boards via this surface can rely on immediate viewport accuracy

### What's fragile
- WasmPcbEngineAdapter JS fallback for trace mutations — works but adds a layer that diverges from WASM. If Rust engine adds `add_trace_json` export, the adapter must be updated to prefer the WASM path.
- Pad hit-testing depends on interactionState.viewport being synced — if any code path loads a board without calling the sync (e.g. URL-based load), hits will miss

### Authoritative diagnostics
- `window.__routingState` — complete routing state machine state, trustworthy because E2E tests validate it every run
- `window.__viewport` — live viewport state, trustworthy because E2E coordinate math depends on it

### What assumptions changed
- Assumed WASM module had trace mutation APIs — it doesn't. JS fallback was necessary and is now the stable path until Rust exports are added.
- Assumed `__loadBoard` already synced interaction state — it didn't. This was a latent bug affecting all file-loading scenarios, not just E2E tests.
