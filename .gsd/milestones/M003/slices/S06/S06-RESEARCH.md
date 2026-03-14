# S06: JLCPCB Integration & 3D Models — Research

**Date:** 2026-03-13

## Summary

The slice has two coupled deliverables: (1) a component search panel that queries JLCPCB/LCSC catalogs and shows part metadata, and (2) a 3D model loading pipeline that replaces placeholder boxes with real component models in the 3D view.

The critical finding is that no single API gives us both search results and 3D models — we need a two-API pipeline. **Search** goes through tscircuit's jlcsearch (`jlcsearch.tscircuit.com`), which is open, CORS-enabled (`Access-Control-Allow-Origin: *`), requires no auth, and returns rich component data including LCSC part numbers, pricing, stock, datasheets, and manufacturer info via `full=true` flag. **3D models** come from EasyEDA's undocumented but stable model hosting: given an LCSC part number, we hit `easyeda.com/api/products/{lcsc_id}/components` to get the footprint data which contains a 3D model UUID, then fetch the OBJ geometry from `modules.easyeda.com/3dmodel/{uuid}` — also CORS-enabled (`Access-Control-Allow-Origin: *`).

The main complication is format: EasyEDA serves a non-standard OBJ variant with inline `newmtl`/`endmtl` material blocks (not standard `.mtl` file). Three.js's OBJLoader can't parse this directly. We need a lightweight custom parser (~100 lines) that extracts vertices, faces, and material colors from the EasyEDA OBJ text and builds Three.js `BufferGeometry` + `MeshStandardMaterial` directly. This is simpler and more reliable than trying to shim the OBJ into a standard loader pipeline.

## Recommendation

**Use tscircuit/jlcsearch for component search + EasyEDA undocumented API for 3D model loading, with a custom OBJ parser for the non-standard format.**

Rationale:
- jlcsearch is the only CORS-enabled, auth-free, well-structured JLCPCB component API available. The official LCSC API requires authentication (API key + nonce + signature), which is impractical for a client-side-only app.
- EasyEDA's 3D model endpoints have been stable for years (used by easyeda2kicad, JLC2KiCadLib, and multiple KiCad plugins). CORS is open. The OBJ format is simple enough that a custom parser is safer than depending on format compatibility with standard loaders.
- The existing `loadComponentModel(url, refdes)` method in renderer3d.ts uses GLTFLoader — it needs to be generalized to also accept OBJ data (or we add a parallel `loadComponentFromOBJ(objText, refdes)` method).

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|-------------------|------------|
| Component search API | tscircuit/jlcsearch (`jlcsearch.tscircuit.com`) | CORS-enabled, no auth, returns LCSC data with metadata, well-maintained |
| 3D model hosting | EasyEDA model CDN (`modules.easyeda.com/3dmodel/{uuid}`) | Stable for years, CORS open, covers most LCSC parts |
| 3D model UUID extraction | EasyEDA API (`easyeda.com/api/products/{id}/components`) | Only way to get the 3D model UUID for a given LCSC part |
| GLB model loading | Three.js GLTFLoader (already integrated) | S02 established this pipeline |
| Settings persistence | `settings.ts` `getPreference/setPreference` | S04 built the typed settings API |
| UI panel pattern | `project-manager.ts` overlay pattern | S05 established overlay/modal UI patterns |

## Existing Code and Patterns

- `viewer/src/renderer3d.ts:366` — `loadComponentModel(url, refdes)` replaces placeholder box with GLTF model by `component-{refdes}` name convention. **Must extend to support OBJ text input** (not just GLB URL).
- `viewer/src/renderer3d.ts:16` — `GLTFLoader` already imported in lazy-loaded module. Add custom OBJ parser alongside (not Three.js OBJLoader — the format is non-standard).
- `viewer/src/types.ts:50` — `model_3d: string | null` on `ComponentInfo`. Currently always `null`. Can store LCSC part number or model UUID for cache key.
- `viewer/src/wasm.ts:275` — `parseSource()` sets `model_3d: null`. If DSL components have `lcsc` properties, this is where to extract them.
- `viewer/src/settings.ts` — Settings API with subscribe pattern. Follow for any JLCPCB-related preferences.
- `viewer/src/project-manager.ts` — Overlay pattern with `show()/hide()`, DOM construction, callback interface. Follow for search panel.
- `viewer/index.html:729-765` — Toolbar structure. Add search button here.
- `viewer/index.html:767-823` — Preferences modal pattern (z-index 200). Search panel can use z-index 100 (below prefs, above canvas).
- `viewer/src/main.ts` — 1909 lines. New module integration follows the import + init + wire pattern. Keep new logic in a dedicated module, wire through callbacks.

## Constraints

- **No server-side proxy** — both APIs are CORS-enabled, direct browser `fetch()` works. If EasyEDA ever blocks CORS, we'd need a proxy, but that's a future problem.
- **EasyEDA API is undocumented** — no SLA, could change. Rate limiting unknown. Must handle failures gracefully (fallback to placeholder box).
- **OBJ format is EasyEDA-specific** — inline `newmtl`/`endmtl` blocks with `Ka`/`Kd`/`Ks`/`d` properties. Vertices, faces, and `usemtl` are standard OBJ. Face format uses `f v// v// v//` (empty normal/texture indices).
- **3D model availability varies** — not all LCSC parts have 3D models. The footprint `packageDetail.dataStr.shape` array may or may not contain an `outline3D` entry.
- **jlcsearch data freshness** — tscircuit updates their database periodically from JLCPCB scrapes, not real-time. Stock numbers may lag.
- **`loadComponentModel` assumes GLB** — needs refactoring to handle OBJ text or a new companion method.
- **main.ts is 1909 lines** — keep new wiring minimal, delegate to a new `jlcpcb.ts` module.
- **Lazy loading** — 3D code is lazy-loaded via `import()`. JLCPCB search module should also be lazy or at least tree-shakeable.

## Common Pitfalls

- **EasyEDA OBJ face format** — uses `f 219// 229// 228//` (double-slash, no normals). A naive parser that splits on `/` must handle empty segments. Must compute normals from face geometry.
- **LCSC ID format mismatch** — jlcsearch returns bare numbers (`17414`), EasyEDA API expects `C17414`. Must prepend `C` when bridging.
- **EasyEDA API rate limiting** — fetching footprint data + 3D model for every search result would hammer the API. Only fetch 3D when user selects a specific component, not on search.
- **3D model scale mismatch** — EasyEDA OBJ coordinates are in mm but centered differently. The existing placeholder boxes use `body_width_nm/body_height_nm` from pad bounding box. Must align OBJ model center/scale with the placeholder's position/rotation.
- **Material opacity** — EasyEDA OBJ uses `d 0.0` for some materials (fully transparent in standard OBJ). This likely means "no dissolve" in their convention, not "invisible". Must treat `d 0.0` as fully opaque or ignore the `d` parameter.
- **Memory leaks on repeated search** — if user searches multiple times and loads 3D models each time, must dispose previous models. The existing `loadedModels` Map in renderer3d.ts handles this for GLTF but needs to cover OBJ too.
- **Debouncing search input** — raw keystroke-triggered searches would flood the API. Debounce 300-500ms.

## Open Risks

- **EasyEDA API stability** — undocumented endpoint could change URL structure, response format, or add auth. Mitigated by having the existing easyeda2kicad community as canary (they'd notice and fix).
- **jlcsearch availability** — third-party service, could go down. Should show graceful error state, not crash.
- **OBJ model quality** — some components may have broken OBJ data (malformed faces, missing vertices). Parser needs to be tolerant.
- **Performance with many 3D models** — loading 20+ OBJ models simultaneously after search could spike network and GPU memory. Should limit concurrent loads and lazy-load models only when 3D view is active.
- **Test coverage for API integration** — E2E tests can't hit real external APIs reliably. Need mock/intercept strategy in Playwright (route interception).

## Requirements Targeted

| Requirement | How This Slice Addresses It |
|-------------|---------------------------|
| LIB-01 (search by name, MPN, value, category) | jlcsearch `/api/search` endpoint with `q` parameter matches all these |
| LIB-03 (associate 3D STEP models with components) | EasyEDA 3D model loaded into 3D view for components with LCSC part numbers |
| LIB-05 (import from JLCPCB API) | Component search panel returns JLCPCB/LCSC catalog data |
| LIB-08 (preview footprints before adding) | Search results show package/footprint info; 3D preview when available |
| LIB-09 (view component metadata) | Search results display price, stock, datasheet link, manufacturer, attributes |
| LIB-12 (unified search across library sources) | Single search bar queries JLCPCB catalog (first library source integrated) |

## Data Pipeline

```
User types "0805 10k" in search panel
  → GET jlcsearch.tscircuit.com/api/search?q=0805+10k&limit=20&full=true
  → Parse response: extract lcsc, mfr, package, stock, price, extra.attributes, extra.datasheet
  → Display results in search panel

User clicks a result (e.g. C17414)
  → GET easyeda.com/api/products/C17414/components?version=6.4.19.5
  → Parse response: extract packageDetail.dataStr.shape[].outline3D → UUID
  → If UUID found:
    → GET modules.easyeda.com/3dmodel/{uuid}
    → Parse non-standard OBJ text → Three.js BufferGeometry + materials
    → Call renderer3d.loadComponentFromOBJ(geometry, materials, refdes)
  → If no 3D model: keep placeholder box, show "no 3D model" indicator
```

## API Response Shapes

### jlcsearch `/api/search` (with `full=true`)
```json
{
  "components": [{
    "lcsc": 17414,
    "mfr": "0805W8F1002T5E",
    "package": "0805",
    "is_basic": true,
    "stock": 15457503,
    "price": 0.001642857,
    "extra": "{\"manufacturer\":{\"name\":\"UNI-ROYAL\"},\"attributes\":{\"Resistance\":\"10kΩ\"},\"datasheet\":{\"pdf\":\"...\"},\"images\":[...]}"
  }]
}
```

### EasyEDA component API
```json
{
  "result": {
    "packageDetail": {
      "dataStr": {
        "shape": ["SVGNODE~{...\"c_etype\":\"outline3D\",\"uuid\":\"c7acac53bcbc44d68fbab8f60a747688\"...}"]
      }
    }
  }
}
```

### EasyEDA 3D model OBJ (non-standard)
```obj
v 0.8 0.65 0.55
v 0.8 0.65 0.6
newmtl 1
Ka 0.85 0.85 0.85
Kd 0.85 0.85 0.85
Ks 0.43 0.43 0.43
d 0.0
endmtl
usemtl 1
f 1// 2// 3//
f 3// 4// 1//
```

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| JLCPCB/LCSC search | `takazudo/jlcpcb-parts-finder-skill@jlcpcb-component-finder` | available (15 installs) |
| EDA research | `l3wi/claude-eda@eda-research` | available (11 installs) |
| Three.js loaders | `cloudai-x/threejs-skills@threejs-loaders` | available (1.4K installs) |

The Three.js loaders skill has high install count and could be useful for OBJ→Three.js geometry pipeline knowledge. The JLCPCB parts finder is low-install but directly relevant.

## Sources

- jlcsearch API works without auth, CORS-enabled with `Access-Control-Allow-Origin: *` (verified: `curl -sI -H "Origin: http://localhost:5173"`)
- EasyEDA 3D model OBJ endpoint returns data with `Access-Control-Allow-Origin: *` (verified: `curl -sI -H "Origin: http://localhost:5173" "https://modules.easyeda.com/3dmodel/{uuid}"`)
- EasyEDA API response structure for LCSC components (verified: `curl -s "https://easyeda.com/api/products/C17414/components?version=6.4.19.5"`)
- `easyeda2kicad` Python source (`easyeda/easyeda_api.py`) — documents all three API endpoints and their patterns (source: [GitHub](https://github.com/uPesy/easyeda2kicad.py))
- JLCPCB 3D models are STEP/OBJ only, no GLB (source: Google Search research, CDFER/JLCPCB-KiCad-Library)
- tscircuit/jlcsearch provides typed category endpoints (`/resistors/list.json`, etc.) and generic `/api/search` (source: [tscircuit.com](https://jlcsearch.tscircuit.com))
