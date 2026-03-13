# S05: DSL v2 — Modules, Units & Constraints

**Goal:** Extend the `.cypcb` grammar and parser to support modules, typed interfaces, physical units, and constraint assertions — while all existing v1 `.cypcb` files continue to parse identically.
**Demo:** Parse a v2 `.cypcb` file containing `import`, `module`, `interface`, `assert`, physical units (`10kohm`, `3.3V`, `100nF`), and tolerance syntax — with full AST output. All 10 existing v1 examples parse with zero diff.

## Must-Haves

- Tree-sitter grammar extended with `import_statement`, `module_definition`, `interface_definition`, `assert_statement`, `pin_declaration`, `physical_value`, tolerance syntax
- New AST types: `ImportDef`, `ModuleDef`, `InterfaceDef`, `AssertDef`, `PinDeclaration`, `PhysicalValue`, `Tolerance`
- `Definition` enum extended with `Module`, `Interface`, `Import`, `Assert` variants
- `PhysicalUnit` enum in cypcb-core covering resistance, capacitance, inductance, voltage, current, frequency, power
- Parser `convert_*` methods for all new grammar rules
- All 10 existing `.cypcb` examples parse identically (backward compat snapshot tests)
- New v2 `.cypcb` example files exercising every new construct
- `sync.rs` match arms handle new `Definition` variants (graceful no-op for now)
- Monaco tokenizer updated with new keywords
- LSP completions/hover updated for new keywords
- All existing 58 parser tests pass, new tests cover every v2 construct

## Proof Level

- This slice proves: contract (parse-level correctness of new grammar)
- Real runtime required: no (parse + AST verification only; constraint evaluation deferred to S06/S07)
- Human/UAT required: no

## Verification

- `cd /workspace/codeyourpcb && cargo test --manifest-path crates/cypcb-parser/Cargo.toml` — all existing + new tests pass
- `cd /workspace/codeyourpcb && cargo test --manifest-path crates/cypcb-core/Cargo.toml` — PhysicalUnit tests pass
- `cd /workspace/codeyourpcb && cargo build` — full workspace compiles (sync.rs, LSP, render all handle new variants)
- `cd /workspace/codeyourpcb && cargo build --target wasm32-unknown-unknown -p cypcb-render` — WASM build succeeds
- Backward compat: dedicated test parses all 10 v1 `.cypcb` files and asserts zero parse errors
- Forward test: v2 example files parse to expected AST structure

## Observability / Diagnostics

- Runtime signals: parser errors with span information for new constructs (same pattern as v1)
- Inspection surfaces: `parse()` function returns `ParseResult` with errors vec — new error variants carry context
- Failure visibility: new `ParseError` variants include source spans and descriptive messages for each v2 construct
- Redaction constraints: none

## Integration Closure

- Upstream surfaces consumed: `crates/cypcb-parser/grammar/grammar.js`, `crates/cypcb-parser/src/{ast,parser,errors}.rs`, `crates/cypcb-core/src/units.rs`
- New wiring introduced in this slice: new `Definition` variants propagate to `sync.rs` (no-op), LSP, Monaco tokenizer
- What remains before the milestone is truly usable end-to-end: constraint evaluation wired to DRC/autorouter (S06/S07), module instantiation semantics, import resolution

## Tasks

- [x] **T01: Extend Tree-sitter grammar with modules, interfaces, imports, asserts, and physical units** `est:3h`
  - Why: Core grammar is the foundation — everything else depends on parser.c being generated with the new rules. This is the highest-risk task because grammar conflicts can break v1 parsing.
  - Files: `crates/cypcb-parser/grammar/grammar.js`, `crates/cypcb-parser/src/parser.c` (generated), `crates/cypcb-parser/src/ast.rs`, `crates/cypcb-parser/src/parser.rs`, `crates/cypcb-parser/src/errors.rs`
  - Do: Add grammar rules for `import_statement`, `module_definition`, `interface_definition`, `assert_statement`, `pin_declaration`, `physical_value` (with all electrical unit suffixes), tolerance syntax (`+/- N%`, `+/- NV`, `N to N`). Add new AST types. Implement all `convert_*` methods in parser.rs. Add new error variants. Run `tree-sitter generate` after grammar changes. Write backward compat tests first — parse all 10 v1 examples and assert zero errors. Then write forward tests for every new construct.
  - Verify: `cargo test --manifest-path crates/cypcb-parser/Cargo.toml` — all existing 58 tests pass + new tests pass
  - Done when: All new grammar constructs parse into typed AST nodes, all v1 files parse identically

- [x] **T02: Add PhysicalUnit system to cypcb-core and wire value property** `est:1.5h`
  - Why: The parser needs typed physical values (not just length dimensions) to represent component values like `10kohm`, `3.3V`, `100nF`. This is separate from T01's grammar work because it touches the core crate's type system and needs its own conversion/normalization logic.
  - Files: `crates/cypcb-core/src/units.rs` (or new `physical_units.rs`), `crates/cypcb-core/src/lib.rs`, `crates/cypcb-parser/src/ast.rs` (PhysicalValue node), `crates/cypcb-parser/src/parser.rs` (convert physical values)
  - Do: Add `PhysicalUnit` enum with categories (Resistance, Capacitance, Inductance, Voltage, Current, Frequency, Power) and unit variants. Add `PhysicalValue { value: f64, unit: PhysicalUnit, tolerance: Option<Tolerance> }` to AST. Implement `FromStr` for PhysicalUnit. Add normalization to base SI units. Wire the `value` property in component definitions to accept `physical_value` OR string literal. Add unit tests for all physical unit categories with parsing and normalization.
  - Verify: `cargo test --manifest-path crates/cypcb-core/Cargo.toml` + `cargo test --manifest-path crates/cypcb-parser/Cargo.toml`
  - Done when: `value 10kohm`, `value 3.3V +/- 5%`, `value 100nF to 220nF` all parse into typed `PhysicalValue` AST nodes with correct normalization

- [x] **T03: Wire downstream consumers, update Monaco/LSP, write v2 example files** `est:2h`
  - Why: New `Definition` variants cause compile errors in match statements across the workspace. Monaco and LSP need keyword updates for the editor experience. V2 example files prove the full feature set works end-to-end at the parser level.
  - Files: `crates/cypcb-world/src/sync.rs`, `crates/cypcb-lsp/src/completion.rs`, `crates/cypcb-lsp/src/hover.rs`, `crates/cypcb-lsp/src/diagnostics.rs`, `viewer/src/editor/cypcb-language.ts`, `examples/v2-modules.cypcb`, `examples/v2-interfaces.cypcb`, `examples/v2-constraints.cypcb`
  - Do: Add match arms in `sync.rs` for Import/Module/Interface/Assert (log + skip for now — semantic handling deferred). Update Monaco tokenizer keywords list. Add LSP completions for `module`, `interface`, `import`, `assert`, `pin`, physical unit suffixes. Add hover info for new keywords. Write 3 v2 example files exercising all new features: one with modules/imports, one with interfaces, one with constraints/physical units. Run full workspace build + WASM build.
  - Verify: `cargo build` (full workspace) + `cargo build --target wasm32-unknown-unknown -p cypcb-render` + `cargo test` (all crates)
  - Done when: Full workspace compiles clean, WASM builds, all tests pass, v2 examples parse without errors

## Files Likely Touched

- `crates/cypcb-parser/grammar/grammar.js`
- `crates/cypcb-parser/src/parser.c` (generated)
- `crates/cypcb-parser/src/ast.rs`
- `crates/cypcb-parser/src/parser.rs`
- `crates/cypcb-parser/src/errors.rs`
- `crates/cypcb-core/src/units.rs` (or new `physical_units.rs`)
- `crates/cypcb-core/src/lib.rs`
- `crates/cypcb-world/src/sync.rs`
- `crates/cypcb-lsp/src/completion.rs`
- `crates/cypcb-lsp/src/hover.rs`
- `crates/cypcb-lsp/src/diagnostics.rs`
- `viewer/src/editor/cypcb-language.ts`
- `examples/v2-modules.cypcb`
- `examples/v2-interfaces.cypcb`
- `examples/v2-constraints.cypcb`
