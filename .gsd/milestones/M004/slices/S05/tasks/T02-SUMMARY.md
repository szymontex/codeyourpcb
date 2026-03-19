---
id: T02
parent: S05
milestone: M004
provides:
  - PcbEngine.auto_route_with_params() in interface + WasmPcbEngineAdapter + MockPcbEngine
  - AutorouteParams TypeScript interface in settings.ts with deep-copy persistence
  - Collapsible tuning panel with 4 range sliders (Via Cost, Layer Preference, Roundness, Density)
  - Debounced (300ms) reactive re-routing via auto_route_with_params on slider input
  - window.__tuningPanel debug surface exposing panel visibility and current params
  - Playwright E2E test suite (7 tests) verifying panel interaction, persistence, and re-route trigger
key_files:
  - viewer/src/wasm.ts
  - viewer/src/settings.ts
  - viewer/src/main.ts
  - viewer/index.html
  - viewer/e2e/tuning-panel.spec.ts
key_decisions:
  - Tuning panel is a collapsible dropdown adjacent to Route button (⚡ toggle), not inside prefs modal
  - auto_route_with_params added alongside auto_route, not replacing it
  - Slider debounce at 300ms using setTimeout pattern (matches editor change debounce)
  - Rust-side snake_case field names in JSON (via_cost, layer_preference, roundness, density); TypeScript-side camelCase (viaCost, layerPreference, roundness, density)
patterns_established:
  - AutorouteParams deep-copied in getPreference/setPreference/getSettings/resetSettings/loadFromStorage (same pattern as layerColors)
  - Tuning slider panel follows view-menu-dropdown CSS pattern (position:absolute, top:100%, z-index:160)
  - window.__tuningPanel debug surface for E2E inspection of slider state
observability_surfaces:
  - window.__tuningPanel exposes { visible: boolean, params: AutorouteParams }
  - window.__settings.autorouteParams reflects persisted values
  - console.log('[Tuning] Re-routing with params:', rustParams) on debounced re-route
  - console.log('[Tuning] Panel visible/hidden') on toggle
  - console.warn('[Tuning] Route failed/error:') on WASM failure
duration: 20m
verification_result: passed
completed_at: 2026-03-14
blocker_discovered: false
---

# T02: Extend PcbEngine interface, build tuning slider panel with debounced reactive re-routing

**Extended PcbEngine with auto_route_with_params, built collapsible 4-slider tuning panel with 300ms debounced re-routing, persisted params in AppSettings, and added 7-test Playwright E2E suite.**

## What Happened

1. **PcbEngine interface extended** in `wasm.ts`: Added `auto_route_with_params(params: string): string` to the `PcbEngine` interface, `WasmPcbEngine` raw interface, `WasmPcbEngineAdapter` (calls WASM + invalidates cached snapshot), and `MockPcbEngine` (returns mock error JSON).

2. **Settings persistence** in `settings.ts`: Defined `AutorouteParams` TypeScript interface with 4 fields. Added `autorouteParams` to `AppSettings` with defaults `{ viaCost: 1.0, layerPreference: 0.0, roundness: 0.5, density: 1.0 }`. Deep-copy applied in all get/set/reset paths, matching the `layerColors` pattern.

3. **Tuning panel HTML/CSS** in `index.html`: Added `<button id="tuning-toggle">⚡</button>` after cancel-route-btn, wrapped in `.toolbar-anchor` span. Collapsible `#tuning-panel` div with 4 range inputs (Via Cost 0.1–10, Layer Preference -1–1, Roundness 0–1, Density 0.5–2). Each row has label, slider, and monospace value display. Styled as dropdown panel matching view-menu-dropdown pattern.

4. **Slider event wiring** in `main.ts`: On page load, sliders initialized from `getPreference('autorouteParams')`. Each slider's `input` event updates value display, persists via `setPreference()`, and triggers 300ms debounced re-route. Debounce fires `engine.auto_route_with_params()` with Rust-side snake_case JSON, then `pullSnapshot()` + `dirty = true`. Route button shows "Routing..." during call via `updateRoutingUI`. Click-outside closes panel. `window.__tuningPanel` debug surface exposed with live params and visibility state.

5. **Playwright E2E test** in `viewer/e2e/tuning-panel.spec.ts`: 7 tests covering toggle button existence, panel show/hide, 4 slider defaults, value display updates on slider change, settings persistence to `__settings`, debug surface accuracy, localStorage persistence across reload, and debounced re-route trigger confirmation via console log inspection.

## Verification

- `npx vitest run --reporter=verbose` — **127 tests passed** (all existing viewer unit tests, no regressions)
- `npx playwright test tuning --reporter=list` — **7 tests passed** (all tuning panel E2E tests)
- Browser manual verification: loaded board, opened tuning panel, verified 4 sliders render correctly with labels/values, panel toggles on button click

### Slice-level verification status (T02 of 2 — final task):
- ✅ `cargo test -p cypcb-autoroute --lib --release` — 118 passed
- ✅ `cargo test --test tuning_params --release` — 4 passed
- ✅ `cargo check -p cypcb-render --target wasm32-unknown-unknown` — compiles clean
- ✅ `npx vitest run --reporter=verbose` — 127 passed
- ✅ Playwright E2E tuning panel — 7 passed (panel opens/closes, sliders work, settings persist, re-route triggers)
- ✅ `cargo test -p cypcb-autoroute --lib --release -- params_from_json_invalid` — 1 passed (failure-path check)
- ✅ WASM `auto_route_with_params` returns `{"ok":false,"error":"..."}` on malformed JSON (verified in integration test)

## Diagnostics

- `window.__tuningPanel` — `{ visible: boolean, params: { viaCost, layerPreference, roundness, density } }` for E2E and runtime inspection
- `window.__settings.autorouteParams` — persisted values via existing settings debug surface
- Console logs prefixed with `[Tuning]` — panel toggle, re-route trigger with params, route failure warnings
- Route button shows "Routing..." text during debounced re-route (same UI pattern as triggerRouting)

## Deviations

- None

## Known Issues

- None

## Files Created/Modified

- `viewer/src/wasm.ts` — Added `auto_route_with_params` to PcbEngine interface, WasmPcbEngine raw interface, WasmPcbEngineAdapter, and MockPcbEngine
- `viewer/src/settings.ts` — Added `AutorouteParams` interface, `autorouteParams` field in AppSettings with defaults, deep-copy in all get/set/reset/load paths
- `viewer/src/main.ts` — Added tuning panel logic: element refs, toggle handler, slider initialization from settings, debounced re-route wiring, debug surface
- `viewer/index.html` — Added tuning toggle button, tuning panel HTML with 4 range sliders, CSS styles matching view-menu-dropdown pattern
- `viewer/e2e/tuning-panel.spec.ts` — Created 7-test Playwright E2E suite for tuning panel
- `.gsd/milestones/M004/slices/S05/tasks/T02-PLAN.md` — Added Observability Impact section (pre-flight fix)
