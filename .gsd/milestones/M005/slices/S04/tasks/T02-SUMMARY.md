---
id: T02
parent: S04
milestone: M005
provides:
  - Route button triggers variant generation flow (triggerVariantRouting)
  - Variant-result handler applies board snapshot to canvas
  - transformVariantResults() integration for Rust→TS data conversion
  - Click-to-apply re-routes via worker with selected variant params
  - __routingWorker.lastResult set in variant-result handler for E2E tests
key_files:
  - viewer/src/main.ts
key_decisions:
  - Variant name → AutorouteParams mapping hardcoded in onClick callback (matches Rust default_variant_configs)
patterns_established:
  - Click-to-apply spawns a new routing worker with route-with-params message type
  - Variant-result handler mirrors route-result pattern for snapshot application
observability_surfaces:
  - "[Variants] Re-routing with variant: <name>" on click-to-apply
  - "[Variants] Applied variant: <name>" after click-apply completes
  - "[Variants] Apply failed: <message>" on click-to-apply worker error
  - window.__routingWorker.lastResult set by both single-route and variant flows
duration: 8m
verification_result: passed
completed_at: 2026-03-19T19:41:00Z
blocker_discovered: false
---

# T02: Wire Route button to variant routing with snapshot application and click-to-apply

**Rewired Route button to generate variants via worker, apply board snapshot to canvas, transform variant data, and re-route on click-to-apply with per-variant params.**

## What Happened

Six changes in `viewer/src/main.ts`:

1. **Route button rewired** — Changed `routeBtn.addEventListener('click', () => triggerRouting())` to call `triggerVariantRouting()`. The old `triggerRouting()` remains intact for editor auto-route (Ctrl+R / Tauri events at line ~2379).

2. **Import added** — Added `import { transformVariantResults } from './variant-transform'` to wire T01's data transformation layer.

3. **Variant-result handler updated** — The `variant-result` case now: (a) applies the board snapshot to the canvas (`snapshot = msg.snapshot`, rebuilds `padNetMap`, sets `dirty = true`), following the exact pattern from `triggerRouting()`'s `route-result` handler; (b) uses `transformVariantResults(msg.variants, nets)` instead of raw `JSON.parse()` to convert Rust-serialized data to `VariantData[]`; (c) sets `__routingWorker.lastResult = msg.variants` so E2E tests that check this debug surface work with the variant flow.

4. **Click-to-apply implemented** — Replaced the stub `onClick` callback in `initVariantPanel()` with a full implementation that: maps variant names ("PathFinder Default", "PathFinder Low-Via", "PathFinder High-Density") to their Rust `AutorouteParams`, spawns a new routing worker via `spawnRoutingWorker()`, posts `route-with-params` with the selected config, and applies the returned snapshot to the canvas.

5. **Variant state clearing** — Added `hideVariants()`, `variantPreview = null`, and `storedVariants = []` at the start of `triggerVariantRouting()` to clear stale state from previous runs.

6. **triggerRouting() preserved** — Kept as-is for editor-triggered auto-route paths (line ~2379).

## Verification

- `npx tsc --noEmit` — zero TypeScript errors
- `npx vitest run` — 138 tests pass (12 files), no regressions
- `npx vite build` — builds successfully in 24.5s
- Code inspection confirms all 8 must-haves met

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `npx tsc --noEmit` | 0 | ✅ pass | 1.9s |
| 2 | `npx vitest run` | 0 | ✅ pass | 25.4s |
| 3 | `npx vite build` | 0 | ✅ pass | 24.5s |
| 4 | `npx playwright test e2e/variant-panel.spec.ts` | — | ⏭️ deferred to T04 | — |
| 5 | `npx playwright test e2e/autoroute-worker.spec.ts` | — | ⏭️ deferred to T04 | — |

## Diagnostics

- **Click-to-apply flow:** Console shows `[Variants] Re-routing with variant: <name>` when a user clicks a variant, followed by `[Variants] Applied variant: <name>` when the worker returns the re-routed snapshot.
- **Click-to-apply failure:** Console shows `[Variants] Apply failed: <message>` if the route-with-params worker errors.
- **Debug surface:** `window.__routingWorker.lastResult` is now set by both `triggerRouting()` (route-result) and `triggerVariantRouting()` (variant-result), ensuring E2E test compatibility.
- **Snapshot application:** After variant-result, the canvas updates because `snapshot`, `padNetMap`, and `dirty` are all set — same pattern as single-route flow.

## Deviations

None — implementation follows the task plan exactly.

## Known Issues

None.

## Files Created/Modified

- `viewer/src/main.ts` — Rewired Route button to `triggerVariantRouting()`, updated variant-result handler with snapshot application + transformVariantResults + lastResult, implemented click-to-apply with per-variant worker routing, added variant state clearing at start of `triggerVariantRouting()`, added `transformVariantResults` import
