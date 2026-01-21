# Roadmap: CodeYourPCB

**Created:** 2026-01-21
**Phases:** 6
**Target:** Code-first PCB design tool

## Phase Overview

| # | Phase | Goal | Requirements | Status |
|---|-------|------|--------------|--------|
| 1 | Foundation | Working parser and board model | 12 | Complete |
| 2 | Rendering | Visual feedback with hot reload | 7 | Complete |
| 3 | Validation | DRC prevents invalid designs | 8 | Planned |
| 4 | Export | Manufacturable output | 5 | Pending |
| 5 | Intelligence | Autorouting and IDE integration | 6 | Pending |
| 6 | Desktop | Full application experience | v2 | Pending |

---

## Phase 1: Foundation

**Goal:** Working DSL parser that produces a valid board model

**Requirements:**
- DSL-01: Custom Tree-sitter grammar for .cypcb files
- DSL-02: Board definition (size, layers, stackup)
- DSL-03: Component instantiation with footprint reference
- DSL-04: Net connections with constraint syntax
- BRD-01: Component placement (absolute and relative)
- BRD-02: Multi-layer support (2-32 layers)
- BRD-03: Net/connection tracking
- BRD-04: Board outline definition
- BRD-06: Spatial indexing (R*-tree)
- FTP-01: Basic SMD footprints (0402-2512)
- FTP-02: Basic through-hole footprints
- DEV-03: Error messages with line/column info

**Success Criteria:**
1. User can write a .cypcb file defining a board with components
2. Parser produces valid AST with error recovery
3. Board model contains all components and nets
4. CLI can parse file and output JSON representation
5. Integer nanometer coordinates throughout (no floating-point)

**Key Decisions:**
- DSL syntax design (critical - affects everything downstream)
- ECS vs traditional OOP for board model
- Coordinate system (origin, units, precision)

**Plans:** 9 plans in 5 waves

Plans:
- [x] 01-01-PLAN.md -- Project setup (workspace, crates, dependencies)
- [x] 01-02-PLAN.md -- Core types (Nm, Point, Rect, Unit)
- [x] 01-03-PLAN.md -- Tree-sitter grammar definition
- [x] 01-04-PLAN.md -- ECS components for board model
- [x] 01-05-PLAN.md -- AST types and parser implementation
- [x] 01-06-PLAN.md -- BoardWorld and spatial indexing
- [x] 01-07-PLAN.md -- Footprint library (SMD + THT)
- [x] 01-08-PLAN.md -- AST-to-ECS synchronization
- [x] 01-09-PLAN.md -- CLI with parse/check commands

---

## Phase 2: Rendering

**Goal:** Minimal UI to verify board rendering with hot reload

**Scope (MINIMAL - verification only):**
- DSL-06: Hot reload on file save
- RND-01: 2D top/bottom board view
- RND-02: Layer visibility toggle
- RND-03: Zoom/pan navigation
- RND-04: Component selection and highlighting
- DEV-04: Web-based viewer

**Deferred to later:**
- DSL-05: Module/import system (not needed for verification)
- RND-05: Net highlighting (can add later)
- RND-06: Grid snapping (grid display only, no snap)
- Dark mode (light mode default)
- Flip view (can add later)

**Success Criteria:**
1. Saving .cypcb file triggers re-render within 500ms
2. Board outline and component pads visible
3. User can zoom/pan to navigate
4. User can select components (visual highlight)
5. Layer visibility toggles work

**Key Decisions:**
- Canvas 2D for MVP (simpler than WebGL/wgpu)
- KiCad-style colors (red=top, blue=bottom)
- Light mode default
- WASM bridge via serde-wasm-bindgen

**Plans:** 9 plans in 6 waves

Plans:
- [x] 02-01-PLAN.md -- WASM crate setup (cypcb-render, BoardSnapshot)
- [x] 02-02-PLAN.md -- Frontend scaffolding (Vite, TypeScript, HTML)
- [x] 02-03-PLAN.md -- WASM build and integration (mock fallback)
- [x] 02-04-PLAN.md -- Canvas renderer with viewport transforms
- [x] 02-05-PLAN.md -- Interaction (zoom, pan, select, layer toggles)
- [x] 02-06-PLAN.md -- Hot reload (file watcher, WebSocket)
- [x] 02-07-PLAN.md -- Visual verification checkpoint
- [x] 02-08-PLAN.md -- [GAP CLOSURE] Fix WASM build (getrandom compatibility)
- [x] 02-09-PLAN.md -- [GAP CLOSURE] Enable real WASM integration

**Gap Closure Complete:**
Both gaps successfully resolved — WASM builds with feature flags, real PcbEngine integrated via adapter.

---

## Phase 3: Validation

**Goal:** DRC prevents manufacturing-invalid designs

**Requirements:**
- BRD-05: Zones and keepouts
- DRC-01: Clearance checking (trace-trace, trace-pad)
- DRC-02: Minimum trace width validation
- DRC-03: Minimum drill size validation
- DRC-04: Unconnected pin detection
- DRC-05: Real-time DRC feedback
- FTP-03: QFP/SOIC/SOT packages
- FTP-04: Custom footprint definition in DSL

**Success Criteria:**
1. DRC runs in <1s for 100-component board
2. Violations shown in renderer with markers
3. Violations listed with location and rule violated
4. All basic manufacturability rules covered
5. No false positives on valid designs

**Key Decisions:**
- Manufacturer presets (JLCPCB, PCBWay) as base rules
- Single severity level (errors only) for MVP
- DRC runs on file save (like ESLint)
- Non-invasive error display (status bar + markers)

**Plans:** 10 plans in 6 waves

Plans:
- [ ] 03-01-PLAN.md -- DRC crate setup (types, traits, violation struct)
- [ ] 03-02-PLAN.md -- IC footprints (SOIC, SOT, QFP families)
- [ ] 03-03-PLAN.md -- Manufacturer presets (JLCPCB, PCBWay rules)
- [ ] 03-04-PLAN.md -- Custom footprint DSL syntax and library registration
- [ ] 03-05-PLAN.md -- Clearance checking rule (spatial index)
- [ ] 03-06-PLAN.md -- Drill size, trace width, and connectivity rules
- [ ] 03-07-PLAN.md -- DRC integration with rendering pipeline
- [ ] 03-08-PLAN.md -- Violation display (markers, status bar, panel)
- [ ] 03-09-PLAN.md -- Visual verification checkpoint
- [ ] 03-10-PLAN.md -- Zones and keepouts (BRD-05)

**Notes:**
- DRC-02 (trace width) is a placeholder until traces exist (Phase 5)
- Plan 03-04 now includes library registration wiring
- Plan 03-10 added for zones/keepouts requirement

---

## Phase 4: Export

**Goal:** Generate files manufacturers can use

**Requirements:**
- EXP-01: Gerber X2 export (all layers)
- EXP-02: Excellon drill file export
- EXP-03: BOM generation (CSV/JSON)
- EXP-04: Pick and place file
- DEV-01: CLI interface for headless operation

**Success Criteria:**
1. Gerber files pass validation in gerbv and online viewers
2. Drill files align with Gerber copper layers
3. Files accepted by JLCPCB/PCBWay DFM check
4. BOM contains all components with values
5. CLI can export without GUI (`cypcb export project.cypcb`)

**Key Decisions:**
- Gerber X2 vs X3 features
- Output folder structure
- Manufacturer presets

---

## Phase 5: Intelligence

**Goal:** Autorouting and professional IDE experience

**Requirements:**
- FTP-05: KiCad footprint import
- DEV-02: LSP server for IDE integration
- INT-01: Autorouter integration (FreeRouting)
- INT-02: Trace width calculator (IPC-2221)
- INT-03: Electrical-aware constraints (crosstalk, impedance hints)

**Success Criteria:**
1. FreeRouting can route exported board and import results
2. LSP provides autocomplete for components and nets
3. Hover shows component/net info
4. Diagnostics appear as squiggles in editor
5. Trace width suggestions based on current requirements

**Key Decisions:**
- FreeRouting integration method (DSN export or embedded)
- LSP feature prioritization
- Constraint syntax for electrical properties

---

## Phase 6: Desktop & Polish

**Goal:** Full desktop application experience

**Requirements (v2):**
- ADV-01: 3D board preview
- DSK-01: Tauri desktop application
- DSK-02: Native file dialogs
- DSK-03: Undo/redo system
- Additional v2 features as prioritized

**Success Criteria:**
1. Native app launches in <2s
2. 3D preview shows component heights and board thickness
3. Undo/redo works for all editing operations
4. File open/save uses native dialogs
5. App works offline

---

## Dependency Graph

```
Phase 1 (Foundation)
    |
    +------------------+
    v                  v
Phase 2 (Rendering)  Phase 3 (Validation)
    |                  |
    +--------+---------+
             v
      Phase 4 (Export)
             |
             v
      Phase 5 (Intelligence)
             |
             v
      Phase 6 (Desktop)
```

**Notes:**
- Phase 2 and 3 can run in parallel after Phase 1
- Phase 4 requires both rendering (visual verification) and validation (DRC pass)
- Phase 5 builds on complete core functionality
- Phase 6 is polish and can be started earlier for basic Tauri shell

---

## Risk Mitigation

| Risk | Phase | Mitigation |
|------|-------|------------|
| DSL syntax lock-in | 1 | Version grammar, dogfood extensively |
| Floating-point precision | 1 | Integer nanometers from start |
| Gerber edge cases | 4 | Test with multiple viewers + fabs |
| FreeRouting determinism | 5 | Verify early, patch if needed |
| Performance at scale | 3 | Benchmark 1K+ component boards |

---

*Roadmap created: 2026-01-21*
*Last updated: 2026-01-21 - Phase 3 revised (10 plans in 6 waves, added zones/keepouts)*
