# M005: WASM Routing — Off Main Thread

**Gathered:** 2026-03-15
**Status:** Ready for planning

## Project Description

CodeYourPCB's autorouter (PathFinder negotiated congestion, M004) works natively but is broken in the browser. WASM routing runs synchronously on the main thread, freezing the browser for 60-160+ seconds. No spinner, no cancel, no progress. The routing result is also poor — 5/25 connections unrouted on the simplest template (Blink LED, 8 nets). CI tests pass green because they test native Rust, not WASM-in-browser. This milestone fixes all three problems.

## Why This Milestone

User says "autorouter nie działa" — and they're right. A feature that freezes the browser and produces incomplete results is worse than no feature. CI giving green when the product is broken means the test suite is lying.

## User-Visible Outcome

### When this milestone is complete, the user can:

- Click Route → see a spinner/progress overlay → browser stays responsive → result appears
- Cancel routing mid-execution if it takes too long
- See all 25 connections routed on Blink LED (0 ratsnest remaining)
- Route button generates multiple variants with score panel and hover preview (via Worker)

### Entry point / environment

- Entry point: Route button in viewer toolbar
- Environment: browser (Vite dev, Cloudflare Pages prod)
- Live dependencies involved: none (all local WASM)

## Completion Class

- Contract complete means: Web Worker routing tests pass, E2E tests verify responsiveness and quality
- Integration complete means: Route button → Worker → WASM → result displayed with variants
- Operational complete means: browser never freezes during routing

## Final Integrated Acceptance

To call this milestone complete, we must prove:

- Load Blink LED template, click Route → browser stays responsive, spinner visible, result shows 0 unrouted
- Cancel button stops routing mid-execution
- Variant panel shows ranked alternatives with hover preview
- CI E2E test catches both UI freeze and routing quality regressions

## Risks and Unknowns

- **WASM in Web Worker initialization** — wasm-pack `--target web` modules need specific init() pattern inside workers. importScripts() or ES module import must work.
- **Message serialization overhead** — board state must be passed to worker via postMessage. Large boards could have serialization cost.
- **PathFinder convergence on Blink LED** — 5 unrouted nets needs root-cause debugging. Could be grid resolution, pad mapping, or convergence failure.
- **Cancel mechanism** — WASM has no preemption. Cancel requires either worker.terminate() + restart, or cooperative checking (impractical in synchronous WASM call).

## Existing Codebase / Prior Art

- `viewer/src/wasm.ts` — current WASM loading, PcbEngine interface, WasmPcbEngineAdapter
- `viewer/src/main.ts:1459` — `triggerRouting()` synchronous call to `engine.auto_route()`
- `viewer/pkg/cypcb_render.js` — wasm-bindgen generated glue, `__wbg_init()` for initialization
- `viewer/pkg/cypcb_render_bg.wasm` — 638KB WASM binary
- `crates/cypcb-render/src/lib.rs` — PcbEngine WASM exports (auto_route, auto_route_variants, auto_route_with_params)
- `crates/cypcb-autoroute/src/pathfinder_v2.rs` — PathFinder algorithm, MAX_PATHFINDER_ITERATIONS=50
- `viewer/e2e/variant-panel.spec.ts` — existing variant E2E tests (use waitForTimeout, not responsive)
- `viewer/vite.config.ts` — uses vite-plugin-wasm + vite-plugin-top-level-await

> See `.gsd/DECISIONS.md` for all architectural and pattern decisions.

## Relevant Requirements

- R201 — Web Worker routing (main thread never blocked)
- R202 — Routing progress visible
- R203 — Cancel routing
- R204 — 0 unrouted on Blink LED
- R205 — E2E test: UI responsive during routing
- R206 — E2E test: routing result quality
- R207 — Variant generation via Worker

## Scope

### In Scope

- Web Worker for WASM routing (new `routing-worker.ts`)
- Message protocol: main ↔ worker (route request, progress, result, cancel)
- Worker-side WASM init and PcbEngine instantiation
- Cancel via worker.terminate() + fresh worker
- Fix PathFinder convergence on Blink LED (debug 5 unrouted nets)
- E2E tests: UI responsiveness during routing, routing quality assertions
- Variant generation through worker (reuse auto_route_variants)
- Tuning slider re-routing through worker

### Out of Scope / Non-Goals

- SharedArrayBuffer / multi-threaded WASM (requires COOP/COEP headers)
- Progressive/incremental routing display (show routes as they're found)
- Renderer upgrade (M006)
- Routing performance optimization beyond fixing convergence

## Technical Constraints

- Web Worker cannot share memory with main thread (no SharedArrayBuffer without COOP/COEP)
- WASM module must be initialized separately inside the worker
- Board state serialized as JSON string via postMessage (PcbEngine.load_source)
- Cancel = worker.terminate() — no cooperative cancellation inside synchronous WASM call
- Vite handles Web Worker bundling via `new Worker(new URL(...), { type: 'module' })`

## Integration Points

- `triggerRouting()` in main.ts → refactor to postMessage to worker instead of direct WASM call
- Tuning slider handler → route through worker instead of direct auto_route_with_params
- Variant panel → receive variant results from worker message
- pullSnapshot() → must be called after worker returns result (engine state updated on main thread)

## Open Questions

- Should the worker maintain a persistent PcbEngine or create fresh per-route? Persistent avoids re-init overhead but complicates cancel (terminate kills engine).
- How to update main thread engine state after worker routes? Worker runs its own PcbEngine copy — need to sync routed board back.
