# S04 — Variant Generation & Tuning via Worker — Research

**Date:** 2026-03-19
**Depth:** Light research — straightforward wiring of existing APIs and UI components already in the codebase.

## Summary

S04 is integration wiring, not new architecture. The S01 slice already built 90% of the infrastructure:

- `routing-worker.ts` handles `route-variants` and `route-with-params` messages with full WASM lifecycle.
- `triggerVariantRouting()` exists in `main.ts` and is exposed on `window.__triggerVariantRouting`.
- `variant-panel.ts` has `showVariants()`, `hideVariants()`, hover callbacks, click callbacks, and a `__variantPanel` debug surface.
- The renderer already draws variant ghost previews (`drawVariantPreview()` in `renderer.ts`) with cyan overlay + trace dimming.
- Tuning sliders already re-route via Worker (`onTuningSliderInput()` spawns a `tuningWorker`).
- The `VariantData` interface in `variant-panel.ts` matches the serialized `VariantResult` from Rust.

**What's actually missing** is narrow:

1. **Route button calls `triggerRouting()` (single route) instead of `triggerVariantRouting()`.** The Route button click handler at line ~1832 needs to call `triggerVariantRouting()` instead.

2. **`triggerVariantRouting()` doesn't update the main-thread snapshot after variant generation.** The worker calls `auto_route_variants()` (which applies the best variant to its internal engine) but only returns the variant JSON — no snapshot is posted back. The main thread canvas never shows the routed board after variant generation. Fix: the worker's `route-variants` handler must call `engine.get_snapshot()` and include it in the `VariantResultResponse`. The protocol type needs a `snapshot` field added.

3. **Click-to-apply is display-only** — the `onClick` callback in `initVariantPanel({onClick})` just logs and sets `dirty`. It should re-route via worker with the clicked variant's params to actually apply a different variant's routes to the board. Alternatively, since all variant route/via data is already stored in `storedVariants`, the renderer can draw the clicked variant's data directly without re-routing.

4. **Score panel shows minimal metrics** — `showVariants()` in `variant-panel.ts` shows `name`, `composite`, and a terse `{via_count}v · {routes.length}r` string. Per the roadmap's "score panel shows ranked results," a more descriptive breakdown (DRC violations, smoothness, total length) is expected.

## Recommendation

Wire the existing pieces together in 4 focused tasks:

1. **Fix the worker protocol + worker handler** — Add `snapshot` field to `VariantResultResponse` in `worker-protocol.ts`. Update the `route-variants` handler in `routing-worker.ts` to call `engine.get_snapshot()` before `engine.free()` and include it in the response.

2. **Wire Route button → variant routing** — Change `routeBtn.addEventListener('click', () => triggerRouting())` to call `triggerVariantRouting()`. Update `triggerVariantRouting()` to apply the snapshot from the worker response (same pattern as `triggerRouting()`'s `route-result` handler).

3. **Improve score panel + click-to-apply** — Enhance `showVariants()` to display a richer metric breakdown. Implement click-to-apply by re-routing via worker with the selected variant's params (sends `route-with-params` with the clicked variant's params).

4. **E2E test updates** — Update `variant-panel.spec.ts` tests so they work with the new flow (Route button now generates variants). Add assertions for snapshot update after variant generation.

## Implementation Landscape

### Key Files

- `viewer/src/worker-protocol.ts` — Add `snapshot: BoardSnapshot` field to `VariantResultResponse`.
- `viewer/src/routing-worker.ts` — `route-variants` case: call `engine.get_snapshot()` and include snapshot in response.
- `viewer/src/main.ts` — (a) Route button → `triggerVariantRouting()`, (b) `triggerVariantRouting()` handler applies snapshot from worker, (c) `onClick` callback re-routes with selected variant's config.
- `viewer/src/variant-panel.ts` — Enhance `showVariants()` metric display. Possibly extend `VariantPanelCallbacks` or add data to `VariantData`.
- `viewer/e2e/variant-panel.spec.ts` — Tests already structured for this flow; should pass once the Route button calls variant routing.
- `viewer/e2e/autoroute-worker.spec.ts` — Test 3 asserts `lastResult` from `triggerRouting()` — may need updating since Route button now calls `triggerVariantRouting()`.

### Build Order

1. **Protocol + worker fix first** — Add `snapshot` to `VariantResultResponse` and return it from worker. This unblocks everything else since without the snapshot, variant routing can't update the canvas.
2. **Main thread wiring** — Route button → variant routing, snapshot application, click-to-apply. This is the core behavior change.
3. **Score panel enhancement** — Cosmetic but user-visible. Depends on working variant flow.
4. **E2E test updates** — Depends on all behavior being correct.

### Verification Approach

- `npx tsc --noEmit` — zero type errors (protocol changes compile-checked)
- `npx vitest run` — existing unit tests pass (no regressions)
- `npx vite build` — worker bundles correctly with protocol change
- Manual: load Blink LED template → Route → score panel shows 3 variants → hover previews cyan ghost → click applies different variant → canvas updates
- `npx playwright test e2e/variant-panel.spec.ts` — all variant panel tests pass
- `npx playwright test e2e/autoroute-worker.spec.ts` — worker tests still pass
- Verify: `window.__variantPanel.variantCount >= 3` after Route click
- Verify: `window.__routingWorker.lastResult` is set after variant routing completes (need to ensure debug surface is updated in the variant flow)

## Constraints

- **`BoardWorld` is not Clone** — variant generation runs sequentially in the worker. Best variant is auto-applied by Rust's `generate_variants()`, so the snapshot after variant gen reflects the best variant's routed board.
- **Worker creates a fresh engine per request** — click-to-apply a different variant requires a new worker spawn + `route-with-params` (can't just tell the worker "apply variant 2" since the engine is freed after each request).
- **Variant data includes raw route segments in Nm coordinates** — the renderer's `drawVariantPreview()` expects `{ start: [number, number]; end: [number, number] }` format. Check that `VariantResult` serialization matches this shape (the Rust `RouteSegment` struct uses `Point` with `x`/`y` fields, not tuple arrays — there may be a format mismatch to handle in the TypeScript parsing layer).

## Common Pitfalls

- **Snapshot not applied after variant-result** — The critical bug. Without updating `snapshot` and `padNetMap` on the main thread, the canvas shows the pre-route board. Must follow the exact pattern from `triggerRouting()`'s `route-result` handler: `snapshot = msg.snapshot; padNetMap = buildPadNetMap(...)`.
- **`autoroute-worker.spec.ts` test 3 regression** — Currently checks `__routingWorker.lastResult` which is set in the `route-result` handler. If Route button now triggers variants, `lastResult` won't be set (variant-result handler doesn't set it). Either add `lastResult` setting in the variant-result handler or adjust the test.
- **Variant route/via format mismatch between Rust serialization and TypeScript `VariantData` interface** — This is the biggest integration risk. The Rust `VariantResult` serializes with serde as:
  - `routes[].start` / `routes[].end` → `{"x": <i64>, "y": <i64>}` (Point struct with Nm fields)
  - `routes[].net_id` → `<u32>` (NetId newtype)
  - `routes[].layer` → `"TopCopper"` / `"BottomCopper"` (Layer enum)
  - `routes[].width` → `<i64>` (Nm newtype)
  - `vias[].position` → `{"x": <i64>, "y": <i64>}` (nested Point)
  - `vias[].net_id` → `<u32>` (NetId newtype)
  - `vias[].drill` → `<i64>` (Nm field, serialized as DrillDiameter → Nm)

  But `VariantData` in `variant-panel.ts` expects:
  - `routes[].net_name` → `string` (not `net_id: number`)
  - `routes[].segments[].start` → `[number, number]` tuple (not `{x, y}` object)
  - `vias[].x` / `vias[].y` → flat numbers (not `position: {x, y}`)
  - `vias[].net_name` → `string` (not `net_id: number`)

  The fix is a transformation function in `triggerVariantRouting()` that maps Rust-serialized `VariantResult[]` to `VariantData[]`. The coordinate values are in Nm (nanometers) but the renderer's `drawVariantPreview()` expects mm-scale world coordinates, since `worldToScreen()` multiplies by `vp.scale`. Check existing trace rendering to confirm the coordinate convention — snapshot traces use Nm values, and `worldToScreen` handles Nm-to-pixel conversion, so Nm values should be passed through directly.

  Alternatively, update the `VariantData` interface to match the Rust serialization and update `drawVariantPreview()` to handle `{x, y}` objects instead of tuples. This approach avoids runtime transformation overhead.
