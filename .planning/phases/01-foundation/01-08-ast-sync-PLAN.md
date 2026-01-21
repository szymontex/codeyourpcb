---
phase: 01-foundation
plan: 08
type: execute
wave: 4
depends_on: ["01-05", "01-06", "01-07"]
files_modified:
  - crates/cypcb-world/src/sync.rs
  - crates/cypcb-world/src/lib.rs
autonomous: true

must_haves:
  truths:
    - "AST converts to ECS entities correctly"
    - "Source spans are preserved for error reporting"
    - "Semantic errors are collected (unknown footprint, duplicate refdes)"
  artifacts:
    - path: "crates/cypcb-world/src/sync.rs"
      provides: "AST to ECS synchronization"
      exports: ["sync_ast_to_world", "SyncError"]
      min_lines: 100
  key_links:
    - from: "crates/cypcb-world/src/sync.rs"
      to: "cypcb_parser::ast::SourceFile"
      via: "function parameter"
      pattern: "ast: &SourceFile"
    - from: "crates/cypcb-world/src/sync.rs"
      to: "BoardWorld"
      via: "function parameter"
      pattern: "world: &mut BoardWorld"
---

<objective>
Implement the AST-to-ECS synchronization layer that converts parsed AST to board entities.

Purpose: Bridge the parser and board model, converting declarative AST nodes into ECS entities while collecting semantic errors.

Output: sync_ast_to_world function that populates BoardWorld from parsed AST.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/phases/01-foundation/01-RESEARCH.md

This is the critical integration point:
- Parser produces AST (Plan 05)
- BoardWorld manages entities (Plan 06)
- Footprints provide pad data (Plan 07)
- Sync layer connects them
</context>

<tasks>

<task type="auto">
  <name>Task 1: Implement sync function and error types</name>
  <files>
    crates/cypcb-world/src/sync.rs
    crates/cypcb-world/Cargo.toml
  </files>
  <action>
Update Cargo.toml to depend on cypcb-parser:
```toml
[dependencies]
cypcb-parser = { path = "../cypcb-parser" }
```

Create sync.rs with:
```rust
use cypcb_parser::ast::{SourceFile, Definition, BoardDef, ComponentDef, NetDef, Span};
use cypcb_core::{Nm, Point, Unit};
use crate::world::BoardWorld;
use crate::components::*;
use crate::footprint::FootprintLibrary;
use miette::{Diagnostic, SourceSpan};
use thiserror::Error;
use std::collections::HashMap;

#[derive(Error, Debug, Diagnostic)]
pub enum SyncError {
    #[error("Unknown footprint: {name}")]
    #[diagnostic(code(cypcb::sync::unknown_footprint))]
    UnknownFootprint {
        name: String,
        #[source_code]
        src: String,
        #[label("footprint not found")]
        span: SourceSpan,
    },

    #[error("Duplicate reference designator: {refdes}")]
    #[diagnostic(code(cypcb::sync::duplicate_refdes))]
    DuplicateRefDes {
        refdes: String,
        #[source_code]
        src: String,
        #[label("first definition")]
        first: SourceSpan,
        #[label("duplicate")]
        duplicate: SourceSpan,
    },

    #[error("Reference to unknown component: {component}")]
    #[diagnostic(code(cypcb::sync::unknown_component))]
    UnknownComponent {
        component: String,
        #[source_code]
        src: String,
        #[label("not defined")]
        span: SourceSpan,
    },
}

pub struct SyncResult {
    pub errors: Vec<SyncError>,
    pub warnings: Vec<String>,
}

/// Synchronize AST to BoardWorld
pub fn sync_ast_to_world(
    ast: &SourceFile,
    source: &str,
    world: &mut BoardWorld,
    footprint_lib: &FootprintLibrary,
) -> SyncResult {
    let mut result = SyncResult { errors: vec![], warnings: vec![] };
    let mut refdes_spans: HashMap<String, Span> = HashMap::new();
    let mut component_entities: HashMap<String, Entity> = HashMap::new();

    for def in &ast.definitions {
        match def {
            Definition::Board(board) => {
                sync_board(board, source, world, &mut result);
            }
            Definition::Component(comp) => {
                sync_component(comp, source, world, footprint_lib,
                              &mut refdes_spans, &mut component_entities, &mut result);
            }
            Definition::Net(net) => {
                sync_net(net, source, world, &component_entities, &mut result);
            }
        }
    }

    // Rebuild spatial index after all entities are added
    world.rebuild_spatial_index();

    result
}

fn sync_board(board: &BoardDef, source: &str, world: &mut BoardWorld, result: &mut SyncResult) {
    let (width, height) = if let Some(size) = &board.size {
        (dimension_to_nm(&size.width), dimension_to_nm(&size.height))
    } else {
        result.warnings.push("Board has no size, defaulting to 100mm x 100mm".into());
        (Nm::from_mm(100.0), Nm::from_mm(100.0))
    };

    let layers = board.layers.unwrap_or(2);
    world.set_board(board.name.name.clone(), (width, height), layers);
}

fn sync_component(comp: &ComponentDef, ...) { /* Implementation */ }
fn sync_net(net: &NetDef, ...) { /* Implementation */ }
fn dimension_to_nm(dim: &Dimension) -> Nm { /* Convert using unit */ }
```
  </action>
  <verify>
`cargo build -p cypcb-world` compiles
sync_ast_to_world function exists
  </verify>
  <done>Sync function and error types implemented</done>
</task>

<task type="auto">
  <name>Task 2: Complete sync implementation with tests</name>
  <files>
    crates/cypcb-world/src/sync.rs
    crates/cypcb-world/src/lib.rs
  </files>
  <action>
Complete the sync implementation:

sync_component should:
1. Check for duplicate refdes, add error if found
2. Look up footprint in library, add error if not found
3. Convert position from AST to Nm coordinates
4. Spawn entity with components: RefDes, FootprintRef, Position, Rotation, Value, SourceSpan, ComponentKind
5. Store entity in component_entities map for net resolution

sync_net should:
1. Intern net name to NetId
2. For each pin reference:
   - Look up component entity, error if not found
   - Add NetConnection component to entity (or append to existing)

Add helper functions:
- dimension_to_nm: Convert Dimension (value + unit) to Nm
- span_to_source_span: Convert ast::Span to miette::SourceSpan

Add tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_parser::CypcbParser;

    #[test]
    fn test_sync_simple_board() {
        let source = r#"
version 1
board test {
    size 50mm x 30mm
    layers 2
}
component R1 resistor "0402" {
    value "10k"
    at 10mm, 15mm
}
"#;
        let mut parser = CypcbParser::new();
        let ast = parser.parse(source).unwrap();
        let mut world = BoardWorld::new();
        let lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&ast, source, &mut world, &lib);

        assert!(result.errors.is_empty());
        // Verify board was created
        // Verify component was spawned
    }

    #[test]
    fn test_sync_unknown_footprint() {
        // Test that unknown footprint produces error
    }

    #[test]
    fn test_sync_duplicate_refdes() {
        // Test that duplicate refdes produces error
    }
}
```

Update lib.rs to export sync module.
  </action>
  <verify>
`cargo test -p cypcb-world` all tests pass
Sync correctly populates BoardWorld from AST
Errors are collected with source spans
  </verify>
  <done>Full sync implementation with tests</done>
</task>

</tasks>

<verification>
- AST board -> Board entity with size and layers
- AST component -> Component entity with all attributes
- AST net -> NetId interned, connections attached to components
- Unknown footprint produces SyncError with span
- Duplicate refdes produces SyncError with both spans
- Spatial index rebuilt after sync
</verification>

<success_criteria>
1. All AST node types convert to ECS entities
2. Source spans preserved on entities (SourceSpan component)
3. Semantic errors include miette-compatible spans
4. Net connections correctly link components
5. Spatial index up-to-date after sync
</success_criteria>

<output>
After completion, create `.planning/phases/01-foundation/01-08-SUMMARY.md`
</output>
