---
id: T01
parent: S06
milestone: M003
provides:
  - EasyEDA OBJ parser (parseEasyEdaOBJ) for non-standard format with inline materials
  - JLCPCB search client (searchComponents) via tscircuit jlcsearch API
  - EasyEDA 3D model pipeline (fetch3DModel) — LCSC ID → UUID → OBJ text
  - loadComponentFromOBJ method on Renderer3D for OBJ → Three.js geometry
key_files:
  - viewer/src/easyeda-obj-parser.ts
  - viewer/src/jlcpcb.ts
  - viewer/src/renderer3d.ts
  - viewer/src/__tests__/easyeda-obj-parser.test.ts
  - viewer/src/__tests__/jlcpcb.test.ts
key_decisions:
  - OBJ parser treats d 0.0 as fully opaque (EasyEDA convention, inverted from standard OBJ)
  - Fan triangulation for OBJ polygons >3 vertices
  - parseSearchResult and extract3DModelUUID exported separately for unit testing without mocking fetch
  - loadComponentFromOBJ disposes previous OBJ model for same refdes on reload (handles repeated searches)
patterns_established:
  - Pure function parsing modules (easyeda-obj-parser.ts, jlcpcb.ts parseSearchResult/extract3DModelUUID) tested without browser or fetch mocks
  - API client functions return null/empty on error — never throw
  - component-${refdes} naming convention extended to OBJ models (same as GLB pipeline)
observability_surfaces:
  - "[3D] OBJ loaded for ${refdes}" console log on successful OBJ model load
  - "[3D] OBJ parse failed: ${error}" on parse failure
  - "[JLCPCB] Search error: ${error}" and "[JLCPCB] 3D fetch error: ${error}" on API failures
  - window.__renderer3d.objModelCount for loaded OBJ model count
duration: 20m
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T01: JLCPCB search client, EasyEDA 3D pipeline, and OBJ parser

**Built OBJ parser for EasyEDA's non-standard format, JLCPCB search + 3D model API clients, and extended Renderer3D with loadComponentFromOBJ — all unit-tested.**

## What Happened

Created three modules:

1. **easyeda-obj-parser.ts** (~180 lines) — Two-pass parser: first pass collects vertices and materials (inline `newmtl`/`endmtl` blocks with Ka/Kd/Ks/d), second pass collects faces per `usemtl` group. Computes face normals via cross-product. Handles `f v// v// v//` double-slash format. Treats `d 0.0` as opaque. Returns `OBJGeometryGroup[]` with Float32Array positions/normals and material colors.

2. **jlcpcb.ts** (~160 lines) — Two async functions: `searchComponents(query, limit)` hits `jlcsearch.tscircuit.com/api/search` with `full=true`, parses the `extra` JSON string for manufacturer/attributes/datasheet. `fetch3DModel(lcscId)` chains EasyEDA component API (`C${lcscId}`) → UUID extraction from shape array `outline3D` → OBJ fetch from `modules.easyeda.com`. Both return null/empty on error. Helper functions `parseSearchResult` and `extract3DModelUUID` are exported for unit testing.

3. **renderer3d.ts extension** — Added `loadComponentFromOBJ(objText, refdes)` following the same pattern as `loadComponentModel()`: finds placeholder by `component-${refdes}` name, copies transform, disposes placeholder, builds `THREE.Group` with `BufferGeometry` + `MeshStandardMaterial` per material group, adds to `layer-top` group. Tracks in `loadedModels` Map and `_objModelCount`. Added `objModelCount` to debug surface. Resets count in `clearBoardGroup()`.

## Verification

- `npx vitest run` — 127 tests pass (11 test files), including 9 new OBJ parser tests and 9 new JLCPCB client tests
- `npx tsc --noEmit` — zero type errors
- OBJ parser tests cover: basic cube, material parsing, double-slash faces, d 0.0 opaque convention, multiple material groups, empty/malformed input, comment lines, computed normals, EasyEDA-style sample data

### Slice-level verification (partial — T01 is intermediate):
- ✅ `npx vitest run` — all unit tests pass
- ⬜ `npx playwright test e2e/jlcpcb-search.spec.ts` — E2E test file not yet created (T03)
- ⬜ `npx playwright test` — full E2E suite (T03)

## Diagnostics

- OBJ parser: call `parseEasyEdaOBJ(text)` with any string to get geometry groups. Empty array = parse failure or no geometry.
- API parsing: call `parseSearchResult(raw)` or `extract3DModelUUID(compData)` with mock data to test response parsing.
- Runtime: `window.__renderer3d.objModelCount` shows how many OBJ models are loaded in the 3D scene.
- Console logs: grep for `[3D] OBJ` or `[JLCPCB]` to trace model loading and API calls.

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `viewer/src/easyeda-obj-parser.ts` — NEW: pure function OBJ parser for EasyEDA format
- `viewer/src/jlcpcb.ts` — NEW: JLCPCB search client + EasyEDA 3D model pipeline
- `viewer/src/renderer3d.ts` — Extended with `loadComponentFromOBJ()`, `_objModelCount`, `objModelCount` on debug surface
- `viewer/src/__tests__/easyeda-obj-parser.test.ts` — NEW: 9 unit tests for OBJ parser
- `viewer/src/__tests__/jlcpcb.test.ts` — NEW: 9 unit tests for API response parsing
