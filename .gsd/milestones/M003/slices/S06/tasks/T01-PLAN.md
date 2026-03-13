---
estimated_steps: 6
estimated_files: 5
---

# T01: JLCPCB search client, EasyEDA 3D pipeline, and OBJ parser

**Slice:** S06 — JLCPCB Integration & 3D Models
**Milestone:** M003

## Description

Build the three core logic modules that power the JLCPCB integration: (1) a jlcsearch API client that queries `jlcsearch.tscircuit.com/api/search` and parses component results, (2) an EasyEDA API client that takes an LCSC part number, fetches the component footprint data, extracts the 3D model UUID, fetches the OBJ geometry, and returns parsed Three.js-ready data, and (3) a custom OBJ parser that handles EasyEDA's non-standard OBJ format with inline material blocks and double-slash face indices.

Also extends `renderer3d.ts` with a `loadComponentFromOBJ(objText, refdes)` method that parses the OBJ text and replaces the placeholder box mesh, following the same pattern as the existing `loadComponentModel(url, refdes)`.

All logic is unit-testable without a browser — API response parsing and OBJ parsing are pure functions.

## Steps

1. Create `viewer/src/easyeda-obj-parser.ts` — parse EasyEDA OBJ text into arrays of `{ positions: Float32Array, normals: Float32Array, materialColor: {r,g,b}, opacity: number }` per material group. Handle: `v` lines → vertex array, `newmtl`/`endmtl` blocks → material properties (Ka/Kd/Ks/d), `usemtl` → material switch, `f v// v// v//` → triangle faces with computed normals. Treat `d 0.0` as opaque (EasyEDA convention). Export a `parseEasyEdaOBJ(text: string)` function returning geometry groups.

2. Create `viewer/src/jlcpcb.ts` — two async functions: (a) `searchComponents(query: string, limit?: number): Promise<JLCPCBComponent[]>` that fetches from `jlcsearch.tscircuit.com/api/search?q=${query}&limit=${limit}&full=true`, parses the response, and returns typed results with `lcsc` (number), `mfr`, `package`, `stock`, `price`, `manufacturer`, `attributes`, `datasheetUrl`. Parse `extra` field from JSON string. (b) `fetch3DModel(lcscId: number): Promise<string | null>` that calls `easyeda.com/api/products/C${lcscId}/components?version=6.4.19.5`, extracts 3D UUID from shape array's `outline3D` entry, fetches OBJ from `modules.easyeda.com/3dmodel/${uuid}`, returns OBJ text or null if no 3D model available. Both functions handle fetch errors gracefully (return empty array / null, log errors).

3. Add `loadComponentFromOBJ(objText: string, refdes: string)` to `renderer3d.ts` — calls `parseEasyEdaOBJ(objText)`, builds `THREE.BufferGeometry` with position + normal attributes per group, applies `MeshStandardMaterial` with parsed color, creates a `THREE.Group` containing all meshes, replaces placeholder box using the same name-convention (`component-${refdes}`) and transform-copy pattern as `loadComponentModel`. Track in `loadedModels` Map for disposal. Add `objModelCount` getter to debug surface.

4. Write `viewer/src/__tests__/easyeda-obj-parser.test.ts` — test cases: basic cube (8 vertices, 12 faces), material parsing (Ka/Kd/Ks/d), double-slash face format, `d 0.0` treated as opaque, multiple material groups, empty/malformed input returns empty array, comment lines ignored.

5. Write `viewer/src/__tests__/jlcpcb.test.ts` — test cases: parse valid search response (mock the JSON shape), handle empty `components` array, parse `extra` JSON string for manufacturer/attributes/datasheet, extract 3D UUID from EasyEDA shape array, handle missing `outline3D` (returns null). Use mock data matching the API response shapes from research.

6. Run `npx vitest run` and `npx tsc --noEmit` — verify all tests pass and no type errors.

## Must-Haves

- [ ] `parseEasyEdaOBJ(text)` correctly parses EasyEDA OBJ sample into geometry groups with positions, normals, and materials
- [ ] `searchComponents(query)` returns typed `JLCPCBComponent[]` from jlcsearch API response
- [ ] `fetch3DModel(lcscId)` chains EasyEDA component API → UUID extraction → OBJ fetch
- [ ] LCSC ID formatted as `C${id}` when calling EasyEDA API (bare number → prefixed)
- [ ] `loadComponentFromOBJ(objText, refdes)` replaces placeholder box with parsed OBJ geometry in 3D scene
- [ ] OBJ parser computes face normals from vertex cross-products (EasyEDA OBJ has no normals)
- [ ] `d 0.0` treated as fully opaque (EasyEDA convention, not standard OBJ)
- [ ] Unit tests pass for parser edge cases and API response parsing

## Verification

- `npx vitest run` — all unit tests pass including new `easyeda-obj-parser.test.ts` and `jlcpcb.test.ts`
- `npx tsc --noEmit` — zero type errors
- OBJ parser test with real EasyEDA sample data produces non-empty geometry (positions.length > 0)

## Observability Impact

- Signals added: `[3D] OBJ loaded for ${refdes}` console log on successful OBJ model load; `[3D] OBJ parse failed: ${error}` on parse failure; `[JLCPCB] Search error: ${error}` and `[JLCPCB] 3D fetch error: ${error}` on API failures
- How a future agent inspects this: `window.__renderer3d.objModelCount` for loaded OBJ model count; function return values are typed for programmatic inspection
- Failure state exposed: API functions return null/empty on failure (never throw), console errors logged with context

## Inputs

- `viewer/src/renderer3d.ts` — existing `loadComponentModel()` pattern, `boardGroup`, `loadedModels` Map, `clearBoardGroup()` disposal, `component-${refdes}` naming convention (S02)
- `viewer/src/types.ts` — `ComponentInfo.model_3d` field (currently always null)
- S06 Research — API response shapes, OBJ format details, LCSC ID formatting, `d 0.0` convention

## Expected Output

- `viewer/src/easyeda-obj-parser.ts` — pure function OBJ parser (~100-150 lines)
- `viewer/src/jlcpcb.ts` — API client module with `searchComponents()`, `fetch3DModel()`, typed interfaces
- `viewer/src/renderer3d.ts` — extended with `loadComponentFromOBJ()` and `objModelCount` on debug surface
- `viewer/src/__tests__/easyeda-obj-parser.test.ts` — 6-8 unit tests
- `viewer/src/__tests__/jlcpcb.test.ts` — 5-7 unit tests
