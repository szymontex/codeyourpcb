# T10: Zones & Keepouts

**Slice:** S03 — **Milestone:** M001

## Description

Implement zones and keepouts for board model and DRC.

Purpose: Fulfill BRD-05 requirement for zones and keepouts. Keepouts define regions where copper is prohibited, essential for antenna clearance, mechanical clearance, and manufacturing constraints.

Output: Zone AST, Zone ECS component, keepout sync, and KeepoutRule for DRC.

## Must-Haves

- [ ] "Zones can be defined in DSL with rectangular bounds"
- [ ] "Keepouts prevent copper in specified regions"
- [ ] "KeepoutRule detects copper features inside keepout zones"
- [ ] "Zones appear in board model with layer and type info"

## Files

- `crates/cypcb-parser/grammar/grammar.js`
- `crates/cypcb-parser/src/ast.rs`
- `crates/cypcb-parser/src/parser.rs`
- `crates/cypcb-world/src/components/mod.rs`
- `crates/cypcb-world/src/components/zone.rs`
- `crates/cypcb-world/src/sync.rs`
- `crates/cypcb-drc/src/rules/keepout.rs`
- `crates/cypcb-drc/src/rules/mod.rs`
