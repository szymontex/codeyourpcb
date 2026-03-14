# M002: CodeYourPCB v2.0 — Professional EDA Platform — Context

**Gathered:** 2026-03-13
**Status:** Ready for planning

## Project Description

CodeYourPCB is a standalone, code-first PCB design tool. v1.0+v1.1 proved the concept: DSL → parser → board model → renderer → DRC → export → autorouting → IDE integration, running in browser and desktop. v2.0 transforms it from a working prototype into a professional EDA platform that competes with and surpasses atopile, KiCad, and commercial tools.

## Why This Milestone

Competition exists and is active. atopile has constraint solver, module system, typed interfaces, physical units, auto-picking from LCSC, package registry, VS Code extension, and MCP server. diodeinc/pcb has a Rust-based approach with KiCad integration, MCP server, and starlark-based DSL. circuit-synth wraps KiCad with Python. None of them are fully standalone — that remains our differentiator.

The current autorouter is a FreeRouting JAR wrapper — opaque, slow, uncontrollable. A custom autorouter is the single biggest technical differentiator we can build. Combined with a 3D viewer, advanced DSL features, and professional UI polish, this makes CodeYourPCB the tool that senior PCB engineers would actually switch to.

## User-Visible Outcome

### When this milestone is complete, the user can:

- Write `.cypcb` files with modules, typed interfaces, physical units, and constraints
- See their board in both 2D and interactive 3D views with JLCPCB 3D models
- Run a custom autorouter that produces professional-quality routing with constraint awareness
- Manually edit traces in the viewer (click-drag routing like KiCad)
- Run comprehensive E2E tests that prove every user flow works
- Experience a polished, responsive UI that feels like a 2030-era tool

### Entry point / environment

- Entry point: `http://localhost:5173` (dev), deployed web URL, or Tauri desktop app
- Environment: browser (Chrome/Firefox/Safari/Edge) or native desktop (Win/Mac/Linux)
- Live dependencies: none (fully standalone)

## Completion Class

- Contract complete means: E2E tests cover every user action, DRC rules verified, export validated, autorouter tested against reference boards
- Integration complete means: DSL → parse → model → render (2D+3D) → DRC → autoroute → export pipeline works end-to-end
- Operational complete means: web app loads <3s, desktop <1s startup, autorouter handles 500+ component boards in <30s

## Final Integrated Acceptance

To call this milestone complete, we must prove:

- A real-world board (Arduino shield or similar) routes successfully through our custom autorouter and produces valid Gerbers
- 3D viewer renders the board with component models loaded from JLCPCB
- Module/import system lets user compose reusable circuit blocks
- Every UI interaction is exercised by automated tests with screenshots
- Performance benchmarks pass on target hardware

## Risks and Unknowns

- Custom autorouter quality — can we match FreeRouting quality? Risk retired by S01 (A* prototype on real boards)
- 3D model loading from JLCPCB — API availability and format compatibility
- DSL complexity — adding modules/constraints without breaking existing syntax
- WASM performance for autorouter — may need web worker isolation
- Browser WebGL/WebGPU compatibility for 3D viewer

## Existing Codebase / Prior Art

- `crates/cypcb-router/` — current FreeRouting wrapper (DSN export, SES import)
- `crates/cypcb-render/` — WASM board snapshot bridge
- `crates/cypcb-parser/` — Tree-sitter grammar and AST
- `crates/cypcb-world/` — ECS board model with spatial index
- `crates/cypcb-drc/` — DRC engine
- `viewer/src/renderer.ts` — Canvas 2D renderer
- `/workspace/competitors/atopile/` — reference for DSL features, 3D viewer, constraint solver
- `/workspace/competitors/pcb/` — reference for Rust PCB tooling patterns
- `.gsd/research/` — existing architecture, features, pitfalls, stack research

## Scope

### In Scope

- Massive PCB design knowledge collection (IPC standards, textbooks, Reddit community tips, manufacturer docs)
- Clone and analyze KiCad, LibrePCB, Horizon EDA repos in /workspace/competitors/
- Deep research of closed-source tools via manuals/tutorials (Altium, Allegro, OrCAD, EAGLE)
- Custom autorouter (A*, constraint-aware, multi-layer) informed by knowledge base
- 3D board viewer (Three.js/WebGPU with JLCPCB models)
- DSL v2: modules, typed interfaces, physical units, constraints
- Manual trace editing in viewer (KiCad-style click-drag routing)
- Professional UI/visual polish — match or exceed atopile's PCB prototype display quality
- Comprehensive E2E test suite with screenshots, click simulation, error triggering, web reliability
- Competition feature parity — no feature any tool has should be unknown to us
- PCB design knowledge base encoded as structured Rust rule sets
- License audit and ATTRIBUTION.md for any borrowed patterns

### Out of Scope / Non-Goals

- Schematic capture (deferred to v3)
- Real-time collaboration
- Component marketplace
- ngspice simulation integration
- Mobile app
- Manufacturing ordering integration

## Technical Constraints

- Rust for core logic (parser, board model, DRC, autorouter, export)
- TypeScript for frontend (viewer, editor, UI)
- WASM for browser deployment
- Must maintain backward compatibility with existing .cypcb files
- No git commits mentioning AI/claude/gsd — author: szymontex <szymontex@gmail.com>
- All competition analysis stays in /workspace/competitors/
- Performance: autorouter <30s for 500-component boards, 3D render at 60fps
- No questionable/dubious content goes into the repo — clean, professional code only
- License compliance: if any pattern resembles competitor code, add attribution in ATTRIBUTION.md with license reference
- Every task must be preceded by relevant knowledge base research before implementation
- Agent has CTO authority on architectural decisions — choose the direction that best serves the project
- File and document organization must be exemplary — clear naming, logical structure, no orphan files
- Closed-source tool research (Altium manuals, Allegro tutorials, OrCAD guides) informs feature design

## Integration Points

- JLCPCB API — 3D model downloads, component catalog
- Tree-sitter — grammar extensions for DSL v2
- Three.js — 3D rendering engine
- Web Workers — autorouter isolation for non-blocking UI
