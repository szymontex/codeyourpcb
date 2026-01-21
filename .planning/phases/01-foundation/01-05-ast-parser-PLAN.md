---
phase: 01-foundation
plan: 05
type: execute
wave: 3
depends_on: ["01-02", "01-03"]
files_modified:
  - crates/cypcb-parser/src/lib.rs
  - crates/cypcb-parser/src/ast.rs
  - crates/cypcb-parser/src/parser.rs
  - crates/cypcb-parser/src/errors.rs
  - crates/cypcb-parser/Cargo.toml
autonomous: true

must_haves:
  truths:
    - "Parser converts Tree-sitter CST to typed AST"
    - "All AST nodes carry source spans for error reporting"
    - "Parse errors include line/column information"
  artifacts:
    - path: "crates/cypcb-parser/src/ast.rs"
      provides: "Typed AST node definitions"
      exports: ["SourceFile", "BoardDef", "ComponentDef", "NetDef"]
      min_lines: 100
    - path: "crates/cypcb-parser/src/parser.rs"
      provides: "Tree-sitter to AST conversion"
      exports: ["CypcbParser", "parse"]
    - path: "crates/cypcb-parser/src/errors.rs"
      provides: "Parse error types with miette"
      exports: ["ParseError"]
  key_links:
    - from: "crates/cypcb-parser/src/parser.rs"
      to: "tree_sitter_cypcb::language()"
      via: "Parser::set_language"
      pattern: "set_language"
    - from: "crates/cypcb-parser/src/errors.rs"
      to: "miette::Diagnostic"
      via: "derive macro"
      pattern: "#\\[derive.*Diagnostic"
---

<objective>
Implement the AST type definitions and Tree-sitter to AST conversion logic.

Purpose: Transform the raw parse tree into strongly-typed AST nodes that preserve source locations for error reporting and can be converted to ECS entities.

Output: Working parser that produces typed AST with comprehensive error handling.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/phases/01-foundation/01-RESEARCH.md

Requirements covered:
- DEV-03: Error messages with line/column info
- DSL-01 through DSL-04 (parse all syntax)
</context>

<tasks>

<task type="auto">
  <name>Task 1: Define AST types</name>
  <files>
    crates/cypcb-parser/src/ast.rs
    crates/cypcb-parser/Cargo.toml
  </files>
  <action>
Update Cargo.toml to add cypcb-core dependency:
```toml
[dependencies]
cypcb-core = { path = "../cypcb-core" }
tree-sitter = { workspace = true }
miette = { workspace = true }
thiserror = { workspace = true }
serde = { workspace = true }
```

Create ast.rs with typed AST nodes (all with Span fields):

```rust
use cypcb_core::Unit;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceFile {
    pub version: Option<u32>,
    pub definitions: Vec<Definition>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub enum Definition {
    Board(BoardDef),
    Component(ComponentDef),
    Net(NetDef),
}

#[derive(Debug, Clone, Serialize)]
pub struct BoardDef {
    pub name: Identifier,
    pub size: Option<SizeProperty>,
    pub layers: Option<u8>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct SizeProperty {
    pub width: Dimension,
    pub height: Dimension,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComponentDef {
    pub refdes: Identifier,
    pub kind: ComponentKind,
    pub footprint: StringLit,
    pub value: Option<StringLit>,
    pub position: Option<PositionExpr>,
    pub rotation: Option<f64>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum ComponentKind {
    Resistor,
    Capacitor,
    Inductor,
    IC,
    LED,
    Connector,
    Generic,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetDef {
    pub name: Identifier,
    pub connections: Vec<PinRef>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct PinRef {
    pub component: Identifier,
    pub pin: PinId,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub enum PinId {
    Number(u32),
    Name(String),
}

// Plus: Identifier, Dimension, PositionExpr, StringLit
```
  </action>
  <verify>
`cargo build -p cypcb-parser` compiles
AST types are serializable to JSON
  </verify>
  <done>All AST node types defined with spans</done>
</task>

<task type="auto">
  <name>Task 2: Implement parser and error types</name>
  <files>
    crates/cypcb-parser/src/parser.rs
    crates/cypcb-parser/src/errors.rs
    crates/cypcb-parser/src/lib.rs
  </files>
  <action>
Create errors.rs with miette-compatible error types:
```rust
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
    #[diagnostic(code(cypcb::parse::unknown_component))]
    UnknownComponent {
        name: String,
        #[source_code]
        src: String,
        #[label("unknown type")]
        span: SourceSpan,
    },

    // Add more error variants as needed
}
```

Create parser.rs with CypcbParser struct:
- new() -> Self: Initialize tree-sitter parser with cypcb language
- parse(&mut self, source: &str) -> Result<SourceFile, Vec<ParseError>>
- Internal methods for each AST node type conversion:
  - convert_source_file(source, node, errors) -> SourceFile
  - convert_board(source, node, errors) -> Option<BoardDef>
  - convert_component(source, node, errors) -> Option<ComponentDef>
  - convert_net(source, node, errors) -> Option<NetDef>
  - Helper: get_child_by_field(node, name) -> Option<Node>
  - Helper: node_text(source, node) -> &str
  - Helper: span_of(node) -> Span

Handle ERROR nodes gracefully - collect errors but continue parsing.

Update lib.rs:
- Declare modules: ast, parser, errors
- Re-export: CypcbParser, parse function, SourceFile, ParseError
- Keep the language() function from grammar compilation
  </action>
  <verify>
`cargo test -p cypcb-parser` passes
Parser can parse simple .cypcb content and return AST
  </verify>
  <done>Parser converts Tree-sitter output to typed AST</done>
</task>

<task type="auto">
  <name>Task 3: Add parser tests</name>
  <files>
    crates/cypcb-parser/src/parser.rs
  </files>
  <action>
Add #[cfg(test)] module with tests:

1. Test basic board parsing:
```rust
#[test]
fn test_parse_board() {
    let source = r#"
version 1

board test {
    size 100mm x 50mm
    layers 2
}
"#;
    let mut parser = CypcbParser::new();
    let ast = parser.parse(source).unwrap();
    assert_eq!(ast.version, Some(1));
    assert_eq!(ast.definitions.len(), 1);
    // Check board properties
}
```

2. Test component parsing with position

3. Test net parsing with pin references

4. Test error recovery - parse file with syntax error, verify:
   - Errors are collected with correct spans
   - Valid parts still parse

5. Test span accuracy - verify start/end byte positions
  </action>
  <verify>
`cargo test -p cypcb-parser` all tests pass
Tests cover happy path and error cases
  </verify>
  <done>Parser has comprehensive test coverage</done>
</task>

</tasks>

<verification>
- Parser initializes with Tree-sitter cypcb language
- All grammar constructs convert to AST nodes
- Errors include source spans convertible to line/column
- Error recovery allows partial parsing
- AST is serializable to JSON
</verification>

<success_criteria>
1. Parse valid .cypcb files to typed AST
2. Collect multiple errors without stopping
3. All AST nodes have accurate source spans
4. Errors display with miette (code snippets, colors)
5. Tests cover board, component, net, and error cases
</success_criteria>

<output>
After completion, create `.planning/phases/01-foundation/01-05-SUMMARY.md`
</output>
