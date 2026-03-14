# M002: CodeYourPCB v2.0 — Professional EDA Platform

**Vision:** Transform CodeYourPCB from a working prototype into the world's best standalone code-first PCB design tool. Build a custom autorouter that produces professional-quality routing, add interactive 3D visualization with real component models, evolve the DSL to match atopile's power (modules, constraints, units) while staying fully standalone, and deliver a UI so polished that senior engineers would switch from KiCad.

## Success Criteria

- Custom autorouter routes a 500-component board in <30s with quality matching or exceeding FreeRouting
- 3D viewer renders boards with real component models at 60fps with orbit/zoom controls
- DSL supports modules, typed interfaces (`I2C`, `SPI`), physical units (`10kohm`, `3.3V`), and constraints
- User can manually edit traces by click-dragging in the 2D viewer
- E2E test suite covers every user action with automated screenshots and input validation
- Web app loads in <3s, desktop starts in <1s
- Zero duplicate code paths (measured by cargo-deny/clippy/dedup analysis)
- All linters pass: clippy, eslint, rustfmt

## Key Risks / Unknowns

- Custom autorouter quality — building a competitive autorouter from scratch is the hardest engineering task
- 3D model pipeline — JLCPCB model availability and STEP/GLB conversion for web
- DSL backward compatibility — adding modules/constraints without breaking existing .cypcb files
- WASM autorouter performance — may need web worker + SharedArrayBuffer

## Proof Strategy

- PCB design rules completeness → retire in S01 by encoding IPC/manufacturer rules and validating against real reference boards
- Custom autorouter quality → retire in S02 by routing real reference boards and comparing output quality against FreeRouting
- 3D pipeline → retire in S04 by rendering a real board with JLCPCB models in browser
- DSL extensions → retire in S05 by parsing atopile-equivalent examples with full backward compat
- WASM performance → retire in S02 by benchmarking autorouter in WASM target

## Verification Classes

- Contract verification: Rust unit tests, integration tests, property-based tests for autorouter, snapshot tests for renderer
- Integration verification: full pipeline tests (DSL → parse → model → route → DRC → export → render)
- Operational verification: performance benchmarks, WASM bundle size, load time measurements
- UAT / human verification: visual screenshots of routed boards, 3D renders, UI interaction flows

## Milestone Definition of Done

This milestone is complete only when all are true:

- Custom autorouter produces valid routes for reference boards (blink, Arduino shield, 4-layer design)
- 3D viewer renders boards with real component models loaded from JLCPCB catalog
- DSL v2 parser handles modules, constraints, and units while still parsing all v1 files
- Manual trace editing works in the 2D viewer with DRC live feedback
- E2E test suite passes with 100% coverage of user actions
- Performance benchmarks pass (autorouter <30s/500 components, 3D at 60fps, web load <3s)
- All linters pass, zero code duplication above threshold
- Competition feature matrix shows parity or advantage on all key features

## Slices

- [x] **S01: PCB Knowledge Base & Design Rules** `risk:high` `depends:[]`
  > After this: comprehensive PCB design rule database exists — IPC standards (IPC-2221, IPC-7351, IPC-2581), manufacturer constraints (JLCPCB/PCBWay/OSHPark DRC presets), signal integrity rules, thermal management, trace geometry best practices. Sourced from: scraped Reddit (r/PrintedCircuitBoard, r/AskElectronics, r/electronics tips parsed into rules), Altium/KiCad/Allegro/OrCAD/EAGLE documentation and tutorials, reference PCB design textbooks, competitor codebases (KiCad router internals, atopile solver, Horizon EDA). KiCad/LibrePCB/Horizon EDA repos cloned to /workspace/competitors/. Delivered as structured `docs/pcb-knowledge/` directory AND `crates/cypcb-rules/` Rust crate with typed rule sets. License audit completed for any borrowed patterns with attribution file.
- [x] **S02: Custom Autorouter Core** `risk:high` `depends:[S01]`
  > After this: user can autoroute a multi-layer board using our own A*-based router with constraint awareness informed by S01 knowledge base, producing results comparable to FreeRouting — verified by routing reference boards and comparing output quality
- [x] **S03: Renderer Upgrade & Manual Trace Editing** `risk:high` `depends:[S02]`
  > After this: user can see traces from autorouter rendered with proper widths/clearances, and can click-drag to manually route or edit individual traces in the 2D viewer with live DRC feedback
- [x] **S04: 3D Board Viewer** `risk:high` `depends:[S03]`
  > After this: user can toggle to a 3D view showing the board with component models (loaded from JLCPCB), orbit/zoom/pan controls, and layer visibility — rendered with Three.js at 60fps
- [x] **S05: DSL v2 — Modules, Units & Constraints** `risk:high` `depends:[S02]`
  > After this: user can write `.cypcb` files with import/module system, typed interfaces (I2C, SPI, Power), physical units (10kohm, 3.3V, 100nF), and constraint assertions — all v1 files still parse correctly
- [x] **S06: Competition Feature Parity & UI Polish** `risk:medium` `depends:[S04,S05]`
  > After this: deep-dive feature matrix vs atopile/KiCad/Altium/Allegro/OrCAD/EAGLE/EasyEDA/diodeinc — every feature they have is catalogued and ours matches or exceeds. Missing features implemented: grid snap, undo/redo, net highlighting, component rotation UI, board outline editing. Visual quality polished to match atopile's PCB prototype display quality. Closed-source tool UX studied via manuals/tutorials/videos and best patterns adopted.
- [x] **S07: E2E Test Suite & Quality Gates** `risk:medium` `depends:[S03,S04,S05]`
  > After this: every user action is covered by automated E2E tests with screenshots and click simulation, all inputs sanitized, all errors described with user-friendly messages, all linters passing (clippy, eslint, rustfmt), zero code duplication above threshold. Web reliability tested: hot reload stability, WASM load failure recovery, reconnection after disconnect, edge cases with malformed .cypcb files. Error triggering scenarios exercised systematically.
- [x] **S08: Performance & Polish** `risk:low` `depends:[S06,S07]`
  > After this: web loads in <3s, desktop starts in <1s, autorouter handles 500-component boards in <30s, 3D viewer at 60fps, UI feels like a 2030-era professional tool

## Boundary Map

### S01 → S02

Produces:
- `crates/cypcb-rules/` or `docs/pcb-knowledge/` — structured PCB design rule database
- `RoutingRuleSet` trait and default implementations (IPC, JLCPCB, PCBWay presets)
- Signal integrity classification rules (digital, analog, power, high-speed)
- Trace geometry best practices encoded as constraints

Consumes:
- nothing (first slice — pure research + rule encoding)

### S02 → S03

Produces:
- `crates/cypcb-autoroute/` — custom autorouter crate with A* pathfinding, multi-layer support, constraint-aware routing
- `AutorouteResult` type with routed traces, vias, and quality metrics
- Integration with existing `BoardWorld` ECS model

Consumes:
- S01 routing rule database and constraint types
- `crates/cypcb-world/` — board model with spatial index
- `crates/cypcb-drc/` — design rule constraints

### S02 → S05

Produces:
- Autorouter engine API that DSL constraints can drive

Consumes:
- S01 rule database

### S03 → S04

Produces:
- Upgraded renderer with proper trace/via rendering
- Interaction system for trace editing (click targets, drag handlers)
- Testable UI interaction API

Consumes:
- S02 autorouter output (trace geometry)

### S04 → S07

Produces:
- 3D renderer with component model loading
- Model cache system for JLCPCB 3D assets

Consumes:
- S03 upgraded renderer infrastructure

### S05 → S06

Produces:
- Extended parser with module/constraint/unit support
- Backward-compatible grammar
- New AST node types

Consumes:
- Existing `crates/cypcb-parser/` grammar
- S02 autorouter constraint interface
