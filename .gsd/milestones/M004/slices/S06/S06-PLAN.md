# S06: Variant Generation & Preview UI

**Goal:** Route button generates 2-4 routing variants with different strategies/configs, ranks them by score, auto-applies the best, and lets the user hover alternatives to preview them on canvas.

**Demo:** User clicks Route → status shows "Generating variants…" → variant panel appears showing 3-4 ranked variants with composite scores → best is auto-applied and rendered → hovering a non-active variant shows ghost traces in cyan on canvas → clicking a variant applies it.

## Must-Haves

- `generate_variants()` in `cypcb-autoroute` runs 3-4 strategy/param configs sequentially on the same `&mut BoardWorld`, scoring each, returning ranked results
- `RouteSegment` and `ViaPlacement` in `cypcb-router` derive `Serialize` so variant route data can be serialized to JSON
- `auto_route_variants()` WASM entry point returns JSON array of `{ name, score, routes, vias }` with best variant auto-applied to the world
- Variant panel in viewer shows ranked variants with name + composite score + key metrics
- Hovering a non-active variant renders its traces/vias as ghost overlay (cyan, 0.4 alpha) while dimming active traces
- Tuning slider re-route does NOT trigger variant generation (only Route button does)
- Variant panel clears on new Route click or tuning re-route
- `window.__variantPanel` debug surface for E2E testability

## Proof Level

- This slice proves: integration (Rust engine → WASM bridge → viewer UI → canvas overlay)
- Real runtime required: yes (WASM compilation + Playwright E2E)
- Human/UAT required: no (automated assertions on debug surfaces and DOM state)

## Verification

- `cargo test -p cypcb-autoroute --lib --release` — variant generation unit tests pass
- `cargo test --test variant_generation --release` — integration test: generate_variants returns 3+ variants, best has lowest composite
- `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` — WASM compiles
- `cargo check -p cypcb-render` — auto_route_variants() compiles
- `cd viewer && npx playwright test variant-panel.spec.ts` — E2E tests pass: panel appears, variants listed, hover preview triggers, debug surface reflects state
- Failure-path check: `auto_route_variants()` returns `{"ok":false,"error":"..."}` JSON on invalid input; `generate_variants()` logs `tracing::warn!` for individual variant failures and returns partial results (non-panic)

## Observability / Diagnostics

- Runtime signals: `tracing::info!` per variant (name, composite score, route count) in `generate_variants()`; `tracing::info!` on variant generation summary (count, best variant name, total time)
- Inspection surfaces: `window.__variantPanel` — `{ visible, variantCount, activeIndex, variants: [{ name, composite }] }`
- Failure visibility: variant generation errors returned in JSON `{ ok: false, error: "..." }`; console.warn for individual variant failures
- Redaction constraints: none

## Integration Closure

- Upstream surfaces consumed: `RoutingStrategy` trait + 2 implementations (S03), `score_board()` (S02), `smooth_routes()` (S04), `AutorouteParams` (S05), `clear_autorouted_traces()` + `apply_routes()` (existing)
- New wiring introduced in this slice: Route button → `auto_route_variants()` WASM call → variant panel population → hover → renderer overlay
- What remains before the milestone is truly usable end-to-end: S07 (benchmark validation, strategy selection, regression testing)

## Tasks

- [x] **T01: Rust variant generation engine and WASM bridge** `est:60m`
  - Why: Core engine for generating, scoring, and serializing multiple routing variants. Required before any UI work.
  - Files: `crates/cypcb-router/Cargo.toml`, `crates/cypcb-router/src/types.rs`, `crates/cypcb-autoroute/src/variant.rs`, `crates/cypcb-autoroute/src/lib.rs`, `crates/cypcb-render/src/lib.rs`, `crates/cypcb-autoroute/tests/variant_generation.rs`
  - Do: (1) Add serde dep to cypcb-router and derive Serialize on RouteSegment, ViaPlacement. (2) Create variant.rs module with VariantConfig, VariantResult, and generate_variants() that iterates configs sequentially: clear → route → apply → rebuild → score → serialize routes/vias → clear. Apply best at end. (3) Add auto_route_variants() to PcbEngine in cypcb-render following existing auto_route() pattern. Returns JSON array. (4) Unit tests for variant types + integration test on led_blink fixture.
  - Verify: `cargo test -p cypcb-autoroute --lib --release` passes; `cargo test --test variant_generation --release` passes; `cargo check -p cypcb-autoroute --target wasm32-unknown-unknown` clean; `cargo check -p cypcb-render` clean
  - Done when: generate_variants() returns 3+ scored variants sorted by composite, best variant auto-applied, WASM compiles

- [x] **T02: Variant panel UI with hover preview overlay and E2E tests** `est:60m`
  - Why: User-facing variant ranking panel and canvas preview overlay — the visible deliverable for R113. Plus E2E tests proving integration.
  - Files: `viewer/index.html`, `viewer/src/variant-panel.ts`, `viewer/src/renderer.ts`, `viewer/src/main.ts`, `viewer/src/wasm.ts`, `viewer/e2e/variant-panel.spec.ts`
  - Do: (1) Add variant panel HTML/CSS in index.html (collapsible panel below route button, variant rows with name + score). (2) Create variant-panel.ts: showVariants(), hideVariants(), hover/click handlers, formatScore(). (3) Add variantPreview field to RenderState + drawVariantPreview() in renderer.ts — ghost traces at 0.4 alpha in cyan, active traces dimmed to 0.3 alpha when preview active. (4) Wire main.ts: Route button calls auto_route_variants(), parses result, calls showVariants(), stores variant data. Hover sets renderState.variantPreview. Tuning re-route clears variant panel. (5) Add auto_route_variants() to PcbEngine interface in wasm.ts. (6) Expose window.__variantPanel debug surface. (7) E2E tests: panel appears after route, shows multiple variants, hover triggers preview state, debug surface reflects variant count and active index.
  - Verify: `cd viewer && npx vitest run` passes; `cd viewer && npx playwright test variant-panel.spec.ts` passes
  - Done when: Route button shows variant panel with ranked results, hovering alternative variant shows ghost overlay on canvas (verified via debug surface), panel clears on tuning re-route

## Files Likely Touched

- `crates/cypcb-router/Cargo.toml`
- `crates/cypcb-router/src/types.rs`
- `crates/cypcb-autoroute/src/variant.rs`
- `crates/cypcb-autoroute/src/lib.rs`
- `crates/cypcb-autoroute/tests/variant_generation.rs`
- `crates/cypcb-render/src/lib.rs`
- `viewer/index.html`
- `viewer/src/variant-panel.ts`
- `viewer/src/renderer.ts`
- `viewer/src/main.ts`
- `viewer/src/wasm.ts`
- `viewer/e2e/variant-panel.spec.ts`
