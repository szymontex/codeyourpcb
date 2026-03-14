# T01: Trace/Via ECS & Net Constraints

**Slice:** S05 — **Milestone:** M001

## Description

Add Trace and Via ECS components to the board model, plus DSL extensions for net electrical constraints, manual traces, and trace locking.

Purpose: The autorouter (Plan 05-06) produces traces that must be stored in the ECS. Additionally, users need to define electrical constraints (current, width, clearance) on nets for the router to respect.
Output: Trace/Via components, extended grammar with net constraints and manual trace syntax

## Must-Haves

- [ ] "Traces can be represented as ECS entities with start/end/width/layer"
- [ ] "Net constraints (width, clearance, current) are parseable in DSL"
- [ ] "Manual trace waypoints can be defined in DSL"
- [ ] "Locked traces flag prevents autorouter from modifying them"

## Files

- `crates/cypcb-world/src/components/trace.rs`
- `crates/cypcb-world/src/components/mod.rs`
- `crates/cypcb-world/src/lib.rs`
- `crates/cypcb-parser/grammar/grammar.js`
- `crates/cypcb-parser/src/ast.rs`
- `crates/cypcb-parser/src/parser.rs`
