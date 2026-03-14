---
id: T02
parent: S03
milestone: M003
provides:
  - routing-test.cypcb fixture (3 components, 3 nets, known positions)
  - 6 E2E tests covering start/complete/cancel/highlight/angle-toggle/layer-flip routing flows
  - __viewport diagnostic surface for E2E coordinate computation
  - WasmPcbEngineAdapter JS fallback for add_trace/remove_trace when WASM module lacks those methods
key_files:
  - viewer/e2e/fixtures/routing-test.cypcb
  - viewer/e2e/routing-ux.spec.ts
  - viewer/src/main.ts
  - viewer/src/wasm.ts
key_decisions:
  - Exposed __viewport diagnostic surface with live getters (centerX, centerY, scale, width, height) so E2E tests can accurately compute pad screen coordinates without reimplementing fitBoard logic
  - WasmPcbEngineAdapter now falls back to JS-side snapshot mutation for add_trace/remove_trace/run_drc_incremental/trace_count when WASM module doesn't expose those methods — fixes pre-existing crash during route completion
patterns_established:
  - getPadScreenCoords helper reads __viewport + __pcbEngine snapshot to convert pad world positions to page coordinates for reliable Playwright clicks
  - loadFixture helper loads .cypcb via __loadBoard then waits 600ms for render settle
observability_surfaces:
  - window.__viewport — live viewport state for E2E tests
  - window.__routingState — routing mode, net, snap state, target pads
  - window.__renderDiag.highlightedNet — net highlight during/after routing
duration: 35min
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T02: E2E routing tests with test fixture

**Built deterministic routing test fixture and 6 Playwright E2E tests exercising full routing UX flow — start, complete, cancel, highlight, angle toggle, layer flip — all via diagnostic surfaces.**

## What Happened

Created `routing-test.cypcb` with R1 (20mm,30mm), R2 (50mm,30mm), LED1 (50mm,50mm) and three nets (POWER, SIGNAL, GROUND). Each net connects exactly 2 pads for clean routing tests.

First run: all pad clicks missed because `__loadBoard` wasn't syncing `interactionState.viewport` — the click handler's `screenToWorld` used stale default viewport coords. Fixed by adding `interactionState.viewport = viewport` and `interactionState.snapshot = snapshot` sync after fitBoard in `__loadBoard`.

Second issue: route completion crashed with `add_trace_json is not a function` — the WASM module doesn't export trace mutation methods. The `WasmPcbEngineAdapter` was calling non-existent WASM functions directly. Added JS-side fallback that mutates the cached snapshot (same logic as MockPcbEngine) when WASM methods aren't available. Same treatment for `remove_trace`, `run_drc_incremental`, and `trace_count`.

The viewport fix exposed a latent issue in `renderer-quality.spec.ts` — its "click canvas for focus then press F" pattern now correctly hit-tests pads (as intended by the fix). Adjusted that test to press F without a canvas click and add an Escape guard.

## Verification

- `cd viewer && npx playwright test e2e/routing-ux.spec.ts` — 6/6 pass
- `cd viewer && npx playwright test` — 58/58 pass (0 failures)
- `cd viewer && npx vitest run` — 77/77 unit tests pass (7 files)
- `cd viewer && npx tsc --noEmit` — zero TypeScript errors

## Diagnostics

- `window.__viewport` — live viewport for coordinate checks
- `window.__routingState` — full routing state inspection
- `window.__renderDiag.highlightedNet` — net highlight state
- Console: `[Route] idle → routing: ...` and `[Route] routing → idle: ...` log state transitions

## Deviations

- Fixed `__loadBoard` viewport/snapshot sync bug (required for any E2E routing test to work)
- Added JS-side fallback for trace mutations in WasmPcbEngineAdapter (required for route completion to succeed)
- Fixed `renderer-quality.spec.ts` net highlight test affected by viewport sync fix

## Known Issues

- WASM module (`cypcb_render.wasm`) lacks `add_trace_json`, `remove_trace`, `run_drc_incremental`, `trace_count`, `get_trace_at_point`, `rotate_component` methods — all trace/mutation operations run in JS fallback. Not blocking; the JS fallback is functionally equivalent.

## Files Created/Modified

- `viewer/e2e/fixtures/routing-test.cypcb` — test fixture with 3 components and 3 nets
- `viewer/e2e/routing-ux.spec.ts` — 6 E2E tests for routing UX flow
- `viewer/src/main.ts` — added __viewport diagnostic surface; fixed __loadBoard interactionState sync
- `viewer/src/wasm.ts` — JS fallback for add_trace/remove_trace/run_drc_incremental/trace_count in WasmPcbEngineAdapter
- `viewer/e2e/renderer-quality.spec.ts` — fixed net highlight test to avoid accidental pad click
