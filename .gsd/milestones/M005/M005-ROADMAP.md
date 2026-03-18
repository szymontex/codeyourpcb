# M005: WASM Routing — Off Main Thread

**Vision:** Move WASM autorouter to a Web Worker so the browser never freezes. Fix routing quality so Blink LED has 0 unrouted. Add E2E tests that catch real regressions.

## Success Criteria

- Route button never freezes the browser — main thread stays responsive during routing
- Spinner/progress overlay visible while routing executes in background
- Cancel button terminates routing mid-execution
- Blink LED template routes 25/25 connections (0 unrouted, 0 ratsnest remaining)
- Variant generation works via Web Worker with score panel and hover preview
- E2E tests verify UI responsiveness and routing quality in CI

## Key Risks / Unknowns

- WASM initialization inside Web Worker — wasm-bindgen init() pattern must work in worker context
- PathFinder convergence failure on Blink LED — 5 nets unrouted, root cause unknown
- Cancel mechanism — WASM is synchronous, only option is worker.terminate()

## Proof Strategy

- WASM-in-Worker init → retire in S01 by proving WASM loads and routes inside Worker
- PathFinder convergence → retire in S02 by proving 0 unrouted on Blink LED natively and in WASM
- E2E responsiveness → retire in S03 by proving Playwright can interact with UI during routing

## Verification Classes

- Contract verification: unit tests for worker message protocol, PathFinder convergence tests
- Integration verification: WASM routing via Worker produces correct result displayed in UI
- Operational verification: browser responsiveness during routing (no page unresponsive dialogs)
- UAT / human verification: visual confirmation of routed board quality

## Milestone Definition of Done

This milestone is complete only when all are true:

- Web Worker routing integrated — Route button, tuning sliders, and variant generation all route via Worker
- Blink LED routes fully (0 unrouted) both natively and via WASM
- E2E tests pass in CI verifying responsiveness and quality
- Cancel button works (terminates worker, resets UI)
- Variant panel functional with score ranking and hover preview via Worker

## Requirement Coverage

- Covers: R201, R202, R203, R204, R205, R206, R207
- Partially covers: none
- Leaves for later: R120 (renderer upgrade)
- Orphan risks: none

## Slices

- [ ] **S01: Web Worker WASM Routing** `risk:high` `depends:[]`
  > After this: Route button sends routing to background worker, browser stays responsive, spinner visible, result appears without freeze. Cancel terminates worker.

- [ ] **S02: Routing Quality — 0 Unrouted on Blink LED** `risk:medium` `depends:[]`
  > After this: Blink LED template routes 25/25 connections natively (cargo test proves 0 unrouted). PathFinder convergence fixed.

- [ ] **S03: E2E Regression Tests** `risk:low` `depends:[S01,S02]`
  > After this: CI pipeline has E2E tests that catch UI freeze regressions and routing quality regressions. Green pipeline.

- [ ] **S04: Variant Generation via Worker** `risk:low` `depends:[S01]`
  > After this: Route button generates 3+ variants via Worker, score panel shows ranked results, hover preview renders alternatives.

## Boundary Map

### S01 → S03

Produces:
- `viewer/src/routing-worker.ts` — Web Worker script that loads WASM, accepts route/cancel messages, posts results
- `triggerRouting()` refactored to postMessage pattern — returns immediately, result via onmessage callback
- Cancel mechanism via worker.terminate() + fresh worker spawn
- Routing overlay visible during Worker execution (setTimeout yield not needed — Worker is non-blocking)
- `window.__routingWorker` debug surface for E2E: `{ active: bool, lastResult: string }`

Consumes:
- nothing (first slice)

### S01 → S04

Produces:
- Worker message protocol: `{ type: 'route' | 'route-variants' | 'route-with-params', source: string, params?: string }`
- Worker response protocol: `{ type: 'result' | 'variant-result' | 'error', data: string }`
- Worker lifecycle: spawn, ready detection, terminate, respawn

Consumes:
- nothing (first slice)

### S02 → S03

Produces:
- PathFinder convergence fix — 0 unrouted on Blink LED in `cargo test`
- Regression test: `test_blink_zero_unrouted` in cypcb-autoroute

Consumes:
- nothing (parallel slice)

### S03

Produces:
- `viewer/e2e/autoroute-worker.spec.ts` — E2E test: load board, click Route, verify UI responsive + result quality
- Tests assert: overlay visible during routing, cancel works, 0 unrouted on simple board

Consumes from S01:
- Worker-based routing flow (triggerRouting via postMessage)
- Debug surface `window.__routingWorker`

Consumes from S02:
- 0 unrouted guarantee on Blink LED

### S04

Produces:
- Variant generation via Worker (`auto_route_variants` message type)
- Score panel populated from Worker results
- Hover preview functional
- Tuning sliders route via Worker

Consumes from S01:
- Worker message protocol and lifecycle
- `triggerRouting()` postMessage pattern
