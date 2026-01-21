# Project Research Summary

**Project:** CodeYourPCB
**Domain:** Code-first PCB Design Tool (EDA)
**Researched:** 2026-01-21
**Confidence:** HIGH

## Executive Summary

CodeYourPCB is a code-first PCB design tool where engineers write declarative code that generates circuit boards. This is a paradigm shift from traditional GUI-first EDA tools (KiCad, Eagle, Altium) where the file format is a side effect of clicking. Our research confirms this approach is viable and differentiating — similar concepts have proven successful in other domains (OpenSCAD for 3D, Terraform for infrastructure, HDLs for digital circuits).

The recommended approach is to build a layered Rust architecture with WebAssembly compilation for browser deployment and Tauri for desktop. The core innovation is the file format itself — a human-readable DSL that's git-friendly, AI/LLM-editable, and deterministic. Tree-sitter provides incremental parsing, ECS (Entity Component System) provides the board model, and wgpu provides GPU-accelerated rendering. FreeRouting integration provides autorouting without reinventing the wheel.

Key risks include: DSL syntax lock-in (early decisions become permanent), floating-point non-determinism (use integer nanometers like KiCad), Gerber export edge cases (extensive testing required), and learning curve for traditional EDA users. Mitigation requires careful upfront DSL design, comprehensive test suites, and excellent documentation with examples.

## Key Findings

### Recommended Stack

Rust + WebAssembly provides near-native performance in the browser (8-10x faster than JS for compute) with memory safety and 30+ year longevity. Tauri 2.0 provides desktop deployment with 50% less RAM than Electron. Tree-sitter enables incremental parsing with error recovery, critical for hot-reload developer experience.

**Core technologies:**
- **Rust 1.84+**: Core language — memory safe, compiles to WASM, used by Mozilla/Google/Microsoft
- **Tree-sitter 0.25**: DSL parser — incremental, error-tolerant, powers GitHub/Neovim
- **wgpu 24.0**: 2D/3D rendering — WebGPU standard, compute shaders for routing
- **bevy_ecs 0.15**: Board model — parallel queries, spatial indexing integration
- **Tauri 2.0**: Desktop shell — lightweight, Rust backend

### Expected Features

**Must have (table stakes):**
- Component placement and net connections (core DSL functionality)
- Design Rule Check (clearance, width, drill rules)
- Gerber export (universal manufacturing format)
- 2D board view with zoom/pan
- Multi-layer support

**Should have (differentiators):**
- Git-friendly deterministic file format
- Hot reload on file changes
- LSP/IDE integration (autocomplete, hover)
- Electrical-aware constraints (crosstalk_sensitive, high_speed)
- CI/CD testable DRC

**Defer (v2+):**
- Custom GPU-accelerated autorouter
- Ngspice simulation integration
- WASM plugin system
- Schematic view (optional separate file)

### Architecture Approach

Five-layer architecture: Language Layer (Tree-sitter parser → AST), Domain Layer (ECS board model with R*-tree spatial indexing), Validation Layer (DRC engine), Rendering Layer (wgpu 2D, Three.js 3D), Platform Layer (Tauri desktop, WASM web). The board model uses Entity Component System for composition, cache-friendliness, and parallel queries. Command pattern enables undo/redo from the start.

**Major components:**
1. **cypcb-parser**: Tree-sitter grammar + AST types
2. **cypcb-world**: ECS board model with spatial indexing
3. **cypcb-drc**: Parallel design rule checking
4. **cypcb-render**: 2D canvas with layer composition
5. **cypcb-export**: Gerber/IPC-2581 generation

### Critical Pitfalls

1. **DSL Syntax Lock-in** — Version the grammar from day one, start minimal, extensive dogfooding before 1.0
2. **Floating-Point Geometry** — Use integer nanometers (32-bit signed, like KiCad) for all coordinates
3. **Gerber Edge Cases** — Test against multiple viewers AND manufacturer DFM tools, not just one
4. **Autorouter Determinism** — Pin random seeds, use deterministic hash maps, fixed-point costs
5. **DRC Performance Cliffs** — R*-tree spatial indexing from the start, not retrofitted

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Foundation
**Rationale:** Parser and data model are prerequisites for everything else
**Delivers:** Working DSL parser, board data model, CLI skeleton
**Addresses:** DSL syntax, component/net definitions
**Avoids:** Syntax lock-in (careful design), floating-point (integer coords from start)

### Phase 2: Rendering
**Rationale:** Must see what you're designing — hot reload enables rapid iteration
**Delivers:** 2D board view, file watching, zoom/pan/select
**Uses:** wgpu/Canvas, notify crate
**Implements:** Rendering layer

### Phase 3: Validation
**Rationale:** DRC prevents invalid designs before manufacturing
**Delivers:** Basic DRC (clearance, width, drill), error reporting
**Uses:** R*-tree spatial indexing, parallel execution
**Avoids:** Performance cliffs (spatial index from start)

### Phase 4: Export
**Rationale:** Manufacturing output makes the tool actually useful
**Delivers:** Gerber X2, drill files, BOM
**Uses:** gerber-types crate
**Avoids:** Gerber edge cases (extensive testing)

### Phase 5: Intelligence
**Rationale:** Autorouting and advanced features after core is solid
**Delivers:** FreeRouting integration, LSP server, 3D preview
**Uses:** Tower-lsp, Three.js
**Implements:** Full development workflow

### Phase 6: Desktop & Polish
**Rationale:** Desktop app and UX polish after features complete
**Delivers:** Tauri desktop app, undo/redo, component library management
**Uses:** Tauri 2.0, Command pattern

### Phase Ordering Rationale

- Parser → Model → Render follows data dependencies (can't render what doesn't exist)
- DRC before Export ensures we don't ship invalid designs
- FreeRouting integration deferred because manual routing works for MVP validation
- Desktop shell last because web version proves concept first

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 3 (DRC):** Need to research specific DRC rules for different board classes
- **Phase 5 (Intelligence):** FreeRouting integration details, LSP protocol specifics

Phases with standard patterns (skip research-phase):
- **Phase 1 (Foundation):** Tree-sitter well-documented, ECS patterns established
- **Phase 2 (Rendering):** Canvas/wgpu rendering is standard
- **Phase 4 (Export):** Gerber format well-specified (RS-274X)

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Benchmarked, proven in production (Figma, Cloudflare, etc.) |
| Features | HIGH | Based on established EDA tools, clear differentiation |
| Architecture | HIGH | Follows KiCad patterns, ECS proven in game dev |
| Pitfalls | MEDIUM | Community wisdom + domain research, some extrapolation |

**Overall confidence:** HIGH

### Gaps to Address

- **DSL syntax design:** Need concrete grammar before Phase 1, validate with users
- **FreeRouting determinism:** Need to verify/patch if non-deterministic
- **Manufacturer testing:** Need relationships with PCB fabs for Gerber validation
- **Performance at scale:** 10K+ component boards need benchmarking

## Sources

### Primary (HIGH confidence)
- KiCad Developer Documentation (file formats, architecture)
- Tree-sitter official documentation
- wgpu/WebGPU specifications
- IPC-2221/Gerber RS-274X specifications

### Secondary (MEDIUM confidence)
- tscircuit project (similar code-first approach)
- JITX marketing materials
- EDA community forums (KiCad, Altium users)
- Brainstorm session benchmarks and research

### Tertiary (LOW confidence)
- Autorouter determinism (limited documentation, needs validation)
- Adoption curves for code-first tools (extrapolated from HDL adoption)

---
*Research completed: 2026-01-21*
*Ready for roadmap: yes*
