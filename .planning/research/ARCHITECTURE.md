# Architecture Patterns: Code-First PCB Design Tool

**Domain:** Code-First PCB/EDA Tool (CodeYourPCB)
**Researched:** 2026-01-21
**Confidence:** HIGH (verified against established patterns from KiCad, tscircuit, bevy_ecs, tower-lsp)

---

## Executive Summary

CodeYourPCB requires a layered architecture separating:
1. **Language layer** - DSL parsing, LSP, hot reload
2. **Domain layer** - ECS-based board model with spatial indexing
3. **Validation layer** - Parallel DRC with rule engine
4. **Rendering layer** - 2D canvas (wgpu) + 3D preview (Three.js)
5. **Platform layer** - Tauri shell with WASM plugin system

The architecture follows the principle of **"code-first, graphics-second"** - the source DSL is the single source of truth, and all other representations (schematic, layout, 3D view) are derived views.

---

## System Overview Diagram

```
+------------------------------------------------------------------+
|                         TAURI SHELL                               |
|  +-------------------------------------------------------------+  |
|  |                    PLATFORM LAYER                           |  |
|  |  [File Watcher] [IPC Bridge] [Native Dialogs] [Plugin Host] |  |
|  +-------------------------------------------------------------+  |
|                              |                                    |
|  +---------------------------+----------------------------------+ |
|  |                     CORE ENGINE (WASM)                       | |
|  |                                                              | |
|  |  +----------------+    +------------------+    +-----------+ | |
|  |  | LANGUAGE LAYER |    |   DOMAIN LAYER   |    | RENDERING | | |
|  |  |                |    |                  |    |   LAYER   | | |
|  |  | [Tree-sitter]  |--->| [ECS World]      |--->| [wgpu 2D] | | |
|  |  | [LSP Server]   |    | [Spatial Index]  |    | [Canvas]  | | |
|  |  | [Hot Reload]   |    | [Command Stack]  |    +-----------+ | |
|  |  +----------------+    +--------+---------+                  | |
|  |                                 |                            | |
|  |                    +------------v-----------+                | |
|  |                    |   VALIDATION LAYER     |                | |
|  |                    |                        |                | |
|  |                    | [DRC Engine]           |                | |
|  |                    | [ERC Engine]           |                | |
|  |                    | [Rule Definitions]     |                | |
|  |                    +------------------------+                | |
|  +--------------------------------------------------------------+ |
|                              |                                    |
|  +---------------------------v----------------------------------+ |
|  |                    WEBVIEW (Frontend)                        | |
|  |  [Three.js 3D] [React UI] [Monaco Editor] [Canvas Overlay]  | |
|  +--------------------------------------------------------------+ |
+------------------------------------------------------------------+
```

---

## Component Boundaries

### 1. Language Layer

**Responsibility:** Parse DSL source into typed AST, provide IDE features, enable hot reload.

| Component | Purpose | Inputs | Outputs |
|-----------|---------|--------|---------|
| **DSL Parser** | Tree-sitter grammar for PCB DSL | Source text | Concrete Syntax Tree |
| **Semantic Analyzer** | Type checking, symbol resolution | CST | Typed AST, Diagnostics |
| **LSP Server** | IDE integration (completion, hover, goto-def) | LSP requests | LSP responses |
| **Hot Reload Controller** | Watch files, trigger incremental rebuild | File events | Rebuild commands |

**Key decisions:**
- Tree-sitter for incremental parsing (sub-millisecond re-parse on edit)
- tower-lsp for LSP implementation (async, tower-based)
- Maintain document state in-memory alongside tree-sitter tree for incremental updates

```
Source File --> [Tree-sitter Parser] --> CST
                                          |
                                          v
                                  [Semantic Analyzer]
                                          |
                                          v
                                    Typed AST --> [ECS Sync]
```

### 2. Domain Layer (ECS Board Model)

**Responsibility:** Central data model for the PCB design, spatial queries, undo/redo.

| Component | Purpose | Inputs | Outputs |
|-----------|---------|--------|---------|
| **ECS World** | Entity-component storage for all board elements | Commands | State changes |
| **Spatial Index** | R*-tree for fast region/collision queries | Geometry queries | Entity sets |
| **Command Stack** | Undo/redo via command pattern | User actions | State mutations |
| **Netlist Manager** | Track electrical connectivity | Component changes | Net updates |

**Entity Types (ECS):**
```rust
// Core entities
Component   // Resistor, Capacitor, IC, etc.
Footprint   // Physical pad layout
Net         // Electrical connection between pads
Pad         // Individual connection point
Track       // Copper trace segment
Via         // Layer transition
Zone        // Copper pour region
Layer       // PCB layer definition
```

**Component Types (ECS):**
```rust
// Transform & Geometry
Position { x: f64, y: f64 }
Rotation { angle: f64 }
BoundingBox { min: Vec2, max: Vec2 }

// Electrical
NetId(u32)
PinNumber(String)
ElectricalType { passive, power, input, output, ... }

// Physical
LayerMask(u32)
Clearance(f64)
TraceWidth(f64)

// Metadata
RefDes(String)      // R1, C1, U1
Value(String)       // 10k, 100nF
SourceSpan { start: usize, end: usize }  // Link back to DSL
```

**Spatial Index Pattern:**
```rust
// Using rstar crate for R*-tree
use rstar::{RTree, AABB};

struct SpatialIndex {
    tree: RTree<SpatialEntry>,
}

struct SpatialEntry {
    entity: Entity,
    envelope: AABB<[f64; 2]>,
    layer_mask: u32,
}

impl SpatialIndex {
    fn query_region(&self, bounds: AABB, layer: u32) -> Vec<Entity>;
    fn query_point(&self, point: [f64; 2], layer: u32) -> Vec<Entity>;
    fn nearest(&self, point: [f64; 2], count: usize) -> Vec<Entity>;
}
```

### 3. Validation Layer (DRC/ERC)

**Responsibility:** Check design against manufacturing and electrical rules.

| Component | Purpose | Inputs | Outputs |
|-----------|---------|--------|---------|
| **DRC Engine** | Design rule checking (spacing, width, etc.) | Board state, Rules | Violations |
| **ERC Engine** | Electrical rule checking (connectivity, types) | Netlist, Rules | Violations |
| **Rule Engine** | Rule definition and evaluation | Rule definitions | Check functions |
| **Violation Store** | Track and manage violations | Violations | UI markers |

**Parallel DRC Architecture:**
```
                    +------------------+
                    |  Board Partitioner|
                    +--------+---------+
                             |
              +--------------+--------------+
              |              |              |
              v              v              v
        +----------+   +----------+   +----------+
        | Worker 1 |   | Worker 2 |   | Worker N |
        | (Region) |   | (Region) |   | (Region) |
        +----+-----+   +----+-----+   +----+-----+
             |              |              |
             v              v              v
        +------------------------------------------+
        |           Violation Collector            |
        +------------------------------------------+
```

**DRC Rule Categories:**
- **Spacing rules:** Track-to-track, track-to-pad, pad-to-pad clearances
- **Width rules:** Minimum trace width, annular ring
- **Manufacturing rules:** Drill sizes, solder mask expansion
- **Layer rules:** Via span, layer stackup constraints

### 4. Rendering Layer

**Responsibility:** Visual representation of the board.

| Component | Purpose | Inputs | Outputs |
|-----------|---------|--------|---------|
| **2D Canvas** | wgpu-based PCB layout view | Board state | GPU commands |
| **Layer Compositor** | Composite multiple layers with visibility | Layer data | Final image |
| **Selection Manager** | Hit testing, selection highlighting | Mouse events | Selection state |
| **3D Preview** | Three.js board visualization | Board state | WebGL scene |

**2D Rendering Pipeline:**
```
Board State --> [Layer Extractor] --> Per-layer geometry
                                            |
                                            v
                                   [Tessellator] --> GPU buffers
                                            |
                                            v
                                   [Layer Compositor] --> Display
```

**wgpu Canvas Architecture:**
```rust
struct PcbCanvas {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layers: Vec<LayerBuffer>,
    viewport: Viewport,
}

struct LayerBuffer {
    layer_id: LayerId,
    tracks: wgpu::Buffer,      // Track geometry
    pads: wgpu::Buffer,        // Pad geometry
    zones: wgpu::Buffer,       // Zone fill geometry
    visible: bool,
    color: [f32; 4],
}
```

### 5. Platform Layer

**Responsibility:** Desktop integration, plugin hosting, file system access.

| Component | Purpose | Inputs | Outputs |
|-----------|---------|--------|---------|
| **Tauri Shell** | Native window, menus, dialogs | - | Platform services |
| **File Watcher** | Monitor source files for changes | FS events | Change notifications |
| **IPC Bridge** | Communication between Rust and WebView | Messages | Responses |
| **Plugin Host** | WASM-based plugin system | Plugin requests | Plugin responses |

**Plugin System Architecture (WASM-based):**
```
+------------------+     +-------------------+
|   Plugin Host    |     |    Plugin (WASM)  |
|   (Rust/Tauri)   |     |                   |
|                  |     |  +-------------+  |
|  +-----------+   |     |  |   Plugin    |  |
|  | Wasmtime  |<--+---->|  |   Code      |  |
|  | Runtime   |   |     |  +-------------+  |
|  +-----------+   |     |                   |
|       |          |     |  Capabilities:    |
|       v          |     |  - Read board     |
|  +-----------+   |     |  - Add components |
|  | Capability|   |     |  - Run DRC rules  |
|  | Sandbox   |   |     |  - Generate output|
|  +-----------+   |     +-------------------+
+------------------+

WIT Interface (WebAssembly Interface Types):
- query_components(filter) -> Vec<Component>
- add_component(spec) -> Result<ComponentId>
- register_drc_rule(rule) -> Result<RuleId>
- generate_output(format) -> Result<Vec<u8>>
```

---

## Project Structure

```
codeyourpcb/
├── Cargo.toml                    # Workspace manifest
├── .planning/
│   └── research/
│       └── ARCHITECTURE.md       # This file
│
├── crates/
│   ├── cypcb-core/              # Core data types, shared across all crates
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── entities.rs      # Entity type definitions
│   │   │   ├── components.rs    # ECS component definitions
│   │   │   ├── units.rs         # Physical units (mm, mil, etc.)
│   │   │   └── geometry.rs      # Geometric primitives
│   │   └── Cargo.toml
│   │
│   ├── cypcb-parser/            # DSL parser (tree-sitter based)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── grammar.rs       # Tree-sitter grammar bindings
│   │   │   ├── ast.rs           # Typed AST definitions
│   │   │   ├── semantic.rs      # Semantic analysis
│   │   │   └── errors.rs        # Parse error types
│   │   ├── tree-sitter-cypcb/   # Tree-sitter grammar definition
│   │   │   ├── grammar.js
│   │   │   └── src/
│   │   └── Cargo.toml
│   │
│   ├── cypcb-world/             # ECS world and domain model
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── world.rs         # ECS world wrapper
│   │   │   ├── commands.rs      # Command pattern implementation
│   │   │   ├── spatial.rs       # R*-tree spatial index
│   │   │   ├── netlist.rs       # Netlist management
│   │   │   └── sync.rs          # AST-to-ECS synchronization
│   │   └── Cargo.toml
│   │
│   ├── cypcb-drc/               # Design rule checking
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── engine.rs        # DRC engine
│   │   │   ├── rules/           # Rule implementations
│   │   │   │   ├── spacing.rs
│   │   │   │   ├── width.rs
│   │   │   │   └── manufacturing.rs
│   │   │   ├── parallel.rs      # Parallel execution
│   │   │   └── violations.rs    # Violation types
│   │   └── Cargo.toml
│   │
│   ├── cypcb-render/            # 2D rendering (wgpu)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── canvas.rs        # Main canvas
│   │   │   ├── layers.rs        # Layer management
│   │   │   ├── tessellation.rs  # Geometry tessellation
│   │   │   ├── shaders/         # WGSL shaders
│   │   │   └── selection.rs     # Selection/hit testing
│   │   └── Cargo.toml
│   │
│   ├── cypcb-lsp/               # LSP server
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── server.rs        # tower-lsp server impl
│   │   │   ├── completion.rs    # Autocomplete
│   │   │   ├── hover.rs         # Hover information
│   │   │   ├── diagnostics.rs   # Error reporting
│   │   │   └── goto.rs          # Go to definition
│   │   └── Cargo.toml
│   │
│   ├── cypcb-plugins/           # Plugin system
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── host.rs          # Wasmtime plugin host
│   │   │   ├── sandbox.rs       # Capability sandbox
│   │   │   └── api.rs           # Plugin API definitions
│   │   ├── wit/                 # WIT interface definitions
│   │   │   └── plugin.wit
│   │   └── Cargo.toml
│   │
│   ├── cypcb-export/            # Export formats
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── gerber.rs        # Gerber output
│   │   │   ├── kicad.rs         # KiCad S-expression export
│   │   │   ├── bom.rs           # Bill of materials
│   │   │   └── pick_place.rs    # Pick and place files
│   │   └── Cargo.toml
│   │
│   └── cypcb-wasm/              # WASM bindings for core
│       ├── src/
│       │   └── lib.rs           # wasm-bindgen exports
│       └── Cargo.toml
│
├── src-tauri/                   # Tauri application
│   ├── src/
│   │   ├── main.rs
│   │   ├── commands.rs          # Tauri IPC commands
│   │   ├── file_watcher.rs      # File system watcher
│   │   └── plugin_loader.rs     # Plugin loading
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── src/                         # Frontend (WebView)
│   ├── components/
│   │   ├── Editor.tsx           # Monaco editor wrapper
│   │   ├── Canvas2D.tsx         # 2D board view (wgpu)
│   │   ├── Preview3D.tsx        # Three.js 3D preview
│   │   ├── PropertyPanel.tsx    # Component properties
│   │   └── ErrorList.tsx        # DRC violations list
│   ├── stores/
│   │   ├── board.ts             # Board state
│   │   └── editor.ts            # Editor state
│   ├── App.tsx
│   └── main.tsx
│
└── examples/                    # Example DSL files
    ├── blink.cypcb              # Simple LED blink circuit
    └── arduino_shield.cypcb     # More complex example
```

---

## Data Flow Diagrams

### 1. Source-to-Board Flow (Initial Load)

```
+-------------+     +---------------+     +------------------+
| Source File |---->| Tree-sitter   |---->| Concrete Syntax  |
| (.cypcb)    |     | Parser        |     | Tree (CST)       |
+-------------+     +---------------+     +--------+---------+
                                                   |
                                                   v
                                          +------------------+
                                          | Semantic         |
                                          | Analyzer         |
                                          +--------+---------+
                                                   |
                    +------------------------------+
                    |
                    v
+------------------+     +------------------+     +------------------+
| Typed AST       |---->| ECS Sync         |---->| ECS World        |
|                 |     | (creates/updates)|     | (Components +    |
|                 |     |                  |     |  Spatial Index)  |
+------------------+     +------------------+     +--------+---------+
                                                          |
                    +-------------------------------------+
                    |
                    v
+------------------+     +------------------+
| Render Extract  |---->| GPU Buffers      |---->  Display
| (per layer)     |     | (wgpu)           |
+------------------+     +------------------+
```

### 2. Hot Reload Flow (File Change)

```
File Save Event
      |
      v
+------------------+     +------------------+
| File Watcher     |---->| Debounce/Batch   |
| (notify crate)   |     | (100ms window)   |
+------------------+     +--------+---------+
                                  |
                                  v
                         +------------------+
                         | Tree-sitter      |
                         | Incremental      |
                         | Re-parse         |
                         +--------+---------+
                                  |
                    +-------------+-------------+
                    |                           |
                    v                           v
           +---------------+           +---------------+
           | Changed Nodes |           | Unchanged     |
           | (diff)        |           | Nodes (skip)  |
           +-------+-------+           +---------------+
                   |
                   v
           +------------------+
           | Incremental ECS  |
           | Update (commands)|
           +--------+---------+
                    |
                    v
           +------------------+
           | Partial Re-render|
           | (dirty regions)  |
           +------------------+
```

### 3. DRC Flow (Parallel Checking)

```
Board Change Event
      |
      v
+------------------+     +------------------+
| DRC Scheduler    |---->| Board Partitioner|
| (debounced)      |     | (spatial grid)   |
+------------------+     +--------+---------+
                                  |
         +------------------------+------------------------+
         |                        |                        |
         v                        v                        v
+------------------+     +------------------+     +------------------+
| Worker Thread 1  |     | Worker Thread 2  |     | Worker Thread N  |
| (Region A)       |     | (Region B)       |     | (Region N)       |
|                  |     |                  |     |                  |
| - Spacing checks |     | - Spacing checks |     | - Spacing checks |
| - Width checks   |     | - Width checks   |     | - Width checks   |
+--------+---------+     +--------+---------+     +--------+---------+
         |                        |                        |
         +------------------------+------------------------+
                                  |
                                  v
                         +------------------+
                         | Violation        |
                         | Collector        |
                         | (merge + dedup)  |
                         +--------+---------+
                                  |
                                  v
                         +------------------+
                         | UI Update        |
                         | (markers, list)  |
                         +------------------+
```

### 4. LSP Flow (IDE Integration)

```
                         +------------------+
                         | External IDE     |
                         | (VSCode, etc.)   |
                         +--------+---------+
                                  |
                         LSP Protocol (JSON-RPC)
                                  |
                                  v
+------------------+     +------------------+
| tower-lsp        |<--->| Document State   |
| Server           |     | Manager          |
+--------+---------+     +------------------+
         |
         |  textDocument/didChange
         v
+------------------+     +------------------+
| Incremental      |---->| Semantic         |
| Parse            |     | Analysis         |
+------------------+     +--------+---------+
                                  |
         +------------------------+------------------------+
         |                        |                        |
         v                        v                        v
+------------------+     +------------------+     +------------------+
| Diagnostics      |     | Completion       |     | Hover Info       |
| (errors/warnings)|     | Items            |     |                  |
+------------------+     +------------------+     +------------------+
         |                        |                        |
         +------------------------+------------------------+
                                  |
                                  v
                         +------------------+
                         | LSP Response     |
                         | (to IDE)         |
                         +------------------+
```

### 5. Plugin Execution Flow

```
Plugin Request (from UI or API)
      |
      v
+------------------+     +------------------+
| Plugin Host      |---->| Capability       |
| (wasmtime)       |     | Check            |
+------------------+     +--------+---------+
                                  |
                         Allowed? |
                    +-------------+-------------+
                    | YES                       | NO
                    v                           v
           +------------------+         +------------------+
           | WASM Sandbox     |         | Permission       |
           | Execution        |         | Denied Error     |
           +--------+---------+         +------------------+
                    |
                    v
           +------------------+
           | Host Function    |
           | Calls (via WIT)  |
           +--------+---------+
                    |
         +----------+----------+
         |                     |
         v                     v
+------------------+   +------------------+
| Query Board      |   | Modify Board     |
| (read-only)      |   | (via commands)   |
+------------------+   +------------------+
                    |
                    v
           +------------------+
           | Plugin Response  |
           | (to host)        |
           +------------------+
```

---

## Architectural Patterns

### Pattern 1: ECS for Board Model

**What:** Use Entity-Component-System architecture for the board data model instead of traditional OOP hierarchies.

**Why:**
- Natural fit for spatial queries (entities with Position component)
- Efficient parallel iteration for DRC
- Easy to add new "aspects" to entities without modifying existing code
- Memory-efficient for large boards (SoA layout)

**Example:**
```rust
// Creating a resistor
let resistor = world.spawn((
    RefDes("R1".into()),
    Value("10k".into()),
    Position { x: 25.4, y: 12.7 },
    Rotation { angle: 0.0 },
    Footprint::from_lib("0805"),
    NetConnections(vec![
        (PinNumber("1".into()), NetId(1)),
        (PinNumber("2".into()), NetId(2)),
    ]),
));

// Query all components on a specific net
for (entity, pos, refdes) in world.query::<(Entity, &Position, &RefDes)>()
    .filter(|e| world.get::<NetConnections>(e).contains_net(net_id))
{
    // Process components on net
}
```

### Pattern 2: Command Pattern for Undo/Redo

**What:** Encapsulate all state modifications as reversible command objects.

**Why:**
- Full undo/redo support
- Transaction batching (multiple operations as single undo step)
- Enables optimistic UI updates
- Audit trail of all changes

**Example:**
```rust
trait Command: Send + Sync {
    fn execute(&self, world: &mut World) -> Result<(), CommandError>;
    fn undo(&self, world: &mut World) -> Result<(), CommandError>;
    fn description(&self) -> &str;
}

struct MoveComponentCommand {
    entity: Entity,
    old_position: Position,
    new_position: Position,
}

impl Command for MoveComponentCommand {
    fn execute(&self, world: &mut World) -> Result<(), CommandError> {
        world.get_mut::<Position>(self.entity)?.clone_from(&self.new_position);
        Ok(())
    }

    fn undo(&self, world: &mut World) -> Result<(), CommandError> {
        world.get_mut::<Position>(self.entity)?.clone_from(&self.old_position);
        Ok(())
    }

    fn description(&self) -> &str {
        "Move component"
    }
}
```

### Pattern 3: Incremental Parsing with Tree-sitter

**What:** Use tree-sitter's incremental parsing to efficiently re-parse only changed portions.

**Why:**
- Sub-millisecond re-parse times on edit
- Enables real-time syntax highlighting and error checking
- Concrete syntax tree preserves all source information

**Example:**
```rust
struct DocumentState {
    source: String,
    tree: Tree,
    parser: Parser,
}

impl DocumentState {
    fn apply_edit(&mut self, edit: &TextEdit) {
        // Apply text edit to source
        self.source.replace_range(edit.range(), &edit.new_text);

        // Tell tree-sitter about the edit
        self.tree.edit(&InputEdit {
            start_byte: edit.start_byte,
            old_end_byte: edit.old_end_byte,
            new_end_byte: edit.new_end_byte,
            start_position: edit.start_position,
            old_end_position: edit.old_end_position,
            new_end_position: edit.new_end_position,
        });

        // Incremental re-parse (only changed regions)
        self.tree = self.parser.parse(&self.source, Some(&self.tree)).unwrap();
    }
}
```

### Pattern 4: Capability-based Plugin Sandbox

**What:** Use WASM with explicit capability grants for plugin isolation.

**Why:**
- Plugins cannot access anything not explicitly granted
- Prevents malicious/buggy plugins from damaging system
- Fine-grained permission control
- Cross-platform (plugins work on any OS)

**Example:**
```rust
// Plugin capabilities
enum PluginCapability {
    ReadBoard,              // Can query board state
    ModifyBoard,            // Can make changes (via commands)
    RegisterDrcRule,        // Can add custom DRC rules
    FileSystemRead(PathBuf), // Can read specific paths
    NetworkAccess(String),  // Can access specific hosts
}

struct PluginSandbox {
    engine: wasmtime::Engine,
    capabilities: HashSet<PluginCapability>,
}

impl PluginSandbox {
    fn call_host_function(&self, func: &str, args: &[WasmValue]) -> Result<WasmValue> {
        // Check capability before allowing host function call
        let required_cap = self.required_capability(func);
        if !self.capabilities.contains(&required_cap) {
            return Err(PluginError::PermissionDenied(func.to_string()));
        }
        // Execute host function...
    }
}
```

### Pattern 5: Spatial Indexing with R*-tree

**What:** Use R*-tree for all spatial queries (collision detection, region queries, hit testing).

**Why:**
- O(log n) queries instead of O(n)
- Critical for DRC performance
- Enables efficient hit testing for selection
- Bulk loading for initial board load

**Example:**
```rust
use rstar::{RTree, AABB, PointDistance};

struct BoardSpatialIndex {
    // Separate trees per layer for efficiency
    per_layer: HashMap<LayerId, RTree<SpatialEntry>>,
    // Combined tree for cross-layer queries
    all_layers: RTree<SpatialEntry>,
}

impl BoardSpatialIndex {
    fn query_clearance_violations(&self, layer: LayerId, clearance: f64) -> Vec<(Entity, Entity)> {
        let mut violations = Vec::new();
        let tree = &self.per_layer[&layer];

        for entry in tree.iter() {
            let expanded = entry.envelope.expanded(clearance);
            for nearby in tree.locate_in_envelope_intersecting(&expanded) {
                if entry.entity != nearby.entity {
                    let dist = entry.envelope.distance_2(&nearby.envelope);
                    if dist < clearance {
                        violations.push((entry.entity, nearby.entity));
                    }
                }
            }
        }
        violations
    }
}
```

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Bidirectional Source-Model Sync

**What:** Trying to sync changes from both DSL source AND graphical editing back to a single source of truth.

**Why bad:**
- Extremely complex to implement correctly
- Round-trip conversions lose formatting/comments
- Merge conflicts become intractable
- Code drift from visual edits

**Instead:** Source file is ALWAYS the source of truth. Graphical edits generate DSL code that gets inserted/modified in the source, then re-parsed.

### Anti-Pattern 2: Monolithic State Object

**What:** Single massive struct holding all board state.

**Why bad:**
- Cannot parallelize access
- All updates touch same memory
- Hard to serialize incrementally
- Difficult to add new features

**Instead:** Use ECS with components. State is distributed across many small, focused components that can be accessed independently.

### Anti-Pattern 3: Direct DOM/GPU Updates on Every Change

**What:** Immediately updating rendering on every state change.

**Why bad:**
- Thrashing during rapid edits (typing)
- Wasted work for intermediate states
- Poor perceived performance

**Instead:** Batch updates with frame-aligned rendering. Debounce state changes, then render at vsync.

### Anti-Pattern 4: Synchronous DRC

**What:** Running DRC synchronously on the main thread.

**Why bad:**
- Blocks UI during checks
- Cannot leverage multi-core
- Poor UX for large boards

**Instead:** Run DRC in background workers. Report results incrementally. Allow cancellation on new edits.

### Anti-Pattern 5: Tight Coupling Between Parser and ECS

**What:** Parser directly creates ECS entities.

**Why bad:**
- Cannot reuse parser for LSP (which needs AST, not ECS)
- Hard to test parser in isolation
- Cannot do incremental ECS updates

**Instead:** Parser produces typed AST. Separate "sync" layer converts AST to ECS operations. This allows AST diffing for incremental updates.

---

## Build Order (Dependency Graph)

The crates should be built in this order based on dependencies:

```
Level 0 (No internal deps):
  cypcb-core          # Shared types, units, geometry

Level 1 (Depends on core):
  cypcb-parser        # DSL parsing (tree-sitter)
  cypcb-world         # ECS world

Level 2 (Depends on parser + world):
  cypcb-drc           # Design rule checking
  cypcb-render        # 2D rendering
  cypcb-export        # Export formats

Level 3 (Depends on all above):
  cypcb-lsp           # LSP server
  cypcb-plugins       # Plugin system
  cypcb-wasm          # WASM bindings

Level 4 (Application):
  src-tauri           # Desktop app
  src/                # Frontend
```

**Suggested Implementation Phases:**

1. **Phase 1: Foundation**
   - `cypcb-core` - Define all shared types
   - `cypcb-parser` - Basic DSL grammar and parsing
   - Minimal working parser that can read simple circuits

2. **Phase 2: Domain Model**
   - `cypcb-world` - ECS world with spatial indexing
   - AST-to-ECS synchronization
   - Command pattern infrastructure

3. **Phase 3: Rendering**
   - `cypcb-render` - Basic 2D canvas
   - Layer rendering, zoom/pan
   - Selection and hit testing

4. **Phase 4: Validation**
   - `cypcb-drc` - DRC engine
   - Basic rules (spacing, width)
   - Parallel execution

5. **Phase 5: IDE Integration**
   - `cypcb-lsp` - LSP server
   - Hot reload system
   - Diagnostics

6. **Phase 6: Export & Plugins**
   - `cypcb-export` - Gerber, KiCad export
   - `cypcb-plugins` - WASM plugin system

7. **Phase 7: Desktop Application**
   - Tauri integration
   - Full UI
   - 3D preview

---

## Scalability Considerations

| Concern | At 100 components | At 10K components | At 100K components |
|---------|-------------------|-------------------|-------------------|
| **Parsing** | <10ms | <100ms | <500ms (incremental: <10ms) |
| **ECS Queries** | Trivial | Fast (use archetypes) | Use parallel queries |
| **Spatial Index** | Trivial | Fast (R*-tree) | Bulk load, consider grid |
| **DRC** | <100ms | 1-5s (parallel) | 10-30s (partition + parallel) |
| **Rendering** | 60fps easy | 60fps with culling | LOD + culling required |
| **Memory** | <50MB | <200MB | <1GB |

---

## Sources

### Architecture References
- [KiCad Developer Documentation - Components](https://dev-docs.kicad.org/en/components/index.html)
- [KiCad S-Expression Format](https://dev-docs.kicad.org/en/file-formats/sexpr-intro/)
- [KiCad Board File Format](https://dev-docs.kicad.org/en/file-formats/sexpr-pcb/index.html)
- [tscircuit - Code-First PCB Design](https://tscircuit.com/)
- [JITX - Software-defined Electronics](https://www.jitx.com/)
- [SKiDL - Python PCB Description](https://devbisme.github.io/skidl/)

### ECS and Bevy
- [Bevy ECS Quick Start](https://bevy.org/learn/quick-start/getting-started/ecs/)
- [bevy_ecs Crate Documentation](https://docs.rs/bevy_ecs/latest/bevy_ecs/)
- [bevy_ecs GitHub](https://github.com/bevyengine/bevy/blob/main/crates/bevy_ecs/README.md)

### Rendering
- [wgpu Documentation](https://docs.rs/wgpu/latest/wgpu/)
- [wgpu_canvas Crate](https://crates.io/crates/wgpu_canvas)

### Spatial Indexing
- [rstar - R*-tree for Rust](https://github.com/georust/rstar)
- [rstar Documentation](https://docs.rs/rstar/)

### LSP
- [tower-lsp GitHub](https://github.com/ebkalderon/tower-lsp)
- [tower-lsp Documentation](https://docs.rs/tower-lsp)

### Tree-sitter
- [Tree-sitter GitHub](https://github.com/tree-sitter/tree-sitter)

### Command Pattern
- [Command Pattern in Rust](https://refactoring.guru/design-patterns/command/rust/example)
- [Rust Design Patterns - Command](https://rust-unofficial.github.io/patterns/patterns/behavioural/command.html)

### Plugin Systems
- [Extism Plugin System](https://extism.org/docs/concepts/plug-in-system/)
- [WASM Component Model Plugins](https://tartanllama.xyz/posts/wasm-plugins/)

### DRC
- [Design Rule Checking - Synopsys](https://www.synopsys.com/glossary/what-is-design-rule-checking.html)
- [DRC - Wikipedia](https://en.wikipedia.org/wiki/Design_rule_checking)
