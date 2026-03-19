# S01 — Web Worker WASM Routing — Research

**Date:** 2026-03-18
**Depth:** Deep (new architecture — WASM in Web Worker, message protocol, worker lifecycle)

## Summary

S01 moves all WASM autorouting calls (`auto_route`, `auto_route_with_params`, `auto_route_variants`) off the main thread into a Web Worker. Currently, `triggerRouting()` in `viewer/src/main.ts:1459` calls `engine.auto_route()` synchronously on the main thread, with only a 50ms `setTimeout` yield before blocking the browser for 60-160+ seconds. The cancel button is non-functional (just sets `isRouting = false` in JS — no way to interrupt WASM).

The primary challenge is that the WASM module (`cypcb_render.js` + `cypcb_render_bg.wasm`) must be re-initialized inside the Worker because Workers have separate memory spaces. The board source must be sent via `postMessage`, the Worker must parse it (using the same JS `parseSource()` from `wasm.ts`) and load it into a Worker-side `PcbEngine`, route it, then send back the snapshot and result JSON. The main thread engine stays alive for non-routing operations (`query_point`, `add_trace`, etc.) and receives the routed snapshot to replace its cached state.

Key decisions already made: D-M005-004 (worker routes on its own PcbEngine copy, posts snapshot back), D-M005-005 (fresh worker per route for simple cancel), D-M005-006 (Vite `new Worker(new URL(...))` pattern).

## Recommendation

**Approach:** Create `viewer/src/routing-worker.ts` as an ES module worker. The worker imports the WASM glue from `../pkg/cypcb_render.js`, initializes WASM, creates a `PcbEngine`, and responds to messages. Main thread refactors `triggerRouting()` to spawn a worker, post the board source, show overlay, and handle results asynchronously. Cancel = `worker.terminate()`.

**Why this approach:** Aligns with all 6 existing decisions (D-M005-001 through D-M005-006). The `--target web` wasm-pack output produces ES module glue code that can be `import()`-ed inside an ES module worker. Vite's built-in worker bundling handles the asset URL resolution for the `.wasm` file. The main thread `WasmPcbEngineAdapter` already has a `cachedSnapshot` that can be replaced wholesale when the worker posts results back.

## Implementation Landscape

### Key Files

- `viewer/src/main.ts` (2239 lines) — Contains `triggerRouting()` at line 1459 (synchronous `engine.auto_route()`), `updateRoutingUI()` at line 1434, `cancelRouting()` at line 1621, tuning slider handler at line 1730 (synchronous `engine.auto_route_with_params()`). All three routing callsites must be refactored to use the worker.
- `viewer/src/wasm.ts` (1005 lines) — Contains `parseSource()` (line 224) which the worker needs for `load_snapshot()`. Also contains `WasmPcbEngineAdapter` (line 583) which manages the `cachedSnapshot` that must be replaced with worker results. The `PcbEngine` interface (line 26) defines all methods the worker-side engine exposes.
- `viewer/src/variant-panel.ts` (197 lines) — `showVariants()` and `hideVariants()`. Currently called from main.ts after synchronous variant generation. Will be called after worker posts variant results.
- `viewer/index.html` — Contains `#routing-status` overlay (line 1203), `#cancel-route-btn` (line 1051), `#route-btn` (line 1050). The overlay HTML exists but is only shown briefly before WASM blocks rendering. With the worker, it will remain visible.
- `crates/cypcb-render/src/lib.rs` — WASM exports: `PcbEngine::new()`, `load_snapshot(JsValue)`, `get_snapshot() -> JsValue`, `auto_route() -> String`, `auto_route_with_params(String) -> String`, `auto_route_variants() -> String`. All use `#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]`. No Rust changes needed — worker uses the same WASM binary.
- `viewer/vite.config.ts` — Uses `vite-plugin-wasm` + `vite-plugin-top-level-await`. The worker will use Vite's native `new Worker(new URL(...), { type: 'module' })` pattern. No config changes needed for worker bundling.
- `viewer/build-wasm.sh` — Builds with `wasm-pack build --target web`. The `--target web` output creates an ES module with `default()` init function that accepts an optional `URL` or `Response` for the `.wasm` file. This init function works inside Workers.

### New File

- `viewer/src/routing-worker.ts` — The Web Worker module. Responsibilities:
  1. Import WASM glue: `import init, { PcbEngine } from '../pkg/cypcb_render.js'`
  2. On load: call `await init()` to initialize WASM, post `{type:'ready'}` to main thread
  3. On `{type:'route', source}`: create PcbEngine, call `load_snapshot()` with parsed source, call `auto_route()`, post `{type:'route-result', snapshot, routeResult}` back
  4. On `{type:'route-variants', source}`: same but call `auto_route_variants()`
  5. On `{type:'route-with-params', source, params}`: same but call `auto_route_with_params(params)`

### Build Order

1. **T01: Create `routing-worker.ts` with WASM init + `route` message handler.** This is the riskiest piece — WASM init inside a Vite-bundled ES module worker. Prove it by having the worker post a `ready` message and successfully route a minimal board. Verification: Vitest or manual browser console test.

2. **T02: Refactor `triggerRouting()` to use worker.** Replace the synchronous `engine.auto_route()` call with worker spawn → postMessage → onmessage result handler. Show overlay immediately, handle result asynchronously. Replace `pullSnapshot()` with direct snapshot assignment from worker's result. Wire cancel button to `worker.terminate()`. Expose `window.__routingWorker = { active, lastResult }` debug surface.

3. **T03: Refactor tuning slider handler to route via worker.** The `onTuningSliderInput` handler (line 1730) currently calls `engine.auto_route_with_params()` synchronously. Refactor to post `{type:'route-with-params', source, params}` to worker.

4. **T04: Wire variant generation through worker.** Add `{type:'route-variants'}` support. When result comes back, call `showVariants()` with the variant data. This unblocks S04 (variant panel via worker).

5. **T05: E2E smoke test — overlay visible + cancel works during routing.** Playwright test loads a board, clicks Route, asserts overlay is visible and cancel button is clickable while routing runs. This proves main thread is not blocked. Also assert `__routingWorker.active === true` during routing and `false` after.

### Verification Approach

1. **Worker WASM init:** Open browser, check console for `[Worker] WASM initialized` log after clicking Route. No errors.
2. **Main thread responsiveness:** During routing, click cancel button — it responds immediately. The spinner animation plays smoothly.
3. **Routing result:** After worker completes, board shows routed traces (snapshot replaced). Status text shows "Routed N segments in Xs".
4. **Cancel:** Click Route → immediately click Cancel → worker terminates, overlay hidden, board returns to pre-route state.
5. **Debug surface:** `window.__routingWorker` shows `{ active: true }` during routing, `{ active: false, lastResult: '...' }` after.
6. **Tuning sliders:** Adjust slider → board re-routes via worker, no freeze.
7. **E2E test:** `viewer/e2e/autoroute-worker.spec.ts` — overlay visible during routing, cancel works, result has expected routed count.

## Constraints

- **Worker WASM import:** The `cypcb_render.js` glue from `wasm-pack --target web` exports a default `init()` function and named class exports. Inside an ES module worker, `import init, { PcbEngine } from '../pkg/cypcb_render.js'` should work with Vite's bundler resolving the `.wasm` URL. However, `vite-plugin-wasm` and `vite-plugin-top-level-await` do NOT apply to workers — the worker has its own module graph. The worker must call `init()` explicitly and await it.
- **No `vite-plugin-wasm` in workers:** The top-level-await plugin won't transform worker code. The worker must handle WASM init manually with an explicit `await init()` call. This is actually simpler — no magic.
- **`parseSource()` duplication:** The worker needs `parseSource()` from `wasm.ts` to convert source text to a `BoardSnapshot` for `load_snapshot()`. Options: (a) extract `parseSource()` to a shared module both main thread and worker import, or (b) duplicate it. Option (a) is cleaner — extract to `viewer/src/parse-source.ts`.
- **Snapshot serialization:** `PcbEngine.get_snapshot()` returns `JsValue` (a plain JS object via `serde_wasm_bindgen`). Plain JS objects are structured-clone-able by `postMessage` — no manual serialization needed.
- **`auto_route()` return type:** Returns a JSON string (not JsValue). Can be posted directly.
- **Main thread engine state:** After routing, the main thread needs to update its `cachedSnapshot` in `WasmPcbEngineAdapter`. Currently `cachedSnapshot` is private. Either: (a) add a public method to inject snapshot, or (b) bypass the adapter and directly assign `snapshot` in main.ts (simpler since `pullSnapshot()` just calls `engine.get_snapshot()` and stores the result). Recommended: main.ts directly replaces its local `snapshot` variable with the worker's result, same as `pullSnapshot()` does.
- **`lastLoadedSource` needed by worker:** The worker needs the board source string to create its PcbEngine. Main thread tracks this in `lastLoadedSource` (line 229). Must be sent with each route request message.

## Common Pitfalls

- **WASM URL resolution in worker:** Vite may not resolve the `.wasm` file URL correctly inside a worker bundle. If `init()` fails to fetch the WASM binary, pass the URL explicitly: `init(new URL('../pkg/cypcb_render_bg.wasm', import.meta.url))`. This pattern ensures Vite rewrites the URL at build time.
- **Worker message types lost:** Without a shared type definition for worker messages, it's easy to get message shape mismatches. Define `WorkerRequest` and `WorkerResponse` discriminated union types in a shared file (e.g., `viewer/src/worker-protocol.ts`).
- **Race condition on rapid Route clicks:** If user clicks Route while a worker is already running, must terminate the existing worker before spawning a new one. The `isRouting` flag already guards against this, but with async workers the flag must be set synchronously before `await`.
- **Variant panel data shape:** `auto_route_variants()` returns a JSON string containing routes with `{start: [x,y], end: [x,y]}` format (per `VariantData` in `variant-panel.ts`), not the `TraceSegmentInfo` format from `BoardSnapshot`. Worker must post this JSON string as-is for main thread to parse.
- **Error propagation:** WASM panics inside worker will crash the worker silently. Use `worker.onerror` and `worker.onmessageerror` handlers. The existing `console_error_panic_hook` (D-M004-036) will log to worker console but main thread won't see it — catch and forward.
- **`load_snapshot` expects JsValue:** In the worker, after `parseSource()` returns a JS object, `engine.load_snapshot(snapshot)` should work because `serde_wasm_bindgen::from_value` accepts any JsValue. But the snapshot object must match `BoardSnapshot` shape exactly.

## Open Risks

- **Vite worker WASM bundling untested in this codebase.** The combination of `vite-plugin-wasm` on the main thread + manual WASM init in worker hasn't been proven. If Vite's worker bundler doesn't handle the `.wasm` import correctly, may need to use `?worker&url` import or configure `worker.format: 'es'` in vite.config.ts.
- **WASM init time in worker.** D-M005-005 says ~100ms, but this hasn't been measured. If init is slow, the "fresh worker per route" strategy adds noticeable latency. Mitigation: measure during T01; if >500ms, switch to persistent worker with `terminate()` only for cancel.
- **`parseSource()` extraction.** Moving `parseSource()` out of `wasm.ts` into a shared module may break imports or require updating test imports. Low risk but needs care.

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Web Worker + WASM | `pluginagentmarketplace/custom-plugin-rust@rust-wasm` | available (24 installs) |
| Vite | `antfu/skills@vite` | available (10.2K installs) |

## Sources

- wasm-pack `--target web` output produces ES module with `init()` function that accepts optional URL param for WASM binary (source: [wasm-pack documentation](https://rustwasm.github.io/docs/wasm-pack/commands/build.html))
- Vite Web Worker support uses `new Worker(new URL(...), { type: 'module' })` pattern with full module bundling (source: [Vite Worker docs](https://vite.dev/guide/features#web-workers))
