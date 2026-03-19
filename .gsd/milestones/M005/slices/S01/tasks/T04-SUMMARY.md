---
id: T04
parent: S01
milestone: M005
provides:
  - Worker-based onTuningSliderInput() — no synchronous engine.auto_route_with_params() call
  - triggerVariantRouting() function that posts route-variants to worker
  - Zero synchronous WASM routing calls remaining in main.ts
key_files:
  - viewer/src/main.ts
key_decisions:
  - Separate tuningWorker variable tracks tuning-specific workers independently from routingWorker
  - cancelRouting() terminates both routingWorker and tuningWorker
  - triggerVariantRouting() exposed on window.__triggerVariantRouting for S04 integration
patterns_established:
  - Tuning re-route uses same spawn→ready→post pattern as Route button but with independent tuningWorker reference
observability_surfaces:
  - window.__routingWorker.active — true during tuning re-routes AND Route-button routing
  - window.__triggerVariantRouting — callable function for variant generation
  - Console logs with [Tuning] and [Variants] prefixes
duration: 20m
verification_result: passed
completed_at: 2026-03-18
blocker_discovered: false
---

# T04: Refactor tuning sliders and variant generation to route via Worker

**Replaced synchronous engine.auto_route_with_params() in onTuningSliderInput() and added triggerVariantRouting() — zero synchronous WASM routing calls remain in main.ts**

## What Happened

Refactored `onTuningSliderInput()` debounce callback to spawn a Web Worker instead of calling `engine.auto_route_with_params()` synchronously. Added `triggerVariantRouting()` that posts `route-variants` to worker and calls `showVariants()` with results. Updated `__routingWorker.active` getter to cover both routing and tuning workers. Updated `cancelRouting()` to terminate both worker types.

## Verification

- `grep -n "engine\.auto_route" viewer/src/main.ts` → zero matches (exit 1)
- `npx tsc --noEmit` → clean
- `npx vitest run` → 127/127 pass
- Browser: worker spawns, error forwarded correctly, UI resets, `__routingWorker.active === false` after error, `typeof __triggerVariantRouting === "function"`

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `grep -n "engine\.auto_route" viewer/src/main.ts` | 1 | ✅ pass (zero matches) | <1s |
| 2 | `npx tsc --noEmit` | 0 | ✅ pass | 2.9s |
| 3 | `npx vitest run --reporter=verbose` | 0 | ✅ pass (127/127) | 3.1s |
| 4 | Browser: `typeof window.__triggerVariantRouting` | — | ✅ pass ("function") | — |
| 5 | Browser: `window.__routingWorker.active` after error | — | ✅ pass (false) | — |

## Diagnostics

- `window.__routingWorker.active` → live boolean for both routing and tuning
- `window.__triggerVariantRouting()` → callable from console
- Console: `[Tuning] Worker spawned/ready/result`, `[Variants] Worker spawned/result`
- Errors: `[Worker Error] Tuning:`, `[Worker Error] Variants:`

## Deviations

Added separate `tuningWorker` variable rather than reusing `routingWorker`.

## Known Issues

None — WASM 403 in sandbox is environment-specific, not a code issue.

## Files Created/Modified

- `viewer/src/main.ts` — refactored onTuningSliderInput(), added triggerVariantRouting(), added tuningWorker, updated debug surface and cancelRouting()
- `.gsd/milestones/M005/slices/S01/tasks/T04-PLAN.md` — added Observability Impact section
