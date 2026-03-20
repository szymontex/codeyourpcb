---
estimated_steps: 7
estimated_files: 4
---

# T01: Add snapshot to variant-result protocol and build data transformation

**Slice:** S04 — Variant Generation & Tuning via Worker
**Milestone:** M005

## Description

The worker's `route-variants` handler currently returns only a `variants` JSON string — no board snapshot. Without the snapshot, the main thread can't update the canvas after variant generation. Additionally, the Rust-serialized `VariantResult` format uses `net_id: number`, `Point {x, y}` objects, and nested `position` fields, but the TypeScript `VariantData` interface expects `net_name: string`, `[x, y]` tuples, and flat `x`/`y` via coordinates. This task fixes the protocol and builds the data transformation layer.

**Relevant skills:** None specific — pure TypeScript wiring.

## Steps

1. **Add `snapshot` field to `VariantResultResponse`** in `viewer/src/worker-protocol.ts`:
   ```typescript
   export interface VariantResultResponse {
     type: 'variant-result';
     variants: string;
     snapshot: BoardSnapshot;  // ← add this
   }
   ```
   The `BoardSnapshot` type is already imported in this file.

2. **Update the `route-variants` handler** in `viewer/src/routing-worker.ts`:
   - After `const variants = engine.auto_route_variants();`, call `const snapshot = engine.get_snapshot();`
   - Move `engine.free()` AFTER both calls
   - Update the response: `{ type: 'variant-result', variants, snapshot }`
   - The `get_snapshot()` call returns the board state after the best variant was auto-applied by Rust's `generate_variants()`, so the snapshot reflects the best-routed board.

3. **Create `viewer/src/variant-transform.ts`** with the transformation function. Define the Rust-side raw types:
   ```typescript
   /** Raw Rust-serialized variant result (from serde JSON) */
   interface RawVariantResult {
     name: string;
     score: {
       total_length: number;  // Nm as i64
       via_count: number;     // u32
       drc_violations: number; // u32
       smoothness: number;    // f64
       crossings: number;     // u32
       layer_balance: number; // f64
       composite: number;     // f64
     };
     routes: Array<{
       net_id: number;         // NetId(u32) serialized as number
       layer: string;          // Layer enum: "TopCopper" | "BottomCopper"
       width: number;          // Nm as i64
       start: { x: number; y: number };  // Point { x: Nm, y: Nm }
       end: { x: number; y: number };
     }>;
     vias: Array<{
       net_id: number;
       position: { x: number; y: number };
       drill: number;          // Nm as i64
       start_layer: string;
       end_layer: string;
     }>;
   }
   ```

4. **Implement `transformVariantResults()`** that takes `(rawJson: string, nets: NetInfo[])` and returns `VariantData[]`:
   - Parse JSON string into `RawVariantResult[]`
   - Build a `Map<number, string>` from `nets` (net.id → net.name) for net_id → net_name lookup
   - For each raw variant, transform:
     - `routes`: Group by `net_id` (same net_id routes get merged into one entry with multiple segments). Each route entry gets `net_name` from the map (fallback: `"net_${net_id}"`), `layer` passes through, `width` passes through, `segments` mapped from `{start: {x,y}, end: {x,y}}` to `{start: [x, y], end: [x, y]}`
     - `vias`: Map `position.x` → `x`, `position.y` → `y`, `drill` passes through, `net_id` → `net_name`
     - `score`: passes through directly (field names and types already match)
     - `name`: passes through
   - Return the array sorted by composite score (ascending — lower is better)

5. **Consider the route grouping carefully**: The Rust side emits one `RouteSegment` per trace segment (each with its own net_id, layer, width, start, end). The TypeScript `VariantData.routes` expects entries grouped by net, where each entry has `segments[]`. Group consecutive segments with the same `net_id` and `layer` into one route entry with multiple segments. If grouping is complex, a simpler approach: create one route entry per segment (each with one segment in `segments[]`) — the renderer's `drawVariantPreview()` iterates `route.segments` anyway so single-segment entries work fine.

6. **Add a unit test** in `viewer/src/__tests__/variant-transform.test.ts`:
   - Test `transformVariantResults()` with a mock raw JSON string and mock NetInfo array
   - Assert the output has correct `net_name` (not `net_id`), `[x,y]` tuples (not `{x,y}` objects), flat via `x`/`y` (not nested `position`)
   - Test fallback net_name when net_id not found in nets

7. **Type-check everything**: Run `npx tsc --noEmit` to ensure all protocol changes compile cleanly.

## Must-Haves

- [ ] `VariantResultResponse` in `worker-protocol.ts` has a `snapshot: BoardSnapshot` field
- [ ] Worker's `route-variants` handler calls `engine.get_snapshot()` and includes snapshot in response
- [ ] `transformVariantResults()` function exists and correctly maps Rust format → TypeScript `VariantData[]`
- [ ] Net ID → net name resolution works (with fallback for missing nets)
- [ ] Point `{x, y}` → `[x, y]` tuple conversion works
- [ ] Via `position.x/y` → flat `x`/`y` conversion works
- [ ] Unit test verifies the transformation
- [ ] `npx tsc --noEmit` passes with zero errors

## Verification

- `npx tsc --noEmit` — zero TypeScript errors
- `npx vitest run` — all tests pass including the new transform test
- `npx vite build` — worker bundles correctly with protocol change
- The unit test in `variant-transform.test.ts` covers format conversion and edge cases

## Observability Impact

- **Protocol change:** `VariantResultResponse` now carries `snapshot: BoardSnapshot` — any handler consuming `variant-result` messages gains access to the post-routing board state for canvas updates.
- **Transform diagnostics:** `transformVariantResults()` should log `[Variants] Transformed ${count} variants` on success and `[Variants] Failed to parse variant result: <error>` on JSON parse or transform failure, making data-flow issues visible in the console.
- **Inspection:** After this task, `engine.get_snapshot()` is called in the `route-variants` worker handler — the snapshot in the posted message can be inspected via message logging or the `__routingWorker.lastResult` debug surface (wired in T02).
- **Failure state:** If `transformVariantResults()` receives malformed JSON or unknown net IDs, it falls back to `net_${net_id}` names rather than crashing — the fallback names are visible in the variant panel UI and test assertions.

## Inputs

- `viewer/src/worker-protocol.ts` — current `VariantResultResponse` without `snapshot` field
- `viewer/src/routing-worker.ts` — current `route-variants` handler without snapshot
- `viewer/src/variant-panel.ts` — `VariantData` interface (the target shape)
- `viewer/src/types.ts` — `BoardSnapshot` type (has `nets: NetInfo[]`), `NetInfo` type (has `id: number`, `name: string`)
- Rust serialization: `VariantResult` has `routes: Vec<RouteSegment>` where `RouteSegment` has `net_id: NetId(u32)`, `start: Point { x: Nm(i64), y: Nm(i64) }`, `end: Point`, `layer: Layer`, `width: Nm(i64)`; `vias: Vec<ViaPlacement>` where `ViaPlacement` has `net_id: NetId(u32)`, `position: Point`, `drill: Nm(i64)`, `start_layer: Layer`, `end_layer: Layer`

## Expected Output

- `viewer/src/worker-protocol.ts` — `VariantResultResponse` with `snapshot: BoardSnapshot` field added
- `viewer/src/routing-worker.ts` — `route-variants` case calls `engine.get_snapshot()` before `engine.free()`, includes snapshot in response
- `viewer/src/variant-transform.ts` — **new file** — exports `transformVariantResults(rawJson: string, nets: NetInfo[]): VariantData[]` and the `RawVariantResult` interface
- `viewer/src/__tests__/variant-transform.test.ts` — **new file** — unit test for transformation logic
