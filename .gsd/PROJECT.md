# CodeYourPCB

## Current State

**Version:** 0.1.0-beta (M003 complete 2026-03-13)
**Status:** Professional-grade user-facing quality — 2D renderer with pad numbers/net labels/layer colors, working 3D view, routing UX with magnetic snap, clean UI with preferences, project manager, JLCPCB search
**Codebase:** ~39,300 lines (Rust + TypeScript)

**What works:**
- Write .cypcb files → see board in 2D and 3D viewers with hot reload
- DSL v2: modules, typed interfaces (I2C, SPI, Power), physical units (23 variants), constraint assertions
- Custom A*-based autorouter (500-component board in 0.05s, multi-layer, constraint-aware)
- Manual trace editing with click-drag routing, angle snapping, and live DRC feedback
- 3D board viewer (Three.js) with procedural component bodies, orbit/zoom/pan, layer visibility — renders real geometry (components, pads, traces, vias) with EasyEDA OBJ model loading pipeline
- Grid snap, command-pattern undo/redo, net highlighting, component rotation, board outline resize
- Automatic DRC with visual violation markers
- Export manufacturing files (Gerber X2, Excellon, BOM, CPL) verified with JLCPCB
- LSP for IDE integration (VS Code, any LSP-compatible editor) with v2 keyword support
- KiCad footprint library import, multi-source library management
- Monaco code editor with syntax highlighting and LSP bridge
- Tauri v2 desktop application (native menus, file dialogs, installer)
- Web deployment (Cloudflare Pages, File System Access API, URL sharing)
- 8-stage quality gate: cargo fmt, clippy, cargo test, eslint, vitest, playwright, autorouter benchmark, jscpd
- 127 Vitest unit tests + 94 Playwright E2E tests (all passing)
- Professional 2D renderer with LOD, per-pad net highlighting, component body outlines, pad numbers, net labels, drill marks
- Routing UX: net-aware target pad highlighting, ratsnest guide, magnetic snap to destination, angle constraint toggle (A key), keyboard handlers (Escape/F/A)
- Clean toolbar with essential tools only; View dropdown for layer/grid/ratsnest/net-labels; Preferences modal for theme/units/grid/colors
- Unit system (mm/mil/µm) with formatDimension/parseUserDimension wired to all display sites
- Settings persistence (localStorage) with typed get/set/subscribe API

**Active milestone:** M004 — Production-Grade Autorouter (S01 ✅, S02 ✅, S03 ✅, S04 ✅, S05 ✅, S06 ✅, S07 remaining)

**Known tech debt / deferred items:**
- DSL v2 constructs are parse-only — no module instantiation, import resolution, or constraint evaluation
- 3D viewer uses procedural component bodies by default — EasyEDA OBJ model loading available via JLCPCB search panel (CORS-limited from localhost)
- Desktop crates excluded from quality gates (require system deps unavailable in CI)
- Board outline editing is rectangle-only (polygon editing deferred)
- Copper fill zones not rendered (no Zone type in ECS data model)
- Silkscreen uses rectangular body outlines (real KiCad silkscreen has complex curves/text)
- Library management still needs depth (JLCPCB search exists but no "add to library" flow)
- Pre-existing E2E flake in errors.spec.ts:102 ("Ready" vs "Reloaded" status race) — stable in current runs
- ThemeManager has separate 'theme' localStorage key from settings 'cypcb-settings' key (by design for FART prevention)

## Completed Milestone: v1.0 + v1.1 "Full Stack PCB Design Tool" ✅

**Goal:** Prove the code-first PCB design concept end-to-end and build desktop/web deployment foundation.

**Delivered:**
- ✅ Custom DSL with Tree-sitter grammar, parser, and LSP
- ✅ ECS board model with spatial indexing
- ✅ 2D Canvas renderer with hot reload
- ✅ DRC engine, Gerber/Excellon/BOM export
- ✅ FreeRouting autorouter integration
- ✅ Multi-source library management (KiCad, JLCPCB, custom)
- ✅ Platform abstraction layer (FileSystem, Dialog, Storage, Menu traits)
- ✅ Dark mode and polished UI/UX with WCAG AA compliance
- ✅ Tauri v2 desktop application
- ✅ Web deployment (Cloudflare Pages, File System Access API, URL sharing)
- ✅ Embedded Monaco code editor with LSP bridge
- ✅ Comprehensive user documentation

## Completed Milestone: M002 "Infrastructure & Engine" ✅

**Goal:** Build core engine infrastructure: autorouter, 3D framework, DSL v2, test suite.

**Delivered (backend/engine — user-facing quality still prototype):**
- ✅ Custom autorouter (A*-based, constraint-aware, multi-layer, 0.05s for 500 components)
- ✅ 3D framework (Three.js lazy-loaded, geometry pipeline built — but renders empty in practice)
- ✅ Manual trace editing skeleton in 2D viewer with live DRC feedback
- ✅ DSL v2: modules, typed interfaces, physical units (23 variants), constraints (parse-level)
- ✅ Undo/redo, net highlighting, component rotation, board resize infrastructure
- ✅ PCB design rule database (IPC standards, manufacturer presets)
- ✅ E2E test suite: 41 tests, 8-stage quality gate
- ✅ Performance: autorouter 0.05s/500 components, web load 105ms

## Completed Milestone: M003 "From Prototype to Tool" ✅

**Goal:** Every user-facing surface upgraded from prototype to professional quality.

**Delivered:**
- ✅ S01: Professional 2D renderer (pad numbers, net labels, layer colors, refdes, LOD, per-pad highlighting)
- ✅ S02: 3D view fix — component bodies, pads, traces, vias render correctly; GLTFLoader pipeline ready
- ✅ S03: Routing UX — net-aware target highlighting, ratsnest guide, magnetic snap, angle constraint toggle
- ✅ S04: UI architecture — clean toolbar, View dropdown, Preferences modal, unit system (mm/mil/µm), settings persistence
- ✅ S05: Project manager — startup overlay with 3 templates + blank board, recent files with thumbnails, editor→board reflow
- ✅ S06: JLCPCB integration — component search via jlcsearch API, EasyEDA OBJ 3D model pipeline, search panel UI
- ✅ S07: Polish & verification — quality gate clean, version 0.1.0-beta, JLCPCB error handling, 94 E2E tests

## Milestone Sequence

- [x] M001: CodeYourPCB v1.0 + v1.1 — Full stack PCB design tool
- [x] M002: Infrastructure & Engine — Autorouter, 3D, DSL v2, test suite
- [x] M003: From Prototype to Tool — Professional board view & UX
- [ ] M004: Production-Grade Autorouter — Multi-strategy, scored, realtime-tunable (S01 ✅, S02 ✅, S03 ✅, S04 ✅, S05 ✅, S06 ✅)
- [ ] M005: PCB Renderer Upgrade — KiCad/Atopile visual standard

## What This Is

A code-first PCB design tool where you write code and it generates circuit boards. Instead of clicking in a GUI and getting XML as a side effect, you write declarative code that defines components, connections, and constraints — the visual representation is computed from this source of truth. Designed for engineers who want git-friendly collaboration, AI/LLM-assisted editing, and deterministic builds.

## Core Value

**The source file is the design.** A human-readable, git-diffable, LLM-editable PCB project file that always produces the same board. If the file is clear enough for Claude to edit, it's clear enough for anyone.

## Requirements

### Validated

- ✅ Custom DSL parser (Tree-sitter grammar) — v1.0
- ✅ Board data model (components, nets, layers, spatial indexing) — v1.0
- ✅ 2D board view renderer (Canvas) with hot reload — v1.0
- ✅ Component placement (absolute and relative) — v1.0
- ✅ Net connections with constraints (width, clearance, current) — v1.0
- ✅ Comprehensive DRC (clearance, trace width, drill size, connectivity) — v1.0
- ✅ Gerber X2 export (all layers) — v1.0
- ✅ Excellon drill file export — v1.0
- ✅ BOM and pick-and-place file generation — v1.0
- ✅ Autorouter integration (FreeRouting DSN/SES → custom A* in v2.0) — v1.0/v2.0
- ✅ Trace width calculator (IPC-2221) — v1.0
- ✅ LSP server for IDE integration — v1.0, extended with v2 keywords in v2.0
- ✅ KiCad footprint import (.kicad_mod) — v1.0
- ✅ Web-based viewer with zoom/pan/selection — v1.0
- ✅ Touchpad navigation controls — v1.0
- ✅ File picker with drag-and-drop — v1.0
- ✅ Multi-source library management (KiCad, JLCPCB, custom) — v1.1
- ✅ 3D model handling for components — v1.1
- ✅ Dark mode theme system — v1.1
- ✅ Tauri desktop application — v1.1
- ✅ Web deployment infrastructure — v1.1
- ✅ Embedded Monaco code editor — v1.1
- ✅ Comprehensive documentation — v1.1
- ✅ 3D board preview (Three.js) — v2.0
- ✅ Undo/redo system — v2.0
- ✅ Custom autorouter (A*-based, constraint-aware) — v2.0
- ✅ Manual trace editing — v2.0
- ✅ DSL v2 modules/interfaces/units/constraints (parse-level) — v2.0
- ✅ Grid snap — v2.0
- ✅ Net highlighting — v2.0
- ✅ Component rotation UI — v2.0
- ✅ Board outline resize — v2.0
- ✅ E2E test suite and quality gates — v2.0
- ✅ Performance benchmarks (autorouter, web load, 3D FPS) — v2.0

### Deferred

- ✅ Project templates — 3 bundled templates + blank scaffold, PM overlay on startup (M003/S05)
- [ ] DSL v2 semantic evaluation (module instantiation, import resolution, constraint enforcement)
- [ ] JLCPCB 3D model loading from production (CORS-limited from localhost, pipeline built)
- [ ] Supplier API integration depth (JLCPCB search built, LCSC/Mouser not yet)

### Future (v3.0+)

- [ ] Schematic view generation
- [ ] ngspice simulation integration
- [ ] IPC-2581 export
- [ ] WASM plugin system
- [ ] Impedance calculator (microstrip, stripline)
- [ ] Differential pair routing
- [ ] Length matching

### Out of Scope

- **Mobile app** — Desktop/web first, mobile adds complexity without core value
- **Real-time collaboration** — Git-based workflow is the collaboration model
- **Component marketplace** — Use existing libraries (KiCad, etc.)
- **Manufacturing ordering** — Export files, let user choose fab
- **Training custom AI models** — Use existing LLMs, focus on file format clarity
- **GUI schematic capture** — Code-first is the identity; competing on schematic GUI dilutes focus

## Context

**Problem:**
Current PCB tools (KiCad, Eagle, Altium) are GUI-first. The project file is a binary or XML blob that's a side effect of clicking. This makes:
- Git collaboration painful (meaningless diffs, merge hell)
- AI/LLM assistance nearly impossible (can't edit XML blobs)
- Automation difficult (scripting is afterthought)
- Reproducibility uncertain (same project, different tool version = different output?)

**User:**
- Engineers who code (comfortable with text files, git, CLI)
- Teams wanting proper version control on hardware designs
- Anyone wanting to leverage LLMs for PCB design assistance

**Competitive Position (as of v2.0):**
- Strongest areas: standalone platform (no KiCad dependency), built-in autorouter, web+desktop, collaboration-friendly
- Weakest area: library management depth (JLCPCB search exists but no "add to library" flow — still #1 adoption blocker per feature matrix)
- Feature matrix covers 9 EDA tools (atopile, KiCad, Altium, Allegro, OrCAD, EAGLE, EasyEDA, Flux.ai, diodeinc/pcb) across 11 categories

## Constraints

- **Language:** Rust (performance, safety, WASM compilation, 30+ year longevity)
- **Platform:** Web-first (WASM), with Tauri for desktop standalone
- **Rendering:** Canvas for 2D, Three.js for 3D
- **Parser:** Tree-sitter (incremental, error-tolerant, LSP-ready)
- **Performance:** Autorouter <30s for 500 components (actual: 0.05s), web load <3s (actual: 105ms), 3D at 60fps
- **Determinism:** Same source file = identical output, always
- **Compatibility:** Export to industry standard formats (Gerber, Excellon, BOM, CPL)

---
*Last updated: 2026-03-14 after completing M004/S06*
