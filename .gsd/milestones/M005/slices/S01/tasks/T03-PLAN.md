---
estimated_steps: 8
estimated_files: 1
---

# T03: Refactor triggerRouting and cancelRouting to use Web Worker

**Slice:** S01 — Web Worker WASM Routing
**Milestone:** M005

## Description

This is the main integration task — replace the synchronous `engine.auto_route()` call in `triggerRouting()` with worker-based async routing. This directly delivers R201 (main thread never blocked), R202 (spinner visible throughout), and R203 (cancel terminates immediately).

Currently `triggerRouting()` (line 1459 of `main.ts`) does a 50ms setTimeout yield, then calls `engine.auto_route()` synchronously — blocking the main thread for 60-160+ seconds. `cancelRouting()` (line 1620) just sets `isRouting = false` with no way to interrupt WASM.

After this task: Route button spawns a Web Worker, posts the board source, and handles the result asynchronously. The main thread stays free to paint the spinner overlay and respond to cancel clicks. Cancel calls `worker.terminate()`.

## Steps

1. **Add worker import pattern at the top of the routing section in `main.ts`:**
   Import the `WorkerResponse` type from `worker-protocol.ts` for typing `onmessage` events.

2. **Add worker state variables** near the existing `isRouting` and `routingStartTime` declarations:
   ```typescript
   let routingWorker: Worker | null = null;
   ```

3. **Create `spawnRoutingWorker()` helper function:**
   ```typescript
   function spawnRoutingWorker(): Worker {
     return new Worker(
       new URL('./routing-worker.ts', import.meta.url),
       { type: 'module' }
     );
   }
   ```

4. **Refactor `triggerRouting()` to use the worker:**
   - Keep the existing guards (`isRouting`, `!snapshot?.board`).
   - Set `isRouting = true`, call `updateRoutingUI()`, set `routingStartTime` — all synchronous.
   - Remove the 50ms `setTimeout` yield hack (line ~1494: `await new Promise(resolve => setTimeout(resolve, 50))`).
   - Terminate any existing worker if present (`routingWorker?.terminate()`).
   - Spawn new worker via `spawnRoutingWorker()`, store in `routingWorker`.
   - Set up `worker.onmessage` handler:
     - On `{type:'ready'}`: log `[Routing] Worker WASM ready`, then post `{type:'route', source: lastLoadedSource!}`.
     - On `{type:'route-result', snapshot: workerSnapshot, routeResult}`: replace the local `snapshot` variable with `workerSnapshot`, rebuild `padNetMap` from the new snapshot's nets (same as `pullSnapshot()` logic), set `dirty = true`, parse `routeResult` JSON for routed/unrouted counts, update `statusText`, set `isRouting = false`, call `updateRoutingUI()`, clean up worker reference.
     - On `{type:'error', message}`: log error, update `statusText`, set `isRouting = false`, call `updateRoutingUI()`, clean up.
   - Set up `worker.onerror` handler: log the error, reset UI state.
   - Update `__routingWorker` debug surface.
   - The function should now return immediately (no await on routing result).

5. **Refactor `cancelRouting()`:**
   - If `routingWorker` exists: call `routingWorker.terminate()`, set `routingWorker = null`.
   - Set `isRouting = false`, call `updateRoutingUI()`, update status text.
   - Update `__routingWorker` debug surface.
   - Log `[Routing] Worker terminated (cancel)`.

6. **Expose `window.__routingWorker` debug surface:**
   ```typescript
   (window as any).__routingWorker = {
     get active() { return isRouting && routingWorker !== null; },
     lastResult: null as string | null,
   };
   ```
   Update `lastResult` when worker posts `route-result`. Set it to `null` when a new route starts.

7. **Handle edge cases:**
   - Rapid Route clicks: `triggerRouting()` terminates existing worker before spawning new one (step 4 covers this).
   - Worker crash (onerror): reset `isRouting`, hide overlay, log error.
   - Board reload during routing: `cancelRouting()` should be safe to call anytime.

8. **Verify in browser:**
   - Start dev server: `cd viewer && npm run dev`
   - Open browser, load a board
   - Click Route: overlay appears instantly, spinner plays, cancel button visible
   - Click Cancel during routing: overlay disappears, board returns to pre-route state
   - Let routing complete: board shows routed traces, status shows segment count
   - Check console: `[Worker] WASM initialized`, `[Routing] Worker result received`
   - Check `window.__routingWorker`: `active` is `true` during routing, `false` after

## Must-Haves

- [ ] `triggerRouting()` spawns a Web Worker — no synchronous `engine.auto_route()` call
- [ ] Overlay/spinner visible for the entire routing duration
- [ ] `cancelRouting()` calls `worker.terminate()` — routing stops immediately
- [ ] Worker result replaces `snapshot` and rebuilds `padNetMap` (same as `pullSnapshot()` behavior)
- [ ] `window.__routingWorker` debug surface with `active` (boolean getter) and `lastResult`
- [ ] Error handling: worker errors surface to main thread and reset UI
- [ ] 50ms setTimeout yield hack removed
- [ ] `isRouting` flag set synchronously before any async work (prevents double-click)

## Observability Impact

- Signals added: `[Routing] Worker spawned`, `[Routing] Worker WASM ready`, `[Routing] Worker result received`, `[Routing] Worker terminated (cancel)`, `[Worker Error]` prefix for onerror
- How a future agent inspects this: `window.__routingWorker.active` → boolean, `window.__routingWorker.lastResult` → JSON string or null
- Failure state exposed: Worker errors logged with `[Worker Error]` prefix; `__routingWorker.active` remains `false` on failure; status text shows error message

## Verification

- Dev server: Route button → overlay stays visible → cancel works → board routes successfully
- Console shows worker lifecycle logs
- `window.__routingWorker.active` returns correct boolean during/after routing
- No synchronous `engine.auto_route()` calls remain in `triggerRouting()` (`grep "engine.auto_route()" viewer/src/main.ts` should show zero hits in the triggerRouting function)

## Inputs

- `viewer/src/routing-worker.ts` — Web Worker module (from T02)
- `viewer/src/worker-protocol.ts` — shared message types (from T01)
- `viewer/src/main.ts` — current `triggerRouting()` at line 1459, `cancelRouting()` at line 1620, `updateRoutingUI()` at line 1434, `pullSnapshot()` at line 250, `snapshot` at line 224, `lastLoadedSource` at line 229, `isRouting` at line 204, `padNetMap` usage after pullSnapshot
- Existing debug surfaces: `__pcbEngine`, `__routingState`, `__viewport`, `__loadBoard`

## Expected Output

- `viewer/src/main.ts` — refactored `triggerRouting()` (worker-based), `cancelRouting()` (terminate-based), new `spawnRoutingWorker()` helper, `__routingWorker` debug surface. All synchronous WASM routing calls removed from the Route button path.
