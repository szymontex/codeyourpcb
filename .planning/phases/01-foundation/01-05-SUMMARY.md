---
phase: 01-foundation
plan: 05
subsystem: parser
tags: [tree-sitter, ast, parsing, error-handling, miette]

dependency_graph:
  requires: ["01-02", "01-03"]
  provides: ["typed-ast", "parse-errors", "cst-to-ast-conversion"]
  affects: ["01-06", "01-08"]

tech_stack:
  added: []
  patterns:
    - "CST to AST conversion pattern"
    - "Error recovery with partial results"
    - "miette Diagnostic integration"

key_files:
  created:
    - crates/cypcb-parser/src/ast.rs
    - crates/cypcb-parser/src/parser.rs
    - crates/cypcb-parser/src/errors.rs
  modified:
    - crates/cypcb-parser/src/lib.rs
    - crates/cypcb-parser/Cargo.toml

decisions:
  - id: ast-span-tracking
    choice: "All AST nodes carry Span for source mapping"
    rationale: "Required for error reporting with line/column info"
  - id: error-recovery
    choice: "ParseResult returns partial AST + errors"
    rationale: "Continue parsing after errors for better tooling"
  - id: choice-node-handling
    choice: "Unwrap Tree-sitter choice nodes to get actual type"
    rationale: "Grammar uses choice() for board_property/net_constraint"

metrics:
  duration: "7 minutes"
  completed: "2026-01-21"
  tests_added: 17
  tests_total: 36
  lines_added: ~1900
---

# Phase 01 Plan 05: AST Parser Summary

Tree-sitter CST to typed AST conversion with miette error reporting

## What Was Built

### AST Types (ast.rs - ~500 lines)

Typed AST node definitions that mirror the grammar structure:

- **SourceFile**: Root node with optional version and definitions list
- **Definition**: Enum of Board, Component, Net
- **BoardDef**: Board name, size, layers, optional stackup
- **ComponentDef**: RefDes, kind, footprint, value, position, rotation
- **NetDef**: Net name, constraints, connections
- **Supporting types**: Span, Dimension, Identifier, StringLit, PinRef, PinId

All types:
- Derive `Serialize` and `Deserialize` for JSON output
- Carry `Span` for source location tracking
- Implement `Clone` and `Debug`

### Error Types (errors.rs - ~200 lines)

Miette-compatible error types for rich error display:

- **ParseError**: 8 variants covering syntax, semantic, and validation errors
  - `Syntax`: Tree-sitter ERROR nodes
  - `UnknownComponent`, `UnknownLayerType`, `UnknownUnit`
  - `InvalidNumber`, `Missing`, `InvalidVersion`, `InvalidLayers`

- **ParseResult<T>**: Error recovery wrapper
  - Returns partial AST even with errors
  - `is_ok()`, `has_errors()` query methods
  - `into_result()` for strict error handling

### Parser (parser.rs - ~1000 lines)

Tree-sitter to AST conversion:

- **CypcbParser**: Main parser struct
  - `new()`: Initialize with cypcb language
  - `parse(&mut self, source: &str) -> ParseResult<SourceFile>`

- **Conversion methods** for each node type:
  - `convert_source_file`, `convert_board`, `convert_component`, `convert_net`
  - Handles nested choice nodes (board_property, net_constraint)
  - Collects ERROR nodes for error reporting

- **Helper functions**: `node_text()`, `get_child_by_field()`, `span_of()`

- **Convenience function**: `parse(source: &str) -> ParseResult<SourceFile>`

### Test Coverage

36 tests total (17 new parser tests + existing):

1. **Board parsing**: size, layers, default units
2. **Component parsing**: all 10 types, position, rotation, value
3. **Net parsing**: pin refs (numeric/named), constraints
4. **Error handling**: syntax errors, error recovery, multiple errors
5. **Edge cases**: version only, no version, decimal dimensions, comments

## Verification Results

| Criteria | Status |
|----------|--------|
| Parser initializes with Tree-sitter cypcb language | PASS |
| All grammar constructs convert to AST nodes | PASS |
| Errors include source spans (miette SourceSpan) | PASS |
| Error recovery allows partial parsing | PASS |
| AST is serializable to JSON | PASS |

## Decisions Made

### 1. Span on Every AST Node

Every AST node carries a `Span` with start/end byte offsets. This enables:
- Error messages with source location
- Hover information in LSP
- Code formatting preservation

### 2. Error Recovery with Partial Results

`ParseResult<T>` returns both value and errors. Even with syntax errors, the parser:
- Extracts valid definitions
- Reports all errors found
- Enables incremental tooling

### 3. Choice Node Unwrapping

Tree-sitter's `choice()` creates wrapper nodes. The parser:
- Checks for wrapper node kinds (board_property, net_constraint)
- Extracts the actual child node
- Handles both wrapped and unwrapped cases

## Deviations from Plan

None - plan executed exactly as written.

## Next Phase Readiness

### For 01-06 Board World

- AST types ready for conversion to ECS entities
- Span information enables SourceSpan component linking
- Definition enum provides clear entity creation dispatch

### For 01-08 AST Sync

- Parser supports incremental re-parsing
- Span tracking enables change detection
- Error recovery allows partial updates

## Files Changed

```
crates/cypcb-parser/
  src/
    ast.rs        [NEW] ~500 lines - AST type definitions
    parser.rs     [NEW] ~1000 lines - CST to AST conversion
    errors.rs     [NEW] ~200 lines - Parse error types
    lib.rs        [MOD] Added module exports and re-exports
  Cargo.toml      [MOD] Added serde, serde_json dependencies
```

## API Surface

```rust
// Main parsing API
pub fn parse(source: &str) -> ParseResult<SourceFile>;
pub struct CypcbParser { ... }

// AST types
pub struct SourceFile { version, definitions, span }
pub enum Definition { Board, Component, Net }
pub struct BoardDef { name, size, layers, stackup, span }
pub struct ComponentDef { refdes, kind, footprint, value, position, rotation, span }
pub struct NetDef { name, constraints, connections, span }

// Error handling
pub enum ParseError { Syntax, UnknownComponent, ... }
pub struct ParseResult<T> { value, errors }
```

---
*Completed: 2026-01-21 13:50 UTC*
