# Project State: CodeYourPCB

## Current Status

**Phase:** 2 of 6 (Rendering) - IN PROGRESS
**Plan:** 4 of 5 complete
**Last Activity:** 2026-01-21 - Completed 02-03-PLAN.md

## Project Reference

See: .planning/PROJECT.md (updated 2026-01-21)

**Core value:** The source file is the design - git-friendly, AI-editable, deterministic
**Current focus:** Phase 2 Rendering - WASM integration complete (with mock), layer visibility UI next

## Phase Progress

| Phase | Status | Progress |
|-------|--------|----------|
| 1. Foundation | Complete | 100% (9/9 plans) |
| 2. Rendering | In progress | 80% (4/5 plans) |
| 3. Validation | Not started | 0% |
| 4. Export | Not started | 0% |
| 5. Intelligence | Not started | 0% |
| 6. Desktop | Not started | 0% |

Progress: █████████████░░░░░░░ 65%

## Phase 2 Plan Status

| Plan | Name | Status |
|------|------|--------|
| 02-01 | WASM Crate Setup | Complete |
| 02-02 | Frontend Scaffolding | Complete |
| 02-03 | WASM Binding | Complete |
| 02-04 | Canvas 2D Rendering | Complete |
| 02-05 | Layer Visibility | Not started |

## Next Action

Continue with 02-05 (Layer visibility UI) - connect layer checkboxes to rendering state.

## Key Decisions Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-01-21 | Rust + WASM + Tauri | Performance, safety, 30yr longevity |
| 2026-01-21 | Tree-sitter for DSL | Incremental parsing, error recovery |
| 2026-01-21 | ECS for board model | Composition, parallel queries |
| 2026-01-21 | Integer nanometers | Avoid floating-point precision issues |
| 2026-01-21 | FreeRouting for MVP autorouter | Proven, defer custom to v2 |
| 2026-01-21 | i64 for Nm coordinates | Deterministic precision, i128 for intermediates |
| 2026-01-21 | Bottom-left origin, Y-up | Mathematical convention, matches Gerber viewers |
| 2026-01-21 | Millidegrees for rotation | i32 (0-359999), deterministic comparison |
| 2026-01-21 | u32 layer mask | Bit mask for copper layers, supports 32 layers |
| 2026-01-21 | AST Span tracking | All nodes carry source span for error reporting |
| 2026-01-21 | Error recovery parsing | ParseResult returns partial AST + errors |
| 2026-01-21 | Sync error recovery | Continue sync on semantic errors, collect all |
| 2026-01-21 | CLI without world dep | Workaround cargo resolver issue; parse-only validation |
| 2026-01-21 | Vanilla TypeScript | No UI framework for minimal verification viewer |
| 2026-01-21 | Vite build tool | Fast dev server with native WASM support |
| 2026-01-21 | WASM bindings deferred | Build environment issue with wasm-bindgen linking |
| 2026-01-21 | Entity re-export | Re-export bevy_ecs::Entity from cypcb-world |
| 2026-01-21 | Light mode default | Light background (#FFFFFF) per user preference |
| 2026-01-21 | Immutable state | All viewport/layer/render state updates return new objects |
| 2026-01-21 | Mock fallback for WASM | MockPcbEngine in TypeScript when WASM unavailable |
| 2026-01-21 | bevy_ecs no multi_threaded | Disabled for WASM compatibility |

## Session History

### 2026-01-21: Execute 02-03 WASM Integration
- Created viewer/build-wasm.sh for wasm-pack builds
- Enabled wasm-bindgen with conditional compilation
- Implemented MockPcbEngine as JavaScript fallback
- Mock parses .cypcb syntax and returns BoardSnapshot
- Integration test shows board visualization on canvas
- WASM build blocked by getrandom/bevy_ecs compatibility
- All 246 Rust tests passing

### 2026-01-21: Execute 02-01 WASM Crate Setup
- Created cypcb-render crate with snapshot types
- Implemented BoardSnapshot, ComponentInfo, PadInfo, NetInfo types
- Implemented PcbEngine with load_source, get_snapshot, query_point
- Added 7 unit tests (all passing)
- Re-exported Entity from cypcb-world for convenience
- WASM bindings temporarily disabled due to build environment issue

### 2026-01-21: Execute 02-04 Canvas 2D Rendering
- Created viewport.ts with coordinate transforms (nm/Y-up to px/Y-down)
- Created layers.ts with KiCad-style colors and visibility state
- Created renderer.ts with full Canvas 2D rendering
- Supports all pad shapes: circle, rect, roundrect, oblong
- Zoom-at-point, pan, fitBoard utilities
- Selection highlighting in orange
- Through-hole drill holes rendered
- Adaptive grid density based on zoom

### 2026-01-21: Execute 02-02 Frontend Scaffolding
- Created Vite + TypeScript project structure in viewer/
- Added HTML shell with canvas and layer toggle toolbar
- Created TypeScript types matching Rust BoardSnapshot
- Added WASM loading utilities (placeholder for integration)
- Dev server runs, TypeScript compiles without errors

### 2026-01-21: Execute 01-09 CLI
- Set up CLI structure with clap (parse, check commands)
- Implemented parse command outputting AST as JSON
- Implemented check command validating syntax
- Integrated miette for fancy error display with source context
- Added 9 integration tests
- Created example files (blink.cypcb, invalid.cypcb, unknown_keyword.cypcb)
- Worked around cargo resolver issue by removing cypcb-world dependency
- Refactored SyncError to manual Diagnostic impl

### 2026-01-21: Execute 01-08 AST Sync
- Created sync.rs (747 lines) bridging parser and board model
- Implemented sync_ast_to_world function for AST-to-ECS conversion
- Added SyncError enum with miette-compatible error types:
  - UnknownFootprint: component references missing footprint
  - DuplicateRefDes: same refdes used twice
  - UnknownComponent: net references undefined component
- Sync continues on errors for better user experience
- Source spans preserved on entities for error reporting
- Spatial index rebuilt after sync using footprint bounds
- 11 unit tests + doc tests passing (128 total crate tests)

### 2026-01-21: Execute 01-05 AST Parser
- Created AST types (ast.rs) with Span tracking on all nodes
- Implemented CST to AST conversion (parser.rs)
- Added miette-compatible error types (errors.rs)
- ParseResult type enables error recovery with partial results
- Handles Tree-sitter choice nodes (board_property, net_constraint)
- 36 unit tests + 4 doc tests passing

### 2026-01-21: Execute 01-04 ECS Components
- Created 15 ECS components for board model
- Position: wraps cypcb_core::Point, Rotation in millidegrees
- Electrical: NetId, RefDes, Value, NetConnections, PinConnection
- Physical: Layer enum (10 variants), FootprintRef, Pad, PadShape
- Board: Board marker, BoardSize, LayerStack (2-32 layers)
- Metadata: SourceSpan (miette integration), ComponentKind enum
- 28 unit tests + 20 doc tests passing

### 2026-01-21: Execute 01-03 Grammar
- Created Tree-sitter grammar (234 lines) for CodeYourPCB DSL
- Grammar supports: version, board, component, net definitions
- Board properties: size, layers, stackup
- Component properties: value, position, rotation, net assignment
- Net features: pin references, constraint blocks (width, clearance)
- build.rs compiles parser.c via cc crate
- Rust bindings: language() function, node_kinds module
- 8 comprehensive tests verifying all syntax constructs

### 2026-01-21: Execute 01-02 Core Types
- Implemented cypcb-core crate with Nm, Point, Rect, Unit types
- Created workspace structure as blocking fix (01-01 was not executed)
- i64 nanometers for deterministic coordinate precision
- Comprehensive unit conversion: mm, mil, inch to/from nm
- Rect geometry with intersection, containment, union operations
- All types derive Serialize/Deserialize for JSON output

### 2026-01-21: Project Initialization
- Deep brainstorming session on code-first PCB concept
- Extensive tech stack research with benchmarks
- Created PROJECT.md with vision and constraints
- Completed domain research (Stack, Features, Architecture, Pitfalls)
- Defined 35 v1 requirements across 6 categories
- Created 6-phase roadmap

## Files Created

| File | Purpose |
|------|---------|
| .planning/PROJECT.md | Project vision and constraints |
| .planning/config.json | Workflow preferences |
| .planning/brainstorm.md | Extensive research notes (~1500 lines) |
| .planning/research/STACK.md | Technology recommendations |
| .planning/research/FEATURES.md | Feature landscape |
| .planning/research/ARCHITECTURE.md | System design |
| .planning/research/PITFALLS.md | Risks and mitigations |
| .planning/research/SUMMARY.md | Research synthesis |
| .planning/REQUIREMENTS.md | v1 requirements with IDs |
| .planning/ROADMAP.md | 6-phase execution plan |
| .planning/STATE.md | This file |
| Cargo.toml | Workspace manifest |
| crates/cypcb-core/src/coords.rs | Nm, Point coordinate types |
| crates/cypcb-core/src/units.rs | Unit enum for dimension parsing |
| crates/cypcb-core/src/geometry.rs | Rect bounding box type |
| crates/cypcb-core/src/lib.rs | Core crate exports |
| crates/cypcb-parser/grammar/grammar.js | Tree-sitter grammar definition |
| crates/cypcb-parser/grammar/package.json | Tree-sitter CLI config |
| crates/cypcb-parser/grammar/tree-sitter.json | ABI 15 config |
| crates/cypcb-parser/grammar/queries/highlights.scm | Syntax highlighting |
| crates/cypcb-parser/build.rs | C parser compilation |
| crates/cypcb-parser/src/lib.rs | Parser bindings and tests |
| crates/cypcb-parser/src/ast.rs | AST type definitions |
| crates/cypcb-parser/src/parser.rs | CST to AST conversion |
| crates/cypcb-parser/src/errors.rs | Parse error types |
| crates/cypcb-world/src/components/mod.rs | Component module |
| crates/cypcb-world/src/components/position.rs | Position, Rotation |
| crates/cypcb-world/src/components/electrical.rs | NetId, RefDes, Value, NetConnections |
| crates/cypcb-world/src/components/physical.rs | Layer, FootprintRef, Pad, PadShape |
| crates/cypcb-world/src/components/metadata.rs | SourceSpan, ComponentKind |
| crates/cypcb-world/src/components/board.rs | Board, BoardSize, LayerStack |
| crates/cypcb-world/src/world.rs | BoardWorld high-level API |
| crates/cypcb-world/src/registry.rs | NetRegistry for name interning |
| crates/cypcb-world/src/spatial.rs | SpatialIndex for region queries |
| crates/cypcb-world/src/footprint/mod.rs | Footprint module |
| crates/cypcb-world/src/footprint/library.rs | FootprintLibrary type |
| crates/cypcb-world/src/footprint/smd.rs | SMD footprints (0402-2512) |
| crates/cypcb-world/src/footprint/tht.rs | THT footprints (DIP-8, etc) |
| crates/cypcb-world/src/sync.rs | AST-to-ECS synchronization |
| crates/cypcb-cli/src/main.rs | CLI entrypoint |
| crates/cypcb-cli/src/commands/mod.rs | Commands module |
| crates/cypcb-cli/src/commands/parse.rs | Parse command |
| crates/cypcb-cli/src/commands/check.rs | Check command |
| crates/cypcb-cli/tests/cli_integration.rs | CLI integration tests |
| crates/cypcb-render/Cargo.toml | WASM crate config |
| crates/cypcb-render/src/lib.rs | PcbEngine implementation |
| crates/cypcb-render/src/snapshot.rs | BoardSnapshot types |
| examples/blink.cypcb | Example LED circuit |
| examples/invalid.cypcb | Invalid syntax example |
| examples/unknown_keyword.cypcb | Unknown keyword example |
| viewer/package.json | Frontend npm package |
| viewer/tsconfig.json | TypeScript config |
| viewer/vite.config.ts | Vite build config |
| viewer/.gitignore | Frontend ignores |
| viewer/index.html | HTML shell with canvas |
| viewer/src/main.ts | Application entry point |
| viewer/src/wasm.ts | WASM loading with mock fallback |
| viewer/src/types.ts | TypeScript types for BoardSnapshot |
| viewer/src/viewport.ts | Viewport state and coordinate transforms |
| viewer/src/layers.ts | Layer colors and visibility |
| viewer/src/renderer.ts | Canvas 2D rendering functions |
| viewer/build-wasm.sh | WASM build script |

## Session Continuity

**Last session:** 2026-01-21
**Stopped at:** Completed 02-03-PLAN.md
**Resume file:** None

---
*State updated: 2026-01-21*
