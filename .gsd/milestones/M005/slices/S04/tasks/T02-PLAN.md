---
estimated_steps: 8
estimated_files: 2
---

# T02: Wire Route button to variant routing with snapshot application and click-to-apply

**Slice:** S04 — Variant Generation & Tuning via Worker
**Milestone:** M005

## Description

The Route button currently calls `triggerRouting()` (single-route via worker). This task switches it to `triggerVariantRouting()` (generates 3+ variants), updates the variant-result handler to apply the board snapshot to the canvas (so users see the routed board), and implements click-to-apply so users can re-route with a non-default variant's configuration.

This is the core behavioral change that makes variant generation user-visible.

**Relevant skills:** None specific — TypeScript wiring in main.ts.

## Steps

1. **Change Route button click handler** at approximately line 1832 of `viewer/src/main.ts`:
   ```typescript
   // Before:
   routeBtn.addEventListener('click', () => {
     triggerRouting();
   });
   // After:
   routeBtn.addEventListener('click', () => {
     triggerVariantRouting();
   });
   ```
   Keep `triggerRouting()` function intact — it's still used by editor-triggered routing (Ctrl+R, Tauri events at line ~2326).

2. **Import `transformVariantResults`** from `./variant-transform` at the top of main.ts:
   ```typescript
   import { transformVariantResults } from './variant-transform';
   ```

3. **Update `triggerVariantRouting()`'s `variant-result` handler** to apply the snapshot. In the `case 'variant-result':` block (around line 1681), after parsing variants, add snapshot application following the exact pattern from `triggerRouting()`'s `route-result` handler:
   ```typescript
   case 'variant-result': {
     const elapsed = Math.round((Date.now() - routingStartTime) / 1000);
     console.log('[Variants] Worker result received');

     // Apply the routed board snapshot to the canvas
     const workerSnapshot = msg.snapshot;
     snapshot = workerSnapshot;
     padNetMap = workerSnapshot.nets ? buildPadNetMap(workerSnapshot.nets) : new Map();
     dirty = true;

     // Transform Rust-serialized variants to TypeScript VariantData format
     try {
       const nets = workerSnapshot.nets || [];
       const parsed: VariantData[] = transformVariantResults(msg.variants, nets);
       storedVariants = parsed;
       showVariants(parsed, 0);
       statusText.textContent = `Generated ${parsed.length} variants in ${elapsed}s`;
       console.log(`[Variants] ${parsed.length} variants generated`);
     } catch (err) {
       console.warn('[Variants] Failed to parse variant result:', err);
       statusText.textContent = 'Variant generation failed';
     }

     // Set debug surface for E2E tests (critical — autoroute-worker.spec.ts test 3 checks this)
     (window as any).__routingWorker.lastResult = msg.variants;

     // Clean up...
   }
   ```

4. **Set `__routingWorker.lastResult`** in the variant-result handler. This is critical because `autoroute-worker.spec.ts` test 3 waits for `__routingWorker.lastResult !== null`. Currently only the `route-result` handler sets it. Since the Route button now triggers variants, `lastResult` must be set in the variant-result handler too. Use `msg.variants` (the raw JSON string) as the value.

5. **Implement click-to-apply** in the `initVariantPanel({onClick})` callback (around line 873 of main.ts). Currently the onClick just logs and sets dirty. Replace with a re-route via worker using the clicked variant's params:
   ```typescript
   onClick: (index) => {
     if (!storedVariants[index]) return;
     variantPreview = null;
     dirty = true;

     const variant = storedVariants[index];
     console.log(`[Variants] Re-routing with variant: ${variant.name}`);

     // Map variant name to AutorouteParams
     // Variant names from Rust: "PathFinder Default", "PathFinder Low-Via", "PathFinder High-Density"
     const paramsMap: Record<string, Record<string, number>> = {
       'PathFinder Default': { via_cost: 1.0, layer_preference: 0.5, roundness: 0.5, density: 1.0 },
       'PathFinder Low-Via': { via_cost: 5.0, layer_preference: 0.5, roundness: 0.5, density: 1.0 },
       'PathFinder High-Density': { via_cost: 1.0, layer_preference: 0.5, roundness: 0.5, density: 1.5 },
     };

     const params = paramsMap[variant.name] || { via_cost: 1.0, layer_preference: 0.5, roundness: 0.5, density: 1.0 };

     // Spawn a worker with route-with-params to apply this variant's config
     if (tuningWorker) {
       tuningWorker.terminate();
       tuningWorker = null;
     }

     const worker = spawnRoutingWorker();
     tuningWorker = worker;

     worker.onmessage = (event: MessageEvent<WorkerResponse>) => {
       const msg = event.data;
       if (msg.type === 'ready') {
         worker.postMessage({
           type: 'route-with-params',
           source: lastLoadedSource!,
           params: JSON.stringify(params),
         });
       } else if (msg.type === 'route-result') {
         snapshot = msg.snapshot;
         padNetMap = msg.snapshot.nets ? buildPadNetMap(msg.snapshot.nets) : new Map();
         dirty = true;
         tuningWorker = null;
         console.log(`[Variants] Applied variant: ${variant.name}`);
       } else if (msg.type === 'error') {
         console.error('[Variants] Apply failed:', msg.message);
         tuningWorker = null;
       }
     };
   },
   ```

6. **Ensure `triggerVariantRouting()` clears variant state** at the start (it already does via `hideVariants()` and clearing `storedVariants` in `triggerRouting()` — check if `triggerVariantRouting()` also clears these). Looking at the current code, `triggerVariantRouting()` doesn't call `hideVariants()` at the start. Add it:
   ```typescript
   function triggerVariantRouting(): void {
     if (isRouting) { ... }
     if (!snapshot?.board || !lastLoadedSource) { ... }

     // Clear previous variant state
     hideVariants();
     variantPreview = null;
     storedVariants = [];

     isRouting = true;
     // ...rest
   }
   ```

7. **Keep `triggerRouting()` as fallback** — it's still called by:
   - Editor change auto-route (line ~2326: `triggerRouting()` inside `runDrcDebounced`)
   - Tauri desktop events
   Don't remove it. Just change the Route button binding.

8. **Type-check**: Run `npx tsc --noEmit` to verify everything compiles.

## Must-Haves

- [ ] Route button calls `triggerVariantRouting()` instead of `triggerRouting()`
- [ ] Variant-result handler applies snapshot to canvas (snapshot assigned, padNetMap rebuilt, dirty set)
- [ ] `transformVariantResults()` used to convert raw variants before passing to `showVariants()`
- [ ] `__routingWorker.lastResult` set in the variant-result handler
- [ ] Click-to-apply spawns a `route-with-params` worker and applies the returned snapshot
- [ ] `triggerVariantRouting()` clears previous variant state at start
- [ ] `triggerRouting()` still exists and is called from editor/Tauri paths
- [ ] `npx tsc --noEmit` passes

## Verification

- `npx tsc --noEmit` — zero TypeScript errors
- `npx vitest run` — all unit tests pass
- `npx vite build` — builds successfully
- Manual (if WASM available): Load board → Route → canvas shows routed board → score panel visible → click variant → re-routes
- `window.__routingWorker.lastResult` is non-null after Route click completes

## Observability Impact

- Signals added/changed: `[Variants] Re-routing with variant: <name>` on click-to-apply, `[Variants] Applied variant: <name>` after click-apply completes
- How a future agent inspects this: `window.__routingWorker.lastResult` — now set by both single-route and variant flows
- Failure state exposed: `[Variants] Apply failed:` on click-to-apply worker error

## Inputs

- `viewer/src/variant-transform.ts` — T01's `transformVariantResults()` function
- `viewer/src/worker-protocol.ts` — T01's updated `VariantResultResponse` with `snapshot` field
- `viewer/src/main.ts` — existing `triggerRouting()`, `triggerVariantRouting()`, Route button handler, onClick callback
- The `VariantData` format from T01's transformation (routes with `segments: [{start: [x,y], end: [x,y]}]`, vias with flat `x`/`y`)
- Default variant configs from Rust: "PathFinder Default" (via_cost=1.0, density=1.0), "PathFinder Low-Via" (via_cost=5.0), "PathFinder High-Density" (density=1.5) — see `crates/cypcb-autoroute/src/variant.rs` `default_variant_configs()`

## Expected Output

- `viewer/src/main.ts` — Route button calls `triggerVariantRouting()`. Variant-result handler applies snapshot + transforms data + sets lastResult. onClick callback re-routes via worker with variant params. `triggerVariantRouting()` clears state at start. `triggerRouting()` preserved for editor/Tauri paths.
