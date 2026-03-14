# T01: DRC Crate Setup

**Slice:** S03 — **Milestone:** M001

## Description

Create the cypcb-drc crate with foundational types for Design Rule Checking.

Purpose: Establish the DRC infrastructure that all rule implementations will use. This is the foundation for all validation work in Phase 3.

Output: A new cypcb-drc crate with DrcViolation type, ViolationKind enum, and DrcRule trait.

## Must-Haves

- [ ] "DRC crate compiles with all dependencies"
- [ ] "DrcViolation type captures location, entities, and message"
- [ ] "DrcRule trait defines check() interface"

## Files

- `crates/cypcb-drc/Cargo.toml`
- `crates/cypcb-drc/src/lib.rs`
- `crates/cypcb-drc/src/violation.rs`
- `crates/cypcb-drc/src/rules/mod.rs`
- `Cargo.toml`
