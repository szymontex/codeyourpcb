# CodeYourPCB

## What This Is

A code-first PCB design tool where you write code and it generates circuit boards. Instead of clicking in a GUI and getting XML as a side effect, you write declarative code that defines components, connections, and constraints — the visual representation is computed from this source of truth. Designed for engineers who want git-friendly collaboration, AI/LLM-assisted editing, and deterministic builds.

## Core Value

**The source file is the design.** A human-readable, git-diffable, LLM-editable PCB project file that always produces the same board. If the file is clear enough for Claude to edit, it's clear enough for anyone.

## Requirements

### Validated

(None yet — ship to validate)

### Active

**Phase 1: Foundation**
- [ ] Custom DSL parser (Tree-sitter grammar)
- [ ] Board data model (components, nets, layers)
- [ ] Basic 2D board view renderer (Canvas)
- [ ] File watching with hot reload

**Phase 2: Core Features**
- [ ] Component placement (absolute and relative)
- [ ] Net connections with constraints
- [ ] Basic DRC (clearance, trace width)
- [ ] Gerber X2 export

**Phase 3: Intelligence**
- [ ] Autorouter integration (FreeRouting DSN export/import)
- [ ] Trace width calculator (IPC-2221)
- [ ] Impedance calculator (microstrip, stripline)
- [ ] Electrical-aware DRC (current capacity, crosstalk hints)

**Phase 4: Full Experience**
- [ ] 3D board preview (Three.js)
- [ ] Schematic view
- [ ] LSP server for IDE integration
- [ ] Undo/redo system

**Phase 5: Advanced**
- [ ] Custom autorouter (A*, GPU-accelerated)
- [ ] ngspice simulation integration
- [ ] IPC-2581 export
- [ ] WASM plugin system
- [ ] Component library management (KiCad footprint import)

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
*Last updated: 2026-01-21 after initialization*
