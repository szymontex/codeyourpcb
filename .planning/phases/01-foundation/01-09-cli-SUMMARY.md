---
phase: 01
plan: 09
subsystem: cli
tags: [cli, clap, miette, parsing, validation]

dependency-graph:
  requires:
    - 01-05 (AST Parser)
  provides:
    - Command-line interface for cypcb parse/check
  affects:
    - Phase 2-6 (will be extended with export, render commands)

tech-stack:
  added:
    - clap 4.0 (CLI argument parsing)
  patterns:
    - Command pattern with clap derive
    - miette error display integration

key-files:
  created:
    - crates/cypcb-cli/src/commands/mod.rs
    - crates/cypcb-cli/src/commands/parse.rs
    - crates/cypcb-cli/src/commands/check.rs
    - crates/cypcb-cli/tests/cli_integration.rs
    - examples/blink.cypcb
    - examples/invalid.cypcb
    - examples/unknown_keyword.cypcb
  modified:
    - crates/cypcb-cli/src/main.rs
    - crates/cypcb-cli/Cargo.toml
    - crates/cypcb-world/src/sync.rs
    - Cargo.toml

decisions:
  - id: cargo-resolver-workaround
    summary: Disabled cypcb-world dependency in CLI due to cargo resolver issues

metrics:
  duration: ~90 minutes
  completed: 2026-01-21
---

# Phase 1 Plan 9: CLI Summary

**One-liner:** Clap-based CLI with parse (AST JSON output) and check (validation) commands, miette error display.

## Overview

Implemented the command-line interface for CodeYourPCB providing parsing and validation capabilities. The CLI uses clap for argument parsing and miette for user-friendly error display with source context.

## What Was Built

### CLI Structure
- **Binary:** `cypcb` - main CLI entrypoint
- **Commands:**
  - `cypcb parse <file>` - Parse .cypcb file and output JSON
  - `cypcb check <file>` - Validate .cypcb file and report errors

### Parse Command
- Outputs AST as JSON (serialized via serde)
- Supports `--output ast` and `--output json` flags (both currently output AST)
- Reports parse errors with miette fancy display
- Exits with code 1 on parse errors

### Check Command
- Validates .cypcb file syntax
- Displays errors with source context (line numbers, spans)
- Outputs "OK: <file> validated successfully" on success
- Exits with code 1 on validation failure

### Error Display
Miette integration provides:
- Error codes (e.g., `cypcb::parse::invalid_number`)
- Source code snippets with line numbers
- Labeled spans pointing to error locations
- Colored terminal output (fancy feature)

## Key Technical Details

### Cargo Resolver Issue
Encountered a complex cargo workspace resolver issue where building cypcb-cli would fail to find cypcb-world dependencies despite them existing. This was caused by:
- Feature unification conflicts between bevy_ecs, miette, and serde
- Multiple metadata hash splits in the dependency graph
- Resolver "2" handling proc-macro dependencies differently

**Workaround:** Temporarily removed cypcb-world dependency from CLI. The CLI currently only performs parse-level validation. Semantic validation (footprint checks, duplicate refdes detection) will be re-enabled once the resolver issue is investigated further.

### SyncError Refactor
Converted SyncError from derive(Diagnostic) macro to manual Diagnostic trait implementation to work around proc-macro resolution issues in the complex dependency graph.

## Test Coverage

9 integration tests verify:
1. `--help` output format
2. `--version` output
3. `parse --help` subcommand help
4. `check --help` subcommand help
5. Parsing valid file outputs JSON
6. Parse with `--output ast` flag
7. Check passes on valid file
8. Check fails on invalid file with error message
9. Check fails on nonexistent file

## Example Files Created

1. **blink.cypcb** - Valid LED blink circuit with:
   - Board definition (50mm x 30mm, 2 layers)
   - LED and resistor components
   - VCC, LED_ANODE, GND nets

2. **invalid.cypcb** - Missing layer count value (parse error)

3. **unknown_keyword.cypcb** - Unknown "module" keyword (syntax error)

## Deviations from Plan

### [Rule 3 - Blocking] Cargo resolver workaround
- **Found during:** Task 2
- **Issue:** cypcb-cli couldn't compile when depending on cypcb-world due to metadata hash mismatches in cargo's feature resolution
- **Fix:** Removed cypcb-world dependency; CLI uses parser-only validation
- **Impact:** Semantic validation deferred; parse validation fully functional
- **Commit:** c43e223

### [Rule 1 - Bug] SyncError manual Diagnostic impl
- **Found during:** Task 1
- **Issue:** derive(Diagnostic) macro failed due to proc-macro resolution in complex dependency graph
- **Fix:** Implemented Diagnostic trait manually for SyncError
- **Files modified:** crates/cypcb-world/src/sync.rs
- **Commit:** 80a97e4

## Commands and Output

```bash
# Parse and output JSON
$ cypcb parse examples/blink.cypcb
{
  "version": 1,
  "definitions": [...]
}

# Validate file
$ cypcb check examples/blink.cypcb
OK: examples/blink.cypcb validated successfully

# Error display
$ cypcb check examples/invalid.cypcb
cypcb::parse::invalid_number
  × Invalid number: ''
   ╭─[6:29]
 5 │     size 50mm x 30mm
 6 │     layers  // missing value
   ·                             ▲
   ·                             ╰── invalid number
 7 │ }
   ╰────
```

## Next Phase Readiness

Phase 1 Foundation is complete. The CLI provides:
- File parsing capability
- Syntax validation with error reporting
- JSON output for tooling integration
- Extensible command structure for future commands (export, render)

**Known limitation:** Semantic validation (footprint existence, duplicate refdes) requires fixing the cargo resolver issue to re-enable cypcb-world integration.

## Commits

| Hash | Type | Description |
|------|------|-------------|
| 80a97e4 | feat | Set up CLI structure with clap |
| c43e223 | feat | Implement parse and check commands |
| 802b946 | test | Verify miette error display integration |
| f59033a | test | Add CLI integration tests |
