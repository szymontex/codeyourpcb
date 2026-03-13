# S01: Professional 2D Board Renderer

**Goal:** Rewrite the Canvas 2D renderer from prototype to professional-quality PCB visualization — pad pin numbers, net labels, component body outlines, drill marks, zoom-dependent LOD, per-pad net highlighting, and proper silkscreen colors.
**Demo:** Load `blink.cypcb` and see: component body outlines in yellow silkscreen color, pad pin numbers visible when zoomed in, net labels on trace midpoints, readable refdes that scales with zoom, drill crosshair marks on THT pads, per-pad net highlighting (only pads on the highlighted net glow, others dim), all with KiCad-matching layer colors.

## Must-Haves

- `RenderConfig` interface extracted as boundary contract for S03 (routing) and S04 (preferences)
- Pad-to-net lookup built client-side from `NetInfo.connections` — enables per-pad net highlighting
- Component body outlines drawn as silkscreen-colored rectangles (`#C8C800` yellow)
- Pad pin numbers rendered inside/near pads, scaled to pad size, with LOD cutoff
- Net labels rendered at trace midpoints with LOD cutoff
- Refdes text uses world-space sizing (not fixed 10px), scales with zoom, clamped 8–24px
- Drill crosshair marks on THT pads at medium+ zoom
- 3–4 tier LOD system controlling text density by zoom level
- Per-pad net highlighting replaces blanket pad dimming
- Canvas text rendering stays under 16ms frame budget at 500+ components (LOD manages this)
- E2E test verifying renderer output elements are visible in the DOM/Canvas

## Proof Level

- This slice proves: contract + operational (RenderConfig boundary contract, visual rendering correctness)
- Real runtime required: yes (Canvas rendering in browser)
- Human/UAT required: yes (visual comparison against KiCad reference — specific checkable items, not pixel matching)

## Verification

- `cd viewer && npx vitest run --reporter=verbose` — unit tests for pad-to-net lookup, LOD threshold logic, RenderConfig defaults
- `cd viewer && npx playwright test e2e/renderer-quality.spec.ts` — E2E test loading blink.cypcb verifying: canvas renders, component count matches snapshot, renderer exposes LOD state and pad-to-net map for testability
- Manual: load blink.cypcb at various zoom levels, confirm body outlines / pad numbers / net labels / drill marks appear/disappear at appropriate LOD thresholds

## Observability / Diagnostics

- Runtime signals: `window.__renderDiag` exposes current LOD tier, pad-to-net map size, last frame time, text elements drawn count — readable via `page.evaluate()` in E2E and by future agents debugging renderer
- Inspection surfaces: browser console `renderDiag` object, E2E page.evaluate queries
- Failure visibility: LOD tier mismatch (expected text at zoom level but none drawn), frame time exceeding 16ms logged to console as warning
- Redaction constraints: none

## Integration Closure

- Upstream surfaces consumed: `BoardSnapshot` types (unchanged), `viewport.ts` worldToScreen, `layers.ts` LAYER_COLORS/getPadColor/getTraceColor/netColor, existing `drawRoundRect`/`drawOblong` helpers
- New wiring introduced: `RenderConfig` interface in `render-config.ts`, `buildPadNetMap()` utility, LOD tier calculation, diagnostic surface `window.__renderDiag`
- What remains before milestone is truly usable end-to-end: S02 (3D), S03 (routing UX using pad highlighting from this slice), S04 (preferences driving RenderConfig)

## Tasks

- [x] **T01: Rewrite renderer with professional visuals, LOD, and per-pad net highlighting** `est:3h`
  - Why: Core rendering upgrade — all visual features, LOD system, RenderConfig contract, pad-to-net lookup, diagnostic surface. This is the meat of the slice.
  - Files: `viewer/src/render-config.ts` (new), `viewer/src/renderer.ts`, `viewer/src/layers.ts`, `viewer/src/main.ts`, `viewer/src/__tests__/render-config.test.ts` (new)
  - Do: Extract `RenderConfig` interface with layer colors, font config, LOD thresholds. Implement `buildPadNetMap(nets)` from `NetInfo.connections`. Add LOD tier calculation from `vp.scale`. Extend `drawComponent()` with body outline in silkscreen color. Add `drawPadNumber()` with LOD gate. Add `drawTraceNetLabel()` at segment midpoint with LOD gate. Upgrade refdes to world-space font sizing (0.8mm text, clamped 8–24px). Add drill crosshair marks on THT pads. Replace blanket pad dimming with per-pad net lookup. Batch text by font size to minimize ctx.font changes. Wire `RenderConfig` into `render()` and `main.ts`. Expose `window.__renderDiag` with LOD tier, pad-net map size, frame time, text count.
  - Verify: `cd viewer && npx vitest run` passes; app loads blink.cypcb without errors; zoom in/out shows LOD transitions; net highlight dims only non-net pads
  - Done when: all 8 visual features render correctly, RenderConfig interface exists, pad-to-net lookup works, LOD culls text at far zoom, diagnostic surface exposes render metrics

- [x] **T02: E2E tests for renderer quality and visual verification** `est:1h`
  - Why: Objective verification that professional rendering features are present and functional. Creates the `renderer-quality.spec.ts` test file referenced in slice verification.
  - Files: `viewer/e2e/renderer-quality.spec.ts` (new), `viewer/src/__tests__/pad-net-map.test.ts` (new)
  - Do: Write Playwright E2E test that loads app, waits for Ready, then uses `page.evaluate()` to check: `__renderDiag.padNetMapSize > 0` (pad-to-net built), `__renderDiag.lodTier` changes when zooming, `__renderDiag.textElementsDrawn > 0` at close zoom, canvas is non-empty (pixel sampling). Write unit test for `buildPadNetMap()` with mock NetInfo data verifying correct key→net mapping. Test per-pad highlighting: set `highlightedNet`, verify `__renderDiag` reflects it.
  - Verify: `cd viewer && npx vitest run` and `cd viewer && npx playwright test e2e/renderer-quality.spec.ts` both pass
  - Done when: E2E test passes in CI-compatible headless mode, unit test covers pad-net-map edge cases (empty nets, multi-pin components, duplicate pins), all tests green

## Files Likely Touched

- `viewer/src/render-config.ts` (new — RenderConfig interface, LOD thresholds, defaults)
- `viewer/src/renderer.ts` (major rewrite — all visual features, LOD integration, text rendering)
- `viewer/src/layers.ts` (minor — ensure silkscreen colors are exported/used)
- `viewer/src/main.ts` (minor — wire RenderConfig into render loop, expose __renderDiag)
- `viewer/src/__tests__/render-config.test.ts` (new — unit tests for config defaults, LOD calculation)
- `viewer/src/__tests__/pad-net-map.test.ts` (new — unit tests for buildPadNetMap)
- `viewer/e2e/renderer-quality.spec.ts` (new — E2E visual verification)
