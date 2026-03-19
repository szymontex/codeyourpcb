# S05: Realtime Tuning Parameters

**Goal:** User adjusts via-cost / layer-preference / roundness / density sliders and the board re-routes within ~1s, visible on canvas. Parameters persist across sessions.

**Demo:** Open a board, click Route. Open the tuning panel. Move the "Via Cost" slider — board re-routes automatically with fewer/more vias. Move "Roundness" — corner chamfers change. All changes appear on canvas within ~1s (led_blink-level boards).

## Must-Haves

- `AutorouteParams` struct (user-facing subset): `via_cost: f64`, `layer_preference: f64`, `roundness: f64`, `density: f64` with defaults and value clamping
- `AutorouteConfig` consumes `AutorouteParams` — params flow through cost functions and smoother
- Smoother's chamfer aggressiveness parameterized by roundness (0.0 = skip chamfer, 1.0 = max chamfer)
- Layer preference param maps to asymmetric layer_change_cost bias in `RoutingCost`
- Density param scales grid resolution (clamped 0.5×–2.0× auto-derived, floor 10µm)
- WASM `auto_route_with_params(params_json: String) -> String` entry point (existing `auto_route()` unchanged)
- `PcbEngine` interface + `WasmPcbEngineAdapter` + `MockPcbEngine` extended with `auto_route_with_params`
- Collapsible "Tuning" slider panel in viewer (4 sliders: Via Cost, Layer Preference, Roundness, Density)
- Slider changes debounced at 300ms, trigger `auto_route_with_params()` automatically
- Slider values persisted in `AppSettings` via `setPreference()`
- "Routing..." indicator shown during re-route

## Proof Level

- This slice proves: integration (Rust params → WASM bridge → viewer UI → canvas update)
- Real runtime required: yes (WASM build + browser rendering)
- Human/UAT required: no (automated tests verify params flow and UI wiring)

## Verification

- `cargo test -p cypcb-autoroute --lib --release` — all existing + new AutorouteParams unit tests pass
- `cargo test --test tuning_params --release` — integration test: route led_blink with different params, verify score changes (higher via_cost → fewer vias, roundness=0 → lower smoothness vs roundness=1)
- `cargo check -p cypcb-render --target wasm32-unknown-unknown` — WASM compiles with new entry point
- `npx vitest run --reporter=verbose` — viewer unit tests pass including new settings field
- Playwright E2E: tuning panel opens/closes, slider changes trigger re-route, "Routing..." indicator appears
- `cargo test -p cypcb-autoroute --lib --release -- params_from_json_invalid` — deserialization error returns defaults (failure-path check)
- WASM `auto_route_with_params` returns `{"ok":false,"error":"..."}` on malformed JSON input (verified in integration test)

## Observability / Diagnostics

- Runtime signals: `tracing::info!` in `auto_route_with_params` logs received params; smoother logs roundness-adjusted chamfer cap
- Inspection surfaces: `window.__settings` exposes current `autorouteParams`; `window.__tuningPanel` debug surface exposes slider state
- Failure visibility: WASM `auto_route_with_params` returns `{"ok":false,"error":"..."}` on deserialization failure; console.warn on invalid param JSON

## Integration Closure

- Upstream surfaces consumed: `AutorouteConfig` (lib.rs), `smooth_routes()` (smoother.rs), `RoutingCost` (cost.rs), `auto_route()` (cypcb-render/lib.rs), `PcbEngine` interface (wasm.ts), `AppSettings` (settings.ts), `triggerRouting()` (main.ts)
- New wiring introduced: `AutorouteParams` → `AutorouteConfig` consumption, `auto_route_with_params` WASM method, `PcbEngine.auto_route_with_params` interface method, tuning panel ↔ settings ↔ routing pipeline
- What remains before the milestone is truly usable end-to-end: S06 (variant generation + preview UI), S07 (benchmark validation)

## Tasks

- [x] **T01: Add AutorouteParams struct, parameterize routing engine and smoother, add WASM entry point** `est:45m`
  - Why: The entire tuning pipeline starts from Rust — params must influence cost functions, smoother chamfer, grid resolution, and be receivable via WASM
  - Files: `crates/cypcb-autoroute/src/lib.rs`, `crates/cypcb-autoroute/src/smoother.rs`, `crates/cypcb-autoroute/src/cost.rs`, `crates/cypcb-render/src/lib.rs`, `crates/cypcb-autoroute/tests/tuning_params.rs`
  - Do: (1) Create `AutorouteParams` with serde derives and clamped defaults. (2) Add `params` field to `AutorouteConfig`, wire `via_cost` → `via_cost_multiplier`, `layer_preference` → cost bias, `density` → grid resolution scale. (3) Add `roundness` parameter to `smooth_routes()` controlling chamfer aggressiveness. (4) Thread `config.params.roundness` through both strategies' smoother calls. (5) Add `auto_route_with_params(params_json)` to `PcbEngine` in cypcb-render. (6) Unit tests for AutorouteParams defaults/clamping, integration test routing with different params.
  - Verify: `cargo test -p cypcb-autoroute --lib --release` passes, `cargo test --test tuning_params --release` passes, `cargo check -p cypcb-render --target wasm32-unknown-unknown` succeeds
  - Done when: Different `AutorouteParams` produce measurably different routing results (score differences in integration test), WASM entry point compiles

- [x] **T02: Extend PcbEngine interface, build tuning slider panel with debounced reactive re-routing** `est:45m`
  - Why: Completes the user-facing loop — sliders in the viewer call the WASM entry point and update the canvas
  - Files: `viewer/src/wasm.ts`, `viewer/src/settings.ts`, `viewer/src/main.ts`, `viewer/index.html`
  - Do: (1) Add `auto_route_with_params(params: string): string` to `PcbEngine` interface, `WasmPcbEngineAdapter`, and `MockPcbEngine`. (2) Add `autorouteParams` field to `AppSettings` with defaults. (3) Build collapsible tuning panel HTML/CSS adjacent to Route button area (4 range sliders). (4) Wire slider `input` events with 300ms debounce to call `auto_route_with_params()` → `pullSnapshot()` → `dirty = true`. (5) Show "Routing..." during re-route. (6) Persist slider values via `setPreference()`. (7) Expose `window.__tuningPanel` debug surface. (8) Add Playwright E2E test verifying panel visibility, slider interaction, and re-route trigger.
  - Verify: `npx vitest run` passes, Playwright E2E test passes (`npx playwright test tuning`), manual verification of slider → re-route → canvas update
  - Done when: Moving a slider in the browser causes the board to re-route with updated parameters and the canvas reflects the change

## Files Likely Touched

- `crates/cypcb-autoroute/src/lib.rs` — AutorouteParams struct, AutorouteConfig integration
- `crates/cypcb-autoroute/src/smoother.rs` — roundness parameter in smooth_routes() and chamfer_corners()
- `crates/cypcb-autoroute/src/cost.rs` — layer_preference bias in neighbor_cost()
- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — thread roundness to smoother call
- `crates/cypcb-autoroute/src/astar_improved.rs` — thread roundness to smoother call
- `crates/cypcb-autoroute/tests/tuning_params.rs` — integration test
- `crates/cypcb-render/src/lib.rs` — auto_route_with_params() WASM method
- `viewer/src/wasm.ts` — PcbEngine interface + adapter + mock extension
- `viewer/src/settings.ts` — autorouteParams field in AppSettings
- `viewer/src/main.ts` — tuning panel logic, debounced re-routing
- `viewer/index.html` — tuning panel HTML/CSS
