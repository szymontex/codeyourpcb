# M003: From Prototype to Tool — Professional Board View & UX

**Gathered:** 2026-03-13
**Status:** Ready for planning

## Project Description

CodeYourPCB is a standalone code-first PCB design tool. The engine and infrastructure are built (autorouter, parser, DRC, export, 3D framework, test suite). But the user-facing experience is prototype-grade. The 2D board view looks like a student project. The 3D view shows an empty green board. The routing UX forces angles with no guidance. The UI is cluttered. There's no project management, no preferences, no unit switching. This milestone closes the gap between "engine works" and "tool I'd actually use".

## Why This Milestone

User feedback after M002 was clear: the infrastructure is there but the product isn't. Specific pain points:

1. **2D board view is toylike** — no pad numbers, no net labels, no refdes readable at zoom, no professional layer colors. KiCad/atopile look 10x better.
2. **3D view is broken** — renders empty green board, no traces or components visible.
3. **Routing UX is unusable** — forces angle constraints, no net-aware hints, no magnetic pads, no routing guidance.
4. **UI is cluttered** — toolbar has too many items, no View menu, no preferences panel.
5. **No project management** — Open = raw file picker, no recent projects, no templates.
6. **Missing integrations** — no JLCPCB component search, no 3D model import, no unit switching.
7. **Bugs** — theme requires double-click, grid checkbox does nothing, fit icon unreadable.

This is M003 because the previous milestone naming was overambitious. This is still beta — the product version will come after this milestone makes the tool genuinely usable.

## User-Visible Outcome

### When this milestone is complete, the user can:

- Open CodeYourPCB and see a board rendered with professional KiCad-quality visuals (pad shapes with numbers, net labels, refdes, layer colors, copper fills)
- Toggle to 3D and see the board with traces, pads, vias, and component bodies rendered
- Route traces with net-aware guidance: highlighted target pads, magnetic snap to destinations, ratsnest as guide, optional angle constraint (not forced)
- Manage projects: recent files, new project, import existing .cypcb, templates
- Open preferences to set theme, default units (mm/mils/µm), grid spacing, layer colors
- See a clean toolbar with only essential tools; layers/grid/ratsnest in a View menu
- Search JLCPCB/LCSC for components, auto-download 3D models per part number
- Switch display units between mm, mils, and µm throughout the UI

### Entry point / environment

- Entry point: `http://localhost:5173` (dev), deployed web URL, or Tauri desktop app
- Environment: browser (Chrome/Firefox/Safari/Edge) or native desktop (Win/Mac/Linux)
- Live dependencies: JLCPCB/LCSC API for component search (optional, graceful degradation)

## Completion Class

- Contract complete means: E2E tests cover new renderer output, routing UX flows, settings persistence, project management actions
- Integration complete means: DSL → parse → model → professional 2D render + working 3D render, JLCPCB search → model download → 3D display pipeline
- Operational complete means: UI feels responsive, theme/settings persist across sessions, unit switching works everywhere

## Final Integrated Acceptance

To call this milestone complete, we must prove:

- A reference board (blink.cypcb) renders in 2D with pad numbers, net labels, refdes, proper layer colors — visually comparable to KiCad
- The same board renders in 3D with traces, component bodies, and vias visible
- A user can route a trace from pad to pad with net-aware hints and magnetic snap, without being forced into angle constraints
- Preferences panel opens, user changes units to mils, all dimensions throughout UI update
- Project manager shows recent files, user can create new project from template
- A component search for "0805 10k" returns JLCPCB results with 3D model availability

## Risks and Unknowns

- Professional 2D renderer complexity — matching KiCad quality means zoom-dependent detail, proper text rendering, layer compositing. This is the hardest visual engineering task.
- 3D pipeline debugging — unknown why current 3D shows empty board. Could be snapshot data issue, coordinate transform bug, or rendering pipeline break.
- JLCPCB API stability — public API may have rate limits, require auth, or change format.
- Routing UX design — "net-aware guidance" is easy to describe, hard to get right. Need to study KiCad's interactive router behavior closely.

## Existing Codebase / Prior Art

- `viewer/src/renderer.ts` — current 842-line Canvas 2D renderer (basic shapes, needs major upgrade)
- `viewer/src/renderer3d.ts` — 872-line Three.js renderer (geometry pipeline exists but output is empty)
- `viewer/src/interaction.ts` — click-drag interaction system
- `viewer/src/routing.ts` — angle snap, grid snap (needs net-aware guidance)
- `viewer/src/main.ts` — 1604-line app orchestration (needs refactoring for View menu, preferences)
- `viewer/index.html` — toolbar markup (needs cleanup)
- `viewer/src/types.ts` — BoardSnapshot types (pad, trace, via, component — data model is solid)
- `crates/cypcb-render/src/snapshot.rs` — Rust side of snapshot generation
- `crates/cypcb-render/src/lib.rs` — WASM bridge
- `/workspace/competitors/` — KiCad, atopile repos for visual reference

> See `.gsd/DECISIONS.md` for all architectural and pattern decisions — it is an append-only register; read it during planning, append to it during execution.

## Scope

### In Scope

- Complete 2D renderer rewrite for professional PCB visualization
- 3D view fix and component/trace rendering
- Routing UX with net-aware guidance, magnetic pads, optional angle constraint
- UI architecture: clean toolbar, View menu, preferences panel
- Project manager (recent files, new project, import, templates)
- JLCPCB/LCSC component search integration
- 3D model import per component
- Unit system (mm/mils/µm) with display switching
- Fix all UI bugs from feedback (theme double-click, grid toggle, fit icon)
- Update E2E tests for new features
- Version naming correction (this is beta, not v2.0)

### Out of Scope / Non-Goals

- Schematic capture (out of project scope entirely)
- Real-time collaboration
- ngspice simulation
- Custom component footprint editor
- Manufacturing ordering integration
- Mobile app

## Technical Constraints

- Rust for core logic, TypeScript for frontend — unchanged
- WASM for browser deployment — unchanged
- Must maintain backward compatibility with existing .cypcb files
- Canvas 2D renderer (not WebGL for 2D — keep it simple and debuggable)
- Three.js for 3D — already integrated
- JLCPCB API calls must be proxied or CORS-handled for browser use
- No secrets in git-tracked files
- All linters must stay clean (8-stage quality gate)

## Integration Points

- JLCPCB/LCSC API — component search, 3D model URLs, part metadata
- KiCad footprint libraries — existing import, may need enhancement for 3D model paths
- Three.js — 3D rendering, GLTFLoader for model import
- localStorage — preferences persistence (theme, units, recent projects)

## Open Questions

- JLCPCB API auth — do we need an API key or is the search endpoint public? Need to research.
- 3D model format — JLCPCB provides STEP files; do we convert to GLB server-side or find a STEP→Three.js loader? GLB is simpler for web.
- Canvas text rendering performance — professional PCB view needs lots of text (pad numbers, net labels, refdes). Need to verify Canvas text doesn't kill performance at 1000+ components.
