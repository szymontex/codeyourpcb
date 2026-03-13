---
estimated_steps: 5
estimated_files: 8
---

# T03: Wire downstream consumers, update Monaco/LSP, write v2 example files

**Slice:** S05 — DSL v2 — Modules, Units & Constraints
**Milestone:** M002

## Description

Close the integration loop: handle new `Definition` variants in all downstream match statements so the workspace compiles, update Monaco tokenizer with new keywords for syntax highlighting, update LSP completions/hover for editor support, and write v2 example files that exercise every new feature. This task proves the full slice works end-to-end at the parser level.

## Steps

1. **Update `sync.rs` match arms.** In `crates/cypcb-world/src/sync.rs`, add arms for `Definition::Module`, `Definition::Interface`, `Definition::Import`, `Definition::Assert`. These should log a debug message and skip (semantic handling deferred to future slices). Grep for any other match statements on `Definition` across the workspace and update them.

2. **Update Monaco tokenizer.** In `viewer/src/editor/cypcb-language.ts`, add new keywords (`module`, `interface`, `import`, `assert`, `pin`, `within`, `from`) to the keyword list. Add physical unit suffixes to the units token pattern. Add `+/-` as an operator token.

3. **Update LSP completions.** In `crates/cypcb-lsp/src/completion.rs`, add completion items for `module`, `interface`, `import`, `assert`, `pin` keywords with appropriate snippets (e.g., `module Name {\n\t$0\n}`). Add physical unit suffix completions for component value context.

4. **Update LSP hover.** In `crates/cypcb-lsp/src/hover.rs`, add hover documentation for new keywords explaining their syntax and purpose.

5. **Write v2 example files and run full verification:**
   - `examples/v2-modules.cypcb` — module definition with nested components, pin declarations, module instantiation
   - `examples/v2-interfaces.cypcb` — interface definitions (I2C, SPI, Power), usage in modules
   - `examples/v2-constraints.cypcb` — assert statements with comparisons and within/tolerance, physical unit values on components
   - Run `cargo build` (full workspace), `cargo build --target wasm32-unknown-unknown -p cypcb-render` (WASM), `cargo test` (all crates)

## Must-Haves

- [ ] Full workspace compiles with `cargo build` — no match exhaustiveness errors
- [ ] WASM target compiles: `cargo build --target wasm32-unknown-unknown -p cypcb-render`
- [ ] Monaco tokenizer highlights new keywords correctly
- [ ] LSP provides completions for new keywords
- [ ] LSP provides hover info for new keywords
- [ ] 3 v2 example files parse without errors
- [ ] All tests across all crates pass

## Verification

- `cargo build` — full workspace compiles clean
- `cargo build --target wasm32-unknown-unknown -p cypcb-render` — WASM builds
- `cargo test` — all crate tests pass
- Parse each v2 example file in a test and assert zero errors

## Inputs

- T01 output: extended grammar, AST, parser with new Definition variants
- T02 output: PhysicalUnit system in cypcb-core
- `crates/cypcb-world/src/sync.rs` — match on Definition (line 354)
- `crates/cypcb-lsp/src/completion.rs` — keyword completions
- `crates/cypcb-lsp/src/hover.rs` — hover info
- `viewer/src/editor/cypcb-language.ts` — Monaco tokenizer (99 lines)
- S05-RESEARCH.md — v2 syntax examples

## Expected Output

- `crates/cypcb-world/src/sync.rs` — updated with new Definition arms
- `crates/cypcb-lsp/src/completion.rs` — new keyword completions
- `crates/cypcb-lsp/src/hover.rs` — new keyword hover docs
- `crates/cypcb-lsp/src/diagnostics.rs` — updated if needed for new constructs
- `viewer/src/editor/cypcb-language.ts` — new keywords + unit suffixes
- `examples/v2-modules.cypcb` — new file
- `examples/v2-interfaces.cypcb` — new file
- `examples/v2-constraints.cypcb` — new file
