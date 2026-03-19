---
id: T02
parent: S01
milestone: M005
provides:
  - routing-worker.ts Web Worker module with WASM init and 3 routing message handlers
key_files:
  - viewer/src/routing-worker.ts
key_decisions:
  - Fresh PcbEngine created per routing request, freed after result extraction (per D-M005-005)
  - Exhaustive switch with `never` type on default branch ensures compile-time enforcement that all WorkerRequest variants are handled
  - load_snapshot error string checked — non-empty value treated as failure and thrown
patterns_established:
  - Worker-side WASM init pattern: `await init(new URL('../pkg/cypcb_render_bg.wasm', import.meta.url))` — explicit URL required for Vite worker bundling
  - Worker error forwarding pattern: try/catch wrapping handler body, error posted as `{type:'error', message: String(err)}`, full error logged to worker console with `[Worker] Error:` prefix
  - Engine lifecycle pattern in worker: prepareEngine(source) → route method → get_snapshot() → engine.free() — deterministic cleanup
observability_surfaces:
  - "[Worker] WASM initialized" console log on successful init
  - "[Worker] Routing started" / "[Worker] Routing complete" for route requests
  - "[Worker] Routing with params started/complete" for tuned routing
  - "[Worker] Variant generation started/complete" for variant requests
  - "[Worker] Error:" prefix on caught exceptions (full error in worker console)
  - "{type:'ready'}" message posted to main thread on init success
  - "{type:'error', message}" posted for any failure (init or handler)
duration: 20m
verification_result: passed
completed_at: 2026-03-18T18:03Z
blocker_discovered: false
---

# T02: Create routing-worker.ts with WASM init and route message handler

**Created routing-worker.ts Web Worker that initializes WASM explicitly for Vite worker bundling and handles route, route-with-params, and route-variants messages with full error forwarding**

## What Happened

Created `viewer/src/routing-worker.ts` (~110 lines) as an ES module Web Worker. The worker:

1. **Initializes WASM at startup** using `init(new URL('../pkg/cypcb_render_bg.wasm', import.meta.url))` — the explicit URL pattern is required because `vite-plugin-wasm` and `vite-plugin-top-level-await` do not apply inside workers. Posts `{type:'ready'}` on success, `{type:'error'}` on failure.

2. **Handles three routing message types** via `self.onmessage`:
   - `route`: Parses source via shared `parseSource()`, creates fresh `PcbEngine`, calls `auto_route()`, extracts snapshot, posts `{type:'route-result', snapshot, routeResult}`
   - `route-with-params`: Same flow but calls `auto_route_with_params(params)` with the JSON params string
   - `route-variants`: Same setup but calls `auto_route_variants()`, posts `{type:'variant-result', variants}`

3. **Error handling**: All message handlers wrapped in try/catch. Errors posted as `{type:'error', message: String(err)}` and logged to worker console with `[Worker] Error:` prefix. WASM init failures also forwarded.

4. **Type safety**: Uses `WorkerRequest`/`WorkerResponse` discriminated union types from `worker-protocol.ts`. Exhaustive switch with `never` default ensures compile-time coverage of all message types.

5. **Engine lifecycle**: A `prepareEngine()` helper creates a fresh `PcbEngine` per request, loads the parsed snapshot, and returns the engine. After routing and snapshot extraction, `engine.free()` is called for deterministic WASM memory cleanup.

No changes needed to the WASM type declarations — `wasm-pack` generated `.d.ts` files that TypeScript resolves cleanly.

## Verification

- `cd viewer && npx tsc --noEmit` — routing-worker.ts produces zero TypeScript errors (the only error is a pre-existing unused import in main.ts, unrelated)
- File structure verified: `self.onmessage` handler present, `init()` called with explicit URL, all 3 routing message types handled (`route`, `route-with-params`, `route-variants`), error handling wraps all paths
- Full runtime integration verified in T03 when main thread spawns this worker

## Verification Evidence

| Check | Command | Exit Code | Verdict | Duration |
|-------|---------|-----------|---------|----------|
| TypeScript compilation (routing-worker.ts) | `npx tsc --noEmit 2>&1 \| grep routing-worker` | 1 (no matches = no errors) | ✅ pass | 3s |
| TypeScript compilation (full project) | `npx tsc --noEmit` | 2 (pre-existing main.ts issue only) | ✅ pass (no new errors) | 3s |
| File structure: self.onmessage | `grep -c self.onmessage routing-worker.ts` | 0 (1 match) | ✅ pass | <1s |
| File structure: init() with URL | `grep 'await init' routing-worker.ts` | 0 (1 match) | ✅ pass | <1s |
| File structure: 3 routing cases | `grep -c "case '" routing-worker.ts` | 0 (3 matches) | ✅ pass | <1s |
| File structure: error forwarding | `grep -c "type: 'error'" routing-worker.ts` | 0 (2 matches) | ✅ pass | <1s |
| Protocol types used | `grep WorkerRequest routing-worker.ts` | 0 (matches found) | ✅ pass | <1s |

### Slice-Level Verification (partial — T02 is intermediate)

| Check | Status | Notes |
|-------|--------|-------|
| `npx playwright test autoroute-worker.spec.ts` | ⏳ not yet | E2E test created in T05 |
| Overlay visible during routing | ⏳ not yet | Main thread refactor in T03 |
| Cancel button works | ⏳ not yet | Main thread refactor in T03 |
| `__routingWorker` debug surface | ⏳ not yet | Exposed in T03 |

## Inputs Consumed

- `viewer/src/worker-protocol.ts` (T01) — WorkerRequest/WorkerResponse types
- `viewer/src/parse-source.ts` (T01) — parseSource() function
- `viewer/pkg/cypcb_render.js` + `.d.ts` — WASM glue with PcbEngine class
- `viewer/pkg/cypcb_render_bg.wasm` — WASM binary

## Diagnostics

To inspect what this task built:
- **File exists:** `ls viewer/src/routing-worker.ts` — Web Worker module (~110 lines)
- **WASM init pattern:** `grep 'await init' viewer/src/routing-worker.ts` — should show explicit URL init for Vite
- **Message handlers:** `grep "case '" viewer/src/routing-worker.ts` — should show 3 cases (route, route-with-params, route-variants)
- **Error forwarding:** `grep "type: 'error'" viewer/src/routing-worker.ts` — should show 2+ error posts (init failure + handler failure)
- **Runtime:** In browser console, after Route button click, watch for `[Worker] WASM initialized`, `[Worker] Routing started`, `[Worker] Routing complete` messages. Worker errors appear with `[Worker] Error:` prefix.
- **Protocol compliance:** Worker posts `{type:'ready'}` after init, `{type:'route-result'}` / `{type:'variant-result'}` / `{type:'error'}` in response to requests — matches `WorkerResponse` union type.

## Outputs Produced

- `viewer/src/routing-worker.ts` — Web Worker module, ready for T03 to spawn via `new Worker(new URL('./routing-worker.ts', import.meta.url), {type:'module'})`
