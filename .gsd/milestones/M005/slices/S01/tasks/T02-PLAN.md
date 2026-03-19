---
estimated_steps: 7
estimated_files: 1
---

# T02: Create routing-worker.ts with WASM init and route message handler

**Slice:** S01 — Web Worker WASM Routing
**Milestone:** M005

## Description

Create the Web Worker module that initializes WASM in a worker context, creates a `PcbEngine`, and handles routing messages. This is the riskiest piece of S01 — WASM initialization inside a Vite-bundled ES module worker hasn't been proven in this codebase.

The worker imports the WASM glue from `../pkg/cypcb_render.js`, calls `init()` with an explicit URL to the `.wasm` file (Vite resolves this at build time via `import.meta.url`), creates a `PcbEngine` per routing request, and posts results back.

Key constraints from research:
- `vite-plugin-wasm` and `vite-plugin-top-level-await` do NOT apply to workers — the worker must call `init()` explicitly
- Use `init(new URL('../pkg/cypcb_render_bg.wasm', import.meta.url))` for reliable Vite URL resolution
- `PcbEngine.get_snapshot()` returns `JsValue` which is structured-clone-able via `postMessage`
- `auto_route()` and `auto_route_with_params()` return JSON strings
- The worker creates a fresh `PcbEngine` for each request (per D-M005-005: fresh worker per route)
- WASM panics must be caught — `console_error_panic_hook` logs to worker console but main thread won't see it; catch and forward via `{type:'error'}` message

## Steps

1. **Create `viewer/src/routing-worker.ts`** as an ES module Web Worker file.

2. **Add WASM initialization at worker startup:**
   ```typescript
   import init, { PcbEngine } from '../pkg/cypcb_render.js';
   import { parseSource } from './parse-source';
   import type { WorkerRequest, WorkerResponse } from './worker-protocol';
   
   async function initialize() {
     await init(new URL('../pkg/cypcb_render_bg.wasm', import.meta.url));
     const msg: WorkerResponse = { type: 'ready' };
     self.postMessage(msg);
     console.log('[Worker] WASM initialized');
   }
   initialize().catch(err => {
     const msg: WorkerResponse = { type: 'error', message: `WASM init failed: ${err}` };
     self.postMessage(msg);
   });
   ```

3. **Add message handler for `route` messages:**
   - On `{type:'route', source}`: call `parseSource(source)` to get a `BoardSnapshot`, create `new PcbEngine()`, call `engine.load_snapshot(snapshot)`, call `engine.auto_route()` → get `routeResult` (JSON string), call `engine.get_snapshot()` → get updated snapshot, post `{type:'route-result', snapshot, routeResult}` back.

4. **Add message handler for `route-with-params` messages:**
   - On `{type:'route-with-params', source, params}`: same flow as `route`, but call `engine.auto_route_with_params(params)` instead.

5. **Add message handler for `route-variants` messages:**
   - On `{type:'route-variants', source}`: same setup, call `engine.auto_route_variants()`, post `{type:'variant-result', variants}` where `variants` is the JSON string returned by `auto_route_variants()`.

6. **Wrap all message handlers in try/catch:**
   - On any error, post `{type:'error', message: String(err)}` back to main thread.
   - Log the full error to worker console for debugging: `console.error('[Worker] Error:', err)`.

7. **Verify TypeScript compiles:** Run `cd viewer && npx tsc --noEmit`. The worker file imports from `../pkg/cypcb_render.js` which may not have type declarations — use `// @ts-ignore` or declare module if needed. The WASM package types depend on whether `wasm-pack` generated `.d.ts` files. If not, add a minimal type declaration in the worker or a `cypcb-render.d.ts` file.

## Must-Haves

- [ ] `routing-worker.ts` initializes WASM with explicit URL pattern for Vite compatibility
- [ ] Posts `{type:'ready'}` after successful WASM init
- [ ] Handles `{type:'route'}` — creates PcbEngine, parses source, routes, posts snapshot + result
- [ ] Handles `{type:'route-with-params'}` — routes with custom parameters
- [ ] Handles `{type:'route-variants'}` — generates variants, posts variant data
- [ ] All errors caught and forwarded as `{type:'error', message}` to main thread
- [ ] Uses `WorkerRequest` / `WorkerResponse` types from `worker-protocol.ts`

## Verification

- `cd viewer && npx tsc --noEmit` compiles without errors (or only pre-existing WASM type issues)
- File structure review: `routing-worker.ts` has `self.onmessage` handler, `init()` call, all 3 routing message types handled
- Full integration verified in T03 when main thread spawns this worker

## Inputs

- `viewer/src/worker-protocol.ts` — `WorkerRequest` and `WorkerResponse` types (from T01)
- `viewer/src/parse-source.ts` — `parseSource()` function (from T01)
- `viewer/pkg/cypcb_render.js` — WASM glue module (must exist from prior `wasm-pack build --target web`)
- `viewer/pkg/cypcb_render_bg.wasm` — WASM binary
- Research notes on WASM exports: `PcbEngine::new()`, `load_snapshot(JsValue)`, `get_snapshot() -> JsValue`, `auto_route() -> String`, `auto_route_with_params(String) -> String`, `auto_route_variants() -> String`

## Observability Impact

- **New signals:** `[Worker] WASM initialized` on successful init; `[Worker] Routing started` / `[Worker] Routing complete` for each routing request; `[Worker] Error:` prefix on caught exceptions inside message handler
- **Inspection:** Future agent can verify worker exists by checking that `routing-worker.ts` posts `{type:'ready'}` on init — main thread (T03) will expose this via `window.__routingWorker`
- **Failure visibility:** WASM init failure posts `{type:'error', message: 'WASM init failed: ...'}` — main thread handler (T03) will surface this. Worker-side `console.error` captures full stack trace for dev tools.

## Expected Output

- `viewer/src/routing-worker.ts` — new Web Worker module, ~80-120 lines, handling WASM init and all 3 routing message types
