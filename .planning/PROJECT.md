# CodeYourPCB

## Current State

**Version:** v1.0 (shipped 2026-01-29)
**Status:** Production-ready for basic PCB design workflows
**Codebase:** 32,440 lines (30,005 Rust + 2,435 TypeScript)

**What works:**
- Write .cypcb files → see board in web viewer with hot reload
- Automatic DRC with visual violation markers
- Export manufacturing files (Gerber X2, Excellon, BOM, CPL) verified with JLCPCB
- Autoroute with FreeRouting, import traces, see ratsnest
- LSP for IDE integration (VS Code, any LSP-compatible editor)
- KiCad footprint library import
- Alternative navigation (touchpad pan/zoom for laptops)
- File picker with drag-and-drop

**Known tech debt:**
- Phase 3 (Validation) missing formal verification documentation (functionality working)
- Module/import system deferred to v2
- Grid snapping deferred (grid display works)
- Net highlighting deferred

## Current Milestone: v1.1 "Foundation & Desktop"

**Goal:** Build a solid foundation for library management, project organization, and professional desktop experience with web deployment.

**Target features:**
- Multi-source component library management (KiCad, JLCPCB, custom libraries)
- Standardized project structure with intelligent organization
- Dark mode and polished UI/UX
- Tauri desktop application (native, installable)
- Web deployment (browser-based, shareable)
- Embedded code editor (Monaco integration)
- Comprehensive user documentation

**Philosophy:** Focus on groundwork that prevents future problems. "Idiot-proof" library handling, sensible defaults, flexible structure.

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
- ✅ Autorouter integration (FreeRouting DSN/SES) — v1.0
- ✅ Trace width calculator (IPC-2221) — v1.0
- ✅ LSP server for IDE integration (hover, completion, diagnostics, goto) — v1.0
- ✅ KiCad footprint import (.kicad_mod) — v1.0
- ✅ Web-based viewer with zoom/pan/selection — v1.0
- ✅ Touchpad navigation controls — v1.0
- ✅ File picker with drag-and-drop — v1.0

### Active

**v1.1 Foundation & Desktop**
- [ ] Multi-source library management (KiCad, JLCPCB, custom)
- [ ] 3D model handling for components
- [ ] Standardized project structure (folders, config)
- [ ] Dark mode theme system
- [ ] UI polish (responsive, better controls)
- [ ] Tauri desktop application
- [ ] Native file dialogs and menus
- [ ] Web deployment infrastructure
- [ ] Embedded Monaco code editor
- [ ] Comprehensive documentation

**Deferred to v1.2+**
- [ ] 3D board preview (Three.js)
- [ ] Undo/redo system
- [ ] Project templates

**Advanced Features (v2.0+ target)**
- [ ] Schematic view generation
- [ ] Custom autorouter (A*, GPU-accelerated)
- [ ] ngspice simulation integration
- [ ] IPC-2581 export
- [ ] WASM plugin system
- [ ] Impedance calculator (microstrip, stripline)
- [ ] Differential pair routing
- [ ] Length matching
- [ ] Module/import system for reusable blocks

### Out of Scope

- **Mobile app** — Desktop/web first, mobile adds complexity without core value
- **Real-time collaboration** — Git-based workflow is the collaboration model
- **Component marketplace** — Use existing libraries (KiCad, etc.)
- **Manufacturing ordering** — Export files, let user choose fab
- **Training custom AI models** — Use existing LLMs, focus on file format clarity

## Context

**Problem:**
Current PCB tools (KiCad, Eagle, Altium) are GUI-first. The project file is a binary or XML blob that's a side effect of clicking. This makes:
- Git collaboration painful (meaningless diffs, merge hell)
- AI/LLM assistance nearly impossible (can't edit XML blobs)
- Automation difficult (scripting is afterthought)
- Reproducibility uncertain (same project, different tool version = different output?)

**Prior Art:**
- OpenSCAD proved code-first works for 3D modeling
- Terraform proved declarative infrastructure works
- HDLs (Verilog, VHDL) proved code-first works for digital circuits
- LibrePCB has human-readable files but still GUI-first

**User:**
- Engineers who code (comfortable with text files, git, CLI)
- Teams wanting proper version control on hardware designs
- Anyone wanting to leverage LLMs for PCB design assistance

**Existing Ecosystem:**
- KiCad footprint libraries (S-expression format, importable)
- FreeRouting (open source autorouter, Specctra DSN format)
- ngspice (open source SPICE simulator, BSD license)
- Gerber X2 (universal manufacturing format)

## Constraints

- **Language:** Rust (performance, safety, WASM compilation, 30+ year longevity)
- **Platform:** Web-first (WASM), with Tauri for desktop standalone
- **Rendering:** wgpu for 2D (WebGPU), Three.js for 3D
- **Parser:** Tree-sitter (incremental, error-tolerant, LSP-ready)
- **Performance:** Must handle 1000+ component boards smoothly
- **Determinism:** Same source file = identical output, always
- **Compatibility:** Export to industry standard formats (Gerber, IPC-2581)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| External DSL (not embedded in Rust) | Full control over syntax, optimized for human/LLM readability | — Pending |
| Tree-sitter for parsing | Incremental parsing, error tolerance, LSP support, used by GitHub/editors | — Pending |
| Rust + WASM | Near-native performance, runs in browser and desktop, memory safe | — Pending |
| Tauri over Electron | 50% less RAM, <10MB bundle, Rust backend integration | — Pending |
| wgpu over Canvas | WebGPU standard, GPU compute for autorouting, cross-platform | — Pending |
| FreeRouting for MVP autorouter | Proven, open source, allows custom later | — Pending |
| ECS architecture for board model | Composition over inheritance, cache-friendly, parallelizable | — Pending |
| Command pattern for undo/redo | Standard for CAD, handles complex object graphs | — Pending |
| WASM plugins | Sandboxed, cross-language, portable | — Pending |

---
*Last updated: 2026-01-29 after starting v1.1 milestone*
