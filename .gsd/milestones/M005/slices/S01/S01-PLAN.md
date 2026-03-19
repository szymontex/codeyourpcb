# S01: Web Worker WASM Routing

**Goal:** All WASM autorouting executes in a Web Worker. Main thread never freezes during routing. Cancel button terminates routing immediately. Debug surface exposes worker state.

**Demo:** User clicks Route → spinner overlay appears instantly and stays visible, browser remains responsive (can click cancel, scroll page), routed board appears when worker finishes. Cancel terminates routing and resets UI. Tuning sliders and variant generation also route via worker.

## Must-Haves

- Web Worker (`routing-worker.ts`) initializes WASM, creates its own `PcbEngine`, routes boards, and posts snapshot + result back
- Worker message protocol defined as TypeScript discriminated unions in shared `worker-protocol.ts`
- `parseSource()` extracted from `wasm.ts` to shared `parse-source.ts` so worker can import it
- `triggerRouting()` refactored: spawns worker, posts source, handles result via `onmessage`, replaces snapshot
- Overlay/spinner visible for full routing duration (main thread free to paint)
- Cancel button calls `worker.terminate()`, UI resets to pre-route state
- `window.__routingWorker` debug surface: `{ active: boolean, lastResult: string | null }`
- Tuning slider re-route uses worker (`route-with-params` message)
- Variant generation uses worker (`route-variants` message)
- Worker error handling: `onerror` / `onmessageerror` surface errors to main thread

## Proof Level

- This slice proves: integration (Worker ↔ WASM ↔ main thread snapshot replacement)
- Real runtime required: yes (WASM init inside Worker, Vite bundling, postMessage)
- Human/UAT required: no (E2E smoke test in T05 proves responsiveness programmatically)

## Verification

- `npx playwright test viewer/e2e/autoroute-worker.spec.ts` — E2E test proving:
  - Overlay visible during routing (main thread not blocked)
  - Cancel button clickable and terminates routing
  - `__routingWorker.active === true` during routing, `false` after
  - Routed board appears with expected segment count after completion
- Manual verification in dev server: Route button → spinner plays smoothly → cancel responds → board routes successfully
- Diagnostic failure-path check: `window.__routingWorker.active === false` after cancel or error, and `[Worker Error]` prefix appears in console when worker fails (verified by posting invalid source and checking console output)

## Observability / Diagnostics

- Runtime signals: `[Worker] WASM initialized`, `[Worker] Routing started`, `[Worker] Routing complete` console logs from worker; `[Routing] Worker spawned`, `[Routing] Worker result received`, `[Routing] Worker terminated (cancel)` from main thread
- Inspection surfaces: `window.__routingWorker` with `{ active, lastResult }`, existing `window.__lastRouteResult`
- Failure visibility: Worker `onerror` handler logs to main thread console with `[Worker Error]` prefix; WASM panic messages forwarded via `{type:'error', message}` worker message
- Redaction constraints: none

## Integration Closure

- Upstream surfaces consumed: `viewer/src/wasm.ts` (`parseSource()`, `WasmPcbEngineAdapter`, `PcbEngine` interface), `viewer/src/main.ts` (`triggerRouting()`, `cancelRouting()`, `onTuningSliderInput()`, `lastLoadedSource`, `pullSnapshot()`, `snapshot` variable), `viewer/src/variant-panel.ts` (`showVariants()`, `hideVariants()`), WASM binary from `viewer/pkg/cypcb_render.js`
- New wiring introduced in this slice: `routing-worker.ts` ← WASM init + routing; `main.ts` ← worker spawn/terminate/message lifecycle; `worker-protocol.ts` ← shared message types; `parse-source.ts` ← extracted parser
- What remains before the milestone is truly usable end-to-end: S02 (PathFinder quality fix for 0 unrouted), S03 (E2E regression tests in CI), S04 (variant panel UX via worker)

## Tasks

- [x] **T01: Create worker protocol types and extract parseSource to shared module** `est:45m`
  - Why: The worker and main thread need shared TypeScript types for messages, and the worker needs `parseSource()` which currently lives inside `wasm.ts` as a private function. These shared modules must exist before the worker or main-thread refactor can be written.
  - Files: `viewer/src/worker-protocol.ts`, `viewer/src/parse-source.ts`, `viewer/src/wasm.ts`, `viewer/src/types.ts`
  - Do: (1) Create `worker-protocol.ts` with `WorkerRequest` and `WorkerResponse` discriminated union types covering `route`, `route-with-params`, `route-variants`, `ready`, `route-result`, `variant-result`, `error` message types. (2) Extract `parseSource()` from `wasm.ts` into `parse-source.ts`, export it. Update `wasm.ts` to import from `parse-source.ts`. (3) Ensure existing Vitest tests still pass after the extraction.
  - Verify: `cd viewer && npx vitest run --reporter=verbose` passes. TypeScript compiles: `cd viewer && npx tsc --noEmit`.
  - Done when: `worker-protocol.ts` exports `WorkerRequest` and `WorkerResponse` types. `parse-source.ts` exports `parseSource()`. `wasm.ts` imports from `parse-source.ts`. All existing tests pass.

- [x] **T02: Create routing-worker.ts with WASM init and route message handler** `est:1h`
  - Why: This is the core new file — the Web Worker that initializes WASM inside a worker context, creates a `PcbEngine`, and handles routing messages. It's the riskiest piece (WASM init inside Vite-bundled ES module worker) and must be proven working before main-thread refactoring.
  - Files: `viewer/src/routing-worker.ts`
  - Do: (1) Create ES module worker that imports WASM glue from `../pkg/cypcb_render.js` and `parseSource` from `./parse-source.ts`. (2) On load: call `await init(new URL('../pkg/cypcb_render_bg.wasm', import.meta.url))` to initialize WASM (explicit URL for Vite resolution), then post `{type:'ready'}`. (3) Handle `{type:'route', source}`: create `PcbEngine`, call `load_snapshot()` with parsed source, call `auto_route()`, call `get_snapshot()`, post `{type:'route-result', snapshot, routeResult}`. (4) Handle `{type:'route-with-params', source, params}`: same but call `auto_route_with_params(params)`. (5) Handle `{type:'route-variants', source}`: same but call `auto_route_variants()`, post `{type:'variant-result', variants}`. (6) Wrap all handlers in try/catch, post `{type:'error', message}` on failure. (7) Use `WorkerRequest`/`WorkerResponse` types from `worker-protocol.ts`.
  - Verify: File compiles with `npx tsc --noEmit`. Worker can be instantiated in browser (verified in T03's integration).
  - Done when: `routing-worker.ts` exists, handles all 3 routing message types, uses shared protocol types, has error handling, and TypeScript compiles cleanly.

- [x] **T03: Refactor triggerRouting and cancelRouting to use Web Worker** `est:1h30m`
  - Why: This is the main integration — replacing synchronous `engine.auto_route()` with worker-based async routing. This directly delivers R201 (main thread never blocked), R202 (spinner visible throughout), and R203 (cancel terminates immediately).
  - Files: `viewer/src/main.ts`
  - Do: (1) Add worker spawn helper `spawnRoutingWorker()` using Vite pattern: `new Worker(new URL('./routing-worker.ts', import.meta.url), { type: 'module' })`. (2) Refactor `triggerRouting()`: set `isRouting = true` + show overlay synchronously, spawn worker, post `{type:'route', source: lastLoadedSource}`, set up `worker.onmessage` handler that receives result, replaces `snapshot` (same as `pullSnapshot()` does but with worker's snapshot), rebuilds `padNetMap`, updates status text, sets `isRouting = false`. (3) Refactor `cancelRouting()`: call `worker.terminate()`, set `isRouting = false`, hide overlay, reset status. (4) Add `worker.onerror` and `worker.onmessageerror` handlers that log errors and reset UI. (5) Expose `window.__routingWorker = { active: boolean, lastResult: string | null }` debug surface — update on worker spawn, message receipt, cancel, and error. (6) Guard against rapid Route clicks: if worker exists, terminate it before spawning new one. (7) Remove the 50ms `setTimeout` yield hack (no longer needed — main thread is free).
  - Verify: Start Vite dev server (`cd viewer && npm run dev`), load a board, click Route. Overlay appears immediately, stays visible, cancel button works. Browser does not freeze. Board shows routed traces after completion.
  - Done when: `triggerRouting()` routes via worker. Main thread never calls `engine.auto_route()`. Overlay visible throughout. Cancel terminates worker. `__routingWorker` debug surface works.

- [x] **T04: Refactor tuning sliders and variant generation to route via Worker** `est:1h`
  - Why: Two remaining synchronous WASM callsites must also move to the worker: `onTuningSliderInput()` (calls `engine.auto_route_with_params()`) and variant generation (currently disabled/fallback). This completes the "all routing via worker" requirement and produces the S01→S04 boundary contract for variant worker support.
  - Files: `viewer/src/main.ts`
  - Do: (1) Refactor `onTuningSliderInput()` debounce handler: instead of calling `engine.auto_route_with_params()`, spawn worker (or reuse pattern from T03), post `{type:'route-with-params', source: lastLoadedSource, params: rustParams}`, handle result via `onmessage`. Show/hide routing overlay during tuning re-route. Cancel any in-flight tuning worker if slider changes again before result arrives. (2) Add variant routing: create a function `triggerVariantRouting()` that posts `{type:'route-variants', source: lastLoadedSource}` to worker, handles `{type:'variant-result', variants}` response by calling `showVariants()` with the data. (3) Optionally update `triggerRouting()` to call `triggerVariantRouting()` when variant mode is desired (or leave this for S04 to wire up — just ensure the worker message path works). (4) Update `__routingWorker` debug surface to reflect tuning/variant routing states.
  - Verify: Start Vite dev server, load a board, adjust tuning slider — board re-routes via worker without freezing. Status text updates with result.
  - Done when: `onTuningSliderInput()` routes via worker. Variant routing message path exists and is callable. No synchronous WASM routing calls remain in `main.ts`.

- [x] **T05: E2E smoke test — overlay visible and cancel works during Worker routing** `est:1h`
  - Why: Proves main thread responsiveness programmatically. This is the acceptance test for R201/R202/R203 — if the overlay is visible and cancel is clickable during routing, the main thread is definitively not blocked. Catches future regressions if someone accidentally moves WASM back to main thread.
  - Files: `viewer/e2e/autoroute-worker.spec.ts`
  - Do: (1) Create Playwright test file. (2) Test "overlay visible during routing": load a board via `__loadBoard()`, click Route button, immediately assert `#routing-status` is visible and `#cancel-route-btn` is visible. Assert `__routingWorker.active === true`. Wait for routing to complete. Assert `__routingWorker.active === false`. (3) Test "cancel terminates routing": load board, click Route, wait for overlay visible, click Cancel, assert overlay hidden within 1s, assert `__routingWorker.active === false`. (4) Test "routing produces result": load board, click Route, wait for completion (status text changes), assert routed segments > 0 via `__routingWorker.lastResult` or status text. (5) Use the `routing-test.cypcb` fixture from `viewer/e2e/fixtures/`. (6) Use `__loadBoard()` pattern from existing E2E tests to dismiss project manager and load board.
  - Verify: `cd viewer && npx playwright test e2e/autoroute-worker.spec.ts --reporter=list`
  - Done when: All 3 E2E tests pass. Tests prove overlay visibility, cancel functionality, and routing result delivery — all via Web Worker.

## Files Likely Touched

- `viewer/src/routing-worker.ts` (new — Web Worker module)
- `viewer/src/worker-protocol.ts` (new — shared message types)
- `viewer/src/parse-source.ts` (new — extracted parser)
- `viewer/src/main.ts` (refactor triggerRouting, cancelRouting, onTuningSliderInput)
- `viewer/src/wasm.ts` (import parseSource from shared module)
- `viewer/e2e/autoroute-worker.spec.ts` (new — E2E smoke test)
