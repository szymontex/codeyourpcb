---
id: T03
parent: S05
milestone: M002
provides:
  - LSP completions for module, interface, import, assert keywords with snippet templates
  - LSP hover for v2 constructs showing module pins, interface declarations, import paths, assertion details
  - Physical unit suffix completions (23 variants) for component value contexts
  - Monaco tokenizer v2 keywords (module, interface, import, assert, within), physical unit suffixes, +/- and comparison operators
  - 3 v2 example files exercising modules, interfaces, and constraints
  - Parser tests verifying v2 example files parse to expected AST structure
key_files:
  - crates/cypcb-lsp/src/completion.rs
  - crates/cypcb-lsp/src/hover.rs
  - viewer/src/editor/cypcb-language.ts
  - examples/v2-modules.cypcb
  - examples/v2-interfaces.cypcb
  - examples/v2-constraints.cypcb
key_decisions:
  - Hover for assert statements formats the full constraint expression inline (comparison or within+tolerance) rather than showing raw AST — gives immediate readability
  - Physical unit completions exposed as a separate `physical_unit_completions()` function for reuse in value-context completion (not yet wired to context detection — that's future work)
patterns_established:
  - v2 hover functions follow same pattern as v1: check span containment, build markdown lines, return HoverInfo
  - v2 completion items include documentation with syntax examples in fenced code blocks
observability_surfaces:
  - LSP hover returns structured markdown for all v2 constructs — testable via hover_at_position()
  - Parser error variants with spans surface in LSP diagnostics for malformed v2 syntax
duration: ~25 minutes
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T03: Wire downstream consumers, update Monaco/LSP, write v2 example files

**Wired v2 Definition variants through all downstream match arms, added LSP completions/hover for 4 new keywords, extended Monaco tokenizer with v2 syntax, and wrote 3 example files that parse clean.**

## What Happened

1. **sync.rs** — Already handled from T01 (Module/Interface/Import/Assert arms skip with empty body). Verified no other match exhaustiveness issues across workspace — goto.rs uses `_ => {}` catch-all, completion.rs routes v2 to TopLevel context.

2. **Monaco tokenizer** — Added 5 v2 keywords (module, interface, import, assert, within), expanded number regex to match all 23 physical unit suffixes (kohm, nF, uF, MHz, etc.), added `+/-` tolerance operator and comparison operators (>=, <=, ==, !=, >, <) as operator tokens.

3. **LSP completions** — Added 4 keyword completions with snippet templates (module/interface with body, import with from clause, assert with comparison placeholder). Added `physical_unit_completions()` function returning all 23 unit suffixes with human-readable descriptions. Added 4 tests.

4. **LSP hover** — Implemented hover_for_module (shows pin/component/net counts and exposed pins), hover_for_interface (shows pin declarations), hover_for_import (shows path and imported names), hover_for_assert (formats comparison or within+tolerance expression). Added format_operand helper covering all AssertOperand variants including Dimension. Added 4 tests.

5. **v2 example files** — Created 3 files:
   - `v2-modules.cypcb`: 2 modules (PowerSupply with 3 components+3 nets+3 pins, LedDriver with 2 components+1 net+2 pins) + board
   - `v2-interfaces.cypcb`: 4 interfaces (I2C, SPI, Power, UART) + 2 modules with matching pins
   - `v2-constraints.cypcb`: 6 components with physical values, 5 assert statements covering >=, ==, +/- %, +/- absolute, and range tolerance

6. **Parser tests** — Added 3 dedicated tests verifying v2 example files parse to expected AST structure (module count, pin count, interface count, assert count, component count).

## Verification

- `cargo build -p cypcb-{core,parser,world,lsp,render,drc,export,autoroute,cli,calc,kicad,library,router,rules,watcher,platform}` — all 16 crates compile clean ✅
- `cargo build --target wasm32-unknown-unknown -p cypcb-render` — WASM build succeeds ✅
- `cargo test -p cypcb-core` — 49 passed ✅
- `cargo test -p cypcb-parser` — 83 passed (3 new v2 example tests) ✅
- `cargo test -p cypcb-lsp` — 41 passed (4 new completion + 4 new hover tests) ✅
- `cargo test -p cypcb-world` — 133 passed, 1 pre-existing failure (test_sync_named_pin) ✅
- `cargo test -p cypcb-drc` — 32 passed ✅
- `cargo test -p cypcb-render` — 110 passed ✅
- Backward compat test: all 13 example files (10 v1 + 3 v2) parse clean ✅

### Slice-Level Verification Status

- ✅ `cargo test -p cypcb-parser` — all existing + new tests pass
- ✅ `cargo test -p cypcb-core` — PhysicalUnit tests pass
- ✅ `cargo build` (all crates excl. Tauri) — full workspace compiles
- ✅ `cargo build --target wasm32-unknown-unknown -p cypcb-render` — WASM build succeeds
- ✅ Backward compat: all v1 example files parse with zero errors
- ✅ Forward test: v2 example files parse to expected AST structure

## Diagnostics

- `cargo test -p cypcb-lsp -- test_hover_on_module` — verify module hover
- `cargo test -p cypcb-lsp -- test_top_level_completions_v2_keywords` — verify v2 keyword completions
- `cargo test -p cypcb-lsp -- test_physical_unit_completions` — verify all 23 unit suffixes
- `cargo test -p cypcb-parser -- test_v2` — verify all 3 v2 example files parse clean

## Deviations

None — all planned steps executed as specified.

## Known Issues

- `physical_unit_completions()` is defined but not yet wired into context-aware completion (would need a ValueContext variant in CompletionContext) — intentionally deferred, the function is available for future wiring
- `test_sync_named_pin` failure in cypcb-world is pre-existing (noted in T01)
- 2 cypcb-export test failures are pre-existing (filesystem dependency)
- GTK/Tauri build dependency prevents full `cargo build` / `cargo test` — pre-existing

## Files Created/Modified

- `viewer/src/editor/cypcb-language.ts` — Added v2 keywords, physical unit suffixes, tolerance/comparison operators
- `crates/cypcb-lsp/src/completion.rs` — Added module/interface/import/assert completions with snippets, physical_unit_completions(), 4 tests
- `crates/cypcb-lsp/src/hover.rs` — Added hover_for_module/interface/import/assert, format_operand helper, 4 tests
- `crates/cypcb-parser/src/parser.rs` — Added 3 tests for v2 example file parsing
- `examples/v2-modules.cypcb` — New: module definitions with nested components and pins
- `examples/v2-interfaces.cypcb` — New: interface definitions (I2C, SPI, Power, UART) with modules
- `examples/v2-constraints.cypcb` — New: assert statements with comparisons, tolerances, physical units
