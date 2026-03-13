---
estimated_steps: 8
estimated_files: 5
---

# T01: Extend Tree-sitter grammar with modules, interfaces, imports, asserts, and physical units

**Slice:** S05 — DSL v2 — Modules, Units & Constraints
**Milestone:** M002

## Description

Extend the Tree-sitter grammar (`grammar.js`) with all v2 constructs, regenerate `parser.c`, add corresponding AST types, implement parser converter methods, and add error variants. This is the core language extension task — everything else in S05 depends on it.

The critical risk is grammar conflicts breaking v1 parsing. Mitigation: write backward compat tests first, run them after every grammar change.

## Steps

1. **Write backward compat test first.** Add a test that parses all 10 existing `.cypcb` example files and asserts zero parse errors. This test must pass before and after every grammar change.

2. **Extend `grammar.js` with new rules:**
   - `import_statement`: `import string_literal` / `import identifier from string_literal` / `import identifier_list from string_literal`
   - `module_definition`: `module identifier { _definition* pin_declaration* }`
   - `interface_definition`: `interface identifier { pin_declaration* }`
   - `pin_declaration`: `pin identifier`
   - `assert_statement`: `assert assert_expression` with comparison operators (`==`, `!=`, `>=`, `<=`, `>`, `<`), `within` keyword, and tolerance syntax
   - `physical_value`: `number physical_unit` with optional tolerance
   - `physical_unit`: all electrical unit suffixes (ohm, kohm, Mohm, pF, nF, uF, mF, nH, uH, mH, H, mV, V, kV, uA, mA, A, Hz, kHz, MHz, GHz, mW, W)
   - `tolerance`: `+/- number unit_or_percent` or `to physical_value`
   - Extend `_definition` choice to include new variants
   - Extend `value_property` to accept `physical_value` as alternative to string

3. **Run `tree-sitter generate`** to produce new `parser.c`. Verify it compiles.

4. **Add AST types in `ast.rs`:**
   - `ImportDef { names: Vec<Spanned<String>>, path: Spanned<String>, span }`
   - `ModuleDef { name: Spanned<String>, definitions: Vec<Definition>, pins: Vec<PinDeclaration>, span }`
   - `InterfaceDef { name: Spanned<String>, pins: Vec<PinDeclaration>, span }`
   - `AssertDef { expression: AssertExpression, span }`
   - `PinDeclaration { name: Spanned<String>, span }`
   - `PhysicalValue { value: f64, unit: String, tolerance: Option<Tolerance>, span }`
   - `Tolerance { kind: ToleranceKind, span }` with `Percentage(f64)`, `Absolute(PhysicalValue)`, `Range(PhysicalValue)`
   - Extend `Definition` enum with `Module(ModuleDef)`, `Interface(InterfaceDef)`, `Import(ImportDef)`, `Assert(AssertDef)`
   - Extend `Definition::span()` match

5. **Add error variants in `errors.rs`** for new constructs (invalid module, invalid interface, invalid import, invalid assert, invalid physical unit, invalid tolerance).

6. **Implement converter methods in `parser.rs`:**
   - `convert_import_statement()`
   - `convert_module_definition()`
   - `convert_interface_definition()`
   - `convert_pin_declaration()`
   - `convert_assert_statement()`
   - `convert_physical_value()`
   - `convert_tolerance()`
   - Wire into the top-level `convert_definition()` dispatch

7. **Write forward tests:** Test parsing of each new construct individually — module with nested components, interface with pins, import statements, assert with comparison, assert with within/tolerance, physical values with each unit category.

8. **Run full test suite** — all 58 existing tests + new tests pass.

## Must-Haves

- [ ] All 10 existing `.cypcb` files parse with zero errors after grammar changes
- [ ] `import`, `module`, `interface`, `assert`, `pin` parse into typed AST nodes
- [ ] Physical unit suffixes parse correctly (all categories: resistance, capacitance, inductance, voltage, current, frequency, power)
- [ ] Tolerance syntax (`+/- N%`, `+/- NV`, `N to N`) parses into Tolerance AST node
- [ ] `value` property accepts both string literal and physical_value
- [ ] All 58 existing parser tests still pass
- [ ] New tests cover every v2 grammar construct

## Verification

- `cargo test --manifest-path crates/cypcb-parser/Cargo.toml` — all tests pass (existing + new)
- Backward compat test specifically: parses all 10 example files with zero errors
- Grammar generation: `cd crates/cypcb-parser/grammar && npx tree-sitter generate` succeeds

## Inputs

- `crates/cypcb-parser/grammar/grammar.js` — existing v1 grammar (396 lines)
- `crates/cypcb-parser/src/ast.rs` — existing AST types (834 lines, 6 Definition variants)
- `crates/cypcb-parser/src/parser.rs` — existing parser (1763 lines, visitor pattern)
- `crates/cypcb-parser/src/errors.rs` — existing error types (313 lines)
- `examples/*.cypcb` — 10 existing example files (423 lines total)
- S05-RESEARCH.md — proposed DSL v2 design (grammar rules, AST types, syntax examples)

## Expected Output

- `crates/cypcb-parser/grammar/grammar.js` — extended with ~15 new grammar rules
- `crates/cypcb-parser/src/parser.c` — regenerated from updated grammar
- `crates/cypcb-parser/src/ast.rs` — ~8 new AST types + extended Definition enum
- `crates/cypcb-parser/src/parser.rs` — ~7 new convert_* methods
- `crates/cypcb-parser/src/errors.rs` — ~6 new error variants
- All tests passing including backward compat and new v2 construct tests
