# T01: Library Crate Foundation

**Slice:** S09 — **Milestone:** M001

## Description

Create the cypcb-library crate with core data models, database schema, and error types that all other library plans depend on.

Purpose: Foundation for all library management features. Every other plan in this phase imports types from this crate.
Output: New crate with models, schema, and error modules. Compiles as part of workspace.

## Must-Haves

- [ ] "Library crate compiles and is part of workspace"
- [ ] "Component and Library data models represent multi-source components with namespace prefixing"
- [ ] "SQLite schema creates libraries, components, and components_fts tables with sync triggers"

## Files

- `crates/cypcb-library/Cargo.toml`
- `crates/cypcb-library/src/lib.rs`
- `crates/cypcb-library/src/models.rs`
- `crates/cypcb-library/src/schema.rs`
- `crates/cypcb-library/src/error.rs`
- `Cargo.toml`
