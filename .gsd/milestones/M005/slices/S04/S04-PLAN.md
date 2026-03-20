# S04: Variant Generation & Tuning via Worker

**Goal:** Route button generates 3+ routing variants via Web Worker, score panel shows ranked results with detailed metrics, hover preview renders alternative routes, click-to-apply re-routes with selected variant's params, and the canvas updates with the routed board snapshot.

**Demo:** User clicks Route → spinner/overlay appears → worker generates 3 variants → canvas shows routed board (best variant auto-applied) → score panel appears with ranked variants showing DRC violations, smoothness, via count, length → hovering non-active variant shows cyan ghost overlay → clicking a variant re-routes with that config and updates canvas → tuning sliders continue to re-route via worker.

## Must-Haves

- Route button calls `triggerVariantRouting()` instead of `triggerRouting()`
- Worker's `route-variants` handler returns snapshot alongside variant data
- Canvas updates with routed board after variant generation (snapshot applied on main thread)
- Rust-serialized `VariantResult[]` correctly transformed to TypeScript `VariantData[]` (format mismatch handled)
- Score panel shows detailed metric breakdown (DRC violations, smoothness, total length, via count, crossings, layer balance)
- Hover preview renders cyan ghost overlay for non-active variant
- Click-to-apply re-routes via worker with selected variant's params
- `__routingWorker.lastResult` set after variant routing (no regression on debug surface)
- E2E tests pass for both worker routing and variant panel flows

## Proof Level

- This slice proves: integration
- Real runtime required: yes (WASM in worker)
- Human/UAT required: no (E2E tests cover the full flow)

## Verification

- `npx tsc --noEmit` — zero TypeScript errors after protocol and data changes
- `npx vitest run` — all unit tests pass (no regressions)
- `npx vite build` — worker bundles correctly with protocol change
- `npx playwright test e2e/variant-panel.spec.ts` — all variant panel tests pass
- `npx playwright test e2e/autoroute-worker.spec.ts` — worker tests pass (no regression on lastResult)
- `window.__variantPanel.variantCount >= 3` after Route click (in WASM mode)
- `window.__routingWorker.lastResult` is set after variant routing completes
- Score panel rows show metric breakdown beyond just composite number

## Observability / Diagnostics

- Runtime signals: `[Variants] Worker spawned/ready/result`, `[Variants] Applied variant: <name>`, `[Variants] Re-routing with variant params`
- Inspection surfaces: `window.__variantPanel` (visible, variantCount, activeIndex, hoveredIndex), `window.__routingWorker` (active, lastResult), `window.__triggerVariantRouting`
- Failure visibility: `[Worker Error] Variants:` prefix on variant generation failures, `[Variants] Failed to parse variant result:` on transform errors
- Redaction constraints: none

## Integration Closure

- Upstream surfaces consumed: `routing-worker.ts` (worker message protocol), `worker-protocol.ts` (WorkerRequest/WorkerResponse types), `triggerVariantRouting()` pattern from S01, `variant-panel.ts` (showVariants/hideVariants), `renderer.ts` (drawVariantPreview, VariantPreviewData)
- New wiring introduced in this slice: Route button → `triggerVariantRouting()`, worker snapshot applied after variant-result, click-to-apply spawns `route-with-params` worker, data transformation layer (Rust VariantResult → TS VariantData)
- What remains before the milestone is truly usable end-to-end: nothing — this is the final slice

## Tasks

- [x] **T01: Add snapshot to variant-result protocol and build data transformation** `est:45m`
  - Why: The worker's `route-variants` handler currently returns only the variants JSON — no snapshot. Without the snapshot, the main thread can't update the canvas after variant generation. Additionally, the Rust-serialized `VariantResult` format (net_id, Point {x,y}) doesn't match the TypeScript `VariantData` interface (net_name, [x,y] tuples, flat via coords). Both the protocol fix and data transformation must exist before the Route button can be rewired.
  - Files: `viewer/src/worker-protocol.ts`, `viewer/src/routing-worker.ts`, `viewer/src/variant-transform.ts` (new), `viewer/src/variant-panel.ts`
  - Do: (1) Add `snapshot: BoardSnapshot` field to `VariantResultResponse` in `worker-protocol.ts`. (2) Update the `route-variants` case in `routing-worker.ts` to call `engine.get_snapshot()` before `engine.free()` and include it in the response. (3) Create `variant-transform.ts` with a `transformVariantResults()` function that maps Rust-serialized JSON to `VariantData[]` — converting `net_id` to `net_name` using `NetInfo[]` from the snapshot, `Point {x,y}` to `[x,y]` tuples, `ViaPlacement.position.x/y` to flat `x`/`y`. (4) Export the Rust-side interface as `RawVariantResult` in the transform module for type safety.
  - Verify: `npx tsc --noEmit` passes with zero errors. `npx vite build` succeeds. `npx vitest run` passes.
  - Done when: `VariantResultResponse` has `snapshot` field, worker returns it, and `transformVariantResults()` correctly maps format with a unit test.

- [x] **T02: Wire Route button to variant routing with snapshot application and click-to-apply** `est:45m`
  - Why: This is the core behavioral change — the Route button must call `triggerVariantRouting()`, the variant-result handler must apply the snapshot to the canvas, and clicking a variant must re-route with that variant's params. Without this, variants are generated but never displayed on the board.
  - Files: `viewer/src/main.ts`
  - Do: (1) Change `routeBtn.addEventListener('click', () => triggerRouting())` to call `triggerVariantRouting()`. (2) In `triggerVariantRouting()`'s `variant-result` handler, apply the snapshot from the worker response: set `snapshot = msg.snapshot`, rebuild `padNetMap`, set `dirty = true` — same pattern as `triggerRouting()`'s `route-result` handler. (3) Use the new `transformVariantResults()` to convert raw variant JSON to `VariantData[]` before passing to `showVariants()`. (4) Set `(window as any).__routingWorker.lastResult` in the variant-result handler so the debug surface stays consistent. (5) Implement click-to-apply in the `onClick` callback: spawn a new worker with `route-with-params` using the clicked variant's params (build Rust-compatible params JSON from variant name → known param mapping), then apply the returned snapshot and update the canvas. (6) Keep `triggerRouting()` function intact (called by editor-triggered routing via Ctrl+R / Tauri event) but ensure variant flow is the default for the Route button.
  - Verify: `npx tsc --noEmit` passes. Load Blink LED → click Route → canvas shows routed board → score panel visible → `window.__routingWorker.lastResult` is set → `window.__variantPanel.variantCount >= 3`.
  - Done when: Route button generates variants via worker, canvas updates with routed board, click-to-apply re-routes with selected variant's config, `lastResult` debug surface is set.

- [x] **T03: Enhance score panel with detailed metric breakdown** `est:30m`
  - Why: The current `showVariants()` displays only the variant name, composite score, and a terse `Xv · Yr` string. The roadmap requires "score panel shows ranked results" with meaningful metrics. Users need to see DRC violations, smoothness, total length, via count, and crossings to understand why one variant is better.
  - Files: `viewer/src/variant-panel.ts`, `viewer/src/variant-panel.css` (if exists, else inline styles)
  - Do: (1) Replace the `metricsEl.textContent = '{via_count}v · {routes.length}r'` line with a richer breakdown showing: DRC violations count, smoothness percentage, via count, total length (converted from Nm to mm), crossings count, and layer balance. (2) Format the display compactly — e.g. `DRC: 0 | Smooth: 100% | Vias: 6 | 182.5mm | Cross: 0`. (3) Add a CSS class for the metrics detail row to allow wrapping on narrow panels. (4) Ensure the composite score remains prominent (bold/larger font) with the detail metrics as a secondary line.
  - Verify: `npx tsc --noEmit` passes. `npx vitest run` passes. Visual check: score panel rows show metric breakdown, not just composite + terse count.
  - Done when: Each variant row in the score panel shows composite score prominently plus a detailed metric line with DRC violations, smoothness %, via count, length in mm, and crossings.

- [x] **T04: Update E2E tests for variant-first routing flow** `est:30m`
  - Why: The Route button now calls `triggerVariantRouting()` instead of `triggerRouting()`. Test 3 in `autoroute-worker.spec.ts` asserts `__routingWorker.lastResult` which was set in the `route-result` handler — it must still work since T02 sets `lastResult` in the variant-result handler too. Variant panel tests should work but need verification. Add a new assertion that the canvas snapshot updates after variant generation.
  - Files: `viewer/e2e/autoroute-worker.spec.ts`, `viewer/e2e/variant-panel.spec.ts`
  - Do: (1) In `autoroute-worker.spec.ts` test 3, verify `lastResult` is still set after Route button click (should work since T02 sets it in variant-result handler — verify and adjust if needed). (2) In `variant-panel.spec.ts`, add an assertion in the "route button generates variants" test that the canvas snapshot was updated (check via `page.evaluate` that the snapshot has traces or that a render diagnostic changed). (3) Add a test in `variant-panel.spec.ts` for the detailed metrics — assert that `.variant-metrics` text contains DRC/smoothness indicators, not just the terse count. (4) Ensure all existing tests still pass with the new Route button behavior. (5) Run full E2E suite to catch regressions.
  - Verify: `npx playwright test e2e/autoroute-worker.spec.ts` — all 3 tests pass (or skip gracefully in mock mode). `npx playwright test e2e/variant-panel.spec.ts` — all tests pass. `npx playwright test` — full suite green.
  - Done when: All E2E tests pass with the variant-first routing flow. No regressions in worker tests. Variant panel tests assert snapshot update and metric display.

## Files Likely Touched

- `viewer/src/worker-protocol.ts` — add `snapshot` field to `VariantResultResponse`
- `viewer/src/routing-worker.ts` — return snapshot in `route-variants` handler
- `viewer/src/variant-transform.ts` — **new** — Rust VariantResult → TS VariantData transformation
- `viewer/src/main.ts` — Route button → variant routing, snapshot application, click-to-apply, lastResult in variant handler
- `viewer/src/variant-panel.ts` — enhanced metric display in `showVariants()`
- `viewer/e2e/autoroute-worker.spec.ts` — verify lastResult still works with variant flow
- `viewer/e2e/variant-panel.spec.ts` — add snapshot update assertion, metric display test
