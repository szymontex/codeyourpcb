---
estimated_steps: 5
estimated_files: 5
---

# T02: Extend PcbEngine interface, build tuning slider panel with debounced reactive re-routing

**Slice:** S05 — Realtime Tuning Parameters
**Milestone:** M004

## Description

Complete the user-facing loop: extend the TypeScript `PcbEngine` interface with `auto_route_with_params`, build a collapsible slider panel in the viewer with 4 range inputs (Via Cost, Layer Preference, Roundness, Density), debounce slider changes at 300ms to trigger re-routing, persist slider values in `AppSettings`, and add E2E test verifying the full flow.

## Steps

1. **Extend `PcbEngine` interface and implementations in `wasm.ts`**:
   - Add `auto_route_with_params(params: string): string` to `PcbEngine` interface
   - In `WasmPcbEngineAdapter`: call `this.wasmEngine.auto_route_with_params(params)`, invalidate cached snapshot (same pattern as `auto_route()`)
   - Add `auto_route_with_params` to `WasmPcbEngine` raw interface
   - In `MockPcbEngine`: return `'{"ok":false,"error":"Autorouter not available in mock mode"}'` (same as `auto_route()`)

2. **Add `autorouteParams` to settings in `settings.ts`**:
   - Define `AutorouteParams` TypeScript interface: `{ viaCost: number; layerPreference: number; roundness: number; density: number }`
   - Add `autorouteParams: AutorouteParams` to `AppSettings` interface
   - Add default to `DEFAULT_SETTINGS`: `{ viaCost: 1.0, layerPreference: 0.0, roundness: 0.5, density: 1.0 }`
   - Deep-copy `autorouteParams` in `getPreference()` / `setPreference()` (same pattern as `layerColors`)

3. **Build tuning panel HTML/CSS in `index.html`**:
   - Add a collapsible `<div id="tuning-panel">` adjacent to the Route button area (after `#cancel-route-btn`, still inside toolbar)
   - Toggle button: `<button id="tuning-toggle" title="Tuning parameters">⚡</button>` next to Route
   - Panel content (initially hidden): 4 range sliders with labels and value displays
     - Via Cost: `<input type="range" id="tune-via-cost" min="0.1" max="10" step="0.1" value="1.0">`
     - Layer Preference: `<input type="range" id="tune-layer-pref" min="-1" max="1" step="0.1" value="0">`
     - Roundness: `<input type="range" id="tune-roundness" min="0" max="1" step="0.05" value="0.5">`
     - Density: `<input type="range" id="tune-density" min="0.5" max="2" step="0.1" value="1.0">`
   - Style: dropdown panel below toolbar (similar to view dropdown pattern, z-index 50), compact layout
   - Each slider has a `<span class="tune-value">` showing current numeric value

4. **Wire slider events with debounced re-routing in `main.ts`**:
   - Get references to tuning panel elements
   - Toggle button click: show/hide panel
   - On page load: read `autorouteParams` from settings, set slider values
   - For each slider `input` event: update the value display span, update `autorouteParams` in settings via `setPreference()`
   - Debounce: use 300ms `setTimeout` pattern (clear previous timer on each input). After debounce fires, if board is loaded:
     - Build params JSON: `{ "via_cost": N, "layer_preference": N, "roundness": N, "density": N }` (Rust field names, snake_case)
     - Call `engine.auto_route_with_params(JSON.stringify(params))`
     - Parse result, `pullSnapshot()`, `dirty = true`
     - Show "Routing..." on Route button during call (reuse `updateRoutingUI`)
   - Expose `window.__tuningPanel` debug surface: `{ visible: boolean, params: AutorouteParams }`

5. **Add Playwright E2E test `viewer/e2e/tuning-panel.spec.ts`**:
   - Load a board via `__loadBoard()`
   - Assert tuning toggle button exists
   - Click toggle → panel becomes visible
   - Assert 4 sliders exist with correct default values
   - Change a slider value (via `page.fill` or `page.evaluate` to set input value + dispatch input event)
   - Assert settings updated in `__settings.autorouteParams`
   - Click toggle again → panel hides
   - Verify panel state persists across reload (read settings from localStorage)

## Must-Haves

- [ ] `PcbEngine.auto_route_with_params(params: string): string` in interface + both implementations
- [ ] `autorouteParams` field in `AppSettings` with defaults, persisted to localStorage
- [ ] Tuning panel with 4 range sliders, collapsible via toggle button
- [ ] Slider input events debounced at 300ms, trigger `auto_route_with_params()`
- [ ] "Routing..." indicator during re-route
- [ ] Slider values read from settings on page load (persist across sessions)
- [ ] `window.__tuningPanel` debug surface exposed
- [ ] Playwright E2E test passes

## Verification

- `npx vitest run --reporter=verbose` — existing viewer unit tests still pass
- `npx playwright test tuning` — E2E test for tuning panel passes
- Manual: open viewer, load board, open tuning panel, move slider → board re-routes (WASM build required)

## Observability Impact

- **New inspection surface:** `window.__tuningPanel` exposes `{ visible: boolean, params: AutorouteParams }` — future agents can read current slider state programmatically
- **Settings persistence:** `window.__settings.autorouteParams` reflects persisted tuning values (already exposed via existing `__settings` debug surface)
- **Console signals:** `console.warn` on invalid params JSON from `auto_route_with_params` result; `console.log` on panel toggle and debounced re-route trigger
- **Failure visibility:** `auto_route_with_params` returns `{"ok":false,"error":"..."}` on WASM/mock failure — result logged to console and status bar shows error
- **UI indicator:** Route button shows "Routing..." text during debounced re-route, same pattern as `triggerRouting()`

## Inputs

- T01 output: `auto_route_with_params()` WASM method available, `AutorouteParams` JSON contract defined
- `viewer/src/wasm.ts` — current `PcbEngine` interface and implementations
- `viewer/src/settings.ts` — `AppSettings` pattern with `setPreference()` / `getPreference()`
- `viewer/src/main.ts` — `triggerRouting()` pattern, `pullSnapshot()`, `updateRoutingUI()`
- `viewer/index.html` — Route button area (line 931-933), view dropdown CSS pattern

## Expected Output

- `viewer/src/wasm.ts` — `PcbEngine` interface extended with `auto_route_with_params`
- `viewer/src/settings.ts` — `AutorouteParams` type, `autorouteParams` in `AppSettings`
- `viewer/src/main.ts` — tuning panel logic, debounced re-routing, debug surface
- `viewer/index.html` — tuning panel HTML/CSS
- `viewer/e2e/tuning-panel.spec.ts` — E2E test for tuning panel
