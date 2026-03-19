# S05 ("Realtime Tuning Parameters") — Research

**Date:** 2026-03-14

## Summary

S05 delivers interactive routing parameter tuning: the user adjusts sliders (via cost, layer preference, density/spacing, roundness) and the board re-routes within ~1s. This requires changes across three layers: (1) a Rust `AutorouteParams` struct that influences routing cost functions and smoother behavior, (2) a WASM bridge method `auto_route_with_params(json)` that accepts user parameters, and (3) a viewer slider panel with debounced reactive re-routing.

The existing codebase is well-prepared. `AutorouteConfig` already has `via_cost_multiplier` and `prefer_top_layer` fields that flow through `RoutingCost::new()` into both PathFinder and ImprovedAStar strategies. The smoother's chamfer length (`min(len_a, len_b) / 3`, capped at 1mm) is a hardcoded constant ready to be parameterized for "roundness". The `RoutingRuleSet` trait's `via_cost()` and `layer_change_cost()` methods are the actual cost knobs. The viewer has an established settings persistence pattern (`AppSettings` + `setPreference()` + localStorage) and a prefs modal pattern to extend. The main risk is WASM routing time — PathFinder on led_blink takes a measurable fraction of a second, and complex boards could exceed the 1s budget.

The recommended approach: **extend `AutorouteConfig` with user-facing params, add a parameterized WASM entry point, build a lightweight slider panel (not inside prefs modal — separate collapsible panel near the Route button), and use input debouncing (300ms) to trigger re-routes.** No Web Workers needed for V1 — routing runs synchronously on the main thread (same as current `auto_route()`), with a "Routing..." status indicator. If boards exceed ~1s, the UI simply shows the progress and completes when ready.

## Recommendation

**Extend the existing `AutorouteConfig` directly** rather than creating a separate `AutorouteParams` struct. The config already has the right fields (`via_cost_multiplier`, `prefer_top_layer`) and flows through both strategies. Add new fields: `density_factor: f64` (multiplier on grid resolution — higher = denser/finer grid), `roundness: f64` (0.0–1.0, controls chamfer aggressiveness), and `layer_preference: f64` (-1.0 = bottom-heavy, 0.0 = balanced, 1.0 = top-heavy). The WASM bridge gets a new method `auto_route_with_params(params_json: String) -> String` that deserializes into `AutorouteConfig` — the existing `auto_route()` remains as the zero-param default. On the viewer side, a collapsible "Tuning" panel appears adjacent to the Route button area with 4 range sliders. Slider changes are debounced at 300ms and call `auto_route_with_params()` automatically.

However, the boundary map specifies `AutorouteParams` as a separate struct. Follow the boundary map: create `AutorouteParams` as the user-facing subset, and have `AutorouteConfig` consume it. This keeps the internal config clean while providing a focused API for the UI.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| Settings persistence | `viewer/src/settings.ts` — `setPreference()`/`getPreference()` with localStorage | Established pattern, auto-persists, notifies listeners, has debug surface |
| Routing cost adjustment | `AutorouteConfig.via_cost_multiplier` → `RoutingCost::new()` | Already flows through both strategies; just needs more knobs |
| UI overlay pattern | Prefs modal pattern in `viewer/index.html` + `viewer/src/main.ts` | Copy the CSS class structure (`.prefs-section`, `.prefs-row`) for consistent styling |
| Debounce pattern | Editor change debounce in `main.ts` line ~572 (300ms `setTimeout`) | Proven pattern; reuse for slider input debounce |
| Board re-render after route | `pullSnapshot()` + `dirty = true` pattern | Called after every `auto_route()` already |
| JSON serialization for WASM | `serde` + `serde_json` in Rust, `JSON.parse`/`JSON.stringify` in JS | Existing pattern for `auto_route()` return value |

## Existing Code and Patterns

- `crates/cypcb-autoroute/src/lib.rs` — `AutorouteConfig` struct with `via_cost_multiplier: f64`, `prefer_top_layer: bool`, `strategy: StrategyKind`. The `route_board()` function passes `config` through to strategies. **Extend this with new tuning fields.**
- `crates/cypcb-autoroute/src/cost.rs` — `RoutingCost::new(rules, net_id, via_cost_multiplier)` scales via costs. `neighbor_cost()` adds `layer_change_cost() * 0.1` for layer bias. **The 0.1 multiplier is the hook for layer_preference parameter.**
- `crates/cypcb-autoroute/src/smoother.rs` — `chamfer_corners()` line ~340: `max_chamfer = Nm::from_mm(1.0).0` and `chamfer_len = min(len_a, len_b) / 3`. **Parameterize the divisor (3) and cap (1mm) via a roundness factor.**
- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — Uses `config.via_cost_multiplier` at line 285 and `config.resolve_adaptive_grid_resolution()` at line 68. **Grid resolution hook for density parameter.**
- `crates/cypcb-autoroute/src/astar_improved.rs` — Same config usage pattern. Both strategies are symmetric — changes to config fields flow automatically.
- `crates/cypcb-render/src/lib.rs` line 333 — `auto_route()` creates `AutorouteConfig::default()`. **Add `auto_route_with_params(params_json: String)` that deserializes user params into config.**
- `viewer/src/wasm.ts` — `PcbEngine` interface + `WasmPcbEngineAdapter` + `MockPcbEngine`. **Add `auto_route_with_params(params: string): string` to all three.**
- `viewer/src/main.ts` line 1426 — `triggerRouting()` calls `engine.auto_route()`. **Add `triggerRoutingWithParams(params)` or modify existing to pass params.**
- `viewer/src/settings.ts` — `AppSettings` interface, `setPreference()`, `subscribe()`. **Add `autorouteParams` field to persist slider state.**
- `viewer/index.html` line 931-933 — Route button area. **Add tuning panel HTML adjacent to this.**

## Constraints

- **WASM main thread blocking** — Routing runs synchronously on the main WASM thread. There is no Web Worker setup. For led_blink (~7 components, 40×30mm), this is fine (<1s). For STM32-level boards, routing could take several seconds, blocking the UI. Mitigation: show "Routing..." text, accept the block for V1. Web Worker is a future optimization.
- **`route_board()` takes `&mut BoardWorld`** — Cannot run routing concurrently with rendering or other mutations. The current pattern (route → pullSnapshot → dirty=true) is sequential and correct.
- **Grid resolution is discrete** — `resolve_adaptive_grid_resolution()` returns an i64 in nanometers. The density parameter must map to a meaningful multiplier on this value. Too fine = exponentially slower (O(cells²) for A*). Too coarse = routes can't navigate between pads.
- **Smoother always runs** — Per S04 forward intelligence, there's no toggle. The roundness parameter must control chamfer aggressiveness, not on/off. Zero roundness = skip chamfer pass only (staircase collapse + merge still run).
- **`AutorouteConfig` is `#[derive(Debug, Clone)]` but not `Serialize/Deserialize`** — Need to add serde derives for WASM JSON deserialization. The crate already depends on serde (via scoring.rs).
- **Strategy selection is part of config** — User should NOT get a strategy picker in the tuning panel (that's variant territory, S06). Strategy stays as PathFinder default. The tuning panel only adjusts cost parameters.
- **Existing `auto_route()` must remain backward-compatible** — Cannot change its signature (no params). Add a new method alongside it.

## Common Pitfalls

- **Slider spamming without debounce** — Each slider change triggers a full re-route. Without debounce, rapid slider adjustment causes cascading WASM calls that freeze the UI. **Use 300ms debounce (same as editor change pattern).**
- **Grid resolution too fine from density slider** — If density slider maps linearly to resolution, the user could accidentally set a 1µm grid on a 100mm board (100M cells). **Clamp density factor to 0.5–2.0× the auto-derived resolution, with absolute floor at 10µm.**
- **Roundness=0 breaking smoother invariants** — If chamfer_len becomes 0, the smoother may produce degenerate segments. **Guard with minimum chamfer threshold (already exists: `< 1000` nm check at line 380).**
- **Layer preference sign confusion** — The `layer_change_cost(layer)` in `RoutingRuleSet` returns a scalar per layer. To make "prefer top" work, need to increase cost for bottom layer, not decrease cost for top. **Map user's -1..+1 slider to bottom_cost_bias = 0.1 × (1 + preference) and top_cost_bias = 0.1 × (1 - preference).**
- **Settings race with prefs modal** — If tuning params live in `AppSettings`, opening/closing the prefs modal could reset them. **Keep tuning params in `AppSettings` but the tuning panel reads/writes via `setPreference()` directly, same as other prefs.**
- **WASM method name mangling** — wasm_bindgen can mangle method names if they conflict with JS reserved words. `auto_route_with_params` should be fine, but verify the exported name matches the JS call site.

## Open Risks

- **Routing time exceeds 1s on STM32-level boards** — PathFinder on larger grids with adaptive resolution could still take 3-5s. The slider UX degrades from "realtime" to "batch with delay". Mitigation: accept degraded interactivity for complex boards (per roadmap: "show progress"), and document the performance envelope. The 1s target is for "typical boards" (led_blink level).
- **Density parameter may not meaningfully improve routing quality** — Grid resolution is already adaptive. Making it finer may just slow routing without better results (the bottleneck is congestion resolution, not grid granularity). Need to validate that the density slider actually produces visible differences.
- **Smoother roundness may produce DRC violations** — Larger chamfers push trace geometry further from the original path. The per-move DRC check in the smoother should catch this, but needs verification on boards with tight clearances.
- **Mock engine doesn't support auto_route** — Returns error string. Slider tuning will only work with WASM build. Acceptable — mock is dev-only fallback.

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Rust WASM | `pluginagentmarketplace/custom-plugin-rust@rust-wasm` | available (22 installs, low — not recommended) |
| Rust general | coding-guidelines | installed (available_skills) |
| Frontend design | frontend-design | installed (available_skills) |

No strongly relevant skills found. The existing codebase patterns are well-established and the work is project-specific (extending existing config/UI patterns). No external skill installation recommended.

## Sources

- S03-SUMMARY.md — PathFinder produces grid-aligned paths, `route_board()` dispatch, `config.via_cost_multiplier` usage confirmed
- S04-SUMMARY.md — Smoother always active, `smooth_routes()` signature documented, chamfer aggressiveness is hardcoded and ready to parameterize
- `crates/cypcb-autoroute/src/lib.rs` — `AutorouteConfig` struct and `route_board()` entry point
- `crates/cypcb-autoroute/src/cost.rs` — `RoutingCost` with `via_cost_multiplier` and `layer_change_cost * 0.1`
- `crates/cypcb-autoroute/src/smoother.rs` — Chamfer parameters at lines 340, 378-380
- `crates/cypcb-render/src/lib.rs:333` — `auto_route()` WASM method with hardcoded default config
- `viewer/src/wasm.ts` — `PcbEngine` interface, adapter, mock — all need `auto_route_with_params` addition
- `viewer/src/settings.ts` — Settings persistence pattern with `AppSettings`, `setPreference`, `subscribe`
- `viewer/src/main.ts` — `triggerRouting()`, `pullSnapshot()`, debounce pattern, prefs modal pattern
- `viewer/index.html` — Route button HTML, prefs modal CSS/HTML patterns for consistent UI
- M004-ROADMAP.md — S05 boundary: produces `AutorouteParams` struct, slider panel, reactive re-routing
- REQUIREMENTS.md — R110 (slider parameters), R111 (reactive re-routing <1s)
