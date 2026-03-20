# M005: WASM Routing — Off Main Thread

**Vision:** Move WASM autorouter to a Web Worker so the browser never freezes. Fix routing quality so Blink LED has 0 unrouted. Add E2E tests that catch real regressions.

## Success Criteria

- Route button never freezes the browser — main thread stays responsive during routing
- Spinner/progress overlay visible and cancel button clickable while routing executes in background
- Blink LED template routes all connections (0 unrouted, 0 ratsnest remaining)
- Variant generation works via Web Worker with score panel and hover preview
- E2E tests in CI verify UI responsiveness during routing and routing result quality

## Key Risks / Unknowns

- WASM initialization inside Web Worker — wasm-bindgen `--target web` init() must work in worker context with Vite bundling
- PathFinder convergence on multi-pad nets — 5/25 connections unrouted on Blink LED, root cause unknown (grid resolution? spanning tree? pad mapping?)

## Proof Strategy

- WASM-in-Worker → retire in S01 by shipping working Route button that routes via Worker with visible spinner and cancel
- PathFinder convergence → retire in S02 by proving 0 unrouted on Blink LED in `cargo test --release`

## Verification Classes

- Contract verification: cargo tests for PathFinder convergence, Vitest for worker message protocol
- Integration verification: Route button → Worker → WASM → snapshot back → canvas renders traces
- Operational verification: browser responsiveness during routing (Playwright interacts with UI during route)
- UAT / human verification: visual confirmation of routed Blink LED board

## Milestone Definition of Done

This milestone is complete only when all are true:

- Route button routes via Web Worker — browser never freezes
- Cancel button terminates routing immediately
- Blink LED routes 0 unrouted natively and via WASM Worker
- Variant panel shows ranked results via Worker, hover preview works
- E2E tests pass in CI catching both responsiveness and quality regressions
- Tuning sliders re-route via Worker

## Requirement Coverage

- Covers: R201, R202, R203, R204, R205, R206, R207
- Partially covers: none
- Leaves for later: R120 (renderer upgrade)
- Orphan risks: none

## Slices

- [x] **S01: Web Worker WASM Routing** `risk:high` `depends:[]`
  > After this: User clicks Route → spinner overlay visible, browser responsive, cancel button works, routed board appears when done. All proven in browser via Vite dev server.

- [x] **S02: Routing Quality — 0 Unrouted on Blink LED** `risk:medium` `depends:[]`
  > After this: `cargo test` proves PathFinder routes all 25 connections on Blink LED with 0 unrouted. WASM routing via Worker also produces 0 unrouted on Blink LED (verified in browser).

- [x] **S03: E2E Regression Tests** `risk:low` `depends:[S01,S02]`
  > After this: CI has Playwright tests that assert UI is responsive during routing and result has 0 unrouted. Pipeline green.

- [x] **S04: Variant Generation & Tuning via Worker** `risk:low` `depends:[S01]`
  > After this: Route button generates 3+ variants via Worker, score panel shows ranked results, hover preview renders alternatives. Tuning sliders re-route via Worker.

## Boundary Map

### S01 → S03

Produces:
- `viewer/src/routing-worker.ts` — Web Worker that loads WASM, accepts `{type:'route', source}` messages, posts `{type:'route-result', snapshot, routeResult}` back
- `triggerRouting()` refactored: shows overlay, posts to worker, handles result via `worker.onmessage`, calls `pullSnapshot()` with worker's snapshot
- Cancel: `worker.terminate()` + spawn fresh worker, UI reset to pre-route state
- `window.__routingWorker` debug surface: `{ active: boolean, lastResult: string | null }`
- Routing overlay/spinner visible for full duration (main thread free to paint)

Consumes:
- nothing (first slice)

### S01 → S04

Produces:
- Worker message protocol supporting `{type:'route-variants', source}` and `{type:'route-with-params', source, params}`
- Worker response protocol: `{type:'variant-result', variants}` and `{type:'route-result', snapshot, routeResult}`
- Worker lifecycle: `spawnWorker()`, `terminateWorker()`, ready detection via `{type:'ready'}` message from worker after WASM init

Consumes:
- nothing (first slice)

### S02 → S03

Produces:
- PathFinder fix — 0 unrouted on Blink LED in `cargo test -p cypcb-autoroute --release`
- New test: `test_blink_led_zero_unrouted` asserting `unrouted == 0`
- WASM binary rebuilt with the fix

Consumes:
- nothing (parallel to S01)

### S03

Produces:
- `viewer/e2e/autoroute-worker.spec.ts` — tests: overlay visible during routing, cancel works, 0 unrouted result
- CI catches: UI freeze regression (overlay not visible = fail), quality regression (unrouted > 0 = fail)

Consumes from S01:
- Worker-based routing (non-blocking triggerRouting)
- Debug surface `window.__routingWorker`

Consumes from S02:
- 0 unrouted guarantee (PathFinder fix in WASM binary)

### S04

Produces:
- Variant generation via Worker: `auto_route_variants()` in worker, results posted back
- Score panel populated from worker variant results
- Hover preview functional (ghost overlay)
- Tuning sliders send `route-with-params` to Worker

Consumes from S01:
- Worker message protocol, lifecycle, `triggerRouting()` pattern
