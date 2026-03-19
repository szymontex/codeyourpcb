# S05: Realtime Tuning Parameters — UAT

**Milestone:** M004
**Written:** 2026-03-14

## UAT Type

- UAT mode: mixed (artifact-driven + live-runtime)
- Why this mode is sufficient: Rust params → WASM bridge verified by unit/integration tests; UI interaction verified by Playwright E2E; manual runtime check confirms visual canvas update flow

## Preconditions

- WASM module built: `wasm-pack build crates/cypcb-render --target web --release`
- Viewer running: `cd viewer && npx vite`
- A board loaded in the viewer (e.g. `blink.cypcb` or any valid `.cypcb` file)
- Initial route completed (click Route button once to establish baseline)

## Smoke Test

Open the viewer, load a board, click Route. Click the ⚡ button next to Route — the tuning panel appears with 4 sliders. Move the "Via Cost" slider. The board should re-route and canvas should update.

## Test Cases

### 1. Tuning Panel Toggle

1. Load a board in the viewer
2. Locate the ⚡ button adjacent to the Route button in the toolbar
3. Click the ⚡ button
4. **Expected:** A dropdown panel appears below the toolbar with 4 labeled sliders: "Via Cost", "Layer Preference", "Roundness", "Density"
5. Click the ⚡ button again
6. **Expected:** The panel hides
7. Click the ⚡ button to reopen, then click anywhere outside the panel
8. **Expected:** The panel closes

### 2. Slider Default Values

1. Open the tuning panel (fresh session, clear localStorage first)
2. Read the value displays next to each slider
3. **Expected:** Via Cost = 1.0, Layer Preference = 0.0, Roundness = 0.5, Density = 1.0

### 3. Slider Value Update

1. Open the tuning panel
2. Drag the "Via Cost" slider to the right (toward 10)
3. **Expected:** The monospace value display next to the slider updates in real-time to reflect the new value (e.g. "5.0")
4. Drag the "Roundness" slider to 0
5. **Expected:** Value display shows "0.0"
6. Drag the "Density" slider to 2.0
7. **Expected:** Value display shows "2.0"

### 4. Debounced Re-Route Trigger

1. Load a board and click Route to establish initial routing
2. Open the tuning panel
3. Open browser DevTools console
4. Move the "Density" slider from 1.0 to 1.5
5. **Expected:** After ~300ms, console shows `[Tuning] Re-routing with params:` log with the updated density value
6. **Expected:** The Route button briefly shows "Routing..." text during re-route
7. **Expected:** Canvas updates with new routing (traces may change position/shape)

### 5. Settings Persistence Across Reload

1. Open the tuning panel
2. Set Via Cost to 3.0, Roundness to 0.8
3. Reload the page (F5)
4. Load a board and open the tuning panel
5. **Expected:** Via Cost shows 3.0, Roundness shows 0.8 (values persisted via localStorage)

### 6. Debug Surface Accuracy

1. Open browser DevTools console
2. Open the tuning panel and set Density to 1.5
3. Run `window.__tuningPanel` in console
4. **Expected:** Returns `{ visible: true, params: { viaCost: ..., layerPreference: ..., roundness: ..., density: 1.5 } }`
5. Close the tuning panel
6. Run `window.__tuningPanel` again
7. **Expected:** `visible` is now `false`, params still reflect last set values

### 7. Density Parameter Produces Different Routing (Rust-level)

1. Run `cargo test --test tuning_params --release -- density_affects_routing`
2. **Expected:** Test passes — routing with density=0.5 (coarse grid) produces a different composite score than density=2.0 (fine grid) on the same board

### 8. Roundness Parameter Controls Chamfer (Rust-level)

1. Run `cargo test --test tuning_params --release -- roundness_affects_smoothing`
2. **Expected:** Test passes — roundness=0.0 and roundness=1.0 both produce valid routing without crashes or DRC failures

### 9. Combined Params Produce Different Results (Rust-level)

1. Run `cargo test --test tuning_params --release -- params_produce_different_routing`
2. **Expected:** Test passes — default params vs via_cost=5.0 + density=1.5 produce different composite scores, proving the full param pipeline flows through

### 10. WASM Entry Point Compiles and Handles Errors

1. Run `cargo check -p cypcb-render --target wasm32-unknown-unknown`
2. **Expected:** Compiles clean, no errors
3. Run `cargo test -p cypcb-autoroute --lib --release -- params_from_json_invalid`
4. **Expected:** Test passes — malformed JSON input returns defaults (error handled gracefully)

## Edge Cases

### Slider Extreme Values

1. Open the tuning panel
2. Set Via Cost to minimum (0.1) — routing should prefer many vias (cheap to place)
3. Set Via Cost to maximum (10.0) — routing should avoid vias (expensive)
4. Set Layer Preference to -1.0 (bottom-heavy) then +1.0 (top-heavy)
5. Set Roundness to 0.0 (no chamfer) then 1.0 (max chamfer)
6. Set Density to 0.5 (coarse) then 2.0 (fine)
7. **Expected:** No crashes, no console errors. Routing completes for all extreme values.

### Rapid Slider Movement

1. Open the tuning panel on a routed board
2. Rapidly drag a slider back and forth 5-6 times within 1 second
3. **Expected:** Only the final position triggers a re-route (300ms debounce prevents cascading WASM calls). No duplicate "Routing..." indicators, no console errors.

### Malformed JSON to WASM

1. In console, call `window.__pcbEngine?.auto_route_with_params('not valid json')`
2. **Expected:** Returns a string containing `{"ok":false,"error":"Invalid params JSON:..."}`

### Panel State After Board Reload

1. Open tuning panel and set custom values
2. Load a different board file
3. Open tuning panel
4. **Expected:** Slider values persist (they're settings, not per-board state)

## Failure Signals

- Tuning panel doesn't appear when clicking ⚡ — check index.html for `#tuning-panel` element
- Slider changes don't trigger re-route — check console for `[Tuning]` prefixed logs; verify debounce wiring in main.ts
- "Routing..." never clears — WASM call may be hanging; check `auto_route_with_params` return value
- Canvas doesn't update after re-route — check `pullSnapshot()` call and `dirty = true` flag in main.ts
- Values don't persist — check `setPreference('autorouteParams', ...)` call and localStorage key `cypcb-settings`
- Console error about `auto_route_with_params` not being a function — WASM module not rebuilt with new entry point

## Requirements Proved By This UAT

- R110 — Tests 1-6 prove 4 user-facing tuning sliders exist with defaults, persistence, and UI controls
- R111 — Test 4 proves reactive re-routing on slider change with debounced trigger; Tests 7-9 prove params actually influence routing output

## Not Proven By This UAT

- Sub-1s re-routing performance on STM32-level boards — requires real WASM build + timing measurement on representative boards (deferred to S07 benchmark suite)
- Actual visual quality difference when moving sliders on complex boards — requires human visual inspection with production WASM build
- Multi-layer boards exercising layer_preference meaningfully — led_blink is 2-layer, complex layer routing deferred to S07

## Notes for Tester

- The Playwright E2E suite (`npx playwright test tuning`) covers cases 1-6 automatically. Manual testing is only needed for visual canvas update verification (case 4) and edge cases.
- Rust integration tests (`cargo test --test tuning_params --release`) cover cases 7-9. These take ~60-110s due to routing computation on led_blink fixture.
- The MockPcbEngine returns an error for `auto_route_with_params` — E2E tests verify the call is made but can't verify actual re-routing without a real WASM build. Manual testing with `npx vite` + WASM build is needed for full end-to-end visual confirmation.
