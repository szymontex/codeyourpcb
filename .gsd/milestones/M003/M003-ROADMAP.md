# M003: From Prototype to Tool — Professional Board View & UX

**Vision:** Close the gap between "engine works" and "tool I'd actually use". Every user-facing surface — 2D view, 3D view, routing, UI, project management — upgraded from prototype to professional quality. When done, a KiCad user opens CodeYourPCB and thinks "this looks right".

## Success Criteria

- 2D board view renders pad shapes with numbers, net labels on traces, readable refdes, proper per-layer colors (red top / blue bottom / yellow silkscreen), copper fills, drill marks — visually comparable to KiCad at equivalent zoom levels
- 3D view shows traces, pads, vias, and component bodies (not an empty green board)
- User can route a trace pad-to-pad with: target pad highlighting, magnetic snap to destination, ratsnest as guide, and angle constraint as optional toggle (not forced)
- Preferences panel allows setting: theme, default units (mm/mils/µm), grid spacing, layer colors
- Unit display works throughout the UI — coordinates, dimensions, grid spacing all respect the selected unit
- Project manager provides: recent files, new project, import .cypcb, at least one starter template
- JLCPCB/LCSC component search returns results with part metadata and 3D model availability
- Toolbar contains only essential tools; layer visibility, grid, ratsnest moved to View menu/panel
- All M002 UI bugs fixed: theme single-click, grid toggle functional, fit icon readable
- E2E tests cover new 2D renderer output, 3D visibility, routing flow, preferences persistence
- 8-stage quality gate passes (extended with new test coverage)

## Key Risks / Unknowns

- Professional 2D renderer performance — rendering pad numbers + net labels + refdes for 500+ components on Canvas may hit performance limits; zoom-dependent LOD (level of detail) may be necessary
- 3D empty board root cause — unknown whether it's a data pipeline issue, coordinate transform, or rendering bug; must diagnose before fixing
- JLCPCB API access — public API availability, rate limits, auth requirements, CORS for browser use
- 3D model format pipeline — STEP→GLB conversion for web display; may need server-side conversion or client-side STEP parser

## Proof Strategy

- 2D renderer quality → retire in S01 by rendering blink.cypcb with all visual elements and comparing screenshot against KiCad rendering of equivalent board
- 3D empty board → retire in S02 by diagnosing root cause and rendering blink.cypcb with visible traces and components in 3D
- Routing UX → retire in S03 by routing a 10-net board pad-to-pad with net highlighting and magnetic snap, verified by E2E test
- JLCPCB API → retire in S06 by searching "0805 10k" and receiving component results with LCSC part numbers

## Verification Classes

- Contract verification: Rust unit tests, TypeScript unit tests, Playwright E2E screenshot comparisons for renderer output
- Integration verification: full pipeline (DSL → parse → model → professional 2D render + 3D render), JLCPCB API → component display → 3D model load
- Operational verification: preferences persist across page reload, unit switching updates all UI elements, project list persists
- UAT / human verification: visual comparison of 2D output against KiCad reference, 3D model appearance, routing flow smoothness

## Milestone Definition of Done

This milestone is complete only when all are true:

- 2D board view passes visual comparison against KiCad reference (pad numbers, net labels, layer colors, refdes all visible)
- 3D view renders traces, components, and vias for reference board
- Routing flow verified by E2E test: route from pad A to pad B with net highlight and magnetic snap
- Preferences panel opens, unit change persists and propagates to all UI dimensions
- Project manager lists recent files and can create new project from template
- JLCPCB search returns results for common components
- Toolbar is clean, View menu/panel contains layer/grid/ratsnest controls
- All M002 UI bugs verified fixed
- E2E test suite extended with new feature coverage
- 8-stage quality gate passes

## Slices

- [x] **S01: Professional 2D Board Renderer** `risk:high` `depends:[]`
  > After this: user sees blink.cypcb with KiCad-quality visuals — pad shapes with pin numbers, net labels on traces, readable refdes text, per-layer colors (red top, blue bottom, yellow silkscreen), copper fill areas, drill hole marks, zoom-dependent detail levels

- [x] **S02: 3D View Fix & Component Rendering** `risk:high` `depends:[]`
  > After this: user toggles to 3D and sees the board with copper traces rendered as ribbons, pads as metallic shapes, vias as cylinders, and component bodies with correct dimensions — not an empty green board

- [ ] **S03: Routing UX Upgrade** `risk:high` `depends:[S01]`
  > After this: user clicks a pad to start routing and sees target pads for that net highlighted, ratsnest line guiding to nearest unconnected pad, magnetic snap when approaching destination, with angle constraint as a toggleable option (not forced) — verified by E2E test routing a complete net

- [ ] **S04: UI Architecture — Toolbar, View Menu & Settings** `risk:medium` `depends:[S01]`
  > After this: toolbar has only essential tools (select, route, measure, undo/redo, 2D/3D, editor); View menu/panel controls layers, grid, ratsnest, net labels; Preferences panel sets theme (single-click fix), units, grid spacing, layer colors — all settings persist to localStorage

- [ ] **S05: Project Manager & File Handling** `risk:medium` `depends:[S04]`
  > After this: app opens to project manager showing recent files with thumbnails; user can create new project from template, import existing .cypcb, or open recent; editor changes trigger board view update/reflow

- [ ] **S06: JLCPCB Integration & 3D Models** `risk:medium` `depends:[S02,S04]`
  > After this: user searches JLCPCB/LCSC catalog from within the app, sees part metadata (price, stock, datasheet), and components with LCSC part numbers auto-load 3D GLB models in the 3D view

- [ ] **S07: Polish, Bugs & Verification** `risk:low` `depends:[S03,S04,S05,S06]`
  > After this: all UI bugs from feedback list verified fixed, E2E tests cover new features (renderer output, 3D visibility, routing flow, preferences, project manager), quality gate extended and passing, version naming corrected to beta

## Boundary Map

### S01 → S03

Produces:
- Professional 2D renderer with layer-color-aware drawing, pad shape rendering with pin numbers, net label rendering, zoom-dependent LOD system
- `RenderConfig` type with layer colors, font sizes, LOD thresholds
- Updated `drawComponent()`, `drawPad()`, `drawTrace()` methods that accept render config

Consumes:
- Existing `BoardSnapshot` types (unchanged)
- Existing Canvas rendering infrastructure

### S01 → S04

Produces:
- `RenderConfig` with layer color customization hooks
- Understanding of what settings the renderer needs (feeds into Preferences panel design)

Consumes:
- nothing (first slice)

### S02 → S06

Produces:
- Working 3D pipeline that can render traces + component bodies from snapshot
- `loadComponentModel(url: string)` method on Renderer3D for loading GLB models
- Confirmed Three.js GLTFLoader integration

Consumes:
- Existing Three.js infrastructure in renderer3d.ts

### S03 → S07

Produces:
- Routing interaction with net highlighting, magnetic snap, angle toggle
- Testable routing state machine (start→guide→snap→place) for E2E verification

Consumes:
- S01 professional renderer (net labels, pad highlighting capabilities)

### S04 → S05

Produces:
- UI architecture (View menu/panel, Preferences panel with persistence)
- Settings API (`getPreference(key)`, `setPreference(key, value)`)
- Unit display system (`formatDimension(nm, unit)` → "2.54mm" / "100mil" / "2540µm")

Consumes:
- S01 RenderConfig (preferences drive layer colors, font sizes)

### S04 → S06

Produces:
- UI panel infrastructure (side panels, search panels) that JLCPCB search can slot into
- Settings persistence layer

Consumes:
- S01 render config

### S05 → S07

Produces:
- Project manager with recent files, templates, import flow
- Editor↔view sync (code changes trigger board reflow)

Consumes:
- S04 UI architecture, settings API

### S06 → S07

Produces:
- JLCPCB search integration with component results
- 3D model loading from JLCPCB URLs
- BOM cost estimation data

Consumes:
- S02 working 3D pipeline with model loading
- S04 UI panel infrastructure
