# S06: Variant Generation & Preview UI — Research

**Date:** 2026-03-14

## Summary

S06 owns R112 (Routing Variant Generation) and R113 (Auto-Apply Best Variant with Hover Preview), and supports R103 (Routing Quality Scoring). The slice requires a Rust-side `generate_variants()` function that runs multiple strategies/param-configs sequentially, scores each, and returns ranked results — plus a viewer-side score panel with hover-preview overlay on canvas.

The codebase is well-prepared. Two routing strategies already exist (`PathFinder`, `ImprovedAStar`) with a clean `RoutingStrategy` trait dispatch. `score_board()` produces a 7-metric `RoutingScore` with serde Serialize. The WASM bridge (`PcbEngine`) has `auto_route()` and `auto_route_with_params()`. The tuning panel (S05) shows exactly how to wire new UI to the routing engine. The critical constraint is that `BoardWorld` wraps bevy_ecs `World` which does NOT implement Clone — variants must be generated sequentially (route → apply → score → clear → next), not in parallel with cloned worlds. This is the single hardest design issue.

For the preview UI, the renderer already has a `drawRoutingPreview()` function for routing-in-progress and a `RenderState` with overlay capabilities (`routing`, `highlightedNet`). The variant preview can follow this exact pattern: store variant trace/via data as plain TypeScript arrays, and when hovering a variant row, inject it into the render state as a ghost overlay drawn with reduced opacity in a distinct color.

## Recommendation

**Approach: Rust-side sequential variant generation + JS-side variant state management**

1. **Rust (`cypcb-autoroute`)**: Add `generate_variants()` that takes `&mut BoardWorld`, `&FootprintLibrary`, `&dyn RoutingRuleSet`, and a `Vec<VariantConfig>` (strategy + params combinations). Runs each config sequentially: clear → route → apply → rebuild spatial index → score → collect result as `VariantResult { config_name, score, routes, vias }` → clear. Returns `Vec<VariantResult>` sorted by composite score (best first). Apply the best variant at the end.

2. **WASM bridge (`cypcb-render`)**: Add `auto_route_variants() -> String` that calls `generate_variants()` with a hardcoded set of 3-4 configs (PathFinder default, PathFinder low-via, ImprovedAStar default, PathFinder high-density). Returns JSON array of `{ name, score: RoutingScore, routes: [...], vias: [...] }`. The best variant is auto-applied to the world; others are returned as data-only for preview.

3. **Viewer UI**: New `variant-panel.ts` module. Score panel appears after routing completes, showing ranked variants with composite score + key metrics. Hovering a non-active variant injects its traces/vias into `RenderState` as a ghost overlay. Clicking applies that variant (calls a new `apply_variant(index)` WASM method or stores variant data client-side and re-applies).

4. **Renderer**: Add `variantPreview: { traces: TraceInfo[], vias: ViaInfo[] } | null` to `RenderState`. When set, draw these traces/vias with 0.4 alpha in a distinct color (e.g., cyan) on top of the current board, while dimming active traces to 0.3 alpha.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| Strategy dispatch | `RoutingStrategy` trait + `StrategyKind` enum in `strategy.rs` | Already abstracts routing algorithm selection; variant configs just iterate over strategies |
| Board scoring | `score_board()` in `scoring.rs` with `RoutingScore` (serde Serialize) | Complete 7-metric scoring with composite; just call it per variant |
| WASM JSON bridge | `auto_route()` / `auto_route_with_params()` pattern in `cypcb-render/src/lib.rs` | Follow exact same clear → route → apply → rebuild → DRC → JSON pattern |
| UI panel pattern | `#tuning-panel` dropdown in `index.html` + S05 wiring in `main.ts` | Collapsible panel with toggle button, same CSS/layout conventions |
| Trace rendering | `drawTrace()` in `renderer.ts` | Reuse for ghost overlay with modified alpha/color |
| Debug surface | `window.__tuningPanel` pattern | Extend to `window.__variantPanel` for E2E testability |
| Settings persistence | `settings.ts` `getPreference()`/`setPreference()` | Not needed for variants (ephemeral per routing session), but pattern available if needed |

## Existing Code and Patterns

- `crates/cypcb-autoroute/src/lib.rs` — `route_board()` dispatches to strategy via `Box<dyn RoutingStrategy>`. `AutorouteConfig` has `strategy: StrategyKind` + `params: AutorouteParams`. The `generate_variants()` function should live here, iterating configs and calling `route_board()` per variant.
- `crates/cypcb-autoroute/src/strategy.rs` — `StrategyKind` enum (`PathFinder`, `ImprovedAStar`) with `Display` impl. Variant configs combine a StrategyKind with different AutorouteParams.
- `crates/cypcb-autoroute/src/scoring.rs` — `score_board(&mut BoardWorld, &DesignRules, &ScoreWeights) -> RoutingScore`. Requires `&mut BoardWorld` (ECS queries). `RoutingScore` has `#[derive(Serialize)]` — ready for JSON output.
- `crates/cypcb-render/src/lib.rs` — `PcbEngine::auto_route()` and `auto_route_with_params()` show the WASM entry point pattern: clear traces → route → apply → rebuild spatial index → run DRC → return JSON. New `auto_route_variants()` follows this pattern but loops.
- `crates/cypcb-render/src/lib.rs` — `clear_autorouted_traces()` removes non-locked autorouted traces and vias. Essential for resetting between variant runs.
- `crates/cypcb-router/src/lib.rs` — `apply_routes(&mut BoardWorld, &RoutingResult)` applies route segments and vias to ECS. Used between route and score steps.
- `crates/cypcb-router/src/types.rs` — `RoutingResult { status, routes: Vec<RouteSegment>, vias: Vec<ViaPlacement> }`. Does NOT have Serialize — need to serialize routes/vias manually or add serde derive.
- `viewer/src/renderer.ts` — `RenderState` interface with `routing: RoutingState | null` for routing preview overlay. `drawRoutingPreview()` draws ghost traces. Variant preview follows the same pattern with a new field.
- `viewer/src/main.ts:1426-1481` — `triggerRouting()` is the main routing entry point. Variant generation replaces this flow: call `auto_route_variants()`, parse results, show panel, auto-apply best.
- `viewer/src/main.ts:1567-1697` — Tuning panel wiring shows exact pattern for collapsible UI panel with debounced WASM calls.
- `viewer/src/types.ts` — `TraceInfo`, `ViaInfo` interfaces for rendered trace data. Variant preview data maps directly to these.
- `viewer/src/wasm.ts:91-98` — `PcbEngine` interface with `auto_route()` and `auto_route_with_params()`. Add `auto_route_variants()` here.

## Constraints

- **BoardWorld is not Clone** — wraps bevy_ecs `World` which has no Clone impl. Variants MUST be generated sequentially in the same world: route → apply → score → collect data → clear → next. Cannot parallelize in Rust.
- **WASM single-threaded** — no Web Workers for parallel variant computation without SharedArrayBuffer complexity. Sequential is the pragmatic approach for V1.
- **score_board() needs applied routes in ECS** — scoring queries Trace/Via entities and runs DRC. Routes must be `apply_routes()`'d before scoring. Must clear after scoring to prepare for next variant.
- **RoutingResult lacks Serialize** — `RouteSegment` and `ViaPlacement` don't derive serde. Either add Serialize to cypcb-router types or manually convert to JSON-friendly structs in cypcb-render. Adding Serialize is cleaner since S07 will also want serialized results.
- **Variant preview is read-only** — hovering a variant shows its traces as ghost overlay; it does NOT mutate the BoardWorld. Only clicking "Apply" or the auto-applied best actually modifies ECS state.
- **Route button UX change** — currently Route → single result → done. New flow: Route → generate 2-4 variants → show panel → auto-apply best. Must not break existing single-route flow (tuning slider re-route should NOT generate variants, only Route button does).
- **Time budget** — generating 3-4 variants means 3-4× routing time. For led_blink (~100ms), that's ~400ms total — fine. For larger boards, could be seconds. Need progress indication.
- **Renderer overlay** — variant preview traces must be drawn WITHOUT modifying the active `BoardSnapshot`. Store variant data separately in render state; overlay during draw pass only.

## Common Pitfalls

- **Forgetting to clear between variants** — if `clear_autorouted_traces()` is skipped between variant runs, the next route will fail or produce garbage (grid already occupied by previous variant's traces). Must clear + rebuild spatial index between each variant.
- **Serializing routes before clearing** — routes/vias data must be captured from the RoutingResult BEFORE clearing the world for the next variant. The RoutingResult owns the data, not the ECS.
- **Score computed after apply but before clear** — `score_board()` reads from ECS, so routes must be applied first. But clear happens after scoring. The sequence is: route → apply → rebuild index → score → serialize routes → clear.
- **Ghost overlay Z-order** — variant preview traces drawn on top of active traces could be confusing. Use distinct color (cyan/purple) with moderate alpha (0.4) and dim active traces (0.3 alpha) when preview is active. Restore on mouse-leave.
- **Variant panel stale after re-route** — if user adjusts tuning sliders after generating variants, the variant panel becomes stale. Clear variant panel on any tuning re-route or new Route click.
- **Huge JSON payload for complex boards** — each variant includes full routes/vias arrays. For boards with hundreds of trace segments, 4 variants could be 100KB+ of JSON. Manageable but worth noting. Keep only necessary fields (coordinates, width, layer, net_name).

## Open Risks

- **Performance on complex boards** — 4 variants × 3s per route = 12s total for STM32-level boards. May need to reduce to 2 variants or show progress bar. Mitigate: configurable variant count, show incremental results as each variant completes.
- **RoutingResult Serialize scope** — adding `#[derive(Serialize)]` to `RouteSegment`, `ViaPlacement`, and associated types in `cypcb-router` affects a core crate. Check all consumers compile. Alternative: build a separate `VariantData` struct in `cypcb-autoroute` that copies the relevant fields.
- **Interaction between variant panel and tuning panel** — both affect routing. Tuning re-routes the "active" variant only. Generating new variants uses current tuning params as a base with strategy/param variations layered on top. Need clear UX distinction.

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Rust/WASM | — | No specific skill needed; existing patterns sufficient |
| Canvas 2D rendering | — | Existing renderer covers all needed overlay techniques |
| Playwright E2E | — | Existing test patterns (tuning-panel.spec.ts) directly applicable |

No external skills are needed — this slice is purely internal architecture extending existing patterns.

## Sources

- `crates/cypcb-autoroute/src/strategy.rs` — RoutingStrategy trait with 2 implementations (PathFinder, ImprovedAStar)
- `crates/cypcb-autoroute/src/scoring.rs` — score_board() API, RoutingScore with Serialize
- `crates/cypcb-autoroute/src/lib.rs` — route_board() dispatch, AutorouteConfig, AutorouteParams
- `crates/cypcb-render/src/lib.rs:333-437` — auto_route() and auto_route_with_params() WASM bridge patterns
- `crates/cypcb-router/src/types.rs` — RoutingResult, RouteSegment, ViaPlacement (no Serialize)
- `crates/cypcb-world/src/world.rs` — BoardWorld wraps bevy_ecs World (no Clone)
- `viewer/src/renderer.ts:17-45` — RenderState interface with overlay fields
- `viewer/src/renderer.ts:870-900` — drawRoutingPreview() ghost trace rendering pattern
- `viewer/src/main.ts:1426-1481` — triggerRouting() flow
- `viewer/src/main.ts:1567-1697` — Tuning panel wiring pattern
- `viewer/src/types.ts` — TraceInfo, ViaInfo interfaces
- `viewer/e2e/tuning-panel.spec.ts` — E2E test pattern for panel interactions
- D-M004-003 — "Auto-apply best, hover preview alternatives" (decision)
- S03 Summary — RoutingStrategy trait, PathFinder wins 3× on composite
- S04 Summary — Smoother integrated into both strategies post-routing
- S05 Summary — AutorouteParams, tuning panel pattern, debounced re-routing
