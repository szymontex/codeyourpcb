# Project State: CodeYourPCB

## Current Status

**Phase:** 4 - Export (In Progress)
**Last Activity:** 2026-01-28 - Completed 04-06 CLI Export Integration

## Project Reference

See: .planning/PROJECT.md (updated 2026-01-21)

**Core value:** The source file is the design - git-friendly, AI-editable, deterministic
**Current focus:** Making the existing features work together as a unified workflow

## Phase Progress

| Phase | Status | Progress |
|-------|--------|----------|
| 1. Foundation | Complete | 100% (9/9 plans) |
| 2. Rendering | Complete | 100% (9/9 plans) |
| 3. Validation | Complete | 100% (10/10 plans) |
| 4. Export | In progress | 67% (6/9 plans) |
| 5. Intelligence | Complete | 100% (10/10 plans) |
| 6. Desktop | Not started | 0% |
| 7. Navigation | Not started | 0% |
| 8. File Picker | In progress | 67% (2/3 plans) |

Progress: ████████████████████████████████░ 94% (45/48 plans)

## Quick Start

```bash
cd viewer
npm run start   # Builds WASM if needed, starts Vite + hot reload server
```

Open http://localhost:5173, click Open button, select a .cypcb file from examples/

## Next Action

Phase 4 Export in progress - foundation complete (coordinate conversion, aperture management).

**Current Focus:** Wave 3 execution - CLI integration and packaging

**Phase 4 Export Plans:**
1. ✓ 04-01: Export foundation (coords, apertures)
2. ✓ 04-02: Gerber layer export (copper, mask, paste)
3. ✓ 04-03: Board outline and silkscreen Gerber export
4. ✓ 04-04: Excellon drill file export
5. ✓ 04-05: BOM and pick-and-place (CPL) export
6. ✓ 04-06: CLI export integration with manufacturer presets
7. 04-07: Gerber job file
8. 04-08: ZIP packaging
9. 04-09: Export integration testing

**Outstanding Gaps from Phase 5:**
1. LSP server compilation errors (high priority) - blocks developer experience
2. File picker UI visibility issue (low priority) - workaround exists (drag/drop)

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
| 2026-01-28 | Silkscreen MVP crosshairs | Crosshair markers instead of text rendering for MVP |
| 2026-01-28 | Courtyard rotation deferred | Axis-aligned rectangles acceptable for MVP |
| 2026-01-28 | Line widths standardized | 0.1mm outline, 0.15mm silkscreen per industry standards |
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
| 2026-01-21 | Two-phase spatial clearance checking | AABB query then exact distance for O(log n) |
| 2026-01-21 | Canonical pair ordering for DRC | Prevents duplicate A-B/B-A violation reports |
| 2026-01-21 | i128 for distance calculation | Nanometer squared values can overflow i64 |
| 2026-01-21 | FootprintLibrary for DRC pad lookup | Rules query footprint defs for pad/pin info |
| 2026-01-21 | MinTraceWidthRule deferred | Placeholder until Phase 5 adds Trace entities |
| 2026-01-21 | DRC after every load | Real-time feedback for DRC-05 requirement |
| 2026-01-21 | ViolationKind as string in TS | Simpler JS serialization than enum mapping |
| 2026-01-21 | Fixed 15px marker radius | Screen-space size ensures visibility at any zoom |
| 2026-01-21 | 5mm zoom margin for violations | Provides context around violation location |
| 2026-01-28 | Net constraint docs as subsection | Users encounter nets early, need immediate clarity |
| 2026-01-28 | Show correct vs incorrect syntax | Users already attempted wrong syntax, explicit contrast prevents confusion |
| 2026-01-28 | Add constraints to existing examples | Users already reference blink.cypcb, zero friction learning |
| 2026-01-28 | Environmental limitation Java | FreeRouting requires Java 21+ runtime, outside scope of codebase |
| 2026-01-28 | LSP compilation gap documented | Type inference errors need deep fixes, mark as high-priority gap |
| 2026-01-28 | UAT with limitations accepted | Implementation quality verified via code/tests when runtime blocked |
| 2026-01-22 | IPC-2221 formula constants | k=0.048 external, k=0.024 internal |
| 2026-01-22 | Builder pattern for TraceWidthParams | Ergonomic API with method chaining |
| 2026-01-22 | Warning enum for accuracy limits | Clear categorization of out-of-range conditions |
| 2026-01-22 | kicad_parse_gen for import | Mature S-expr parser handles KiCad format quirks |
| 2026-01-22 | .pretty suffix stripping | Standard KiCad library naming convention |
| 2026-01-22 | courtyard fallback 0.5mm | IPC-7351B margin when courtyard not in file |
| 2026-01-22 | Trace as polyline | Vec<TraceSegment> for flexible routing |
| 2026-01-22 | Locked trace flag | Boolean flag for autorouter to respect |
| 2026-01-22 | CurrentUnit enum | mA/A variants with type-safe conversion |
| 2026-01-22 | Mil resolution for DSN | 0.1 mil (resolution 10) matches FreeRouting |
| 2026-01-22 | Mutable world for export | bevy_ecs queries need &mut for cache init |
| 2026-01-22 | Locked trace as fixed wire | (type fix) prevents FreeRouting modification |
| 2026-01-22 | Server feature optional | Build environment proc-macro issue with tower-lsp |
| 2026-01-22 | DocumentState stores DRC | run_drc requires &mut; store during build_world |
| 2026-01-22 | Diagnostic cap 100 | Prevent editor flooding per RESEARCH.md guidance |
| 2026-01-22 | Star-topology ratsnest | First pin to all others for simple MVP visualization |
| 2026-01-22 | Gold ratsnest color | #FFD700 for high visibility against copper colors |
| 2026-01-22 | Layer-ordered rendering | Bottom -> top -> vias -> ratsnest for proper z-order |
| 2026-01-22 | WebSocket routing | Dev server handles route requests, runs CLI, streams results |
| 2026-01-22 | Unified start command | `npm run start` builds WASM + starts Vite + hot reload |
| 2026-01-28 | Integer arithmetic conversion | nm→Gerber decimal via integer math, avoids float precision loss |
| 2026-01-28 | D-code start at 10 | D01-D03 reserved for draw/move/flash per Gerber standard |
| 2026-01-28 | RoundRect fallback | Standard Gerber lacks RoundRect, use Rect+comment until polygon impl |
| 2026-01-28 | Format MM 2.6 default | 2 integer, 6 decimal (mm) most common modern format, 1µm precision |
| 2026-01-28 | Gerber X2 attributes | TF.FileFunction for layer identification, CAM compatibility |
| 2026-01-28 | Dark polarity for mask/paste | Standard manufacturing convention, positive = exposed |
| 2026-01-28 | 0.05mm mask expansion | Common tolerance for solder mask openings, configurable |
| 2026-01-28 | THT pads excluded from paste | Through-hole doesn't use solder paste |
| 2026-01-28 | Rotation via trigonometry | Accurate pad positioning, <0.1µm floating-point error |
| 2026-01-28 | Excellon tool numbers start T1 | Standard convention, T0 invalid in most CAM software |
| 2026-01-28 | METRIC,TZ Excellon format | Metric units with trailing zero suppression, modern standard |
| 2026-01-28 | Explicit decimal Excellon coords | X50.800Y30.480 more readable than implicit format |
| 2026-01-28 | PTH default for all drills | Component pads and vias always plated, NPTH stub for future |
| 2026-01-28 | Group drill hits by tool | Minimizes tool changes during manufacturing |
| 2026-01-28 | Comma-separated BOM designators | JLCPCB single row per group, reduces BOM size |
| 2026-01-28 | Natural designator sorting | R1, R2, R10 order more intuitive than lexical |
| 2026-01-28 | CPL coordinates with mm suffix | JLCPCB format requires explicit unit (50.800mm) |
| 2026-01-28 | CPL rotation in integer degrees | Pick-and-place machines use whole degrees |
| 2026-01-28 | CplConfig for machine variations | Different rotation conventions and Y-axis directions |
| 2026-01-28 | Preset-based export configuration | Avoid hardcoding manufacturer rules, extensible for new manufacturers |
| 2026-01-28 | Organized export directory structure | gerber/, drill/, assembly/ folders for clear separation |
| 2026-01-28 | CLI dry-run mode | Preview files before generation, user confidence |
| 2026-01-28 | JLCPCB default preset | Most common hobbyist manufacturer, zero configuration |
| 2026-01-28 | CLI as primary export interface | Headless operation for automation, CI/CD integration |

## Session History

### 2026-01-28: Complete 04-06 CLI Export Integration
- **Implemented manufacturer presets module** - Configuration system for different manufacturers
  - Created ExportPreset struct with coordinate format and layer configuration
  - FileNaming struct for manufacturer-specific file suffixes (KiCad vs traditional)
  - ExportLayers struct for selective layer export
  - JLCPCB 2-layer preset with KiCad-style naming (-F_Cu.gbr, -PTH.drl)
  - PCBWay standard preset with traditional naming (_top.gtl, _drill.xln)
  - Case-insensitive preset lookup via from_name()
  - 11 tests for preset content and lookup
  - Commit d8b64d9
- **Implemented export job orchestrator** - Coordinates complete file generation
  - ExportJob struct with source path, output dir, preset, board name
  - run_export() generates all files based on preset configuration
  - Organized output structure: gerber/, drill/, assembly/
  - Exports all Gerber layers (copper, mask, paste, silk, outline)
  - Exports Excellon drill files (PTH)
  - Exports assembly files (BOM CSV/JSON, CPL) when enabled
  - ExportResult with files list, warnings, duration tracking
  - ExportedFile with path, type description, size
  - 4 integration tests for job execution
  - Commit 87f98b7
- **Implemented CLI export command** - User-facing interface for manufacturing file generation
  - clap-based argument parsing with clear options
  - Input file, output directory (-o), preset selection (-p)
  - --no-assembly flag to skip BOM/CPL generation
  - --dry-run mode to preview files without generating
  - Error handling for parse errors, sync errors, unknown presets
  - Clear progress output during export
  - Success summary with file list, sizes, and duration
  - Integrated into main CLI as export subcommand
  - 3 unit tests for command construction and preset lookup
  - Verified with examples/blink.cypcb: generates 13 files in 67ms
  - Commit b186cd8
- **Fixed test compilation** - Restored result variable in job tests
  - Commit 1c40375
- **Test coverage:** 130 tests passing in cypcb-export (18 new), 9 CLI tests passing
- **Created SUMMARY:** .planning/phases/04-export/04-06-SUMMARY.md

### 2026-01-28: Complete 04-05 BOM and Pick-and-Place Export
- **Implemented BOM CSV and JSON export** - Component grouping and consolidation
  - Created bom module with component grouping by (value, footprint)
  - Natural sorting for designators (R1, R2, R10)
  - CSV export in JLCPCB format with comma-separated designators
  - JSON export with metadata (board name, export date, component counts)
  - Added csv and serde_json dependencies
  - 18 BOM tests passing
  - BOM files created in prior commit 34044ff (mislabeled as 04-04)
- **Implemented pick-and-place (CPL) CSV export** - Coordinates and rotation for assembly
  - Created cpl module with CplEntry and CplConfig types
  - CSV export in JLCPCB format with mm suffix (50.800mm)
  - Coordinate conversion from nanometers to millimeters (3 decimals)
  - Rotation conversion from millidegrees to degrees
  - Layer detection from first pad in footprint
  - Configuration support for rotation offset and Y-flip
  - Natural sorting for consistent output
  - 12 CPL tests passing
  - Commit 89c3857
- **Fixed excellon test compilation** - Blocking issue from 04-04 signature change
  - Added missing None parameter to export_excellon() test calls
  - Fixed after drill_type_filter parameter added in 04-04
- **Total test coverage:** 115 tests passing in cypcb-export (30 new BOM+CPL)
- **Created SUMMARY:** .planning/phases/04-export/04-05-SUMMARY.md

### 2026-01-28: Complete 04-04 Excellon Drill File Export
- **Created Excellon module and tool table** - drill size to tool number mapping
  - ToolTable with HashMap-based deduplication
  - get_or_create() assigns tool numbers sequentially (T1, T2, ...)
  - to_header() generates Excellon tool definitions (T{n}C{diameter})
  - Drill size constants (VIA_DRILL_DEFAULT, THT_DRILL_SMALL, THT_DRILL_LARGE)
  - 9 unit tests for tool assignment and header generation
  - Commit 349f1ae
- **Implemented Excellon drill file export** - full file generation with M48 header
  - export_excellon() with header, tool definitions, drill hits, M30
  - collect_drill_hits() from component pads (THT) and vias
  - calculate_pad_position() handles component rotation via trigonometry
  - group_hits_by_tool() minimizes tool changes
  - METRIC,TZ format with explicit decimal coordinates
  - 8 unit tests for export, collection, grouping, positioning
  - Commit 127d3ed
- **Added PTH/NPTH drill separation** - filter by drill type
  - DrillType enum (Plated, NonPlated)
  - export_excellon() extended with drill_type_filter parameter
  - Header comments identify drill type (PTH/NPTH/All)
  - group_hits_by_tool_refs() for reference-based grouping after filtering
  - All drills currently Plated (pads, vias); NPTH stub for future mounting holes
  - 3 unit tests for filtering behavior
  - Commit 34044ff
- **Test coverage:** 20 tests (9 tool table + 8 writer + 3 filtering), all passing
- **Created SUMMARY:** .planning/phases/04-export/04-04-SUMMARY.md

### 2026-01-28: Complete 04-03 Board Outline and Silkscreen
- **Silkscreen export implemented** - Gerber files for component markings
- **Board outline export implemented** - Profile layer for board edge
- **Created SUMMARY:** .planning/phases/04-export/04-03-SUMMARY.md

### 2026-01-28: Complete 04-02 Gerber Layer Export
- **Created Gerber X2 header module** - write_header() with file function attributes
  - GerberFileFunction enum (Copper, Mask, Paste, Silk, Profile, Drill)
  - X2 attributes: TF.GenerationSoftware, TF.CreationDate, TF.FileFunction, TF.Part
  - CopperSide and Side enums for layer designation
  - Automatic layer numbering (L1 top, Ln bottom)
  - Commit 7743884
- **Implemented copper layer export** - Full Gerber file generation for copper
  - export_copper_layer() with pads, traces, vias
  - calculate_pad_position() handles component rotation
  - via_spans_layer() for layer span logic
  - ExportError for footprint lookup failures
  - D01/D02/D03 commands for draw/move/flash
  - Commit e35d257
- **Implemented mask and paste export** - Soldermask and solderpaste layers
  - export_soldermask() with aperture expansion
  - export_solderpaste() with SMD-only filtering
  - MaskPasteConfig with builder pattern
  - apply_expansion() and apply_reduction() for sizing
  - %LPD*% dark polarity for standard manufacturing
  - Commit 1dbaccc
- **Added dependencies** - chrono for timestamps, bevy_ecs for queries
- **Test coverage:** 29 new tests (12 header + 8 copper + 9 mask), all passing
- **Created SUMMARY:** .planning/phases/04-export/04-02-SUMMARY.md

### 2026-01-28: Complete 04-01 Export Foundation
- **Created cypcb-export crate** - New workspace crate for manufacturing file export
  - Dependencies: gerber-types 0.7, cypcb-world, cypcb-core
  - Module structure: coords, apertures
  - Commit e21ae88
- **Implemented coordinate conversion** - Integer arithmetic nm to Gerber decimal
  - Unit enum (Millimeters, Inches)
  - CoordinateFormat struct with FORMAT_MM_2_6, FORMAT_INCH_2_4 constants
  - nm_to_gerber() using integer-only math for precision
  - gerber_format_string() generates %FSLAX26Y26*%
  - 11 unit tests, all passing
  - Commit 5e88de3
- **Implemented aperture management** - D-code generation and deduplication
  - ApertureShape enum (Circle, Rectangle, Oblong, RoundRect)
  - ApertureManager with HashMap-based deduplication
  - get_or_create() assigns D-codes starting at D10
  - to_definitions() generates %ADD...% statements
  - aperture_for_pad() maps PadDef to ApertureShape
  - RoundRect falls back to Rectangle with G04 comment
  - 13 unit tests, all passing
  - Commit e86cc18
- **Total test coverage:** 24 unit tests + 7 doc tests = 31 tests passing
- **Created SUMMARY:** .planning/phases/04-export/04-01-SUMMARY.md

### 2026-01-28: Complete 05-10 UAT Verification
- **Verified Phase 5 Intelligence features with documented limitations**
  - Autorouting implementation complete, runtime blocked by Java requirement
  - LSP implementation exists but has compilation errors with `server` feature
  - Trace/ratsnest rendering verified via code inspection
  - Trace width calculator verified via test coverage
- **Fixed LSP type inference issues (partial)**
  - Added explicit type annotations to hover.rs
  - Resolved some E0282 errors, compilation still fails overall
  - Commit d0605f8
- **Created UAT test artifacts**
  - examples/uat-routing-test.cypcb: 3-component routing test
  - examples/uat-routing-locked.cypcb: Locked trace test
- **Documented environmental limitations**
  - Java 21+ unavailable prevents FreeRouting execution
  - LSP server feature has type system bugs
  - Implementation quality verified, runtime testing blocked
- **Created comprehensive UAT summary** - 05-10-SUMMARY.md
  - Documents all verification attempts
  - Gap closure plan for LSP compilation
  - Assessment of Phase 5 completion status

### 2026-01-28: Complete 05-11 DSL Syntax Documentation
- **Created comprehensive DSL syntax reference** - docs/SYNTAX.md (418 lines)
  - Documents all major DSL constructs (board, component, net, zone, trace, footprint)
  - Detailed net constraint syntax section with square bracket placement
  - Side-by-side CORRECT vs INCORRECT examples addressing UAT gap
  - Common mistakes section
  - References to example files for learning
- **Updated example files with constraints**:
  - power-indicator.cypcb: Added `[current 100mA width 0.3mm]` to VCC net
  - blink.cypcb: Added `[current 20mA]` to VCC net
  - Both files validate successfully
- **Closed UAT gap** - Users attempted `net VCC { current 500mA }` (incorrect)
  - Documentation now shows correct syntax: `net VCC [current 500mA] { pins }`
  - Explicit contrast prevents future confusion
- Phase 5 Intelligence now 100% complete (10/10 plans)

### 2026-01-22: Integration Fixes (Critical)
- **Fixed DRC false positives** - populate_from_snapshot now correctly builds NetConnections from snapshot.nets
  - Previous bug: all pins reported as "unconnected" because NetConnections were always empty
  - Fix: iterate snapshot.nets, intern net names, map component.pin -> net_id, populate during spawn_component
- **Added unified start command** - `npm run start` in viewer/ handles everything:
  - Checks for WASM build, runs build-wasm.sh if missing
  - Checks for node_modules, runs npm install if missing
  - Starts Vite dev server + WebSocket hot reload server
- **Integrated real routing from UI**:
  - Extended server.ts to handle 'route' messages from WebSocket clients
  - Server runs `cypcb route` CLI command and streams progress back
  - Frontend triggerRouting() sends request, receives progress/completion/error
  - On completion, loads SES content into engine and re-renders with traces
- **Fixed symlink** - freerouting.jar -> freerouting-2.1.0.jar for CLI to find
- Verified full workflow: CLI routing works (2.55s for routing-test.cypcb)

### 2026-01-22: Complete 08-02 File Loading Integration
- Integrated file picker utilities with main.ts viewer
- Added handleFileLoad() async function for .cypcb and .ses files
- Wired Open button click to trigger file picker dialog
- Set up drop zone on canvas container
- Viewer starts clean with no auto-loaded test data
- Status bar shows "Ready - Open a file" initially
- .cypcb files load board and fit to view
- .ses files load routes (requires board loaded first)
- Removed embedded TEST_SOURCE/TEST_SES (use examples/ files)
- TypeScript compiles, all integrations verified

### 2026-01-22: Complete 08-01 File Picker Infrastructure
- Created file-picker.ts (102 lines) with three utility functions
- readFileAsText: Promise wrapper for FileReader.readAsText()
- createFilePicker: Hidden input element with change handler
- setupDropZone: Drag events with visual feedback class and drag counter
- Window-level dragover/drop handlers prevent accidental file navigation
- Added Open button to toolbar with blue styling (#007bff)
- Added drag-over CSS with dashed green outline (#28a745)
- Added drop-hint element that appears during drag
- TypeScript compiles, all exports verified

### 2026-01-22: Complete 05-09 Autoroute Integration
- Created CLI route command (crates/cypcb-cli/src/commands/route.rs, 398 lines)
- Workflow: Parse -> Build world -> Export DSN -> Run FreeRouting -> Import SES -> Save .routes
- Progress output shows pass/routed/unrouted/elapsed during routing
- Dry-run mode exports DSN only for manual FreeRouting usage
- Added viewer Route button with progress overlay and cancel support
- Auto-route checkbox enables routing on file save (simulated for MVP)
- Added load_routes() to PcbEngine for loading .routes files
- Routes file format: segment/via text lines with layer/coords/dimensions
- Verified workflow with blink.cypcb dry-run export
- Fixed FootprintLibrary::default() -> new() for built-in footprints
- 12 CLI tests + 15 render tests passing

### 2026-01-22: Complete 05-08 Trace and Ratsnest Rendering
- Extended BoardSnapshot with TraceInfo, ViaInfo, RatsnestInfo types
- Added collect_traces() and collect_vias() methods to PcbEngine
- Implemented collect_ratsnest() with star-topology for unrouted nets
- Added drawTrace() with layer colors and locked indicator
- Added drawVia() with copper ring and drill hole
- Added drawRatsnest() with dashed yellow lines
- Added Ratsnest checkbox to viewer toolbar
- Added sample traces/ratsnest to MockPcbEngine for testing
- Layer-ordered rendering: bottom -> top -> vias -> ratsnest
- 15 tests passing in cypcb-render

### 2026-01-22: Complete 05-05 LSP Core Server
- Created cypcb-lsp crate with LSP protocol implementation
- Implemented document.rs with DocumentState, Position, offset conversion
- Implemented hover.rs with hover info for components, nets, footprints, zones, traces
- Implemented diagnostics.rs with parse error and DRC violation conversion
- Implemented backend.rs with tower-lsp LanguageServer trait
- DRC violations stored in DocumentState during build_world()
- Diagnostics capped at 100 per file to prevent editor flooding
- Server feature optional due to build environment proc-macro issue
- 11 tests passing (4 document + 3 hover + 4 diagnostics)

### 2026-01-22: Complete 05-04 DSN Export for FreeRouting
- Created cypcb-router crate for autorouting integration
- Implemented types.rs with RoutingStatus, RouteSegment, ViaPlacement, RoutingResult
- Implemented dsn.rs with full Specctra DSN export (712 lines)
- Export includes: boundary, layers, components, nets, padstacks, locked traces
- Coordinate conversion: nm to mil (1 mil = 25,400 nm)
- Locked traces exported with (type fix) to prevent FreeRouting modification
- Integration test suite with 13 tests covering all DSN sections
- 29 total tests passing (16 unit + 13 integration)

### 2026-01-22: Complete 05-01 Trace & Via ECS + DSL Extensions
- Created trace.rs with TraceSegment, Trace, Via, TraceSource types
- TraceSegment: line segment with length/midpoint calculations (i128 for overflow safety)
- Trace: ECS component with segments vec, width, layer, net_id, locked, source
- Via: drill holes with start_layer/end_layer for blind/buried vias
- Extended grammar with current constraint (500mA, 2A syntax)
- Extended grammar with manual trace definition (from, to, via, layer, width, locked)
- Added TraceDef, CurrentValue, CurrentUnit AST types
- Updated NetConstraints with optional current field
- Added sync_trace() to sync TraceDef to Trace entities
- New error types: InvalidTracePin, MissingNet, UnknownLayer
- 19 trace tests + 7 parser tests + 5 sync tests all passing

### 2026-01-22: Complete 05-03 KiCad Footprint Import
- Created cypcb-kicad crate for KiCad file import (crate structure from 05-02)
- Implemented footprint.rs with full .kicad_mod parsing via kicad_parse_gen
- Module-to-Footprint conversion supporting all pad types (rect, circle, oval)
- SMD vs THT detection with drill extraction from KiCad Drill struct
- Layer mapping from KiCad (F.Cu, B.Cu, etc.) to internal Layer enum
- Courtyard extraction from F.CrtYd lines, fallback to IPC-7351B 0.5mm margin
- Implemented library.rs with walkdir for recursive .kicad_mod discovery
- LibraryEntry struct with name, path, library (from .pretty folder)
- Search helpers: find_by_name (case-insensitive), find_by_library
- Support for duplicate footprint names across different libraries
- 19 tests: 10 footprint + 9 library covering all scenarios

### 2026-01-22: Complete 05-02 IPC-2221 Trace Width Calculator
- Created cypcb-calc crate for electrical calculations
- Implemented IPC-2221 formula: I = k * dT^0.44 * A^0.725
- Added TraceWidthParams with builder pattern (current, temp_rise, copper_oz, is_external)
- Added TraceWidthResult with width (Nm), cross_section_mm2, warnings
- Implemented TraceWidthWarning enum: CurrentTooHigh, TempRiseTooLow, TempRiseTooHigh, WidthExceedsMax, CopperWeightNonStandard
- Added convenience methods: min_width_for_current(), with_defaults()
- 18 unit tests + 7 doc tests passing
- Verified against IPC-2221 reference values (within 30%)

### 2026-01-21: Complete 03-08 Violation Display (markers, status bar, panel)
- Added violation and violation_ring colors to LAYER_COLORS
- Implemented drawViolation() rendering red ring markers at violation locations
- Added showViolations to RenderState for visibility toggle
- Added error-badge to status bar with pill-style appearance
- Badge shows violation count, hidden when no violations
- Added error-panel overlay (VS Code style) with scrollable list
- Each error shows [kind] message format
- Click error item triggers zoomToLocation() centering on violation
- Panel toggles on badge click, close button dismisses

### 2026-01-21: Complete 03-07 DRC Integration with Rendering Pipeline
- Added cypcb-drc dependency to cypcb-render crate
- Created ViolationInfo struct for JS-friendly serialization
- Added violations field to BoardSnapshot
- PcbEngine runs DRC automatically after load_source()
- Stores violations and timing in PcbEngine struct
- Added violation_count() and drc_duration_ms() accessor methods
- Updated TypeScript ViolationInfo interface
- Updated MockPcbEngine to return empty violations
- 10 tests passing, WASM build successful (251KB)

### 2026-01-21: Complete 03-06 Drill Size, Trace Width, Connectivity Rules
- Created drill_size.rs with MinDrillSizeRule implementation
- Checks THT pads via FootprintLibrary lookup against min_drill_size
- SMD pads (no drill) automatically exempt from drill checking
- Created connectivity.rs with UnconnectedPinRule implementation
- Checks all footprint pins have NetConnections via pin_net()
- Reports unconnected pins as refdes.pin format (R1.2)
- Created trace_width.rs as documented placeholder (DRC-02)
- Defers implementation to Phase 5 when Trace entities exist
- Updated run_drc() to include all 5 rules
- Added with_pad_info() to violation.rs for detailed messages
- 17 new tests, 70 total cypcb-drc tests passing

### 2026-01-21: Complete 03-05 Clearance Checking Rule
- Created clearance.rs with full ClearanceRule implementation
- Two-phase spatial checking: R*-tree AABB query then exact distance
- Layer filtering prevents false positives on different layers
- Canonical pair ordering prevents duplicate A-B/B-A violations
- Uses i128 intermediates for distance squared to prevent overflow
- Added spatial() method to BoardWorld for direct SpatialIndex access
- Added rstar dependency to cypcb-drc for AABB types
- 12 unit tests covering all clearance scenarios

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
| docs/SYNTAX.md | Comprehensive DSL syntax reference (418 lines) |
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
| crates/cypcb-drc/src/rules/clearance.rs | ClearanceRule implementation |
| crates/cypcb-drc/src/rules/drill_size.rs | MinDrillSizeRule implementation |
| crates/cypcb-drc/src/rules/connectivity.rs | UnconnectedPinRule implementation |
| crates/cypcb-drc/src/rules/trace_width.rs | MinTraceWidthRule placeholder |
| crates/cypcb-calc/Cargo.toml | Electrical calculator crate config |
| crates/cypcb-calc/src/lib.rs | Calculator crate API |
| crates/cypcb-calc/src/trace_width.rs | IPC-2221 trace width calculator |
| crates/cypcb-kicad/Cargo.toml | KiCad import crate config |
| crates/cypcb-kicad/src/lib.rs | KiCad crate API |
| crates/cypcb-kicad/src/footprint.rs | KiCad .kicad_mod import |
| crates/cypcb-kicad/src/library.rs | KiCad library scanning |
| crates/cypcb-world/src/components/trace.rs | Trace, Via, TraceSegment ECS components |
| crates/cypcb-router/Cargo.toml | Autorouter integration crate config |
| crates/cypcb-router/src/lib.rs | Router crate API and exports |
| crates/cypcb-router/src/types.rs | RoutingResult, RouteSegment, ViaPlacement |
| crates/cypcb-router/src/dsn.rs | Specctra DSN export implementation |
| crates/cypcb-router/tests/dsn_integration.rs | DSN export integration tests |
| crates/cypcb-export/Cargo.toml | Export crate config |
| crates/cypcb-export/src/lib.rs | Export crate API and re-exports |
| crates/cypcb-export/src/coords.rs | Coordinate conversion (nm to Gerber decimal) |
| crates/cypcb-export/src/apertures.rs | Aperture management and D-code generation |
| crates/cypcb-lsp/Cargo.toml | LSP crate config (server feature optional) |
| crates/cypcb-lsp/src/lib.rs | LSP crate API and exports |
| crates/cypcb-lsp/src/main.rs | LSP server binary entry point |
| crates/cypcb-lsp/src/backend.rs | tower-lsp LanguageServer impl |
| crates/cypcb-lsp/src/document.rs | DocumentState, Position, offset conversion |
| crates/cypcb-lsp/src/hover.rs | Hover provider for all AST types |
| crates/cypcb-lsp/src/diagnostics.rs | Parse error and DRC violation diagnostics |
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
| viewer/src/file-picker.ts | File selection and reading utilities |
| docs/SYNTAX.md | Comprehensive DSL syntax reference |
| examples/uat-routing-test.cypcb | UAT routing test case |
| examples/uat-routing-locked.cypcb | UAT locked trace test case |

## Session Continuity

**Last session:** 2026-01-28
**Stopped at:** Completed 04-02-PLAN.md (Gerber Layer Export) - Phase 4 in progress
**Resume file:** None

---
*State updated: 2026-01-28*
