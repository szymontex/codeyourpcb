---
phase: 01-foundation
plan: 03
type: execute
wave: 2
depends_on: ["01-01"]
files_modified:
  - crates/cypcb-parser/grammar/grammar.js
  - crates/cypcb-parser/grammar/package.json
  - crates/cypcb-parser/build.rs
  - crates/cypcb-parser/Cargo.toml
autonomous: true

must_haves:
  truths:
    - "Grammar compiles to C parser without errors"
    - "Grammar parses valid .cypcb files"
    - "Grammar recovers from syntax errors gracefully"
  artifacts:
    - path: "crates/cypcb-parser/grammar/grammar.js"
      provides: "Tree-sitter grammar definition"
      min_lines: 100
    - path: "crates/cypcb-parser/build.rs"
      provides: "Grammar compilation during cargo build"
      contains: "cc::Build"
  key_links:
    - from: "crates/cypcb-parser/build.rs"
      to: "crates/cypcb-parser/grammar/src/parser.c"
      via: "cc::Build compilation"
      pattern: 'file.*parser\.c'
---

<objective>
Create the Tree-sitter grammar for the .cypcb DSL covering board, component, and net definitions.

Purpose: Define the syntax for the CodeYourPCB DSL with error recovery support, enabling incremental parsing and helpful error messages.

Output: Compilable Tree-sitter grammar with build.rs integration.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/phases/01-foundation/01-RESEARCH.md

DSL syntax from research:
```cypcb
version 1

board blink {
    size 30mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value "330"
    at 10mm, 8mm
}

net VCC {
    J1.1
    R1.1
}
```

Requirements covered: DSL-01 (grammar), DSL-02 (board), DSL-03 (component), DSL-04 (net)
</context>

<tasks>

<task type="auto">
  <name>Task 1: Create Tree-sitter grammar definition</name>
  <files>
    crates/cypcb-parser/grammar/grammar.js
    crates/cypcb-parser/grammar/package.json
  </files>
  <action>
Create grammar/package.json for tree-sitter-cli:
```json
{
  "name": "tree-sitter-cypcb",
  "version": "0.1.0",
  "main": "bindings/node",
  "dependencies": {
    "nan": "^2.17.0"
  },
  "devDependencies": {
    "tree-sitter-cli": "^0.25.0"
  }
}
```

Create grammar/grammar.js with rules for:

1. **source_file**: optional version, repeat definitions
2. **version_statement**: `version` NUMBER
3. **board_definition**: `board` IDENTIFIER `{` board_properties `}`
4. **board_property**: size_property | layers_property | stackup_property
5. **size_property**: `size` dimension `x` dimension
6. **layers_property**: `layers` NUMBER
7. **component_definition**: `component` IDENTIFIER component_type STRING `{` component_body `}`
8. **component_type**: resistor | capacitor | inductor | ic | led | connector | generic
9. **component_body**: value_property? position_property? net_assignment*
10. **value_property**: `value` STRING
11. **position_property**: `at` dimension `,` dimension (rotation_clause)?
12. **rotation_clause**: `rotate` NUMBER (`deg` | `degrees`)?
13. **net_definition**: `net` IDENTIFIER `{` pin_refs `}`
14. **pin_ref**: IDENTIFIER `.` (IDENTIFIER | NUMBER)
15. **net_constraint**: (future: `width` dimension, `clearance` dimension)

Terminals:
- **identifier**: /[a-zA-Z_][a-zA-Z0-9_]*/
- **number**: /\d+(\.\d+)?/
- **dimension**: number unit?
- **unit**: 'mm' | 'mil' | 'in' | 'nm'
- **string**: /"[^"]*"/
- **line_comment**: //.*
- **block_comment**: /* ... */

Use `extras: $ => [/\s/, $.line_comment, $.block_comment]` for whitespace/comments.
Use `word: $ => $.identifier` for keyword optimization.
Add `field()` annotations for important nodes (name, width, height, etc).
  </action>
  <verify>
`cd crates/cypcb-parser/grammar && npx tree-sitter generate`
Check grammar/src/parser.c exists and is non-empty
  </verify>
  <done>Grammar generates valid C parser</done>
</task>

<task type="auto">
  <name>Task 2: Create build.rs for grammar compilation</name>
  <files>
    crates/cypcb-parser/build.rs
    crates/cypcb-parser/Cargo.toml
  </files>
  <action>
Update Cargo.toml build-dependencies:
```toml
[build-dependencies]
cc = "1.0"
```

Create build.rs that:
1. Sets cargo:rerun-if-changed for grammar.js and parser.c
2. Compiles grammar/src/parser.c using cc::Build
3. Suppresses C warnings (-Wno-unused-parameter, -Wno-unused-but-set-variable)
4. Names the output library "tree-sitter-cypcb"

Add extern "C" language function declaration to src/lib.rs:
```rust
extern "C" {
    fn tree_sitter_cypcb() -> tree_sitter::Language;
}

pub fn language() -> tree_sitter::Language {
    unsafe { tree_sitter_cypcb() }
}
```

Note: The grammar/src/ directory should be gitignored as it contains generated files,
but the CI must run `npx tree-sitter generate` before cargo build.
  </action>
  <verify>
`cargo build -p cypcb-parser` succeeds
No linker errors for tree_sitter_cypcb symbol
  </verify>
  <done>Grammar compiles as part of cargo build</done>
</task>

</tasks>

<verification>
- `npx tree-sitter generate` in grammar/ succeeds
- `cargo build -p cypcb-parser` compiles without errors
- Grammar handles all syntax from research:
  - version statement
  - board definition with size and layers
  - component definition with value and position
  - net definition with pin references
</verification>

<success_criteria>
1. grammar.js defines all required rules
2. tree-sitter generate produces valid parser.c
3. build.rs compiles parser during cargo build
4. Parser library links correctly
</success_criteria>

<output>
After completion, create `.planning/phases/01-foundation/01-03-SUMMARY.md`
</output>
