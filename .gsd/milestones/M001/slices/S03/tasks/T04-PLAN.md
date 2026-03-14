# T04: Custom Footprint DSL

**Slice:** S03 — **Milestone:** M001

## Description

Add DSL syntax for custom footprint definitions and wire them to the FootprintLibrary.

Purpose: Fulfill FTP-04 requirement allowing users to define custom footprints inline in their .cypcb files. This enables non-standard packages without external library files.

Output: Grammar rules, AST types, parser support, and sync logic for footprint definitions.

## Must-Haves

- [ ] "Footprint can be defined inline in DSL"
- [ ] "Footprint pads have numeric IDs"
- [ ] "Parser produces FootprintDef AST node"
- [ ] "Custom footprints are registered in FootprintLibrary before component sync"

## Files

- `crates/cypcb-parser/grammar/grammar.js`
- `crates/cypcb-parser/src/ast.rs`
- `crates/cypcb-parser/src/parser.rs`
- `crates/cypcb-world/src/sync.rs`
- `crates/cypcb-world/src/footprint/library.rs`
