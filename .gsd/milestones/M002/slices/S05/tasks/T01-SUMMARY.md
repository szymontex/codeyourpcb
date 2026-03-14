---
id: T01
parent: S05
milestone: M002
provides:
  - Tree-sitter grammar rules for module, interface, import, assert, pin, physical_value, tolerance
  - AST types: ModuleDef, InterfaceDef, ImportDef, AssertDef, PinDeclaration, PhysicalValue, Tolerance, AssertExpression, AssertOperand, ComparisonOp
  - Parser converter methods for all v2 constructs
  - Error variants for v2 parse failures
  - Downstream crate compatibility (sync.rs, LSP hover/completion/diagnostics)
key_files:
  - crates/cypcb-parser/grammar/grammar.js
  - crates/cypcb-parser/src/ast.rs
  - crates/cypcb-parser/src/parser.rs
  - crates/cypcb-parser/src/errors.rs
  - crates/cypcb-world/src/sync.rs
  - crates/cypcb-lsp/src/diagnostics.rs
  - crates/cypcb-lsp/src/hover.rs
  - crates/cypcb-lsp/src/completion.rs
key_decisions:
  - Physical values in component value property are converted to StringLit for backward compat — component struct unchanged, physical_value grammar accepted. Richer PhysicalValue field can be added in a later task.
  - ToleranceKind::Absolute and Range use Box<PhysicalValue> to break recursive type cycle.
  - v2 Definition variants (Module, Interface, Import, Assert) get no-op arms in downstream crates — sync.rs, LSP hover/completion/goto — to be wired in later tasks.
  - Grammar conflict between dimension and assert_operand resolved with explicit conflicts declaration.
patterns_established:
  - v2 grammar rules follow same pattern as v1: new _definition variants, dedicated convert_* methods
  - Tolerance syntax: +/- N% (percentage), +/- NV (absolute), to NV (range)
  - assert_expression is a choice of comparison and within variants
  - Module body allows components, nets, pin declarations, and assertions
observability_surfaces:
  - ParseError variants with span info: InvalidModule, InvalidInterface, InvalidImport, InvalidAssert, InvalidPhysicalUnit, InvalidTolerance
  - LSP diagnostics.rs handles all new error variants with proper codes
duration: ~45min
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T01: Extend Tree-sitter grammar with modules, interfaces, imports, asserts, and physical units

**Extended grammar.js with 15 new rules, added 10 AST types, 7 converter methods, 6 error variants — all 80 parser tests pass, all downstream crates compile.**

## What Happened

Started with a backward compat test that parses all 10 example files — confirmed it passes before any changes.

Extended `grammar.js` with: `import_statement`, `module_definition`, `interface_definition`, `pin_declaration`, `assert_statement`, `assert_expression`, `assert_comparison`, `assert_within`, `assert_operand`, `qualified_name`, `comparison_operator`, `physical_value`, `physical_unit` (23 unit suffixes), `tolerance`, `tolerance_plus_minus`, `tolerance_range`, `import_name_list`, `_module_body_item`. Also extended `_definition` choice and `value_property` to accept physical_value.

Ran `tree-sitter generate` — one grammar conflict between `dimension` and `assert_operand` (bare number ambiguity), resolved with explicit conflicts declaration.

Added AST types: `ImportDef`, `ModuleDef`, `InterfaceDef`, `PinDeclaration`, `AssertDef`, `AssertExpression`, `AssertOperand`, `ComparisonOp`, `PhysicalValue`, `Tolerance`, `ToleranceKind`. Extended `Definition` enum with 4 new variants + span() match.

Added 6 error variants and constructor helpers. Implemented 7 converter methods (`convert_import_statement`, `convert_module_definition`, `convert_interface_definition`, `convert_pin_declaration`, `convert_assert_statement`, `convert_physical_value`, `convert_tolerance`) plus `convert_assert_expression` and `convert_assert_operand`.

Fixed downstream crates: `cypcb-world/sync.rs` (no-op arm), `cypcb-lsp/hover.rs` (return None), `cypcb-lsp/completion.rs` (return TopLevel), `cypcb-lsp/diagnostics.rs` (all 6 new error variants handled).

## Verification

- `cargo test --manifest-path crates/cypcb-parser/Cargo.toml` — 80 passed (58 existing + 1 backward compat + 21 new v2 tests)
- `cargo test --manifest-path crates/cypcb-core/Cargo.toml` — 29 passed
- `cargo build -p cypcb-parser -p cypcb-world -p cypcb-lsp -p cypcb-render -p cypcb-drc` — all compile
- `cargo build --target wasm32-unknown-unknown -p cypcb-render` — WASM build succeeds
- `cd crates/cypcb-parser/grammar && npx tree-sitter generate` — succeeds with no warnings
- Backward compat: test_backward_compat_all_example_files passes (all 10 files, zero errors on valid files)
- Forward tests cover: bare import, named import, multi-name import, module with nested components/nets/pins, module with assert, interface, all 6 comparison operators, assert within + percentage tolerance, assert within + absolute tolerance, assert within + range tolerance, physical values for all unit categories (resistance, capacitance, inductance, voltage, frequency, power), value property with string (backward compat), mixed v1/v2 file

## Diagnostics

- New ParseError variants carry span info for every v2 construct — LSP diagnostics surface them
- `parse()` function returns errors vec with full context for debugging
- `cargo test -p cypcb-parser -- test_parse_import` (or any construct name) for targeted testing

## Deviations

- tolerance_range converter: originally tried `get_child_by_field("upper")` but the field wraps both number and physical_unit children, so `get_child_by_field` only returned the first. Fixed by iterating all children of the tolerance_range node directly.
- Component value_property: physical_value is accepted at grammar level but converted to StringLit in the component converter, preserving backward compat. Plan said "extend value_property to accept physical_value" which is done, but the component AST field stays `Option<StringLit>`.

## Known Issues

- `test_sync_named_pin` in cypcb-world fails — pre-existing, not caused by this task (confirmed by reverting changes).
- `parser.c` is gitignored — any fresh clone needs `cd crates/cypcb-parser/grammar && npx tree-sitter generate` before building. This is the existing project pattern.
- v2 constructs have no-op handling in sync.rs and LSP — to be wired in subsequent tasks (T02+).

## Files Created/Modified

- `crates/cypcb-parser/grammar/grammar.js` — Extended with 15+ new grammar rules for v2 constructs
- `crates/cypcb-parser/grammar/src/parser.c` — Regenerated (gitignored, built from grammar.js)
- `crates/cypcb-parser/src/ast.rs` — Added 10 new AST types, extended Definition enum with 4 variants
- `crates/cypcb-parser/src/parser.rs` — Added 7+ converter methods, 22 new tests, backward compat test
- `crates/cypcb-parser/src/errors.rs` — Added 6 new error variants with constructors
- `crates/cypcb-world/src/sync.rs` — Added no-op match arm for v2 Definition variants
- `crates/cypcb-lsp/src/hover.rs` — Added None arm for v2 Definition variants
- `crates/cypcb-lsp/src/completion.rs` — Added TopLevel arm for v2 Definition variants
- `crates/cypcb-lsp/src/diagnostics.rs` — Added diagnostic handling for 6 new ParseError variants
