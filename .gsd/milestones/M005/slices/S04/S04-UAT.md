# S04: Variant Generation & Tuning via Worker — UAT

**Milestone:** M005
**Written:** 2026-03-19

## Not runnable here (checked 2026-08-27)

This script drives the variant run through a Worker that is in no commit here,
and reads two debug surfaces that go with it. The variant panel it displays in
was deleted on purpose after M004.

not_in_this_repository:
  - window.__routingWorker - no commit in this clone ever added it (checked 2026-08-27)
  - window.__triggerVariantRouting - no commit in this clone ever added it (checked 2026-08-27)
  - window.__variantPanel - deleted by a9e8c7a, `refactor(viewer): delete the variant panel, which nothing could reach`

## UAT Type

- UAT mode: mixed (artifact-driven for unit/build/E2E, live-runtime for browser variant interaction)
- Why this mode is sufficient: E2E tests cover the full variant flow in a real browser. WASM-dependent tests run only when the WASM binary is available. Metric display and data transformation are proven by unit tests and visual DOM inspection.

## Preconditions

- Viewer dev server running: `cd viewer && npx vite --port 4321`
- WASM binary built: `wasm-pack build --target web --release` (required for WASM-mode tests)
- Playwright browsers installed: `npx playwright install chromium`
- A board must be loaded (Blink LED template or any `.cypcb` file)

## Smoke Test

1. Open `http://localhost:4321` in a browser
2. Click "New" → select "Blink LED" template
3. Click the **Route** button
4. **Expected:** Spinner overlay appears briefly → canvas shows routed board → score panel appears below Route button with 3+ variant rows showing names, composite scores, and detailed metrics

## Test Cases

### 1. Variant Generation via Worker

1. Open the viewer → New → Blink LED template
2. Open browser DevTools console
3. Click **Route**
4. Wait for routing to complete (overlay disappears)
5. In console, evaluate: `window.__variantPanel`
6. **Expected:** `{ visible: true, variantCount: >= 3, activeIndex: 0, hoveredIndex: -1 }`
7. In console, evaluate: `window.__routingWorker`
8. **Expected:** `{ active: false, lastResult: <non-null JSON string or array> }`

### 2. Canvas Updates After Variant Generation

1. Open viewer → New → Blink LED template
2. Note the canvas shows unrouted board (ratsnest lines visible)
3. Click **Route**
4. **Expected:** After routing completes, canvas shows routed board with traces (no more yellow ratsnest lines for routed nets). The snapshot was applied by the variant-result handler.

### 3. Score Panel Detailed Metrics

1. After routing completes (test 1 or 2), inspect the score panel
2. Each variant row should show two lines:
   - Top: variant name (e.g. "PathFinder Default") + bold composite score number
   - Bottom: `DRC: N | Smooth: N% | Vias: N | N.Nmm | Cross: N`
3. In DevTools, run: `document.querySelectorAll('.variant-metrics').forEach(el => console.log(el.textContent))`
4. **Expected:** Each element's text contains `DRC:`, `Smooth:`, `Vias:`, and `Cross:` with numeric values. Length shown in mm. No `NaN` or `undefined`.

### 4. Hover Preview

1. After routing completes with variants visible in score panel
2. Hover the mouse over a non-active variant row (not the first/active one)
3. **Expected:** Canvas shows a cyan ghost overlay of the hovered variant's routes at reduced opacity. The active variant's traces are dimmed. `window.__variantPanel.hoveredIndex` changes to the hovered row's index.
4. Move mouse away from the variant row
5. **Expected:** Ghost overlay disappears. Canvas returns to showing the active variant's routes normally.

### 5. Click-to-Apply

1. After routing completes with variants visible
2. Click on a non-active variant row (e.g. the second variant)
3. **Expected:** Console shows `[Variants] Re-routing with variant: <name>`. Spinner overlay appears briefly. After re-routing: canvas updates with the new variant's routes, `window.__variantPanel.activeIndex` changes to the clicked row's index, console shows `[Variants] Applied variant: <name>`.

### 6. Debug Surface Consistency (lastResult)

1. Open DevTools console
2. Click **Route** and wait for completion
3. Evaluate: `JSON.parse(window.__routingWorker.lastResult)`
4. **Expected:** Returns a valid JSON value — either an array of variant objects (each with `name`, `score`, `routes`) or an object with `ok` and `routed` fields. Not null, not undefined.

### 7. E2E Test Suite Green

1. Run: `npx playwright test e2e/autoroute-worker.spec.ts`
2. **Expected:** 2 passed, 1 skipped (WASM-dependent)
3. Run: `npx playwright test e2e/variant-panel.spec.ts`
4. **Expected:** 3 passed, 5 skipped (WASM-dependent)
5. Run: `npx playwright test` (full suite)
6. **Expected:** 109+ passed, ~9 skipped, 0 failures

### 8. Unit Tests and Build

1. Run: `npx vitest run`
2. **Expected:** 138 tests passed across 12 files. Variant-transform tests (11) all pass.
3. Run: `npx vite build`
4. **Expected:** Build succeeds. `routing-worker-*.js` chunk present in `dist/assets/`.
5. Run: `npx tsc --noEmit`
6. **Expected:** Zero TypeScript errors.

## Edge Cases

### Rapid Route Clicks

1. Click **Route** while routing is already in progress (before first routing completes)
2. **Expected:** Console shows `[Variants] Already routing`. No second worker spawned. No crash.

### No Board Loaded

1. Without loading any board, click **Route**
2. **Expected:** Console shows `[Variants] No board loaded`. No worker spawned. No crash.

### Click-to-Apply with Unknown Variant Name

1. If a variant name doesn't match the hardcoded params mapping, click-to-apply should still work
2. **Expected:** Falls through to default AutorouteParams — routing proceeds with defaults. No crash.

## Failure Signals

- Score panel not visible after Route click → variant-result handler not applying snapshot or not calling showVariants()
- Canvas still shows unrouted board after Route → snapshot not applied (check `snapshot = msg.snapshot` in variant-result handler)
- `NaN` in metrics display → score fields undefined in VariantData (check transformVariantResults output)
- `window.__routingWorker.lastResult` is null after Route → lastResult not being set in variant-result handler
- `window.__variantPanel.variantCount < 3` → fewer than 3 variants generated (check worker's auto_route_variants call)
- E2E test 3 in autoroute-worker.spec.ts fails on lastResult parse → dual-format assertion not handling current format

## Requirements Proved By This UAT

- R207 — Variant Generation via Web Worker: variants generated via worker, score panel shows results, hover preview works, click-to-apply re-routes
- R201 (supporting) — Worker routing: variant flow routes entirely off main thread
- R113 — Auto-apply best variant with hover preview: best variant auto-applied, hovering shows ghost overlay

## Not Proven By This UAT

- R204 — 0 unrouted on Blink LED (proven by S02 cargo tests and WASM binary, not re-proven here)
- R205/R206 — E2E responsiveness and quality tests (proven by S03, not re-proven here)
- Full WASM-path variant generation in CI (WASM binary not always available — WASM-dependent tests skip)

## Notes for Tester

- WASM-dependent tests (5 variant panel + 1 autoroute worker) skip gracefully when the WASM binary is missing. To run them, build WASM first with `wasm-pack build --target web --release` in the crate root.
- The variant name → params mapping is hardcoded ("PathFinder Default", "PathFinder Low-Via", "PathFinder High-Density"). If Rust's variant configs change names, click-to-apply will silently use default params instead of the variant-specific ones.
- The click-to-apply spawns a fresh worker (~100ms overhead for WASM init). This is acceptable for the current board complexity but may need optimization for complex boards.
- Mock-mode tests use `window.__triggerVariantRouting()` with injected mock data to bypass WASM. Real WASM behavior is only tested when the binary is present.
