---
id: T03
parent: S01
milestone: M005
provides:
  - Worker-based triggerRouting() that spawns a Web Worker instead of calling engine.auto_route() synchronously
  - Worker-termination-based cancelRouting() for immediate cancel
  - spawnRoutingWorker() helper function for Vite module worker instantiation
  - window.__routingWorker debug surface with active (boolean getter) and lastResult (string|null)
key_files:
  - viewer/src/main.ts
key_decisions:
  - triggerRouting() changed from async to sync void — worker callbacks handle async result
  - Worker onmessage handles ready→post route flow (two-phase: init then route)
  - variant-result case stub added for forward compatibility with T04
  - Routing overlay progress text updated from "browser will be unresponsive" to "routing in background"
patterns_established:
  - Worker lifecycle in main thread: spawn → store ref → onmessage/onerror handlers → terminate on cancel or cleanup on result
  - Debug surface getter pattern: `get active() { return isRouting && routingWorker !== null; }` for live boolean
  - Rapid-click guard: terminate existing worker before spawning new one
observability_surfaces:
  - window.__routingWorker.active — boolean getter, true during worker routing
  - window.__routingWorker.lastResult — raw JSON string from last completed route, null during routing
  - Console logs: [Routing] Worker spawned, [Routing] Worker WASM ready, [Routing] Worker result received, [Routing] Worker terminated (cancel), [Worker Error] prefix
duration: 25m
verification_result: passed
completed_at: 2026-03-18T18:03:00Z
blocker_discovered: false
---

# T03: Refactor triggerRouting and cancelRouting to use Web Worker

**Replaced synchronous engine.auto_route() in triggerRouting with Web Worker spawn/message pattern; cancelRouting now calls worker.terminate() for immediate cancellation**

## What Happened

Refactored `triggerRouting()` in `main.ts` to spawn a Web Worker (`routing-worker.ts` from T02) instead of calling `engine.auto_route()` synchronously. The function is now synchronous void — it sets `isRouting = true` and shows the overlay immediately, spawns the worker, and returns. Worker `onmessage` handles the async result by replacing `snapshot`, rebuilding `padNetMap`, updating status text, and resetting UI state.

Refactored `cancelRouting()` to call `routingWorker.terminate()` which immediately kills the WASM execution in the worker thread, then resets all UI state.

Added `spawnRoutingWorker()` helper that uses Vite's `new Worker(new URL(...), {type:'module'})` pattern. Added `window.__routingWorker` debug surface with a live `active` getter and `lastResult` string.

Removed the 50ms `setTimeout` yield hack that was needed when routing blocked the main thread. Updated the routing overlay text from "browser will be unresponsive" to "routing in background — you can continue working or click Cancel."

## Verification

- **TypeScript compilation:** `npx tsc --noEmit` — only pre-existing unused import warning (showVariants), no new errors
- **Vite build:** `npx vite build` — succeeds in 27s, worker bundled correctly
- **Must-have 1:** `grep "engine.auto_route()" main.ts` → 0 matches (no synchronous WASM calls)
- **Must-have 2:** `spawnRoutingWorker()` called in `triggerRouting()`
- **Must-have 3:** `cancelRouting()` calls `routingWorker.terminate()`
- **Must-have 4:** Worker result replaces `snapshot` and rebuilds `padNetMap` via `buildPadNetMap()`
- **Must-have 5:** `window.__routingWorker` debug surface with `active` getter and `lastResult`
- **Must-have 6:** `worker.onerror` and `{type:'error'}` message both handled with `[Worker Error]` prefix
- **Must-have 7:** 50ms setTimeout yield hack removed
- **Must-have 8:** `isRouting = true` set synchronously before any async work

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `npx tsc --noEmit` | 2 (pre-existing showVariants unused) | ✅ pass (no new errors) | 3.1s |
| 2 | `npx vite build` | 0 | ✅ pass | 27.4s |
| 3 | `grep "engine.auto_route()" viewer/src/main.ts` | 1 (no matches) | ✅ pass | <1s |
| 4 | `grep "setTimeout(resolve, 50)" viewer/src/main.ts` | 1 (no matches) | ✅ pass | <1s |
| 5 | Observability signal grep (5 signals) | 0 | ✅ pass | <1s |
| 6 | E2E `autoroute-worker.spec.ts` | — | ⏳ pending T05 | — |

## Diagnostics

- **Runtime inspection:** `window.__routingWorker.active` returns `true` during routing, `false` after completion/cancel/error
- **Last result:** `window.__routingWorker.lastResult` contains the raw JSON route result string after completion
- **Console signals:** `[Routing] Worker spawned` → `[Routing] Worker WASM ready` → `[Routing] Worker result received` for successful routing; `[Worker Error]` prefix for failures
- **Error path:** Worker WASM panics forwarded as `{type:'error', message}`, logged with `[Worker Error]` prefix, UI reset to non-routing state

## Deviations

- Changed `triggerRouting()` from `async function` to `function` (sync void) since the worker callback pattern doesn't need await. This is a simplification over the plan which didn't specify the function signature change, but it's the natural result of removing the await.
- Added a `variant-result` case in the onmessage handler as a forward-compatibility stub for T04, rather than leaving it unhandled.

## Known Issues

- Pre-existing `showVariants` unused import warning in TypeScript compilation (not introduced by this task)
- E2E tests (`autoroute-worker.spec.ts`) not yet created — scheduled for T05

## Files Created/Modified

- `viewer/src/main.ts` — Refactored triggerRouting() to use Web Worker, refactored cancelRouting() to terminate worker, added spawnRoutingWorker() helper, added __routingWorker debug surface, updated overlay progress text, added WorkerResponse import
