---
estimated_steps: 5
estimated_files: 1
---

# T04: Refactor tuning sliders and variant generation to route via Worker

**Slice:** S01 — Web Worker WASM Routing
**Milestone:** M005

## Description

Two remaining synchronous WASM callsites in `main.ts` must also route via the Web Worker:

1. **`onTuningSliderInput()`** (line 1703) — calls `engine.auto_route_with_params(JSON.stringify(rustParams))` synchronously in a 300ms debounce callback. Must instead spawn/reuse a worker and post `{type:'route-with-params'}`.

2. **Variant generation** — currently `auto_route_variants()` is called via a fallback path in the old `triggerRouting()`. Must be callable via worker using `{type:'route-variants'}` message. This produces the S01→S04 boundary contract.

After this task, zero synchronous WASM routing calls remain in `main.ts`.

## Steps

1. **Refactor `onTuningSliderInput()` debounce callback to use worker:**
   - In the `window.setTimeout` callback (line ~1723), instead of calling `engine.auto_route_with_params(...)`:
     - Cancel any existing routing worker (`routingWorker?.terminate()`).
     - Spawn a new worker via `spawnRoutingWorker()`.
     - Show routing overlay via `updateRoutingUI({ isRouting: true, ... })`.
     - Set up `worker.onmessage`:
       - On `{type:'ready'}`: post `{type:'route-with-params', source: lastLoadedSource!, params: JSON.stringify(rustParams)}`.
       - On `{type:'route-result', snapshot, routeResult}`: replace `snapshot`, rebuild `padNetMap`, set `dirty = true`, update status text, call `updateRoutingUI({ isRouting: false, ... })`. Parse routeResult JSON for routed/unrouted counts. Log result.
       - On `{type:'error', message}`: log warning, update status text, reset UI.
     - Set up `worker.onerror` to reset UI.
   - If a slider changes while a tuning worker is running, the debounce timer already handles this — when the debounce fires, it terminates the previous worker and spawns a new one.

2. **Create `triggerVariantRouting()` function:**
   - This function spawns a worker, posts `{type:'route-variants', source: lastLoadedSource!}`.
   - On `{type:'ready'}`: post the route-variants message.
   - On `{type:'variant-result', variants}`: parse the variants JSON string, call `showVariants()` with the parsed data, update status. Also replace `snapshot` with the best variant's snapshot if the worker sends it alongside.
   - On `{type:'error'}`: log, hide variants panel, update status text.
   - Export this as a callable function for S04 to wire into the Route button UX.

3. **Verify no synchronous WASM routing calls remain:**
   Run `grep -n "engine\.auto_route\|engine\.auto_route_with_params\|engine\.auto_route_variants" viewer/src/main.ts` — should return zero matches.

4. **Update `__routingWorker` debug surface** to reflect tuning re-route state (active during tuning reroute, not just Route button).

5. **Test in browser:**
   - Start Vite dev server, load a board.
   - Open tuning panel (⚡ button), adjust a slider.
   - Board should re-route via worker — no freeze, overlay appears briefly during re-route.
   - Status text updates with re-route result.

## Must-Haves

- [ ] `onTuningSliderInput()` routes via worker — no synchronous `engine.auto_route_with_params()` call
- [ ] `triggerVariantRouting()` function exists and posts `route-variants` to worker
- [ ] Zero synchronous WASM routing calls remain in `main.ts` (`engine.auto_route*` grep returns empty)
- [ ] Tuning re-route shows overlay briefly and updates board without freezing

## Verification

- `grep -n "engine\.auto_route" viewer/src/main.ts` returns zero matches (all routing via worker)
- Dev server: adjust tuning slider → board re-routes without freeze → status updates
- `triggerVariantRouting()` is callable (will be fully wired in S04)

## Inputs

- `viewer/src/main.ts` — `onTuningSliderInput()` at line 1703, `showVariants()` / `hideVariants()` imports, `spawnRoutingWorker()` from T03, worker message handling pattern from T03
- `viewer/src/worker-protocol.ts` — `WorkerRequest` / `WorkerResponse` types
- `viewer/src/variant-panel.ts` — `showVariants()` function signature

## Observability Impact

- **New signals:** `[Tuning] Worker spawned`, `[Tuning] Worker WASM ready`, `[Tuning] Worker result received` console logs during tuning re-route; `[Variants] Worker spawned`, `[Variants] Worker result received` during variant generation
- **Changed signals:** `window.__routingWorker.active` now returns `true` during tuning re-routes (not just Route button clicks)
- **Inspection:** `window.__routingWorker.lastResult` updated after tuning re-route with raw JSON result; `window.__triggerVariantRouting` callable from console for manual variant generation
- **Failure visibility:** Worker errors during tuning logged with `[Worker Error]` prefix; UI resets to non-routing state on error/crash

## Expected Output

- `viewer/src/main.ts` — refactored `onTuningSliderInput()` (worker-based), new `triggerVariantRouting()` function, zero synchronous WASM routing calls
