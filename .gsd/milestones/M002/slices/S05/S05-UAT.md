# S05: DSL v2 — Modules, Units & Constraints — UAT

**Milestone:** M002
**Written:** 2026-03-13

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: This slice is purely parse-level — grammar extension, AST types, and editor support. No runtime behavior, no UI interaction, no network calls. Verification is entirely through test suites and build checks.

## Preconditions

- Rust toolchain installed with wasm32-unknown-unknown target
- tree-sitter CLI available (`npx tree-sitter generate` must work)
- parser.c generated from grammar.js (run `cd crates/cypcb-parser/grammar && npx tree-sitter generate` if missing)

## Smoke Test

```bash
cd /workspace/codeyourpcb && cargo test -p cypcb-parser -- test_backward_compat_all_example_files
```
Must pass — proves all v1 .cypcb files still parse correctly after grammar changes.

## Test Cases

### 1. Backward Compatibility — All v1 Files Parse

1. `cargo test -p cypcb-parser -- test_backward_compat_all_example_files`
2. **Expected:** 1 test passes — all 10 v1 example files parsed with zero errors

### 2. Import Statements Parse

1. `cargo test -p cypcb-parser -- test_parse_import`
2. **Expected:** 3 tests pass — bare import, named import, multi-name import

### 3. Module Definitions Parse

1. `cargo test -p cypcb-parser -- test_parse_module`
2. **Expected:** 2 tests pass — module with nested components/nets/pins, module with assert

### 4. Interface Definitions Parse

1. `cargo test -p cypcb-parser -- test_parse_interface`
2. **Expected:** 1 test passes — interface with pin declarations

### 5. Assert Statements Parse

1. `cargo test -p cypcb-parser -- test_parse_assert`
2. **Expected:** 5 tests pass — all 6 comparison operators, within + percentage/absolute/range tolerance

### 6. Physical Values Parse to Typed Units

1. `cargo test -p cypcb-parser -- test_parse_physical_value`
2. **Expected:** 3+ tests pass — resistance, capacitance, inductance, voltage, frequency, power units all resolve to typed PhysicalUnit

### 7. PhysicalUnit SI Normalization

1. `cargo test -p cypcb-core -- physical_units`
2. **Expected:** 17 tests pass — all 23 unit variants with from_str, display, to_base/from_base roundtrip

### 8. V2 Example Files Parse Clean

1. `cargo test -p cypcb-parser -- test_v2`
2. **Expected:** 3 tests pass — v2-modules.cypcb, v2-interfaces.cypcb, v2-constraints.cypcb all parse to expected AST structure

### 9. LSP Completions Include V2 Keywords

1. `cargo test -p cypcb-lsp -- test_top_level_completions_v2_keywords`
2. **Expected:** 1 test passes — module, interface, import, assert present in completions

### 10. LSP Hover Shows V2 Construct Info

1. `cargo test -p cypcb-lsp -- test_hover_on_module`
2. **Expected:** 1 test passes — hover on module name returns pin/component/net summary

### 11. Full Workspace Compiles

1. `cargo build -p cypcb-parser -p cypcb-world -p cypcb-lsp -p cypcb-render -p cypcb-drc -p cypcb-core`
2. **Expected:** All crates compile with no errors (warnings for dead_code in drc/render are pre-existing)

### 12. WASM Build Succeeds

1. `cargo build --target wasm32-unknown-unknown -p cypcb-render`
2. **Expected:** Build succeeds — v2 AST types are WASM-compatible

## Edge Cases

### Malformed Physical Unit

1. Write a .cypcb file with `value 10zorbs`
2. Parse it with the parser
3. **Expected:** ParseError::InvalidPhysicalUnit with span pointing to "zorbs"

### Mixed V1 and V2 in Same File

1. `cargo test -p cypcb-parser -- test_parse_mixed_v1_v2`
2. **Expected:** File with both v1 board/component syntax and v2 module/assert syntax parses correctly

### Empty Module Body

1. Parse `module Empty { }`
2. **Expected:** ModuleDef with zero components, zero nets, zero pins, zero assertions

## Failure Signals

- Any of the 83 parser tests failing — regression in v1 or v2 parsing
- Any of the 49 core tests failing — PhysicalUnit enum broken
- WASM build failing — v2 types incompatible with wasm32 target
- Match exhaustiveness compiler errors — new Definition variants not handled in a downstream crate

## Requirements Proved By This UAT

- EDIT-01 (syntax highlighting) — Monaco tokenizer includes v2 keywords and physical unit suffixes
- EDIT-02 (auto-completion) — LSP completions verified for module/interface/import/assert + 23 unit suffixes
- EDIT-03 (error highlighting) — 6 new ParseError variants handled in LSP diagnostics

## Not Proven By This UAT

- Constraint evaluation — assert statements parse but don't enforce (deferred to S06/S07)
- Module instantiation — modules define structure but can't be instantiated in a board yet
- Import resolution — import statements parse but don't load referenced files
- Physical unit comparison — PhysicalUnit has SI normalization but it's not wired to any runtime comparison logic yet

## Notes for Tester

- `test_sync_named_pin` in cypcb-world fails — this is pre-existing and unrelated to S05
- 2 cypcb-export tests fail due to filesystem dependency — pre-existing
- Full `cargo build` (including Tauri) requires GTK system deps not available in this environment — build individual crates instead
- parser.c is gitignored — run `cd crates/cypcb-parser/grammar && npx tree-sitter generate` on fresh clone before building
