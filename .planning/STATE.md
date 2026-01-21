# Project State: CodeYourPCB

## Current Status

**Phase:** 3 of 6 (Validation) - In Progress
**Plan:** 5 of 10 complete
**Last Activity:** 2026-01-21 - Completed 03-10-PLAN.md (Zones and Keepouts)

## Project Reference

See: .planning/PROJECT.md (updated 2026-01-21)

**Core value:** The source file is the design - git-friendly, AI-editable, deterministic
**Current focus:** Phase 3 Validation - DRC engine and IC footprints

## Phase Progress

| Phase | Status | Progress |
|-------|--------|----------|
| 1. Foundation | Complete | 100% (9/9 plans) |
| 2. Rendering | Complete | 100% (9/9 plans) |
| 3. Validation | In Progress | 50% (5/10 plans) |
| 4. Export | Not started | 0% |
| 5. Intelligence | Not started | 0% |
| 6. Desktop | Not started | 0% |

Progress: ████████████████░░░░ 76%

## Phase 3 Plan Status

| Plan | Name | Status |
|------|------|--------|
| 03-01 | DRC Crate Setup | Complete |
| 03-02 | IC Footprints (SOIC/SOT/QFP) | Complete |
| 03-03 | Manufacturer Presets | Complete |
| 03-04 | Custom Footprint DSL | Complete |
| 03-05 | TBD | Not started |
| 03-06 | TBD | Not started |
| 03-07 | TBD | Not started |
| 03-08 | TBD | Not started |
| 03-09 | TBD | Not started |
| 03-10 | Zones and Keepouts | Complete |

## Next Action

Continue Phase 3 (Validation) - Execute 03-05-PLAN.md through 03-09-PLAN.md

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
| 2026-01-21 | Middle-click pan | Standard CAD convention for panning |
| 2026-01-21 | Zoom factors 1.15x/0.87x | Smooth zoom feel per wheel event |
| 2026-01-21 | Dual watcher implementations | Rust for Tauri, Node.js for dev server |
| 2026-01-21 | WebSocket port 3001 | Separate from Vite (5173) |
| 2026-01-21 | 200ms debounce | Handles editor save patterns |
| 2026-01-21 | tree-sitter feature flag | Conditional compilation for WASM compatibility |
| 2026-01-21 | Split impl blocks for WASM | Separate WASM-exposed from internal methods |
| 2026-01-21 | JS parsing for WASM mode | tree-sitter requires C, so JS handles parsing in WASM |
| 2026-01-21 | WasmPcbEngineAdapter | Adapter bridges JS parsing to WASM load_snapshot() |
| 2026-01-21 | DrcRule trait object-safe | Allows Vec<Box<dyn DrcRule>> for flexible composition |
| 2026-01-21 | Rust structs for DRC presets | Type-safe manufacturer rules, no config file parsing |
| 2026-01-21 | Parametric gullwing generator | gullwing_footprint() for dual-row ICs, reduces duplication |
| 2026-01-21 | Counter-clockwise pin numbering | Standard IC convention, matches KiCad/Altium |
| 2026-01-21 | IPC-7351B courtyard | Body + 0.5mm margin for assembly clearance |
| 2026-01-21 | Preset enum for lookup | from_name() enables DSL string-to-preset mapping |
| 2026-01-21 | Default JLCPCB 2-layer | Most common hobbyist manufacturer |
| 2026-01-21 | Negative dimension support | Footprint pads need negative offsets from origin |
| 2026-01-21 | Clone library for custom FP | Non-breaking change, custom footprints without mutable ref |
| 2026-01-21 | THT pads = TopCopper+BottomCopper | Through-hole naturally spans both copper layers |
| 2026-01-21 | SMD pads = Top+Paste+Mask | Standard SMD pad stack for reflow soldering |
| 2026-01-21 | DrcRule::check takes &mut BoardWorld | bevy_ecs queries need mutable access for cache initialization |
| 2026-01-21 | Keepout checks component center | Simpler than full footprint bounds, adequate for MVP |

## Session History

### 2026-01-21: Complete 03-10 Zones and Keepouts
- Extended grammar with zone_definition rule (zone/keepout keywords)
- Created ZoneDef and ZoneKind AST types with bounds, layer, net support
- Implemented Zone ECS component with bounds, kind, layer_mask, name
- Added sync_zone() to convert ZoneDef to Zone entities
- Implemented KeepoutRule for DRC to detect components in keepout zones
- Changed DrcRule::check to take &mut BoardWorld for ECS query access
- Added KeepoutViolation variant and DrcViolation::keepout() constructor
- Added BoardWorld::zones() query method for zone iteration
- 12 new tests for zone parsing, sync, and DRC checking

### 2026-01-21: Complete 03-04 Custom Footprint DSL
- Extended Tree-sitter grammar with footprint_definition, pad_definition rules
- Added PadShape enum (rect, circle, roundrect, oblong) to AST
- Created FootprintDef and PadDef AST types with full pad geometry
- Implemented convert_footprint_definition() and convert_pad_definition() in parser
- Added support for negative dimensions (-1mm, -3.81mm) in grammar
- Updated sync_ast_to_world() to register custom footprints BEFORE component sync
- Clone FootprintLibrary to allow custom registration without mutable reference
- Conversion applies IPC-7351B courtyard margin (0.5mm) if not explicit
- THT pads default to TopCopper+BottomCopper, SMD to Top+Paste+Mask
- 6 new tests, all 211 tests passing (48 parser + 106 world + 57 doctests)

### 2026-01-21: Complete 03-03 Manufacturer Presets
- Created presets module with full DesignRules struct (7 constraint fields)
- Implemented JLCPCB 2-layer preset: 0.15mm clearance, 0.3mm drill, 0.2mm via
- Implemented JLCPCB 4-layer preset: 0.1mm clearance, 0.2mm drill (tighter)
- Implemented PCBWay standard preset: 0.15mm clearance, 0.22mm silk
- Implemented Prototype preset: 0.2mm clearance, 0.25mm trace (relaxed)
- Added Preset enum with from_name() for DSL string lookup
- Updated rules/mod.rs to import DesignRules from presets module
- 23 new preset tests, 35 total cypcb-drc tests passing

### 2026-01-21: Complete 03-02 IC Footprints
- Created gullwing.rs (687 lines) with parametric IC footprint generator
- Implemented gullwing_footprint() for dual-row packages (SOIC, SSOP)
- Added soic8(), soic14() with 1.27mm pitch, 5.4mm row span
- Added sot23() asymmetric 3-pin, sot23_5() 5-pin layouts
- Added tqfp32() quad-flat 32-pin, 0.8mm pitch
- Counter-clockwise pin numbering from bottom-left (IC standard)
- IPC-7351B courtyard calculation (body + 0.5mm margin)
- Registered all footprints in FootprintLibrary
- 14 new tests, 148 total cypcb-world tests passing

### 2026-01-21: Complete 03-01 DRC Crate Setup
- Created cypcb-drc crate with core DRC infrastructure
- Implemented DrcViolation type with kind, location, entities, source_span, message
- Added ViolationKind enum: Clearance, TraceWidth, DrillSize, UnconnectedPin, ViaDrill, AnnularRing
- Defined object-safe DrcRule trait with name() and check() methods
- Created DesignRules struct with JLCPCB and PCBWay manufacturer presets
- Implemented constructor methods for violations (clearance, drill_size, unconnected_pin)
- Added DrcResult with passed() and violation_count() methods
- Created placeholder rule structs: ClearanceRule, MinDrillSizeRule, UnconnectedPinRule
- 17 unit tests + 7 doc tests passing

### 2026-01-21: Complete 02-09 WASM Integration
- Created WasmPcbEngineAdapter to bridge JS parsing to WASM engine
- Raw WASM has load_snapshot(), adapter provides load_source()
- Extracted parseSource() as shared function for Mock and Adapter
- Fixed query_point to use JS-based hit testing (WASM spatial index not populated)
- Added test-wasm-integration.mjs for full integration verification
- Gap #2 from VERIFICATION.md now closed
- Phase 2 Rendering fully complete with real WASM integration

### 2026-01-21: Complete 02-08 WASM Build Fix
- Added tree-sitter-parser feature to cypcb-parser for conditional compilation
- Added sync feature to cypcb-world to exclude parser dependency for WASM
- Split PcbEngine impl blocks to separate WASM-exposed and internal methods
- Updated build-wasm.sh with --no-default-features --features wasm
- Added GLIBC_TUNABLES workaround for Linux TLS allocation issue
- WASM build produces: cypcb_render_bg.wasm (240KB), cypcb_render.js, types
- Created test-wasm.mjs smoke test (passes all checks)
- Gap #1 from VERIFICATION.md now closed

### 2026-01-21: Complete 02-07 Visual Verification
- Human verification checkpoint passed
- Board outline (yellow) visible
- Component pads (red) for R1, LED1 visible
- Zoom/pan navigation works
- Layer toggle works (unchecking Top hides pads)
- Component selection works (orange highlight)
- Hot reload works (file save updates viewer)
- Phase 2 Rendering officially verified complete

### 2026-01-21: Execute 02-06 Hot Reload
- Created cypcb-watcher crate with notify 7.0 and debouncing
- Created viewer/server.ts with chokidar and WebSocket
- Added WebSocket client to main.ts with auto-reconnect
- Viewport and selection preserved across reloads
- "Reloaded" notification shown for 1.5s
- Graceful fallback without WebSocket server

### 2026-01-21: Execute 02-05 Layer Visibility Integration
- Created interaction.ts (114 lines) with mouse handlers
- Implemented zoom-at-cursor, middle-click pan, left-click select
- Integrated main.ts with all rendering modules
- Connected layer checkboxes to rendering state
- Added coordinate display in mm on mouse move
- Status bar shows selected component
- Added DIP-8 through-hole footprint to mock engine
- Request animation frame render loop with dirty flag

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
| crates/cypcb-parser/build.rs | C parser compilation (conditional) |
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
| crates/cypcb-world/src/components/zone.rs | Zone, ZoneKind components |
| crates/cypcb-world/src/world.rs | BoardWorld high-level API |
| crates/cypcb-world/src/registry.rs | NetRegistry for name interning |
| crates/cypcb-world/src/spatial.rs | SpatialIndex for region queries |
| crates/cypcb-world/src/footprint/mod.rs | Footprint module |
| crates/cypcb-world/src/footprint/library.rs | FootprintLibrary type |
| crates/cypcb-world/src/footprint/smd.rs | SMD footprints (0402-2512) |
| crates/cypcb-world/src/footprint/tht.rs | THT footprints (DIP-8, etc) |
| crates/cypcb-world/src/footprint/gullwing.rs | IC footprints (SOIC, SOT, QFP) |
| crates/cypcb-world/src/sync.rs | AST-to-ECS synchronization |
| crates/cypcb-cli/src/main.rs | CLI entrypoint |
| crates/cypcb-cli/src/commands/mod.rs | Commands module |
| crates/cypcb-cli/src/commands/parse.rs | Parse command |
| crates/cypcb-cli/src/commands/check.rs | Check command |
| crates/cypcb-cli/tests/cli_integration.rs | CLI integration tests |
| crates/cypcb-render/Cargo.toml | WASM crate config (with features) |
| crates/cypcb-render/src/lib.rs | PcbEngine implementation |
| crates/cypcb-render/src/snapshot.rs | BoardSnapshot types |
| crates/cypcb-drc/Cargo.toml | DRC crate config |
| crates/cypcb-drc/src/lib.rs | DrcResult, run_drc() |
| crates/cypcb-drc/src/violation.rs | DrcViolation, ViolationKind |
| crates/cypcb-drc/src/rules/mod.rs | DrcRule trait |
| crates/cypcb-drc/src/presets/mod.rs | DesignRules, Preset enum |
| crates/cypcb-drc/src/presets/jlcpcb.rs | JLCPCB 2-layer/4-layer presets |
| crates/cypcb-drc/src/presets/pcbway.rs | PCBWay and Prototype presets |
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
| viewer/src/interaction.ts | Mouse interaction handlers |
| viewer/build-wasm.sh | WASM build script (working) |
| viewer/test-wasm.mjs | WASM smoke test for Node.js |
| viewer/test-wasm-integration.mjs | WASM integration test (parse+load+query) |
| crates/cypcb-watcher/Cargo.toml | File watcher crate config |
| crates/cypcb-watcher/src/lib.rs | Debounced file watching |
| viewer/server.ts | Dev server with WebSocket |

## Session Continuity

**Last session:** 2026-01-21
**Stopped at:** Completed 03-10-PLAN.md (Zones and Keepouts)
**Resume file:** None

---
*State updated: 2026-01-21*
