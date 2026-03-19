# S01: Web Worker WASM Routing — UAT

**Milestone:** M005
**Written:** 2026-03-18

## UAT Type

- UAT mode: mixed (artifact-driven for code verification + live-runtime for browser behavior)
- Why this mode is sufficient: Worker routing requires real browser + Vite dev server to prove main thread responsiveness. Code checks verify zero synchronous WASM calls. E2E tests automate the browser verification. Full WASM runtime proof requires CI with wasm-pack build.

## Preconditions

1. Vite dev server running: `cd viewer && npm run dev`
2. A `.cypcb` board loaded (use templates or `routing-test.cypcb`)
3. For test 6, WASM binary must be available (run `wasm-pack build --target web` in the project root first)
4. Browser DevTools console open for observability checks

## Smoke Test

Load any board, click Route button. If a spinner/overlay appears immediately and the browser stays responsive (can scroll, move mouse, click cancel), the worker is active. If the browser freezes, the worker is NOT active and S01 is broken.

## Test Cases

### 1. Route button shows overlay immediately

1. Open the viewer in browser (`http://localhost:5173`)
2. Load a board (use the LED Blink template or drag a `.cypcb` file)
3. Click the **Route** button
4. **Expected:** Routing overlay (`#routing-status`) appears within 100ms. Text says "routing in background". Cancel button is visible. Browser does NOT freeze — you can move the mouse, scroll the page, open DevTools.

### 2. Cancel button terminates routing

1. Load a board and click **Route**
2. While overlay is visible, click **Cancel**
3. **Expected:** Overlay disappears within 1 second. Board returns to pre-route state (no new traces added). Console shows `[Routing] Worker terminated (cancel)`. `window.__routingWorker.active === false` in console.

### 3. Routing produces routed board

1. Load the LED Blink template (or `routing-test.cypcb`)
2. Click **Route** and wait for completion
3. **Expected:** Overlay disappears. Status text shows routing result (e.g., "Routed: 20/25 connections"). Board canvas shows new traces. `window.__routingWorker.lastResult` is a non-null JSON string containing `ok: true`.

### 4. Debug surface reflects worker state

1. Open browser console
2. Load a board and check `window.__routingWorker.active` → should be `false`
3. Click **Route**
4. Immediately check `window.__routingWorker.active` → should be `true`
5. Wait for completion
6. Check `window.__routingWorker.active` → should be `false`
7. Check `window.__routingWorker.lastResult` → should be a JSON string
8. **Expected:** All values match the lifecycle above. Console shows `[Routing] Worker spawned` → `[Routing] Worker WASM ready` → `[Routing] Worker result received`.

### 5. Tuning sliders route via worker

1. Load a board and click **Route** to get initial traces
2. Open the tuning panel (⚡ button)
3. Adjust the **Via Cost** slider
4. **Expected:** After 300ms debounce, board re-routes without freezing. Console shows `[Tuning] Worker spawned`. Status text updates with new routing result. Browser remains responsive during re-route.

### 6. Variant routing via worker (requires WASM)

1. Open browser console
2. Type `window.__triggerVariantRouting()` and press Enter
3. **Expected:** Console shows `[Variants] Worker spawned`. If WASM is available, variant results are posted back. If WASM is unavailable (403), console shows `[Worker Error] Variants:` — this is expected in dev without wasm-pack build.

### 7. Rapid Route clicks don't stack workers

1. Load a board
2. Click **Route** rapidly 3 times in quick succession
3. **Expected:** Only one routing operation completes. Console shows `[Routing] Terminated previous worker` for each rapid click after the first. No errors. Final result appears normally.

### 8. Worker error handling

1. Open browser console
2. Call `window.__loadBoard('')` (empty source)
3. Click **Route**
4. **Expected:** Worker receives empty source, encounters parse error, posts `{type:'error'}`. Console shows `[Worker Error]` message. Overlay hides. `window.__routingWorker.active === false`. UI is in a clean non-routing state.

## Edge Cases

### Cancel during WASM init

1. Click **Route** and immediately click **Cancel** within 100ms (before worker posts `ready`)
2. **Expected:** Worker terminates cleanly. Overlay disappears. No console errors beyond the expected `[Routing] Worker terminated (cancel)`. UI resets.

### Route with no board loaded

1. Open the viewer fresh (project manager visible)
2. Try to click **Route** (if accessible)
3. **Expected:** Console shows `[Routing] No board loaded`. No worker spawns. No overlay appears.

### Multiple tuning adjustments in rapid succession

1. Load and route a board
2. Rapidly slide Via Cost from min to max in quick sweeps
3. **Expected:** Debounce (300ms) prevents worker spam. At most 1 tuning worker active at any time. Previous tuning workers terminated before new ones spawn. No freeze.

## Failure Signals

- **Browser freezes during routing** → WASM is running on main thread, not worker. Check that `main.ts` has zero `engine.auto_route()` calls.
- **Overlay doesn't appear** → `isRouting` not set synchronously, or overlay CSS is broken. Check `triggerRouting()` sets `isRouting = true` before spawning worker.
- **Cancel doesn't work** → `worker.terminate()` not called, or worker reference is null. Check `cancelRouting()` implementation.
- **Console shows `[Worker] WASM init failed`** → WASM binary not found. Check that `viewer/pkg/cypcb_render_bg.wasm` exists and Vite `server.fs.allow` includes its path.
- **`__routingWorker.active` stuck on `true`** → Worker completed/errored but handlers didn't reset `isRouting`. Check all exit paths in `onmessage` and `onerror`.
- **E2E tests fail** → Run `npx playwright test e2e/autoroute-worker.spec.ts --reporter=list` and check trace output.

## Requirements Proved By This UAT

- **R201** — Tests 1, 2, 3, 4, 5, 7, 8 prove main thread never blocked (overlay visible, cancel responsive, browser interactive during routing)
- **R202** — Tests 1, 4 prove overlay visible immediately and throughout routing duration
- **R203** — Tests 2, edge case "cancel during WASM init" prove cancel terminates routing and resets UI
- **R207** — Test 6 proves variant routing message path exists (partial — UX wiring in S04)

## Not Proven By This UAT

- **R204** (0 unrouted on Blink LED) — routing quality is S02 scope
- **R205, R206** (E2E regression tests in CI) — CI pipeline integration is S03 scope
- **R207 full** (variant panel, hover preview, score ranking) — variant UX is S04 scope
- **WASM runtime proof with real routing** — E2E test 3 requires WASM binary from wasm-pack build; unavailable in sandbox. Full proof when CI runs with WASM.

## Notes for Tester

- The WASM binary may not be available in development without running `wasm-pack build --target web` first. Without it, workers will error on init (403 from Vite serving). This is expected — the mock engine handles main-thread features, but workers need real WASM.
- The Vite dev server may show warnings about WASM file being "outside of Vite serving allow list" — this is a worktree path issue, not a code bug. In normal checkout it resolves correctly.
- Test 3 is the only test that truly exercises the full WASM routing pipeline. If you only verify one thing manually, verify that Route → routed board appears (test 3 scenario) in an environment with WASM available.
- The pre-existing `showVariants` unused import in TypeScript compilation is harmless — it will resolve when S04 wires up variant routing.
