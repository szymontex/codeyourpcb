# S01: Foundation

**Goal:** Working DSL parser that produces a valid board model with ECS components, spatial indexing, and CLI interface
**Demo:** unit tests prove foundation works — parser reads .cypcb files, produces AST, syncs to ECS board model with spatial queries

## Must-Haves

- [x] Rust workspace with crate structure (cypcb-core, cypcb-parser, cypcb-world, cypcb-cli)
- [x] Integer nanometer coordinate types with unit conversions
- [x] Tree-sitter grammar for .cypcb DSL
- [x] ECS components for board model
- [x] AST types and parser implementation
- [x] BoardWorld with R*-tree spatial indexing
- [x] Footprint library (SMD + THT)
- [x] AST-to-ECS synchronization
- [x] CLI with parse/check commands

## Tasks

- [x] **T01: Project Setup**
  - Set up Rust workspace, crate structure, and dependencies for the CodeYourPCB project.
- [x] **T02: Core Types**
  - Define core coordinate types (Nm, Point, Rect) and unit conversions with type safety.
- [x] **T03: Tree-sitter Grammar**
  - Define Tree-sitter grammar.js for .cypcb DSL with board, component, and net declarations.
- [x] **T04: ECS Components**
  - Define ECS components for board model: Position, NetId, RefDes, FootprintRef, SourceSpan.
- [x] **T05: AST Parser**
  - Implement AST types and parser wrapper converting Tree-sitter nodes to typed AST.
- [x] **T06: BoardWorld & Spatial Indexing**
  - Create BoardWorld ECS container with spatial indexing via R*-tree for efficient queries.
- [x] **T07: Footprint Library**
  - Implement footprint library with SMD (0402-2512) and through-hole pad definitions.
- [x] **T08: AST-to-ECS Synchronization**
  - Implement AST-to-ECS synchronization layer with source span preservation.
- [x] **T09: CLI**
  - Create CLI with parse and check commands for .cypcb file processing.

## Files Likely Touched

- `Cargo.toml`
- `crates/cypcb-core/src/`
- `crates/cypcb-parser/grammar/grammar.js`
- `crates/cypcb-parser/src/`
- `crates/cypcb-world/src/`
- `crates/cypcb-cli/src/`
