# Phase 1: Foundation - Research

**Researched:** 2026-01-21
**Domain:** DSL Parser, ECS Board Model, Rust Project Structure
**Confidence:** HIGH

## Summary

Phase 1 establishes the core infrastructure for CodeYourPCB: a Tree-sitter grammar for the DSL, an ECS-based board model, and the Rust workspace structure. The research confirms the chosen technologies are mature and well-suited for this domain.

**Key findings:**

1. **Tree-sitter** is the correct choice for incremental parsing with error recovery. The grammar.js DSL is well-documented, and Rust bindings are stable (tree-sitter 0.25). A custom grammar requires a build.rs to compile the generated C parser.

2. **bevy_ecs** can be used standalone outside the full Bevy engine. It provides the ECS architecture needed for the board model with excellent query ergonomics and no_std support for future WASM optimization.

3. **Integer nanometers (i64)** should be used for all coordinates. KiCad uses 32-bit signed integers with 1nm resolution (max ~2.14m boards). We should use i64 for headroom and consistency with Rust's preference for 64-bit integers.

4. **DSL syntax** should be declarative and git-diff-friendly. Prior art (tscircuit, SKiDL, JITX) shows different approaches - we should adopt a custom syntax optimized for readability and AI-editability rather than embedding in an existing language.

**Primary recommendation:** Start with a minimal grammar (board, component, net) and expand based on real usage. Version the grammar from day one. Use ECS for the board model with R*-tree spatial indexing from the start.

---

## Standard Stack

The established libraries/tools for this phase:

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tree-sitter | 0.25 | Grammar compilation & parsing | Industry standard (GitHub, Neovim, Zed), incremental, error-tolerant |
| tree-sitter-cli | 0.25 | Grammar development tooling | Required for grammar.js compilation |
| bevy_ecs | 0.15 | Entity Component System | Standalone capable, parallel queries, archetype storage |
| rstar | 0.12 | R*-tree spatial index | Georust ecosystem, O(log n) queries |
| thiserror | 2.0 | Error type definitions | Standard for library error types |
| miette | 7.6 | Error display with code snippets | Beautiful error messages with source spans |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| cc | 1.0 | C compiler integration | build.rs for Tree-sitter grammar |
| serde | 1.0 | Serialization | JSON output from CLI, config files |
| serde_json | 1.0 | JSON serialization | CLI output format |
| clap | 4.0 | CLI argument parsing | Command-line interface |
| tracing | 0.1 | Structured logging | Debug output, performance tracing |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| tree-sitter | rust-sitter | Pure Rust, no C toolchain needed, but less mature ecosystem |
| tree-sitter | LALRPOP | Simpler for small grammars, but no incremental parsing |
| bevy_ecs | hecs | Lighter weight, but less ergonomic queries |
| bevy_ecs | specs | Older, less maintained |
| miette | ariadne | Also excellent, miette has better derive macros |

**Installation:**

```toml
# Cargo.toml - workspace root
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.dependencies]
tree-sitter = "0.25"
bevy_ecs = "0.15"
rstar = "0.12"
thiserror = "2.0"
miette = { version = "7.6", features = ["fancy"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
clap = { version = "4.0", features = ["derive"] }
tracing = "0.1"

[build-dependencies]
cc = "1.0"
```

---

## Architecture Patterns

### Recommended Project Structure

```
codeyourpcb/
├── Cargo.toml                    # Workspace manifest (virtual)
├── Cargo.lock
├── crates/
│   ├── cypcb-core/              # Shared types, coordinates, units
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── coords.rs        # Nanometer coordinate types
│   │   │   ├── units.rs         # Millimeters, mils, inches
│   │   │   └── geometry.rs      # Point, Rect, basic shapes
│   │   └── Cargo.toml
│   │
│   ├── cypcb-parser/            # Tree-sitter grammar + AST
│   │   ├── grammar/             # Tree-sitter grammar source
│   │   │   ├── grammar.js       # Grammar definition
│   │   │   ├── src/             # Generated C parser (gitignored)
│   │   │   └── queries/         # Tree-sitter queries
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── ast.rs           # Typed AST nodes
│   │   │   ├── parser.rs        # Parser wrapper
│   │   │   ├── visitor.rs       # AST visitor trait
│   │   │   └── errors.rs        # Parse errors with spans
│   │   ├── build.rs             # Compiles tree-sitter grammar
│   │   └── Cargo.toml
│   │
│   ├── cypcb-world/             # ECS board model
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── world.rs         # BoardWorld wrapper
│   │   │   ├── components/      # ECS components
│   │   │   │   ├── mod.rs
│   │   │   │   ├── position.rs  # Position, Rotation
│   │   │   │   ├── electrical.rs # NetId, PinNumber
│   │   │   │   ├── physical.rs  # Footprint, Pad, Layer
│   │   │   │   └── metadata.rs  # RefDes, Value, SourceSpan
│   │   │   ├── spatial.rs       # R*-tree spatial index
│   │   │   ├── sync.rs          # AST -> ECS synchronization
│   │   │   └── queries.rs       # Common query patterns
│   │   └── Cargo.toml
│   │
│   └── cypcb-cli/               # Command-line interface
│       ├── src/
│       │   ├── main.rs
│       │   └── commands/
│       │       ├── mod.rs
│       │       ├── parse.rs     # Parse and output JSON
│       │       └── check.rs     # Basic validation
│       └── Cargo.toml
│
├── examples/                     # Example .cypcb files
│   └── blink.cypcb
│
└── tests/                        # Integration tests
    └── fixtures/                 # Test .cypcb files
```

### Pattern 1: Tree-sitter Grammar Structure

**What:** Define the DSL grammar in grammar.js, compile to C, bind to Rust.

**When to use:** Always for Tree-sitter based parsing.

**Example:**

```javascript
// grammar/grammar.js
module.exports = grammar({
  name: 'cypcb',

  // Whitespace and comments can appear anywhere
  extras: $ => [
    /\s/,
    $.line_comment,
    $.block_comment,
  ],

  // Reserved words for keyword optimization
  word: $ => $.identifier,

  rules: {
    // Entry point
    source_file: $ => seq(
      optional($.version_statement),
      repeat($._definition),
    ),

    version_statement: $ => seq('version', $.number),

    _definition: $ => choice(
      $.board_definition,
      $.component_definition,
      $.net_definition,
    ),

    board_definition: $ => seq(
      'board',
      field('name', $.identifier),
      '{',
      repeat($.board_property),
      '}',
    ),

    board_property: $ => choice(
      $.size_property,
      $.layers_property,
    ),

    size_property: $ => seq(
      'size',
      field('width', $.dimension),
      'x',
      field('height', $.dimension),
    ),

    // ... more rules

    // Terminals
    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,
    number: $ => /\d+(\.\d+)?/,
    dimension: $ => seq($.number, optional($.unit)),
    unit: $ => choice('mm', 'mil', 'in', 'nm'),

    line_comment: $ => token(seq('//', /.*/)),
    block_comment: $ => token(seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/')),
  }
});
```

### Pattern 2: Build.rs for Grammar Compilation

**What:** Compile the Tree-sitter C parser during cargo build.

**When to use:** Always for custom Tree-sitter grammars.

**Example:**

```rust
// crates/cypcb-parser/build.rs
fn main() {
    let grammar_dir = std::path::Path::new("grammar");
    let src_dir = grammar_dir.join("src");

    // Rerun if grammar changes
    println!("cargo:rerun-if-changed=grammar/grammar.js");
    println!("cargo:rerun-if-changed=grammar/src/parser.c");

    // Compile the Tree-sitter parser
    cc::Build::new()
        .file(src_dir.join("parser.c"))
        .include(&src_dir)
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .compile("tree-sitter-cypcb");
}
```

### Pattern 3: ECS Component Design

**What:** Define board elements as ECS components with composition.

**When to use:** All board model data.

**Example:**

```rust
// crates/cypcb-world/src/components/position.rs
use bevy_ecs::prelude::*;

/// Position in nanometers from board origin (bottom-left)
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: i64,  // nanometers
    pub y: i64,  // nanometers
}

/// Rotation in millidegrees (0-360000)
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rotation(pub i32);

impl Rotation {
    pub fn degrees(deg: f64) -> Self {
        Self((deg * 1000.0) as i32 % 360_000)
    }
}

// crates/cypcb-world/src/components/electrical.rs
/// Unique identifier for a net (interned from name)
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NetId(pub u32);

/// Component reference designator (R1, C1, U1)
#[derive(Component, Debug, Clone, PartialEq, Eq, Hash)]
pub struct RefDes(pub String);

/// Component value (10k, 100nF, ATmega328P)
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Value(pub String);

// crates/cypcb-world/src/components/metadata.rs
/// Link back to source file location
#[derive(Component, Debug, Clone, Copy)]
pub struct SourceSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: u32,
    pub start_column: u32,
}
```

### Pattern 4: Spatial Index Integration

**What:** Maintain R*-tree index alongside ECS for spatial queries.

**When to use:** Any spatial query (collision, selection, DRC).

**Example:**

```rust
// crates/cypcb-world/src/spatial.rs
use bevy_ecs::prelude::*;
use rstar::{RTree, RTreeObject, AABB, PointDistance};

/// Entry in the spatial index
#[derive(Debug, Clone)]
pub struct SpatialEntry {
    pub entity: Entity,
    pub envelope: AABB<[i64; 2]>,
    pub layer_mask: u32,
}

impl RTreeObject for SpatialEntry {
    type Envelope = AABB<[i64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

/// Spatial index resource (stored as ECS Resource)
#[derive(Resource, Default)]
pub struct SpatialIndex {
    tree: RTree<SpatialEntry>,
}

impl SpatialIndex {
    /// Query all entities in a region
    pub fn query_region(&self, min: [i64; 2], max: [i64; 2]) -> impl Iterator<Item = Entity> + '_ {
        let envelope = AABB::from_corners(min, max);
        self.tree
            .locate_in_envelope_intersecting(&envelope)
            .map(|e| e.entity)
    }

    /// Query entities at a point
    pub fn query_point(&self, point: [i64; 2]) -> impl Iterator<Item = Entity> + '_ {
        let envelope = AABB::from_point(point);
        self.tree
            .locate_in_envelope_intersecting(&envelope)
            .map(|e| e.entity)
    }

    /// Rebuild index from world (call after batch updates)
    pub fn rebuild(&mut self, entries: Vec<SpatialEntry>) {
        self.tree = RTree::bulk_load(entries);
    }
}
```

### Pattern 5: Error Handling with Source Spans

**What:** Preserve source location through error chain for helpful messages.

**When to use:** All errors that relate to source code.

**Example:**

```rust
// crates/cypcb-parser/src/errors.rs
use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
pub enum ParseError {
    #[error("Syntax error: {message}")]
    #[diagnostic(code(cypcb::parse::syntax))]
    Syntax {
        message: String,
        #[source_code]
        src: String,
        #[label("here")]
        span: SourceSpan,
    },

    #[error("Unknown component type: {name}")]
    #[diagnostic(
        code(cypcb::parse::unknown_component),
        help("Valid types are: resistor, capacitor, inductor, ic")
    )]
    UnknownComponent {
        name: String,
        #[source_code]
        src: String,
        #[label("unknown type")]
        span: SourceSpan,
    },

    #[error("Duplicate reference designator: {refdes}")]
    #[diagnostic(code(cypcb::parse::duplicate_refdes))]
    DuplicateRefDes {
        refdes: String,
        #[source_code]
        src: String,
        #[label("first defined here")]
        first: SourceSpan,
        #[label("duplicate here")]
        duplicate: SourceSpan,
    },
}
```

### Anti-Patterns to Avoid

- **Floating-point coordinates:** Never use f32/f64 for positions. Always i64 nanometers.
- **String net names at runtime:** Intern net names to NetId(u32) at parse time.
- **Parser creating ECS directly:** Parser produces AST; separate sync layer creates ECS.
- **Monolithic Component struct:** Use ECS composition, not one struct with all fields.
- **Rebuilding spatial index on every change:** Batch updates, rebuild on demand.

---

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Incremental parsing | Custom parser | Tree-sitter | Error recovery, sub-ms updates |
| Spatial queries | O(n) linear scan | rstar R*-tree | O(log n) queries, bulk loading |
| Entity storage | HashMap<Id, Component> | bevy_ecs | Archetype storage, parallel queries |
| Error display | println! with line numbers | miette | Code snippets, colors, help text |
| Coordinate conversion | Custom math | Unit types + From traits | Type safety prevents unit confusion |
| String interning | HashMap<String, u32> | string_interner crate | Thread-safe, stable IDs |

**Key insight:** The EDA domain has many solved problems. Tree-sitter, ECS, and R*-tree are battle-tested. Custom solutions will be slower and buggier.

---

## Common Pitfalls

### Pitfall 1: DSL Syntax Lock-in

**What goes wrong:** Early syntax decisions become permanent because users write code against them. Changes break all existing designs.

**Why it happens:** Rushing to "something that works" without considering evolution. Not dogfooding before release.

**How to avoid:**
1. Include `version: 1` in every file (mandatory first line)
2. Start minimal - fewer keywords are easier to evolve
3. Reserve likely-needed keywords even if not implemented
4. Extensive dogfooding before v1.0 release
5. Provide migration tools when syntax changes

**Warning signs:** Discussions about "multiple ways to do the same thing," documentation showing deprecated patterns.

### Pitfall 2: Floating-Point Accumulation

**What goes wrong:** Cumulative precision errors. Two traces that should connect don't. DRC reports false violations.

**Why it happens:** Floating-point is the default. Errors are small initially, manifest at scale.

**How to avoid:**
1. Use i64 nanometers for all internal coordinates
2. Convert at boundaries only (parse input, display output)
3. Snap to grid after operations
4. Test with coordinates far from origin

**Warning signs:** Non-deterministic test results, DRC violations that appear/disappear.

### Pitfall 3: Net Name String Comparison

**What goes wrong:** O(n * string_length) performance for net queries. Allocation-heavy. Slow DRC.

**Why it happens:** Net names are strings in source. Easy to compare names directly.

**How to avoid:**
1. Intern net names to NetId(u32) at parse time
2. Use integer comparison everywhere internally
3. Only convert back to strings for display/export

**Warning signs:** DRC performance degrades with component count.

### Pitfall 4: Monolithic AST Traversal

**What goes wrong:** Every operation walks entire AST. Adding features requires touching all traversal code.

**Why it happens:** Simple recursive descent visitor. Works for small files.

**How to avoid:**
1. Convert AST to ECS early (single traversal)
2. Use ECS queries for subsequent operations
3. Maintain SourceSpan components for error reporting

**Warning signs:** Performance degrades linearly with file size for simple operations.

### Pitfall 5: Coordinate System Confusion

**What goes wrong:** Y-axis points wrong direction. Components placed mirrored. Gerber output inverted.

**Why it happens:** Different conventions (KiCad Y-down, Gerber Y-up, math Y-up).

**How to avoid:**
1. Define single internal convention: origin bottom-left, Y-up (mathematical)
2. Convert at import/export boundaries
3. Document the convention prominently
4. Test with asymmetric designs

**Warning signs:** Components appear mirrored, silkscreen text backwards.

---

## Code Examples

### Coordinate Types with Type Safety

```rust
// crates/cypcb-core/src/coords.rs

/// Internal coordinate in nanometers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Nm(pub i64);

impl Nm {
    pub const ZERO: Nm = Nm(0);
    pub const MAX: Nm = Nm(i64::MAX);

    pub fn from_mm(mm: f64) -> Self {
        Nm((mm * 1_000_000.0) as i64)
    }

    pub fn from_mil(mil: f64) -> Self {
        Nm((mil * 25_400.0) as i64)
    }

    pub fn from_inch(inch: f64) -> Self {
        Nm((inch * 25_400_000.0) as i64)
    }

    pub fn to_mm(self) -> f64 {
        self.0 as f64 / 1_000_000.0
    }
}

impl std::ops::Add for Nm {
    type Output = Nm;
    fn add(self, rhs: Nm) -> Nm {
        Nm(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Nm {
    type Output = Nm;
    fn sub(self, rhs: Nm) -> Nm {
        Nm(self.0 - rhs.0)
    }
}

/// 2D point in nanometers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Point {
    pub x: Nm,
    pub y: Nm,
}

impl Point {
    pub fn new(x: Nm, y: Nm) -> Self {
        Self { x, y }
    }

    pub fn from_mm(x: f64, y: f64) -> Self {
        Self {
            x: Nm::from_mm(x),
            y: Nm::from_mm(y),
        }
    }
}

/// Axis-aligned bounding box
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub min: Point,
    pub max: Point,
}

impl Rect {
    pub fn new(min: Point, max: Point) -> Self {
        Self { min, max }
    }

    pub fn width(&self) -> Nm {
        Nm(self.max.x.0 - self.min.x.0)
    }

    pub fn height(&self) -> Nm {
        Nm(self.max.y.0 - self.min.y.0)
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x.0 >= self.min.x.0
            && p.x.0 <= self.max.x.0
            && p.y.0 >= self.min.y.0
            && p.y.0 <= self.max.y.0
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.min.x.0 <= other.max.x.0
            && self.max.x.0 >= other.min.x.0
            && self.min.y.0 <= other.max.y.0
            && self.max.y.0 >= other.min.y.0
    }
}
```

### AST Node with Source Span

```rust
// crates/cypcb-parser/src/ast.rs

use crate::errors::Span;

/// Complete parsed source file
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub version: Option<u32>,
    pub definitions: Vec<Definition>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Definition {
    Board(BoardDef),
    Component(ComponentDef),
    Net(NetDef),
}

#[derive(Debug, Clone)]
pub struct BoardDef {
    pub name: Identifier,
    pub properties: Vec<BoardProperty>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum BoardProperty {
    Size { width: Dimension, height: Dimension, span: Span },
    Layers { count: u32, span: Span },
}

#[derive(Debug, Clone)]
pub struct ComponentDef {
    pub kind: ComponentKind,
    pub refdes: Identifier,
    pub footprint: Identifier,
    pub value: Option<StringLit>,
    pub position: Option<PositionExpr>,
    pub nets: Vec<NetConnection>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ComponentKind {
    Resistor,
    Capacitor,
    Inductor,
    IC,
    Connector,
    // ... extensible
}

#[derive(Debug, Clone)]
pub struct NetDef {
    pub name: Identifier,
    pub connections: Vec<PinRef>,
    pub constraints: Vec<NetConstraint>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Identifier {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Dimension {
    pub value: f64,
    pub unit: Unit,
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub enum Unit {
    Mm,
    Mil,
    Inch,
    Nm,
}

/// Source span for error reporting
#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl From<Span> for miette::SourceSpan {
    fn from(span: Span) -> Self {
        (span.start, span.end - span.start).into()
    }
}
```

### Tree-sitter to AST Conversion

```rust
// crates/cypcb-parser/src/parser.rs

use tree_sitter::{Parser, Tree, Node};
use crate::ast::*;
use crate::errors::{ParseError, Span};

pub struct CypcbParser {
    parser: Parser,
}

impl CypcbParser {
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_cypcb::LANGUAGE.into())
            .expect("Failed to load cypcb grammar");
        Self { parser }
    }

    pub fn parse(&mut self, source: &str) -> Result<SourceFile, Vec<ParseError>> {
        let tree = self.parser.parse(source, None)
            .ok_or_else(|| vec![ParseError::Internal("Parse failed".into())])?;

        let mut errors = Vec::new();
        let ast = self.convert_source_file(source, tree.root_node(), &mut errors);

        if errors.is_empty() {
            Ok(ast)
        } else {
            Err(errors)
        }
    }

    fn convert_source_file(&self, source: &str, node: Node, errors: &mut Vec<ParseError>) -> SourceFile {
        let mut version = None;
        let mut definitions = Vec::new();

        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "version_statement" => {
                    version = self.convert_version(source, child, errors);
                }
                "board_definition" => {
                    if let Some(def) = self.convert_board(source, child, errors) {
                        definitions.push(Definition::Board(def));
                    }
                }
                "component_definition" => {
                    if let Some(def) = self.convert_component(source, child, errors) {
                        definitions.push(Definition::Component(def));
                    }
                }
                "ERROR" => {
                    errors.push(ParseError::Syntax {
                        message: "Unexpected syntax".into(),
                        src: source.into(),
                        span: span_of(child).into(),
                    });
                }
                _ => {}
            }
        }

        SourceFile {
            version,
            definitions,
            span: span_of(node),
        }
    }

    // ... additional conversion methods
}

fn span_of(node: Node) -> Span {
    Span {
        start: node.start_byte(),
        end: node.end_byte(),
    }
}
```

### Board World with ECS

```rust
// crates/cypcb-world/src/world.rs

use bevy_ecs::prelude::*;
use crate::components::*;
use crate::spatial::SpatialIndex;

/// Main board model container
pub struct BoardWorld {
    world: World,
}

impl BoardWorld {
    pub fn new() -> Self {
        let mut world = World::new();
        world.insert_resource(SpatialIndex::default());
        world.insert_resource(NetRegistry::default());
        Self { world }
    }

    /// Spawn a component entity
    pub fn spawn_component(
        &mut self,
        refdes: String,
        value: Option<String>,
        position: Position,
        footprint_ref: String,
        source_span: SourceSpan,
    ) -> Entity {
        let mut entity = self.world.spawn((
            RefDes(refdes),
            position,
            FootprintRef(footprint_ref),
            source_span,
        ));

        if let Some(v) = value {
            entity.insert(Value(v));
        }

        entity.id()
    }

    /// Query all components
    pub fn components(&self) -> impl Iterator<Item = (Entity, &RefDes, &Position)> {
        self.world
            .query::<(Entity, &RefDes, &Position)>()
            .iter(&self.world)
    }

    /// Query components on a specific net
    pub fn components_on_net(&self, net_id: NetId) -> Vec<Entity> {
        self.world
            .query_filtered::<Entity, With<RefDes>>()
            .iter(&self.world)
            .filter(|&entity| {
                self.world
                    .get::<NetConnections>(entity)
                    .map(|conns| conns.contains_net(net_id))
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Rebuild spatial index from current state
    pub fn rebuild_spatial_index(&mut self) {
        let entries: Vec<_> = self.world
            .query::<(Entity, &Position, &BoundingBox)>()
            .iter(&self.world)
            .map(|(entity, pos, bbox)| {
                crate::spatial::SpatialEntry {
                    entity,
                    envelope: bbox.to_aabb(pos),
                    layer_mask: 0xFFFFFFFF, // All layers for now
                }
            })
            .collect();

        self.world.resource_mut::<SpatialIndex>().rebuild(entries);
    }

    /// Get underlying ECS world for advanced queries
    pub fn ecs(&self) -> &World {
        &self.world
    }

    pub fn ecs_mut(&mut self) -> &mut World {
        &mut self.world
    }
}

/// Registry for interned net names
#[derive(Resource, Default)]
pub struct NetRegistry {
    names: Vec<String>,
    lookup: std::collections::HashMap<String, NetId>,
}

impl NetRegistry {
    pub fn intern(&mut self, name: &str) -> NetId {
        if let Some(&id) = self.lookup.get(name) {
            return id;
        }
        let id = NetId(self.names.len() as u32);
        self.names.push(name.to_string());
        self.lookup.insert(name.to_string(), id);
        id
    }

    pub fn name(&self, id: NetId) -> Option<&str> {
        self.names.get(id.0 as usize).map(|s| s.as_str())
    }
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| GUI-first design | Code-first design | 2020+ (tscircuit, JITX) | Git-friendly, AI-editable |
| OOP component model | ECS composition | 2018+ (game engines) | Parallel queries, cache-friendly |
| Hand-written parsers | Tree-sitter | 2018+ (editors) | Incremental, error-tolerant |
| Float coordinates | Integer nm | KiCad 6+ | Deterministic, no precision issues |
| XML/JSON formats | Custom DSLs | 2020+ | Human-readable, diff-friendly |

**Deprecated/outdated:**
- **nom for DSLs**: No incremental parsing, poor error recovery
- **cgmath**: Unmaintained since 2021, use nalgebra or glam
- **specs ECS**: Less maintained than bevy_ecs

---

## DSL Syntax Recommendations

Based on analysis of tscircuit, SKiDL, and JITX, the recommended syntax for CodeYourPCB:

### Design Principles

1. **Declarative over imperative** - Describe what the board is, not how to build it
2. **Git-diff friendly** - One element per line, consistent formatting
3. **AI-editable** - Clear keywords, minimal syntax sugar, no ambiguity
4. **Minimal but complete** - Start small, version for extensibility

### Proposed Syntax

```cypcb
// Example: Simple LED blink circuit
version 1

board blink {
    size 30mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value "330"
    at 10mm, 8mm
}

component LED1 led "0603" {
    at 15mm, 8mm
}

component J1 connector "pin_header_1x2" {
    at 5mm, 8mm
}

net VCC {
    J1.1
    R1.1
}

net GND {
    J1.2
    LED1.cathode
}

net LED_SIGNAL {
    R1.2
    LED1.anode
}
```

### Syntax Rationale

- **version N**: Mandatory, enables future migration
- **board name { }**: Block syntax for board properties
- **component REFDES type "footprint" { }**: All in one declaration
- **at X, Y**: Clear position syntax with units
- **net NAME { pins }**: Explicit net definition with pin references
- **PIN.PINNAME**: Dot notation for pin reference (familiar from OOP)

### Reserved Keywords (Not Yet Implemented)

```
import, module, constraint, trace, via, zone, keepout,
rotate, mirror, grid, layer, stackup, rules, test
```

---

## Open Questions

Things that couldn't be fully resolved:

1. **Relative positioning syntax**
   - What we know: Need to support "R2 is 5mm right of R1"
   - What's unclear: Exact syntax (anchor point, offset direction)
   - Recommendation: Defer to Phase 1 implementation, gather user feedback

2. **Footprint library format**
   - What we know: Need custom format + KiCad import
   - What's unclear: Whether to define footprints in DSL or separate files
   - Recommendation: Start with inline DSL definitions, add library system in Phase 2

3. **Hierarchical modules**
   - What we know: Users want reusable circuit blocks
   - What's unclear: Import syntax, parameter passing
   - Recommendation: Reserve keywords, implement in later phase

---

## Sources

### Primary (HIGH confidence)

- [Tree-sitter Grammar DSL Documentation](https://tree-sitter.github.io/tree-sitter/creating-parsers/2-the-grammar-dsl.html) - Grammar.js syntax
- [bevy_ecs Documentation](https://docs.rs/bevy_ecs/latest/bevy_ecs/) - ECS API
- [rstar Documentation](https://docs.rs/rstar/latest/rstar/) - R*-tree API
- [KiCad Coordinate System Forum](https://forum.kicad.info/t/coordinate-system-grid-and-origins-in-the-pcb-editor/24535) - Nanometer precision
- [miette Documentation](https://docs.rs/miette/latest/miette/) - Error display

### Secondary (MEDIUM confidence)

- [Using Tree-sitter Parsers in Rust](https://rfdonnelly.github.io/posts/using-tree-sitter-parsers-in-rust/) - Build.rs patterns
- [How Bevy's ECS Inspired a New Rust Backend Architecture](https://medium.com/@theopinionatedev/how-bevys-ecs-inspired-a-new-rust-backend-architecture-6426a8681672) - Standalone ECS usage
- [SKiDL Documentation](https://devbisme.github.io/skidl/) - DSL syntax patterns
- [tscircuit Documentation](https://docs.tscircuit.com/) - Code-first PCB approach
- [Large Rust Workspaces](https://matklad.github.io/2021/08/22/large-rust-workspaces.html) - Project structure

### Tertiary (LOW confidence)

- [JITX Documentation](https://docs.jitx.com/) - Commercial, limited public docs
- [IPC-7351 Footprint Naming](https://www.protoexpress.com/blog/features-of-ipc-7351-standards-to-design-pcb-component-footprint/) - Footprint conventions

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries well-documented, production-proven
- Architecture patterns: HIGH - Following established patterns (ECS, Tree-sitter)
- DSL syntax: MEDIUM - Novel design, needs validation through dogfooding
- Coordinate system: HIGH - Following KiCad's proven approach
- Pitfalls: HIGH - Based on established domain research

**Research date:** 2026-01-21
**Valid until:** 2026-03-21 (60 days - stable domain)

---

## Recommended Plan Structure

Based on research, Phase 1 should be structured as:

### Task Groups

1. **Project Setup** (2-3 tasks)
   - Workspace initialization
   - CI/CD setup
   - Development tooling

2. **Core Types** (3-4 tasks)
   - Coordinate types (Nm, Point, Rect)
   - Unit conversions
   - Basic geometry

3. **Tree-sitter Grammar** (4-5 tasks)
   - Grammar.js for minimal syntax
   - Build.rs integration
   - AST type definitions
   - Parser wrapper

4. **ECS Board Model** (4-5 tasks)
   - Component definitions
   - Entity spawning
   - Net registry
   - Spatial index integration

5. **AST-to-ECS Sync** (2-3 tasks)
   - Conversion logic
   - Source span preservation
   - Error collection

6. **CLI Skeleton** (2-3 tasks)
   - Parse command
   - JSON output
   - Error display

7. **Footprint Foundation** (2-3 tasks)
   - Pad data structures
   - Basic SMD footprints (0402, 0603, 0805)
   - Basic through-hole

### Critical Path

```
Project Setup
    |
    v
Core Types --> Grammar --> AST Types --> Parser
    |                                      |
    v                                      v
ECS Components --> Board World <-- AST-to-ECS Sync
    |                    |
    v                    v
Spatial Index -----> CLI <----- Footprints
```

### Estimated Scope

- **Total tasks:** 20-25
- **Estimated effort:** Medium (1-2 weeks for experienced Rust developer)
- **Risk areas:** Grammar design (syntax lock-in), coordinate system (must get right)
