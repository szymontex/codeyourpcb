# CodeYourPCB Architecture

This document explains the architecture of CodeYourPCB, including the codebase structure, crate relationships, and data flow.

## System Overview

CodeYourPCB is a **code-first PCB design tool** where the source file is the design. It's git-friendly, AI-editable, and produces deterministic PCB layouts.

### Technology Stack

- **Backend**: Rust (compiled to native and WASM)
- **Frontend**: TypeScript with Vite
- **Architecture**: 14 Rust crates in a Cargo workspace
- **Rendering**: WebGL via WASM
- **Desktop**: Tauri v2 for native application

The system runs in two modes:
- **Web**: WASM rendering engine + TypeScript UI (static hosting on Cloudflare Pages)
- **Desktop**: Native Tauri application with same rendering engine

## Crate Dependency Graph

```
┌─────────────┐
│  cypcb-cli  │ (CLI entry point)
└──────┬──────┘
       │
       ├──────────────────────────────────────────────┐
       │                                              │
       v                                              v
┌──────────────┐                              ┌──────────────┐
│ cypcb-export │                              │ cypcb-router │
└──────┬───────┘                              └──────┬───────┘
       │                                              │
       │  ┌───────────────┐                           │
       ├─>│  cypcb-world  │<──────────────────────────┤
       │  └───────┬───────┘                           │
       │          │                                   │
       │          ├────────────────┐                  │
       │          │                v                  │
       │          │         ┌──────────────┐          │
       │          │         │  cypcb-drc   │          │
       │          │         └──────┬───────┘          │
       │          │                │                  │
       │          v                v                  │
       │    ┌──────────────┐  ┌──────────┐           │
       │    │ cypcb-parser │  │ cypcb-   │           │
       │    └──────┬───────┘  │ core     │           │
       │           │          └────┬─────┘           │
       │           │               │                  │
       │           └───────────────┘                  │
       │                                              │
       v                                              v
┌──────────────┐                              ┌──────────────┐
│ cypcb-render │ (WASM entry point)           │ cypcb-lsp    │
└──────┬───────┘                              └──────────────┘
       │
       ├──────────────────┐
       │                  │
       v                  v
┌──────────────┐   ┌──────────────┐
│ cypcb-kicad  │   │ cypcb-library│
└──────┬───────┘   └──────┬───────┘
       │                  │
       v                  v
┌──────────────────────────────┐
│     cypcb-platform           │ (Platform abstraction)
└──────────────────────────────┘
       │
       ├──────────────┬──────────────┐
       v              v              v
   (Native)       (WASM)       (Desktop)
  FileSystem    FileSystem      Dialog
   SQLite       localStorage    Menus
                File System API

┌──────────────┐   ┌──────────────┐
│ cypcb-calc   │   │ cypcb-watcher│
└──────────────┘   └──────────────┘
(Utility crates - minimal dependencies)
```

### Key Dependency Flows

1. **Core Foundation**: `cypcb-core` defines fundamental types (units, geometry, coordinates)
2. **Parsing**: `cypcb-parser` (Tree-sitter grammar) → AST nodes
3. **ECS Model**: `cypcb-world` (Bevy ECS) uses parser to build board model
4. **Validation**: `cypcb-drc` queries world for design rule violations
5. **Rendering**: `cypcb-render` (WASM) queries world and renders to WebGL
6. **Export**: `cypcb-export` queries world to generate Gerber/drill files
7. **Routing**: `cypcb-router` converts world to DSN format for FreeRouting

## Crate Descriptions

Each entry used to carry a `**Size**: ~N lines` note. They were snapshots and
they had drifted by whole multiples - `cypcb-drc` said 1,800 against 7,221 -
so they are gone. A line count that moves with every commit ships as the
command that answers it:

```bash
for c in crates/*/; do
  printf '%s %s\n' "$(basename "$c")" \
    "$(find "$c/src" -name '*.rs' -not -path '*grammar*' | xargs wc -l | tail -1 | awk '{print $1}')"
done
```


### cypcb-core

**Purpose**: Foundation types used across the entire codebase

**Key Types**:
- `Nm` - a length in nanometres, the one unit the whole codebase measures in
- `Point`, `Rect` - the 2D geometry every other crate builds on
- `pour` - what a copper zone becomes: rectangle subtraction with the fab's
  clearance, thermal spokes, and the area arithmetic that checks it

`Length`, `Angle`, `Area`, `Circle` and `Polygon` were listed here and none of
them exist - the vocabulary is deliberately small. `Layer` lives in
`cypcb-world`, not here, because it is part of the board model rather than of
geometry.

**Dependencies**: None (standalone)


Core provides the vocabulary for all other crates. Every measurement is an
integer of nanometres rather than a float, so a board cannot drift by rounding
and mm-against-mil confusion cannot compile. The pour geometry sits here
because both the exporter and the renderer need the same answer.

### cypcb-parser

**Purpose**: Tree-sitter-based parser for .cypcb DSL

**Key Types**:
- `Parser` - Tree-sitter wrapper
- `AST` nodes - Typed representation of parsed syntax
- `ParseError` - Rich error diagnostics with spans

**Dependencies**: `tree-sitter`, `cypcb-core`


**Features**:
- `tree-sitter-parser` (default): Includes C-based Tree-sitter parser (requires C compiler)
- Without feature: AST types only (for WASM builds where parsing happens in JavaScript)

Parsing happens differently on native vs WASM:
- **Native**: Full Tree-sitter parser in Rust
- **WASM**: Tree-sitter WASM in JavaScript, pass AST to Rust

### cypcb-fixtures

Boards whose layers cannot be mistaken for one another, shared by the test
suites of the crates that read a layer index.

Three index errors have shipped in this project and each was found by a
mutation or by running the binary rather than by the test meant to cover it.
All three had one cause: a symmetric stack gives neighbouring layers the same
answer, so a rule reading the wrong index produces the right number. This crate
holds a stack on which every copper layer answers differently, so that reading
the wrong one fails.

**Depends on**: cypcb-core, cypcb-world

It is a dev-dependency of both and `publish = false`; nothing that ships links
it.

### cypcb-world

**Purpose**: ECS-based board model (single source of truth for PCB state)

**Key Types**:
- `BoardWorld` - the Bevy ECS world holding every entity
- Components: `RefDes`, `Position`, `Rotation`, `FootprintRef`, `NetConnections`,
  `Trace`, `Via`, `Zone`, `Side`
- `copper::fill_zone` - the copper a pour becomes on one layer, cut against
  every other piece of copper there. The Gerber writer and the viewer's
  snapshot both call it, so the screen and the fabrication files cannot
  disagree

**Dependencies**: `bevy_ecs`, `rstar` (spatial index), `cypcb-parser`, `cypcb-core`


**Features**:
- `sync` (default): AST-to-ECS synchronization (requires parser)
- Without feature: ECS model only (for manual board construction)

The world uses Bevy ECS for performance and flexibility:
- Efficient queries (find all components in area, all nets with violations)
- R*-tree spatial index for collision detection
- Component-based architecture allows extending without modifying core types

### cypcb-drc

**Purpose**: Design Rule Checking engine

**Key Checks**: seventeen rules are registered. The list moves, so it ships as
the command that answers it rather than as a copy that goes stale:

```bash
grep -c 'Box::new(rules::' crates/cypcb-drc/src/lib.rs   # how many run
grep -oE 'Box::new\(rules::[A-Za-z]+\)' crates/cypcb-drc/src/lib.rs  # which
```

The ones worth knowing when reading the code: `ClearanceRule` measures pad
copper per pad with per-pad nets and reports per offending segment;
`CourtyardClearanceRule` keeps placement collisions visible now that clearance
no longer measures part bodies; `ZoneOverlapRule` catches two planes on
different nets over the same copper; and `PourIslandRule` fills every pour and
reports the sheets no thermal spoke reaches - copper connected to nothing.

**Dependencies**: `cypcb-world`, `cypcb-core`, `bevy_ecs`, `rstar`


**Features**:
- `parallel` (optional): Use rayon for multi-threaded checks (not WASM compatible)

`run_drc` runs every rule over the whole board and returns a `DrcResult`;
violations are values in that report, not ECS entities. `PcbEngine::run_drc_incremental`
in `cypcb-render` is a re-run under a name that promises less work than it
does - the incremental path does not exist yet, and naming it here is cheaper
than someone measuring a speed-up that was never implemented.

A violation carries what it measured: `actual` and `required` distances, so a
short at 0.00mm can be told from a gap under spec, and an `area` where the
fault is a piece of copper rather than a point.

### cypcb-autoroute

**Purpose**: Turn a ratsnest into copper. The largest crate in the workspace
and the one this project has spent the most measurement on.

**Key Exports**:
- `route_board` - the entry point the CLI and the viewer both use: pick a
  strategy, route, then run the repair pass if one is configured
- `PathFinderStrategy` - negotiated congestion with rip-up and reroute, the
  default and the better of the two on every benchmark board
- `ImprovedAStarStrategy` - the other strategy, kept because a second opinion
  scores differently and the variant machinery can pick it
- `variant::generate_variants` - route several ways, score each, keep the best
- `score_board` - the quality metrics `cypcb score` prints

**Dependencies**: `cypcb-world`, `cypcb-router`, `cypcb-rules`, `cypcb-drc`,
`cypcb-core`, `pathfinding`

The grid is one cell per legal track position - a trace width plus the
clearance it needs - so neighbouring cells are clearance-legal by
construction. Most of the knobs on `AutorouteConfig` exist because they were
measured to help one board and hurt another; `docs/TRACKER.md` carries the
numbers for each, including the ones that were reverted.

### cypcb-rules

**Purpose**: What a fabricator will make. One table of constraints per house
and process, and the lookup that turns a name into it.

**Key Exports**:
- `RulesPreset` - jlcpcb, pcbway, oshpark and the rest, each with its own
  clearances, widths, drills and annular rings
- `PresetRuleSet` - a preset as the `RoutingRuleSet` the router asks
- `DesignConstraints` - the fields themselves, checked against the struct by
  a test so the count cannot drift

**Dependencies**: `cypcb-core`

Every command that measures a board - `check`, `route`, `score`, `export` -
resolves its `--preset` through this crate, which is what makes their numbers
agree on the same file.

### cypcb-export

**Purpose**: Manufacturing file export (Gerber RS-274X, Excellon drill)

**Key Exports**:
- Gerber layers (copper, soldermask, silkscreen, paste)
- Excellon drill files (PTH, NPTH)
- BOM (Bill of Materials) CSV
- Centroid file for pick-and-place

**Dependencies**: `gerber-types`, `csv`, `cypcb-world`, `cypcb-core`, `bevy_ecs`


Gerber export uses aperture-based rendering:
- Define apertures (circles, rectangles, rounded rectangles)
- Emit draw/flash commands
- Coordinate precision: 5.5 format (µm-level accuracy)

### cypcb-render

**Purpose**: WebGL rendering and WASM entry point

**Key Functions**:
- `init_engine()` - Initialize WASM module
- `load_source()` - Parse .cypcb source and build world
- `render()` - Render board to canvas
- `check_drc()` - Run design rule checks
- `export_gerber()` - Export manufacturing files

**Dependencies**: `wasm-bindgen`, `cypcb-world`, `cypcb-parser`, `cypcb-drc`, `cypcb-core`


**Features**:
- `native` (default): Full parsing support with tree-sitter
- `wasm`: Parsing done in JavaScript (smaller WASM binary)

WASM binary is aggressively optimized:
- Size: 264 KB (gzipped)
- Optimization: opt-level="z", LTO, strip symbols
- wasm-opt: -O4 with bulk-memory and nontrapping-float-to-int

### cypcb-lsp

**Purpose**: Language Server Protocol implementation

**Key Features**:
- Diagnostics (parse errors, DRC violations), published on open, change and save
- Auto-completion (footprints, nets, parts, properties, layers, keywords)
- Hover documentation (parts, nets, footprints, board, zones, traces)
- Go to definition (pin reference or net name to where it is declared)

Semantic tokens were listed here and have never existed - the word `semantic`
appears nowhere in the crate. Syntax highlighting in the browser comes from
Monaco's own grammar, not from this server.

**Dependencies**: `tower-lsp`, `tokio`, `dashmap`, `cypcb-parser`, `cypcb-world`, `cypcb-drc`

**Features**:
- `server`: **on by default**. It was optional, with a note here blaming
  proc-macro loading, and the cost was that `backend.rs` compiled nowhere and
  stopped compiling at all.

The LSP uses a two-level approach:
- **WASM bridge**: Direct engine calls for web mode (no server needed)
- **Server mode**: Stdio-based LSP server for any editor that speaks the
  protocol. Go-to-definition works; find-references does not exist.

`docs/language-server.md` is the page for using it, and a test holds that page
to what the server advertises.

### cypcb-library

**Purpose**: Component library management and search

**Key Features**:
- Multi-source libraries (KiCad, JLCPCB, custom)
- SQLite storage with FTS5 full-text search
- BM25 ranking for relevance scoring
- Namespace-prefixed components (kicad::R_0805 vs jlcpcb::R_0805)

**Dependencies**: `rusqlite`, `lexpr` (S-expression parser), `serde`, `serde_json`


**Features**:
- `jlcpcb` (optional): JLCPCB API integration (requires API key)

Library architecture:
- `LibrarySource` trait for pluggable sources (KiCad, JLCPCB, Custom)
- `LibraryManager` orchestrates all sources behind unified API
- FTS5 index auto-syncs via SQLite triggers (no manual index management)

Search supports optional filters:
- Manufacturer, package type, category
- Dynamic SQL generation based on which filters are set
- Parameterized queries prevent SQL injection

### cypcb-kicad

**Purpose**: KiCad .kicad_mod footprint parser

**Key Features**:
- S-expression parser (Lisp-style tree walking)
- Recursive field search for nested structures
- Auto-organize by category (Resistor_SMD, Capacitor_THT, etc.)

**Dependencies**: `kicad_parse_gen`, `walkdir`, `cypcb-core`, `cypcb-world`


KiCad files use S-expressions with variable structure. Parser walks the tree manually (more maintainable than custom Serde deserializers).

### cypcb-router

**Purpose**: FreeRouting integration (autorouter)

**Key Features**:
- DSN format export (board design)
- SES format import (session/routes)
- Coordinate transformation (cypcb → FreeRouting → cypcb)

**Dependencies**: `cypcb-world`, `cypcb-core`, `bevy_ecs`


Routing workflow:
1. Export board to DSN format (nets, pads, board outline, rules)
2. Run FreeRouting CLI (external Java process)
3. Import SES session file (routes, vias)
4. Merge routes back into world

### cypcb-platform

**Purpose**: Platform abstraction facade (native vs web)

**Key Traits**:
- `FileSystem` - Read/write files (native FS vs File System Access API)
- `Dialog` - File/folder picker (native dialogs vs browser pickers)
- `Storage` - Key-value persistence (SQLite vs localStorage)
- `Menu` - Menu data model (rendered by Tauri or HTML)

**Dependencies**: `async-trait`, `cfg-if`, platform-specific crates

**How much of it is reachable**: one module of four. `src-tauri` is its only
dependant and imports `Menu`, `MenuBar` and `MenuItem`; the file, storage and
dialog modules - about 850 of its 1,300 lines - have no caller anywhere in the
workspace, and the crate that would use them is the one nothing in this
container compiles (it needs system GTK). What the viewer actually does for
files and storage it does in TypeScript. This is the whole of what decision D3
is about, measured rather than remembered.


**Features**:
- `desktop`: Tauri-specific features
- `web`: Web-specific features
- `native-dialogs`: Enable native file dialogs (requires system libraries on Linux)

Platform pattern prevents 800% code duplication:
- Application code imports only `Platform` struct
- Build-time `cfg` attributes select native or WASM implementations
- Both expose identical async APIs

WASM constraints:
- Single-threaded, so traits use `#[async_trait(?Send)]`
- FileHandle can't require `Send+Sync` bounds

### cypcb-calc

**Purpose**: Electrical calculations

**Key Calculations**:
- IPC-2221 trace width for current capacity

The list here used to name microstrip and stripline impedance and a thermal
resistance calculation. The crate is `trace_width.rs` and a twenty-four line
`lib.rs`; the word `impedance` appears once, in a comment saying it is future
work. Both were features the document claimed and the code never had.

**Dependencies**: `cypcb-core`

**How much of it is reachable**: the checker, the language server and the
engine all call one entry point, `TraceWidthCalculator::min_width_for_current`.
The builder around it - temperature rise, ambient temperature, copper weight -
is reachable only through `calculate`, which the design rule check started
using on 2026-08-10 so it could read the fab's copper weight and say what the
width it asks for assumed. The warning type it returns is read by nobody.

### cypcb-watcher

**Purpose**: File watching for hot reload

**Key Features**:
- Debounced file system events (300ms)
- Cross-platform (inotify/FSEvents/ReadDirectoryChangesW)

**Dependencies**: `notify`, `notify-debouncer-full`


Used by CLI for `--watch` mode. Not used in web/desktop (handled by Vite dev server).

### cypcb-cli

**Purpose**: Command-line interface

**Key Commands** (count them rather than trust this list:
`cypcb --help`):
- `cypcb check <file>` - parse, validate and run DRC. Exit code 1 on
  violations, so it is usable from a script. `--preset` picks the fab rules,
  `--no-drc` stops at parsing.
- `cypcb export <file> --output <dir>` - Gerbers, drill, BOM and
  pick-and-place. 13 files.
- `cypcb route <file>` - autorouter. The built-in PathFinder routes the board
  every measured way, keeps the best and writes it back as `.cypcb` trace
  blocks; `--fast` routes once instead. FreeRouting is opt-in, by naming its
  jar with `--freerouting <path>`: it is a Java program this binary cannot
  supply, so it is not something a plain `cypcb route` should need.
- `cypcb score <file>` - route and print quality metrics as JSON.
- `cypcb parse <file>` - the board model as JSON: the components with their
  resolved footprints and nets, the nets with their constraints, the traces,
  the vias and the zones, after every `import` is followed. `-o ast` prints the
  raw syntax tree instead.
- `cypcb parse-kicad <file>` - KiCad board metadata as JSON.

**Dependencies**: `clap`, `cypcb-parser`, `cypcb-world`, `cypcb-export`,
`cypcb-router`, `cypcb-autoroute`, `cypcb-drc`, `cypcb-rules`, `cypcb-kicad`

CLI is standalone binary, useful for CI/CD pipelines and headless builds.

## Data Flow

### Parse → Render Pipeline

```
.cypcb source
    │
    ├─> cypcb-parser (Tree-sitter)
    │        │
    │        v
    │   AST (Abstract Syntax Tree)
    │        │
    │        v
    │   cypcb-world (AST → ECS sync)
    │        │
    │        v
    │   Board (Bevy ECS world)
    │        │
    │        ├─> cypcb-drc (Design Rule Check)
    │        │        │
    │        │        v
    │        │   Violations (ECS entities)
    │        │
    │        ├─> cypcb-render (WebGL)
    │        │        │
    │        │        v
    │        │   Canvas (visual output)
    │        │
    │        └─> cypcb-export (Gerber)
    │                 │
    │                 v
    │            .gbr files (manufacturing)
    │
    v
(stored in world for reference)
```

### Edit Cycle (Live Preview)

```
User types in editor
    │
    ├─> 300ms debounce
    │        │
    │        v
    │   editor.getValue()
    │        │
    │        v
    │   engine.load_source(source)
    │        │
    │        ├─> Parse (AST)
    │        │
    │        ├─> Sync to world (ECS)
    │        │
    │        └─> DRC check
    │                 │
    │                 v
    │   Diagnostics → Monaco markers
    │        │
    │        v
    │   Re-render canvas (WebGL)
    │        │
    │        v
    │   User sees updated board
```

**Suppress-sync flag**: Prevents circular updates during programmatic `setValue()` calls (e.g., when loading a file).

### Export Pipeline

```
Board (ECS world)
    │
    ├─> Query all copper features
    │        │
    │        v
    │   Group by layer (F.Cu, B.Cu, etc.)
    │        │
    │        v
    │   Define apertures (D10, D11, ...)
    │        │
    │        v
    │   Emit Gerber commands (G01, D01, D02, D03)
    │        │
    │        v
    │   .gbr files
    │
    ├─> Query all drills
    │        │
    │        v
    │   Group by size and type (PTH, NPTH)
    │        │
    │        v
    │   Excellon format (T01C0.8, X1000Y2000)
    │        │
    │        v
    │   .drl files
    │
    └─> Query all components
             │
             v
        Extract metadata (reference, value, footprint)
             │
             v
        CSV format (BOM)
```

## Frontend Architecture

### Technology Stack

- **Bundler**: Vite (fast dev server, optimized builds)
- **Language**: TypeScript (strict mode)
- **WASM Loading**: Dynamic import with top-level await
- **Editor**: Monaco Editor (lazy-loaded, 970 KB gzipped)

### Module Structure

```
viewer/src/
├── main.ts              # Entry point, WASM initialization
├── theme.ts             # ThemeManager singleton, CSS custom properties
├── editor.ts            # Monaco editor setup, syntax highlighting
├── completions.ts       # Auto-completion provider
├── hover.ts             # Hover documentation provider
├── diagnostics.ts       # LSP diagnostics → Monaco markers
├── platform.ts          # Platform detection (desktop vs web)
└── styles.css           # Global styles with CSS custom properties
```

### Key Patterns

**1. ThemeManager Singleton**

Coordinates theme state across:
- CSS custom properties (`data-theme="light|dark"`)
- Monaco editor themes
- Canvas rendering (background, grid colors)
- Three.js materials (future)

Prevents Flash of inAccurate coloR Theme (FART) with inline script in HTML head.

**2. Lazy Loading**

Monaco editor loaded dynamically on first toggle:
```typescript
const monaco = await import('monaco-editor');
```

Reduces initial bundle size - editor not loaded until user opens it.

**3. Platform Abstraction**

```typescript
function isDesktop(): boolean {
  return window.__TAURI__ !== undefined;
}
```

Desktop mode uses Tauri IPC for file operations. Web mode uses File System Access API with fallback to input/download.

**4. WASM Bridge (LSP)**

```typescript
const diagnostics = engine.check_drc();
const markers = diagnostics.map(d => ({
  severity: monaco.MarkerSeverity.Error,
  startLineNumber: d.line,
  message: d.message
}));
monaco.editor.setModelMarkers(model, 'cypcb', markers);
```

No WebSocket server needed - WASM engine provides diagnostics directly.

### Build Targets

**Desktop** (`TAURI_ENV_PLATFORM=darwin|windows|linux`):
- Target: `safari13` (macOS), `chrome105` (Windows/Linux)
- Optimization: Smaller bundle size for webview

**Web** (no `TAURI_ENV_PLATFORM`):
- Target: `esnext`
- Optimization: Tree-shaking, code splitting

**WASM**:
- Build via `wasm-pack` with release profile
- Optimization: opt-level="z", LTO, strip
- Post-processing: wasm-opt -O4

## Performance Considerations

### WASM Size

- **Target**: <300 KB gzipped
- **Current**: 264 KB gzipped (29% reduction from initial 374 KB)
- **Techniques**: opt-level="z", LTO, codegen-units=1, panic="abort", strip=true, wasm-opt -O4

### Rendering

- **Canvas**: Single `getComputedStyle()` call per frame (cache theme colors)
- **ECS Queries**: Bevy ECS optimized for iteration
- **Spatial Index**: R*-tree for efficient collision detection (O(log n) vs O(n²))

### Editor Sync

- **Debounce**: 300ms delay before parsing (balances responsiveness with CPU usage)
- **Incremental**: Only re-parse and re-render on actual changes
- **Suppress-sync**: Prevents circular updates during programmatic edits

### Search

- **FTS5**: BM25 ranking for relevance (lower score = better match)
- **Index**: Auto-sync via triggers (no manual maintenance)
- **Filters**: Dynamic SQL with parameterized queries

## Deployment

### Web (Cloudflare Pages)

- **Build**: `npm run build:web` → `dist/`
- **Hosting**: Static files on Cloudflare Pages CDN
- **WASM**: Served with correct `Content-Type: application/wasm`
- **URL State**: Shareable board URLs via base64-encoded source

### Desktop (Tauri)

- **Build**: `npm run build:desktop` → platform-specific installers
- **Platforms**: Windows (MSI), macOS (DMG), Linux (AppImage, deb)
- **Auto-update**: Tauri built-in updater (future enhancement)
- **File Association**: .cypcb files open in desktop app

## Future Architecture Considerations

### Scalability

- **Large Boards**: Currently loads entire board into memory. Future: Viewport culling, level-of-detail rendering
- **Library Size**: FTS5 sufficient for <1M components. Future: Tantivy for >1M
- **Undo/Redo**: Not yet implemented. Future: Event sourcing pattern with command history

### Extensibility

- **Plugins**: No plugin system yet. Future: WASM-based plugins with sandboxed APIs
- **Custom Rules**: DRC rules hardcoded. Future: User-defined rules in DSL
- **Export Formats**: Gerber only. Future: ODB++, IPC-2581

### Multi-User

- **Collaboration**: Not yet implemented. Future: Operational transform (OT) or CRDT for real-time editing
- **Version Control**: Git-friendly DSL (line-based, deterministic). Future: Visual diff/merge tools

## See Also

- [CONTRIBUTING.md](../CONTRIBUTING.md) - Development setup guide
- [README.md](../README.md) - Project overview and quick start
- Phase documentation in `.planning/phases/` - Detailed design decisions for each development phase
