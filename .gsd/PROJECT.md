# CodeYourPCB

## Current State

**Version:** 0.2.0-beta (M004 complete 2026-03-14)
**Status:** Production-grade autorouter with multi-strategy routing, quality scoring and realtime tuning, on top of a 2D/3D renderer, routing UX and the tool stack. **Variant preview is not in this list any more**: the engine keeps `auto_route_variants()`, the panel that showed its output was deleted in `a9e8c7a`, and nothing calls either.
**Codebase:** count it rather than reading a number here:
`find crates -path "*/src/*" -name "*.rs" | xargs wc -l | tail -1` and the same
for `crates/*/tests`, `src-tauri`, `viewer/src` and `viewer/e2e`.
_Read 2026-09-05: **113648 lines of production code** - 93345 Rust under
`crates/*/src`, 393 in `src-tauri`, 19910 TypeScript in `viewer/src` - and
**72120 lines of tests** - 59300, 8123 and 4697. The `~44,000` that stood here
was written in March and was never a total of anything measurable today._

**What works:**
- Write .cypcb files → see board in 2D and 3D viewers with hot reload
- DSL v2: modules, typed interfaces (I2C, SPI, Power), physical units (23 variants), constraint assertions
- **Production-grade autorouter** — PathFinder negotiated congestion + ImprovedAStar, multi-strategy with empirical selection. PathFinder beats A* 3× on composite score.
- **Routing quality scoring** — 7-metric RoutingScore (trace length, via count, DRC violations, smoothness, crossings, layer balance, composite). CLI `cypcb score` command.
- **Trace smoother** — 3-pass post-processor (staircase collapse, corner chamfer, collinear merge) producing clean 45°/90° geometry (smoothness=1.000)
- **Realtime tuning** — 4 sliders (Via Cost, Layer Preference, Roundness, Density) trigger debounced WASM re-routing. Parameters persist in settings.
- **Variant generation** — Route button generates 3-4 variants with different strategies/params, auto-applies best, hover preview shows alternatives as ghost overlay on canvas
- **KiCad .kicad_pcb parser** — parses KiCad 5-8 format into BoardWorld. 3 benchmark fixtures. CLI `cypcb parse-kicad` command.
- **Automated benchmark suite** — CI regression gate (composite ≤ 5501, DRC ≤ 5, smoothness ≥ 0.95), full matrix comparison, screenshot artifacts
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
- 127 Vitest unit tests + 108 Playwright E2E tests (all passing)
- Professional 2D renderer with LOD, per-pad net highlighting, component body outlines, pad numbers, net labels, drill marks
- Routing UX: net-aware target pad highlighting, ratsnest guide, magnetic snap to destination, angle constraint toggle (A key), keyboard handlers (Escape/F/A)
- Clean toolbar with essential tools only; View dropdown for layer/grid/ratsnest/net-labels; Preferences modal for theme/units/grid/colors
- Unit system (mm/mil/µm) with formatDimension/parseUserDimension wired to all display sites
- Settings persistence (localStorage) with typed get/set/subscribe API

**Next milestone:** M005 — WASM Routing Off Main Thread (Web Worker, quality fix, E2E tests)

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
- DRC violations at 3 (not zero) on the led_blink benchmark, and they are real: a trace 0.07mm from a foreign pad, a trace-to-trace overlap and a trace-to-via overlap, all between different nets. The earlier "grid-boundary artifacts, not crossing traces" note was wrong — it was written when the count was 5, measured on a board the router had abandoned partway. See `crates/cypcb-autoroute/tests/drc_report.rs` for the per-violation dump.
- Benchmark fixtures are synthetic (not real downloaded KiCad projects) — functionally equivalent but lack real-world edge case coverage
- Variant click-to-apply is display-only (doesn't re-route with clicked config) — hover preview works correctly

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
- ✅ E2E test suite and quality gate. **Both figures that stood here were wrong**: `npx playwright test --list` reports **141 tests in 32 files**, not 41, and `scripts/quality-gate.sh` runs **10 stages**, not 8 - its own output numbers them `[1/10]` to `[10/10]`.
- ✅ Performance. **The two numbers that stood here are from March and nothing has re-measured them**, so they are not repeated. The autorouter's current figures come from the gate's own stage 8, `[8/10] autorouter benchmark`; the page load has no measurement in this repository at all.

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

## Completed Milestone: M004 "Production-Grade Autorouter" ✅

**Goal:** Replace prototype A* autorouter with production-grade, empirically-validated routing engine.

**Delivered:**
- ✅ S01: KiCad .kicad_pcb parser (KiCad 5-8), 3 benchmark fixtures, CLI parse-kicad command, ratsnest compatibility
- ✅ S02: 7-metric routing quality scoring system (RoutingScore), CLI score command, baseline scores established
- ✅ S03: PathFinder negotiated congestion router beating ImprovedAStar 3× (composite 5001 vs 15544), RoutingStrategy trait
- ✅ S04: 3-pass trace smoother (smoothness=1.000), via optimizer, DRC non-regression proven
- ✅ S05: Realtime tuning — 4 sliders with 300ms debounced WASM re-routing, AutorouteParams struct
- ✅ S06: Variant generation — 3-4 variants per route, ranked score panel, hover ghost preview on canvas
- ✅ S07: Automated benchmark suite — CI regression gate, strategy comparison, screenshot artifacts

## Milestone Sequence

- [x] M001: CodeYourPCB v1.0 + v1.1 — Full stack PCB design tool
- [x] M002: Infrastructure & Engine — Autorouter, 3D, DSL v2, test suite
- [x] M003: From Prototype to Tool — Professional board view & UX
- [x] M004: Production-Grade Autorouter — Multi-strategy routing, quality scoring, realtime tuning, variant preview, benchmark validation
- [ ] M005: WASM Routing Off Main Thread — Web Worker, routing quality fix, E2E regression tests

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
- ✅ Comprehensive DRC - v2.0. The rules are registered in one list, so count them rather than trusting this line: `grep -c "Box::new(rules::" crates/cypcb-drc/src/lib.rs` (15 on 2026-08-06, and the line it replaced still said 12).
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
- ✅ KiCad .kicad_pcb board parser (KiCad 5-8) — M004
- ✅ Routing quality scoring system (7 metrics + composite) — M004
- ✅ Multi-strategy routing engine (PathFinder + ImprovedAStar) — M004
- ✅ Negotiated congestion with rip-up/reroute — M004
- ✅ Strategic via placement — M004
- ✅ Clean 45°/90° trace geometry (smoothness=1.000) — M004
- ✅ Trace smoothing post-processor (3-pass) — M004
- ✅ Realtime tuning parameters (4 sliders, debounced re-routing) — M004
- ✅ Reactive re-routing on parameter change — M004
- ✅ Routing variant generation (3-4 variants) — M004
- ✅ Auto-apply best variant with hover preview — M004
- ✅ Benchmark validation with CI regression gate — M004
- ✅ Visual comparison via screenshot artifacts — M004
- ✅ Empirical strategy selection (PathFinder confirmed) — M004

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

**Competitive Position (as of M004):**
- Strongest areas: standalone platform (no KiCad dependency), **production-grade autorouter with multi-strategy competition and quality scoring**, web+desktop, collaboration-friendly
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
*Last updated: 2026-03-14 after completing M004 (Production-Grade Autorouter — all 7 slices, 14 requirements validated)*
