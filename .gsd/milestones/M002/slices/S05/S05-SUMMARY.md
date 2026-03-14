---
id: S05
parent: M002
milestone: M002
provides:
  - Tree-sitter grammar rules for module, interface, import, assert, pin, physical_value, tolerance (15+ new rules)
  - AST types: ModuleDef, InterfaceDef, ImportDef, AssertDef, PinDeclaration, PhysicalValue, Tolerance, AssertExpression, AssertOperand, ComparisonOp
  - PhysicalUnit enum (23 variants, 7 quantity categories) with SI normalization in cypcb-core
  - Parser converter methods for all v2 constructs with error reporting
  - LSP completions/hover for 4 new keywords + 23 physical unit suffixes
  - Monaco tokenizer updated with v2 keywords, unit suffixes, tolerance/comparison operators
  - 3 v2 example files (modules, interfaces, constraints)
  - Full backward compatibility — all v1 .cypcb files parse identically
requires:
  - slice: S02
    provides: Autorouter engine API (constraint interface not yet wired — deferred to S06/S07)
affects:
  - S06 (constraint evaluation, module instantiation semantics, import resolution)
  - S07 (E2E tests covering v2 constructs)
key_files:
  - crates/cypcb-parser/grammar/grammar.js
  - crates/cypcb-parser/src/ast.rs
  - crates/cypcb-parser/src/parser.rs
  - crates/cypcb-parser/src/errors.rs
  - crates/cypcb-core/src/physical_units.rs
  - crates/cypcb-lsp/src/completion.rs
  - crates/cypcb-lsp/src/hover.rs
  - crates/cypcb-world/src/sync.rs
  - viewer/src/editor/cypcb-language.ts
  - examples/v2-modules.cypcb
  - examples/v2-interfaces.cypcb
  - examples/v2-constraints.cypcb
key_decisions:
  - PhysicalUnit is a separate enum in cypcb-core from Unit (length/Nm) — electrical quantities have different base conversions
  - Component value_property accepts physical_value at grammar level but converts to StringLit in parser — backward compat preserved, richer PhysicalValue field deferred
  - ToleranceKind::Absolute and Range use Box<PhysicalValue> to break recursive type cycle
  - v2 Definition variants get no-op arms in downstream crates (sync.rs, LSP) — semantic handling deferred
  - Grammar conflict between dimension and assert_operand resolved with explicit conflicts declaration
  - Assert statements are parse-only — evaluation wired to DRC/autorouter in S06/S07
  - Import resolution starts file-relative only — project root and registry resolution deferred
patterns_established:
  - v2 grammar rules follow same pattern as v1: new _definition variants, dedicated convert_* methods
  - Tolerance syntax: +/- N% (percentage), +/- NV (absolute), to NV (range)
  - v2 hover/completion functions follow same pattern as v1
  - Physical unit resolution happens during CST→AST conversion with ParseError on invalid units
observability_surfaces:
  - ParseError variants with span info for all v2 constructs (InvalidModule, InvalidInterface, InvalidImport, InvalidAssert, InvalidPhysicalUnit, InvalidTolerance)
  - LSP diagnostics surface all 6 new error variants with proper codes
  - Targeted test filters: `cargo test -p cypcb-parser -- test_parse_import`, `cargo test -p cypcb-core -- physical_units`
drill_down_paths:
  - .gsd/milestones/M002/slices/S05/tasks/T01-SUMMARY.md
  - .gsd/milestones/M002/slices/S05/tasks/T02-SUMMARY.md
  - .gsd/milestones/M002/slices/S05/tasks/T03-SUMMARY.md
duration: ~90min across 3 tasks
verification_result: passed
completed_at: 2026-03-13
---

# S05: DSL v2 — Modules, Units & Constraints

**Extended .cypcb grammar with modules, interfaces, imports, physical units (23 variants), and constraint assertions — 83 parser tests pass, full backward compatibility verified across all v1 files.**

## What Happened

**T01 (grammar + AST):** Extended grammar.js with 15+ new rules covering import_statement, module_definition, interface_definition, pin_declaration, assert_statement (comparison and within variants), physical_value with 23 unit suffixes, and tolerance syntax (percentage, absolute, range). Added 10 AST types and 7 converter methods. Extended Definition enum with Module/Interface/Import/Assert variants. Fixed grammar conflict between dimension and assert_operand with explicit conflicts declaration. Wired downstream crates (sync.rs no-op, LSP hover/completion/diagnostics). 80 parser tests pass including backward compat across all 10 v1 examples.

**T02 (physical units):** Created PhysicalUnit enum in cypcb-core with 23 variants across 7 quantity categories (resistance, capacitance, inductance, voltage, current, frequency, power). Each unit has SI normalization (to_base_f64/from_base_f64), case-sensitive FromStr matching, and Display for round-trip fidelity. Changed PhysicalValue.unit from String to typed PhysicalUnit in AST. Parser resolves unit strings during CST→AST conversion with InvalidPhysicalUnit error on failure. 49 core tests pass.

**T03 (downstream wiring + examples):** Added LSP completions with snippet templates for module/interface/import/assert keywords. Implemented hover for all v2 constructs (module shows pin/component/net counts, interface shows declarations, assert formats expression inline). Added 23 physical unit suffix completions. Updated Monaco tokenizer with v2 keywords, unit suffixes, tolerance and comparison operators. Wrote 3 v2 example files exercising all new features. Added parser tests verifying v2 examples parse to expected AST structure. 41 LSP tests and 83 parser tests pass.

## Verification

- `cargo test -p cypcb-parser` — 83 passed (58 v1 + 1 backward compat + 21 v2 construct tests + 3 v2 example tests)
- `cargo test -p cypcb-core` — 49 passed (including 17 PhysicalUnit tests)
- `cargo test -p cypcb-lsp` — 41 passed (including 4 completion + 4 hover tests for v2)
- `cargo build -p cypcb-{parser,world,lsp,render,drc,core}` — all compile clean
- `cargo build --target wasm32-unknown-unknown -p cypcb-render` — WASM build succeeds
- Backward compat: `test_backward_compat_all_example_files` passes — all 10 v1 files, zero errors
- Forward: 3 v2 example files parse to expected AST (correct module/interface/assert/component counts)

## Requirements Advanced

- EDIT-01 (syntax highlighting) — Monaco tokenizer now highlights v2 keywords, physical units, and operators
- EDIT-02 (auto-completion) — LSP provides completions for module, interface, import, assert, and 23 unit suffixes
- EDIT-03 (error highlighting) — 6 new ParseError variants surface in LSP diagnostics with span info

## Requirements Validated

- None newly validated — v2 constructs are parse-only; full validation requires constraint evaluation (S06/S07)

## New Requirements Surfaced

- None

## Requirements Invalidated or Re-scoped

- None

## Deviations

- Component value_property: physical_value accepted at grammar level but converted to StringLit in parser (not to a richer PhysicalValue field). Plan was ambiguous; this preserves backward compat while the grammar accepts both forms.

## Known Limitations

- v2 constructs (modules, interfaces, imports, assertions) are parse-only — no semantic evaluation, no module instantiation, no import resolution, no constraint checking against DRC/autorouter
- physical_unit_completions() defined but not wired into context-aware completion (needs ValueContext variant)
- test_sync_named_pin in cypcb-world fails — pre-existing, unrelated to S05

## Follow-ups

- Wire constraint assertions to DRC engine (S06/S07) — assert statements parse but don't enforce yet
- Add module instantiation semantics — modules parse as definition blocks, instantiation via component syntax deferred
- Wire import resolution — file-relative path detection exists but no file loading
- Add ValueContext to CompletionContext so unit suffixes appear when typing component values

## Files Created/Modified

- `crates/cypcb-parser/grammar/grammar.js` — 15+ new grammar rules for v2 constructs
- `crates/cypcb-parser/src/ast.rs` — 10 new AST types, Definition enum extended with 4 variants
- `crates/cypcb-parser/src/parser.rs` — 7+ converter methods, 25 new tests
- `crates/cypcb-parser/src/errors.rs` — 6 new error variants with constructors
- `crates/cypcb-core/src/physical_units.rs` — new: PhysicalQuantity + PhysicalUnit enums with 17 tests
- `crates/cypcb-core/src/lib.rs` — pub mod physical_units + re-exports
- `crates/cypcb-world/src/sync.rs` — no-op match arm for v2 Definition variants
- `crates/cypcb-lsp/src/completion.rs` — 4 keyword completions + physical_unit_completions() + 4 tests
- `crates/cypcb-lsp/src/hover.rs` — hover for module/interface/import/assert + 4 tests
- `crates/cypcb-lsp/src/diagnostics.rs` — handles 6 new ParseError variants
- `viewer/src/editor/cypcb-language.ts` — v2 keywords, unit suffixes, operators
- `examples/v2-modules.cypcb` — module definitions with nested components and pins
- `examples/v2-interfaces.cypcb` — interface definitions (I2C, SPI, Power, UART)
- `examples/v2-constraints.cypcb` — assert statements with comparisons and tolerances

## Forward Intelligence

### What the next slice should know
- v2 AST types are complete and stable — S06 can consume ModuleDef, InterfaceDef, AssertDef etc. directly
- PhysicalUnit has full SI normalization — constraint evaluation can compare values across unit prefixes (kohm vs Mohm) using to_base_f64()
- sync.rs has no-op arms for v2 variants — wiring module instantiation means adding real logic there, not changing the match structure

### What's fragile
- Grammar conflicts declaration in grammar.js — adding new numeric/dimension rules may need the conflicts list updated; test with `tree-sitter generate` after any grammar change
- Physical value in component value property converts to StringLit — if a later task wants typed PhysicalValue in component AST, the converter in parser.rs needs to change

### Authoritative diagnostics
- `cargo test -p cypcb-parser -- test_backward_compat` — single test that proves v1 compat; run after any grammar change
- `cargo test -p cypcb-core -- physical_units` — covers all 23 unit variants with conversion roundtrip

### What assumptions changed
- Originally planned richer PhysicalValue field in component AST — ended up converting to StringLit for backward compat. This was the right call but means components don't carry typed electrical values yet.
