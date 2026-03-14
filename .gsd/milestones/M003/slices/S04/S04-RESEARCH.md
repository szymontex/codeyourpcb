# S04: UI Architecture — Toolbar, View Menu & Settings — Research

**Date:** 2026-03-13

## Summary

This slice restructures the UI surface: clean toolbar (essential tools only), View menu/panel for layer/grid/ratsnest/net labels, Preferences panel for theme/units/grid spacing/layer colors, unit display system, and localStorage persistence for all settings. The codebase currently has ~14 toolbar items (Editor, Layers×2, Ratsnest, Grid, Undo/Redo, Fit, 3D, Theme, Coords, Open, Share, Auto-route, Route, Cancel) crammed into one bar. There's no View menu, no Preferences panel, no unit system, and no general settings persistence (only theme goes to localStorage). The grid spacing is hardcoded to 1mm in renderer, grid snap spacing is hardcoded to 50mil in routing. `RenderConfig` exists as the boundary contract from S01 but has no UI to modify it.

The main risk is the refactoring breadth in `main.ts` (1648 lines, 29 `getElementById` calls, all toolbar wiring inline). The approach should be: (1) extract a settings/preferences module that owns localStorage and provides `getPreference`/`setPreference`, (2) build a unit formatting system (`formatDimension`), (3) create View menu as a dropdown/panel with controls moved from toolbar, (4) create Preferences panel as a modal/drawer, (5) clean the toolbar to essential-only items. The work is medium-risk because it's a large UI refactor touching many wiring points, but no algorithmic complexity — it's plumbing.

The theme double-click bug from M002 feedback was already fixed — `ThemeManager.setTheme()` does single-click cycle correctly. The grid checkbox is wired to routing grid snap only, not to grid visibility toggle — that's the "grid toggle does nothing" bug (checking it enables routing grid snap but doesn't show/hide the visual grid). Fit icon uses `⊡` which may be unreadable in some fonts — needs replacement with an SVG or more universal character.

## Recommendation

**Extract settings to a standalone module, build the unit system, then restructure UI last.**

Order of work:
1. `viewer/src/settings.ts` — typed settings API with localStorage persistence, defaults, and event notification. Covers: theme (migrate from ThemeManager), units (mm/mils/µm), grid spacing, grid visibility, layer colors, ratsnest visibility, net label visibility.
2. `viewer/src/units.ts` — `formatDimension(nm, unit)` returning `"2.54mm"` / `"100mil"` / `"2540µm"`, plus `parseUserDimension()` for input fields.
3. Restructure `index.html` toolbar — keep: Editor, Select (implicit), Undo/Redo, Fit, 3D, Theme, Open/Share. Move to View menu: Layers, Grid visibility, Ratsnest, Net labels. Move to Preferences: unit selection, grid spacing, layer colors, LOD thresholds.
4. Build View dropdown/panel — simple CSS dropdown or slide-out panel. Controls toggle boolean settings that feed into `RenderState`.
5. Build Preferences modal — form with sections (Display, Grid, Colors). Changes write to settings, propagate to RenderConfig, and persist to localStorage.
6. Wire units throughout — coords display, status bar dimensions, routing trace width display.
7. Fix fit icon — replace ⊡ with SVG or emoji that renders consistently.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| Settings persistence | localStorage (already used for theme) | Simple, synchronous, no dependencies |
| Theme management | `ThemeManager` in theme-manager.ts | Already works, just needs to integrate with new settings module |
| LOD configuration | `RenderConfig` in render-config.ts | S01 boundary contract — Preferences panel should drive this |
| Event notification | ThemeManager's subscribe() pattern | Copy the pattern for settings changes — Set<listener>, subscribe/unsubscribe |

## Existing Code and Patterns

- `viewer/src/theme/theme-manager.ts` — ThemeManager singleton with `subscribe()` pattern. Settings module should follow same pattern for change notification. Theme storage should delegate to settings module but ThemeManager API stays stable.
- `viewer/src/render-config.ts` — `RenderConfig` interface with `layerColors`, `fontConfig`, `lodThresholds`. S04 Preferences panel drives these. `createDefaultRenderConfig()` provides defaults. Currently created once in main.ts and never modified.
- `viewer/src/renderer.ts` lines 210-238 — `drawGrid()` with hardcoded 1mm spacing. Must accept spacing from settings. Grid visibility controlled by a new flag, not the existing grid-snap checkbox.
- `viewer/src/main.ts` lines 140-170 — 29 `getElementById` calls, all inline. These should stay in main.ts (it's the app shell) but the toolbar HTML moves.
- `viewer/src/routing.ts` line 82 — `gridSpacing: 1_270_000` hardcoded. Should read from settings.
- `viewer/src/layers.ts` — `LAYER_COLORS` as const object + `LayerVisibility` type. Layer colors in RenderConfig override these for rendering; layers.ts colors remain as defaults.
- `viewer/src/main.ts` line 935 — `coordsEl.textContent = '(${xMm}, ${yMm}) mm'` — hardcoded mm. Must use `formatDimension()`.
- `viewer/index.html` toolbar — All 14 items in one `#toolbar` div. View menu items (Layers, Ratsnest, Grid) need to move out.

## Constraints

- **No build tool changes** — Vite config stays as-is, no new dependencies for UI components.
- **Canvas 2D renderer** — Grid visibility and spacing changes feed into `RenderState`, not CSS.
- **Keyboard shortcuts must not conflict** — S03 established Escape/F/A in routing mode; new shortcuts (e.g., V for View menu) must check routing mode and editor focus.
- **RenderConfig is the boundary** — S04 writes to RenderConfig, renderer reads it. No renderer changes beyond accepting config-driven grid spacing and grid visibility flag.
- **ThemeManager API must stay stable** — S05 and S06 depend on `themeManager.subscribe()` and `getTheme()`. Internal storage can change.
- **E2E tests run headless at port 4321** — Playwright config expects `npm run dev` to serve on 4321. New UI elements need data-testid or stable selectors for E2E.
- **All settings must have sensible defaults** — First visit with empty localStorage must produce the same UX as today.
- **Desktop (Tauri) compatibility** — Theme toggle and keyboard shortcuts have `isDesktop()` guards. New View menu should work in both web and desktop modes.

## Common Pitfalls

- **Grid snap vs grid visibility confusion** — Currently `#grid-snap` checkbox controls routing snap. The roadmap wants grid visibility toggle in View menu and snap toggle in routing. Must be two separate controls — don't conflate them.
- **Settings change cascade** — Changing units must update coords display, status bar, trace width tooltip, grid spacing display, board dimensions in status. Easy to miss one. Build `formatDimension()` and use it everywhere — grep for hardcoded `/ 1_000_000` or `.toFixed` to find all dimension display sites.
- **RenderConfig mutation** — Currently `const renderConfig = createDefaultRenderConfig()` in main.ts. It's an object reference — can mutate in place when Preferences change. But must trigger `dirty = true` for re-render. Consider using a getter pattern or explicit `applySettings()`.
- **localStorage quota** — Not a real risk for settings (tiny), but don't accidentally stringify large objects. Keep settings flat.
- **CSS dropdown z-index** — View menu dropdown must appear above canvas but below modal dialogs. Canvas is positioned absolute. Use z-index carefully.
- **Mobile/tablet layout** — Current CSS has `@media (pointer: coarse)` and `@media (max-width: 768px)` blocks. New View menu and Preferences must not break these.

## Open Risks

- **main.ts complexity** — At 1648 lines, any refactor to main.ts is high-touch. The settings wiring alone touches ~15 call sites. Must be surgical — don't attempt a full rewrite of main.ts in this slice.
- **View menu interaction on canvas** — Canvas captures pointer events aggressively (for pan/zoom/select). A dropdown overlaying the canvas must stop event propagation correctly.
- **Unit switching in flight** — If user changes units while a trace tooltip is visible or routing is active, intermediate state could show mixed units. Guard unit switches to idle state, or ensure all display sites read unit from settings on each render frame.
- **E2E test stability** — Moving toolbar elements changes selectors that existing E2E tests use (`#layer-top`, `#grid-snap`, etc.). Must update existing tests or keep IDs stable.

## Requirements Owned/Supported

### Directly Owned
- **UI-01 / UI-02 / UI-03 / UI-04** — Theme support (light/dark/auto, manual toggle). Theme already works; S04 fixes any single-click issues and integrates with Preferences.
- **UI-05** — Theme applies consistently across all surfaces. Preferences panel must follow theme.
- **UI-08** — Monaco editor theme syncs with app theme (already done, verify preserved).
- **UI-09** — Canvas renderer theme syncs (RenderConfig layer colors now user-configurable).

### Supported (not sole owner)
- **EDIT-10** — Editor/viewer side-by-side. Editor toggle stays in toolbar.
- **DESK-05** — Keyboard shortcuts. New shortcuts added (V for View, Comma for Preferences).

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| Monaco Editor | umbraco/umbraco-cms-backoffice-skills@umbraco-monaco-markdown-editor-action | Not relevant (Umbraco-specific) |
| localStorage/Settings | (searched) | None found relevant |
| Canvas 2D | (searched) | None found relevant |

No external skills needed — this is standard TypeScript DOM/Canvas work.

## Sources

- Existing codebase exploration (viewer/src/main.ts, renderer.ts, render-config.ts, theme-manager.ts, layers.ts, routing.ts, index.html)
- S01 summary (RenderConfig boundary contract, LOD thresholds, padNetMap)
- S03 summary (keyboard handler pattern, __viewport diagnostic, routing state)
- M003 roadmap (toolbar cleanup spec, View menu contents, Preferences scope)
