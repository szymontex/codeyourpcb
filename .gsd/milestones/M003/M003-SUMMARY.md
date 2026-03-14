---
id: M003
provides:
  - Professional 2D board renderer with LOD, pad numbers, net labels, refdes, per-layer colors, drill marks, per-pad net highlighting
  - Working 3D view with component bodies, pads, traces, vias (was empty green board)
  - Routing UX with net-aware target highlighting, magnetic snap (dual threshold), ratsnest guide, angle constraint toggle
  - Clean toolbar + View dropdown (layer/grid/ratsnest/net-labels) + Preferences modal (theme/units/grid/colors)
  - Unit system (mm/mil/µm) with formatDimension/parseUserDimension wired to all display sites
  - Project manager startup overlay with 3 templates + blank board, recent files with thumbnails
  - JLCPCB/LCSC component search via jlcsearch API + EasyEDA OBJ 3D model loading pipeline
  - Settings persistence (localStorage) with typed get/set/subscribe API
  - RenderConfig boundary contract consumed by routing (S03) and preferences (S04)
  - Diagnostic surfaces: __renderDiag, __renderer3d, __routingState, __viewport, __settings, __projectManager, __jlcpcbSearch, __editor
  - 94 Playwright E2E tests + 127 Vitest unit tests + 808 Rust tests — all passing
  - 8-stage quality gate clean (cargo fmt, clippy, cargo test, eslint, vitest, playwright, autorouter benchmark, jscpd)
  - Version 0.1.0-beta across package.json, Cargo.toml, tauri.conf.json
key_decisions:
  - Client-side pad-to-net join from NetInfo.connections (avoids Rust/WASM changes for S01)
  - 4-tier LOD system (Far/Medium/Close/Detail) gating text density by viewport scale
  - NaN guard uses !(x > 0) pattern for 3D body dimension safety
  - Custom EasyEDA OBJ parser instead of Three.js OBJLoader (non-standard format)
  - JLCPCB search via tscircuit/jlcsearch proxy (no auth, CORS-enabled)
  - Magnetic snap dual threshold (1mm world OR 15px screen) for zoom-independent UX
  - Angle snap defaults OFF per roadmap ("optional toggle, not forced")
  - Grid visibility and grid snap as independent controls (fixes M002 bug)
  - Settings persistence as single JSON key with partial-merge on load
  - Project manager is file manager, not project abstraction ("project" = one .cypcb file)
  - Recent files store metadata only (FileSystemFileHandle not serializable)
  - WasmPcbEngineAdapter JS fallback for trace mutations (WASM module lacks exports)
  - JLCPCBSearchError class for distinct HTTP error vs empty results states
patterns_established:
  - RenderConfig as boundary contract — extend for new visual features
  - Diagnostic-driven E2E — all assertions through debug surfaces, no pixel comparisons
  - __loadBoard(source) pattern for E2E board loading with viewport sync
  - Settings subscribe pattern — subscribe(listener) returns unsubscribe fn
  - Unit formatting pattern — formatDimension(nm, unit) for display, parseUserDimension(str) for input
  - PM dismissal in E2E tests via __loadBoard(MINIMAL_BOARD) in beforeEach
  - Route interception helper (interceptAPIs) for E2E tests against external APIs
  - buildTraceSegments() shared helper for deduplication in wasm.ts
observability_surfaces:
  - "window.__renderDiag: { lodTier, padNetMapSize, lastFrameMs, textElementsDrawn, highlightedNet }"
  - "window.__renderer3d: { componentCount, traceSegmentCount, padCount, viaCount, objModelCount }"
  - "window.__routingState: { angleSnapEnabled, magneticSnapEnabled, snappedToPad, targetPadsCount }"
  - "window.__viewport: { centerX, centerY, scale, width, height }"
  - "window.__settings: live settings snapshot"
  - "window.__projectManager: { visible, recentFiles, templateCount, show(), hide() }"
  - "window.__jlcpcbSearch: { lastQuery, resultCount, lastError, visible }"
  - "window.__editor: Monaco editor instance"
  - "bash scripts/quality-gate.sh — single command, 8-stage verification"
requirement_outcomes: []
duration: 7 slices across ~8h
verification_result: passed
completed_at: 2026-03-14
---

# M003: From Prototype to Tool — Professional Board View & UX

**Every user-facing surface upgraded from prototype to professional quality — 2D renderer with KiCad-comparable visuals, working 3D view, net-aware routing UX, clean UI architecture with preferences and project management, JLCPCB component search, all verified by 94 E2E tests and 8-stage quality gate.**

## What Happened

Seven slices transformed CodeYourPCB from "engine works" to "tool I'd actually use."

**S01** rewrote the Canvas 2D renderer with 8 professional features: component body outlines in silkscreen yellow, pad pin numbers (LOD ≥ Close), net labels at trace midpoints with rotated dark-background pills, world-space refdes (LOD ≥ Medium), drill crosshair marks on THT pads, per-layer colors (red top / blue bottom / yellow silkscreen), and per-pad net highlighting. A 4-tier LOD system (Far/Medium/Close/Detail) gates text density to keep Canvas fillText under frame budget. The `RenderConfig` interface was extracted as a boundary contract consumed by S03 (routing highlighting) and S04 (preferences). A `buildPadNetMap()` utility joins `NetInfo.connections` client-side into a `Map<"refdes.pin", netName>` — avoids any Rust/WASM changes.

**S02** diagnosed why the 3D view rendered an empty green board: `parseSource()` never set `body_width_nm`/`body_height_nm` on ComponentInfo, producing `NaN` from `undefined * NM_TO_MM`, and the guard `bodyW <= 0` silently passed (NaN <= 0 is false in JS). Fixed with pad-bounding-box computation at parse time and `!(bodyW > 0)` NaN-safe guard. Added `loadComponentModel(url, refdes)` with GLTFLoader for S06's 3D model pipeline.

**S03** extended routing with net-aware target pad highlighting (computed once at route start), magnetic snap (dual threshold: 1mm world OR 15px screen), ratsnest emphasis (active net full alpha + 2x width, others dimmed), and angle constraint toggle via A key (defaults OFF). Exposed a latent bug: `__loadBoard` wasn't syncing `interactionState.viewport`, causing all pad hit-tests to miss. Fixed. Also added JS-side fallback for trace mutations since the WASM module lacks `add_trace_json`/`remove_trace` exports.

**S04** restructured the UI: removed 6 toolbar controls, added a View dropdown (layer/grid/ratsnest/net-labels toggles), and built a Preferences modal (theme single-click fix, units mm/mil/µm, grid visual+snap spacing, 5 layer color pickers). Created `settings.ts` (typed AppSettings with localStorage persistence, change subscription) and `units.ts` (formatDimension/parseUserDimension). Grid visibility became independent of routing grid snap — fixing the M002 "grid toggle does nothing" bug.

**S05** added a project manager startup overlay: 3 bundled templates (Blink LED, Power Indicator, Simple PSU) + blank board scaffold, recent files with thumbnail data URLs (capped at 10), and full lifecycle wiring (dismiss on load, re-show on new file). Required adding PM dismissal (`__loadBoard(MINIMAL_BOARD)` in `beforeEach`) to 7 existing E2E test files. Verified editor→board reflow (editor.setValue triggers board dimension update).

**S06** integrated JLCPCB component search via tscircuit's jlcsearch API (CORS-enabled, no auth). Built a custom OBJ parser (~180 lines) for EasyEDA's non-standard format (inline `newmtl`/`endmtl`, `f v// v// v//` faces). Search panel is a right-side overlay (z-index 100) with debounced input, result cards showing LCSC#, manufacturer, package, price, stock, datasheet. 3D model loading triggers on component click (not on search) to avoid API hammering. E2E tests use route interception — zero real API calls in CI.

**S07** cleaned up the quality gate: fixed ESLint unused import, extracted `buildTraceSegments()` shared helper to eliminate jscpd duplication, improved JLCPCB error handling (distinct HTTP error vs empty results), added prefs-theme E2E test verifying M002 single-click fix, and set version to 0.1.0-beta across all three config files. Final count: 94 E2E + 127 unit + 808 Rust tests, 8/8 quality gate stages passing.

## Cross-Slice Verification

**2D board view renders pad shapes with numbers, net labels, readable refdes, per-layer colors, drill marks:**
✅ S01 renderer-quality.spec.ts — 8 E2E tests verify padNetMap population, LOD tier transitions, text element rendering, net highlight activation, and frame performance. Canvas dimensions, component count, and diagnostic surface shape all asserted. Copper fills NOT rendered — cypcb-world has no Zone/CopperFill type in the ECS. This is a data model limitation, not a renderer limitation, and was documented as a known constraint.

**3D view shows traces, pads, vias, and component bodies:**
✅ S02 three-d-view.spec.ts — 3 new geometry verification tests: componentCount > 0 (was 0 before fix), meshCount > 1, all debug counters are valid numbers. Re-toggle consistency test proves clearBoardGroup + rebuild is deterministic. blink.cypcb verified: componentCount=9, meshCount=12, padCount=24.

**Routing UX: target pad highlighting, magnetic snap, ratsnest guide, angle constraint toggle:**
✅ S03 routing-ux.spec.ts — 6 E2E tests cover start/complete/cancel/highlight/angle-toggle/layer-flip. 14 unit tests verify magnetic snap thresholds, target pad computation, angle snap math. __routingState diagnostic surface exposes all state.

**Preferences panel: theme, units, grid spacing, layer colors:**
✅ S04 ui-architecture.spec.ts — 15 E2E tests across 4 describe blocks (Toolbar Structure, View Menu, Preferences Modal, Persistence). Settings persist across page reload. Color pickers use 'input' event for live preview. Theme single-click fix verified.

**Unit display works throughout the UI:**
✅ S04 units.test.ts — 20 unit tests for formatDimension/parseUserDimension. Coords display and trace tooltip wired to formatDimension with current unit from getPreference('units').

**Project manager: recent files, new project, import, template:**
✅ S05 project-manager.spec.ts — 14 E2E tests covering PM visible on startup, template cards, template click loads board + dismisses PM, blank board scaffold, recent files updated/capped/persisted across reload, editor→board reflow.

**JLCPCB/LCSC component search returns results with part metadata:**
✅ S06 jlcpcb-search.spec.ts — 6 E2E tests with route interception: results with metadata, empty results, API error handling, debounce verification, 3D model fetch pipeline trigger. Live API calls work in production but CORS blocks EasyEDA 3D model fetch from localhost.

**Toolbar clean, View menu/panel contains layer/grid/ratsnest:**
✅ S04 ui-architecture.spec.ts toolbar structure tests — View dropdown verified present, layer/grid/ratsnest toggles relocated inside it.

**M002 UI bugs fixed:**
✅ S07 theme.spec.ts — prefs-theme single-click button verified (asserts label change, not data-theme attribute). S04 fixed grid visibility independence from routing grid snap.

**E2E tests cover new features:**
✅ 94 Playwright E2E tests total (from 41 at M002 end), covering renderer output, 3D geometry, routing flow, UI architecture, preferences, project manager, JLCPCB search, theme.

**8-stage quality gate passes:**
✅ S07 ran `bash scripts/quality-gate.sh` — cargo fmt, clippy, cargo test (808), eslint (0 errors), vitest (127), playwright (94), autorouter benchmark (<30s), jscpd (0 clones). All pass.

**Criterion partially met — copper fills:** The success criteria mention "copper fills" in the 2D view. Copper fill zones are not rendered because `cypcb-world` has no Zone/CopperFill type in the ECS data model. This is not a renderer deficiency — the data doesn't exist to render. Documented as known limitation across S01 and DECISIONS.md. All other 2D visual elements are present and verified.

## Requirement Changes

No requirement status transitions occurred during M003. All 64 requirements were already in "validated" status before M003 began. The milestone improved the quality of existing validated capabilities (2D rendering, 3D view, routing, UI, settings) rather than validating new requirements. S06 advanced LIB-01/03/05/09 capabilities but they were already validated from prior milestones.

## Forward Intelligence

### What the next milestone should know
- Quality gate is solid — 8 stages, 94 E2E + 127 unit + 808 Rust tests. Extend existing test patterns for new features.
- The diagnostic surface pattern (`window.__renderDiag`, `__renderer3d`, `__routingState`, `__viewport`, `__settings`, `__projectManager`, `__jlcpcbSearch`, `__editor`) is the canonical way to verify state in E2E tests. Never use pixel comparison.
- All E2E tests that interact with canvas/editor must call `__loadBoard(MINIMAL_BOARD)` in `beforeEach` to dismiss the project manager overlay.
- JLCPCB search uses tscircuit/jlcsearch (no auth, CORS-enabled). EasyEDA 3D model API is NOT CORS-friendly from localhost — works only in production or behind a proxy.
- WasmPcbEngineAdapter has JS fallbacks for trace mutations (add_trace, remove_trace, run_drc_incremental, trace_count). If Rust engine adds these WASM exports, update the adapter to prefer WASM path.
- `pullSnapshot()` in main.ts is the single refresh point for board state — always use it rather than calling engine methods directly.
- Settings subscription in main.ts is a growing switch/case — extract to dedicated module if many more settings are added.

### What's fragile
- LOD thresholds are hardcoded scale values calibrated to blink.cypcb — boards with very different component densities may need adjustment. S04 made them configurable via RenderConfig but no UI for threshold tuning exists yet.
- EasyEDA API response format (outline3D UUID extraction) is an undocumented internal API — `extract3DModelUUID` is the single parse point that could break without notice.
- `component-${refdes}` naming convention links 2D renderer, 3D renderer, and model loading — if mesh naming changes, `loadComponentModel`/`loadComponentFromOBJ` fail silently.
- ThemeManager maintains its own `'theme'` localStorage key separate from settings `'cypcb-settings'` key (by design for FART prevention) — two sync paths for theme state.
- PM z-index stacking (100 search, 150 PM, 160 view dropdown, 200 prefs) — new overlays must respect this hierarchy.
- errors.spec.ts:102 ("Ready" vs "Reloaded" status race) — stable in current runs but historically flaky.

### Authoritative diagnostics
- `bash scripts/quality-gate.sh` — single command for full 8-stage verification. This is the definitive check.
- `window.__renderDiag` — live LOD tier, pad-net map size, frame time, text count. First place to look for 2D renderer issues.
- `window.__renderer3d` — live 3D geometry counts. If componentCount is 0 after loading in 3D, the parse pipeline broke.
- `window.__routingState` — complete routing state machine. Trustworthy because E2E tests validate it every run.
- `window.__settings` — live settings snapshot. Primary E2E assertion surface for preferences.
- `window.__jlcpcbSearch.lastError` — JLCPCB search error state (null on success, string on HTTP error).

### What assumptions changed
- Assumed 3D empty board was a coordinate transform or rendering bug — actual cause was undefined body dimensions in JS parser producing NaN past a broken guard. Pure data pipeline issue.
- Assumed WASM module had trace mutation APIs — it doesn't. JS fallback was necessary and is now the stable path.
- Assumed `__loadBoard` already synced interaction state — it didn't. Latent bug affecting all file-loading scenarios.
- Assumed JLCPCB provides GLB/STEP models — actual pipeline uses EasyEDA OBJ format, requiring a custom parser.
- Assumed copper fills could be rendered — no Zone/CopperFill type exists in cypcb-world ECS. Requires Rust data model work.
- Assumed recent file click would re-open files — FileSystemFileHandle can't be serialized to localStorage.

## Files Created/Modified

- `viewer/src/render-config.ts` — RenderConfig interface, LodTier enum, defaults, getLodTier(), buildPadNetMap()
- `viewer/src/renderer.ts` — Professional 2D renderer with LOD, text pass, pad highlighting, snap indicator, ratsnest emphasis, grid visibility
- `viewer/src/renderer3d.ts` — NaN-safe guard, GLTFLoader, loadComponentModel, loadComponentFromOBJ, enriched debug surface
- `viewer/src/routing.ts` — Magnetic snap, angle toggle, target pads, resetToIdle
- `viewer/src/interaction.ts` — Routing keyboard handler (Escape/F/A), onRouteStart/onRouteEnd callbacks
- `viewer/src/settings.ts` — Typed AppSettings with localStorage persistence, change subscription, RecentFileEntry
- `viewer/src/units.ts` — Unit formatting/parsing for mm/mil/µm
- `viewer/src/project-manager.ts` — Project manager module (init, show, hide, addRecentFile, generateThumbnail)
- `viewer/src/easyeda-obj-parser.ts` — Custom OBJ parser for EasyEDA non-standard format
- `viewer/src/jlcpcb.ts` — jlcsearch API client + EasyEDA 3D model pipeline + JLCPCBSearchError
- `viewer/src/jlcpcb-panel.ts` — Search panel DOM + event handling + debug surface
- `viewer/src/wasm.ts` — Body dimension computation, JS fallback for trace mutations, buildTraceSegments helper
- `viewer/src/main.ts` — Orchestration wiring for all new features, diagnostic surfaces, settings subscription
- `viewer/index.html` — Toolbar restructure, View dropdown, Preferences modal, PM overlay, search panel, CSS
- `viewer/public/templates/*.cypcb` — 3 bundled template files
- `viewer/e2e/renderer-quality.spec.ts` — 8 E2E tests for 2D renderer
- `viewer/e2e/three-d-view.spec.ts` — 6 E2E tests for 3D view
- `viewer/e2e/routing-ux.spec.ts` — 6 E2E tests for routing UX
- `viewer/e2e/ui-architecture.spec.ts` — 15 E2E tests for UI architecture
- `viewer/e2e/project-manager.spec.ts` — 14 E2E tests for project manager
- `viewer/e2e/jlcpcb-search.spec.ts` — 6 E2E tests for JLCPCB search
- `viewer/e2e/theme.spec.ts` — Prefs-theme single-click E2E test
- `viewer/e2e/fixtures/routing-test.cypcb` — Routing test fixture
- `viewer/src/__tests__/render-config.test.ts` — 15 unit tests
- `viewer/src/__tests__/pad-net-map.test.ts` — 7 unit tests
- `viewer/src/__tests__/routing.test.ts` — 14 unit tests
- `viewer/src/__tests__/settings.test.ts` — 12 unit tests
- `viewer/src/__tests__/units.test.ts` — 20 unit tests
- `viewer/src/__tests__/easyeda-obj-parser.test.ts` — 9 unit tests
- `viewer/src/__tests__/jlcpcb.test.ts` — 9 unit tests
- `viewer/package.json` — Version 0.1.0-beta
- `Cargo.toml` — Version 0.1.0-beta
- `src-tauri/tauri.conf.json` — Version 0.1.0-beta
