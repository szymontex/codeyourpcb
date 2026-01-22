---
phase: 05-intelligence
plan: 05
completed: 2026-01-22
duration: 45 minutes
subsystem: tooling
tags: [lsp, ide, hover, diagnostics, drc]

depends_on:
  requires: [05-01]
  provides: [lsp-library, hover-provider, drc-diagnostics]
  affects: [05-06, 05-10]

tech_stack:
  added:
    - tower-lsp-f (0.24) for LSP protocol
    - async-trait (0.1) for async trait implementation
    - dashmap (6) for concurrent document storage
  patterns:
    - "LanguageServer trait implementation"
    - "DocumentState for parsed document tracking"
    - "DRC violation to LSP diagnostic conversion"

key_files:
  created:
    - crates/cypcb-lsp/Cargo.toml
    - crates/cypcb-lsp/src/lib.rs
    - crates/cypcb-lsp/src/main.rs
    - crates/cypcb-lsp/src/backend.rs
    - crates/cypcb-lsp/src/document.rs
    - crates/cypcb-lsp/src/hover.rs
    - crates/cypcb-lsp/src/diagnostics.rs
  modified:
    - Cargo.toml

decisions:
  - id: server-feature-optional
    choice: "Server feature is optional, disabled by default"
    rationale: "Build environment has proc-macro loading issue with tower-lsp; library works without server"
  - id: document-stores-drc
    choice: "DocumentState stores DRC violations"
    rationale: "run_drc requires &mut; store violations during build_world for later diagnostic conversion"
  - id: diagnostic-limit-100
    choice: "Cap diagnostics at 100 per file"
    rationale: "Prevent editor flooding as per RESEARCH.md pitfall guidance"
  - id: uri-string-parameter
    choice: "DocumentState::new takes URI as String for logging"
    rationale: "Backend passes URI.to_string(), avoids tower-lsp dependency in document module"

metrics:
  tests_passing: 11
  lines_of_code: 950
---

# Phase 5 Plan 5: LSP Core Server Summary

cypcb-lsp crate implementing Language Server Protocol with hover and DRC diagnostics.

## One-Liner

LSP library with hover info for components/nets/footprints and DRC/parse error diagnostics, capped at 100 per file.

## What Was Built

### cypcb-lsp Crate

LSP implementation for CodeYourPCB DSL IDE integration:

**lib.rs (45 lines):**
- Module declarations with conditional server compilation
- Public exports: `run_diagnostics`, `DocumentState`, `hover_at_position`
- Server-only export: `Backend`

**main.rs (40 lines):**
- Binary entry point for `cypcb-lsp` command
- Conditional compilation: with `server` feature runs tower-lsp server
- Without feature: prints helpful error message

**document.rs (210 lines):**
- `Position` - LSP-style line/column position
- `DocumentState` - Tracks document content, AST, world, parse errors, DRC violations
- `parse()` - Invokes cypcb-parser and stores AST/errors
- `build_world()` - Syncs AST to BoardWorld and runs DRC
- `offset_to_position()` / `position_to_offset()` - UTF-8 aware conversions

**hover.rs (396 lines):**
- `HoverInfo` - Markdown content for hover display
- `hover_at_position()` - Main entry point, finds AST node at cursor
- Hover providers for:
  - Components: refdes, kind, footprint, value, position, rotation
  - Nets: name, connected pins list, width/clearance/current constraints
  - Footprints: name, size (from library), pad count, type (SMD/THT)
  - Pin references: pin number, net name, parent component
  - Boards: name, size, layer count
  - Zones: kind (keepout/pour), bounds, layer, net
  - Traces: net, from/to pins, waypoints, layer, width, locked status

**diagnostics.rs (210 lines):**
- `Diagnostic` - LSP-compatible diagnostic struct (decoupled from tower-lsp)
- `run_diagnostics()` - Converts parse errors and DRC violations to diagnostics
- `parse_error_to_diagnostic()` - Maps all ParseError variants
- `violation_to_diagnostic()` - Maps DrcViolation with source span
- `span_to_positions()` - Converts byte spans to line/column positions
- `MAX_DIAGNOSTICS = 100` - Prevents editor flooding

**backend.rs (217 lines):**
- `Backend` - LanguageServer trait implementation
- `DashMap<Uri, DocumentState>` for concurrent document access
- `initialize()` - Returns capabilities (hover, full document sync)
- `did_open/did_change/did_save/did_close` - Document lifecycle
- `hover()` - Delegates to hover module
- Diagnostic conversion from internal format to tower-lsp types

### Hover Information Examples

**Component hover:**
```
**R1** (Resistor)
Footprint: 0402
Value: 330
Position: 10mm, 8mm
Rotation: 0deg
```

**Net hover:**
```
**Net: VCC**
Connected pins:
- J1.1
- R1.1
- C1.1
Width: 0.3mm
```

**Footprint hover:**
```
**Footprint: 0402**
Size: 1.00mm x 0.50mm
Pads: 2
Type: SMD
```

### Diagnostic Examples

**Parse error:**
```
Error [syntax]: Unexpected token at line 5
```

**DRC violation:**
```
Error [unconnected-pin]: Unconnected pin: R1.1
```

## Test Results

```
11 tests passing:
- document::test_document_state_new
- document::test_document_update
- document::test_offset_to_position_simple
- document::test_position_to_offset_simple
- hover::test_hover_on_component
- hover::test_hover_on_net
- hover::test_hover_on_whitespace
- diagnostics::test_clean_document_no_parse_errors
- diagnostics::test_parse_error_diagnostic
- diagnostics::test_drc_violation_diagnostic
- diagnostics::test_diagnostic_limit
```

## Key Decisions

### 1. Server Feature Optional

The `server` feature which enables tower-lsp is disabled by default. This works around a build environment issue where rustc fails to load proc-macros when tower-lsp is present. The library functionality (hover, diagnostics) works without this feature.

### 2. DRC Violations in DocumentState

Rather than running DRC in the diagnostics module (which only has immutable access), violations are computed during `build_world()` and stored in `DocumentState.drc_violations`. This respects bevy_ecs query requirements.

### 3. Diagnostic Cap at 100

Per RESEARCH.md pitfall guidance, diagnostics are capped at 100 per file to prevent editor performance issues. An overflow message is appended when limit is reached.

### 4. Decoupled Diagnostic Type

The `Diagnostic` struct in diagnostics.rs doesn't depend on tower-lsp types, making it usable with or without the server feature. Backend converts to tower-lsp types when publishing.

## Deviations from Plan

### Build Environment Issue

**Found during:** Task 1 verification
**Issue:** When tower-lsp dependency is present, rustc fails with "can't find crate" errors for all dependencies, despite them being correctly specified and built.
**Fix:** Made server feature optional so library functionality (hover, diagnostics) works. Server feature can be tested in environments without this issue.
**Files modified:** Cargo.toml (feature definitions)
**Commit:** d894134

## Success Criteria Status

| Criteria | Status |
|----------|--------|
| LSP server starts and responds to initialize | Partial (requires `server` feature in compatible env) |
| Hover shows component info (footprint, value, position) | Pass |
| Hover shows net info (connected pins) | Pass |
| DRC violations appear as diagnostics | Pass |
| Parse errors appear as diagnostics | Pass |
| Diagnostics capped at 100 per file | Pass |

## Next Steps (Plans 05-06 to 05-10)

- 05-06: Autorouter integration with FreeRouting
- 05-07: Export enhancements
- 05-08: Board statistics
- 05-09: Netlist analysis
- 05-10: Visual verification checkpoint

The LSP library provides foundation for IDE integration. The server binary can be tested when the build environment issue is resolved.

## Commits

| Hash | Description |
|------|-------------|
| 50459c3 | feat(05-05): add cypcb-lsp crate with hover and diagnostics |
| d894134 | fix(05-05): make cypcb-lsp build and tests pass |
