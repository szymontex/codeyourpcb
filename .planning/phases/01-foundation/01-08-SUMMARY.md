---
phase: 01-foundation
plan: 08
subsystem: world
tags: [sync, ast, ecs, semantic-errors]

dependency_graph:
  requires: ["01-05", "01-06", "01-07"]
  provides: ["sync_ast_to_world", "SyncError", "SyncResult"]
  affects: ["01-09", "02-*"]

tech_stack:
  added: []
  patterns: ["miette-error-types", "ast-visitor-pattern", "error-collection"]

key_files:
  created:
    - crates/cypcb-world/src/sync.rs
  modified:
    - crates/cypcb-world/src/lib.rs
    - crates/cypcb-world/Cargo.toml
    - Cargo.lock

decisions:
  - id: sync-error-recovery
    choice: "Continue sync on semantic errors"
    rationale: "Collect all errors for better user experience rather than failing on first"

  - id: footprint-warning
    choice: "Unknown footprints produce errors, not warnings"
    rationale: "Required for correct rendering - better to be explicit"

  - id: default-position
    choice: "Components without position default to origin (0,0)"
    rationale: "Better than failing - user can fix in subsequent edit"

metrics:
  duration: "8 minutes"
  completed: 2026-01-21
---

# Phase 1 Plan 8: AST-to-ECS Sync Summary

**One-liner:** Bridges parser and board model with semantic error collection via `sync_ast_to_world`.

## What Was Built

The AST-to-ECS synchronization layer that converts parsed `.cypcb` files into live ECS board models, collecting semantic errors along the way.

### Core Function

```rust
pub fn sync_ast_to_world(
    ast: &SourceFile,
    source: &str,
    world: &mut BoardWorld,
    footprint_lib: &FootprintLibrary,
) -> SyncResult
```

### Error Types

Three semantic error types with miette integration:

1. **UnknownFootprint** - Component references footprint not in library
2. **DuplicateRefDes** - Same reference designator used twice
3. **UnknownComponent** - Net references component not defined

All errors include source spans for precise error reporting.

### Sync Process

1. Process board definitions - create Board entity with size/layers
2. Process component definitions:
   - Check for duplicate refdes (error if found)
   - Check footprint exists (error if not)
   - Create entity with all components (RefDes, Value, Position, Rotation, FootprintRef, NetConnections, SourceSpan, ComponentKind)
   - Track for net resolution
3. Process net definitions:
   - Intern net name to NetId
   - Link components via NetConnections component
4. Rebuild spatial index using footprint courtyard bounds

## Files Changed

| File | Change |
|------|--------|
| `crates/cypcb-world/src/sync.rs` | New 747-line module |
| `crates/cypcb-world/src/lib.rs` | Export sync module and types |
| `crates/cypcb-world/Cargo.toml` | Add cypcb-parser and thiserror deps |

## Tests Added

11 unit tests covering:
- Simple board sync
- Component sync with all properties
- Net synchronization
- Unknown footprint error
- Duplicate refdes error
- Unknown component in net error
- Named pins (e.g., LED1.anode)
- Complete LED blink example
- Board defaults when size/layers missing
- Multiple nets on same component
- Source span preservation

Total crate tests: 80 unit + 48 doc = 128 tests

## API Integration

The sync function is the critical integration point:

```rust
use cypcb_parser::parse;
use cypcb_world::{BoardWorld, sync_ast_to_world};
use cypcb_world::footprint::FootprintLibrary;

let source = fs::read_to_string("blink.cypcb")?;
let parse_result = parse(&source);

if parse_result.is_ok() {
    let mut world = BoardWorld::new();
    let lib = FootprintLibrary::new();
    let sync = sync_ast_to_world(&parse_result.value, &source, &mut world, &lib);

    if sync.is_ok() {
        // World is ready for rendering/validation
    } else {
        // Report semantic errors
        for error in &sync.errors {
            eprintln!("{:?}", miette::Report::new(error.clone()));
        }
    }
}
```

## Deviations from Plan

None - plan executed exactly as written.

## Next Phase Readiness

With AST-to-ECS sync complete, the CLI (01-09) can now implement the full parse-sync-render pipeline:

1. Parse source file (cypcb-parser)
2. Sync to board model (cypcb-world::sync)
3. Validate/export (future phases)

The foundation phase is nearly complete.

---
*Generated: 2026-01-21*
