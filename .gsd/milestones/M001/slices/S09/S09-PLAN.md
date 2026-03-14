# S09: Library Management Foundation

**Goal:** Create the cypcb-library crate with core data models, database schema, and error types that all other library plans depend on.
**Demo:** Create the cypcb-library crate with core data models, database schema, and error types that all other library plans depend on.

## Must-Haves


## Tasks

- [x] **T01: Library Crate Foundation**
  - Create the cypcb-library crate with core data models, database schema, and error types that all other library plans depend on.

Purpose: Foundation for all library management features. Every other plan in this phase imports types from this crate.
Output: New crate with models, schema, and error modules. Compiles as part of workspace.
- [x] **T02: KiCad S-Expression Parser**
  - Implement KiCad .kicad_mod S-expression parser and .pretty folder importer using lexpr.

Purpose: Enables importing the most popular open-source PCB footprint format. Covers LIB-04 (KiCad import) and LIB-11 (auto-organize dropped folders).
Output: KiCadSource struct implementing LibrarySource trait, parsing .kicad_mod files with lexpr.
- [x] **T03: FTS5 Search Engine**
  - Implement FTS5-based full-text search with BM25 ranking for component discovery.

Purpose: Core search engine enabling LIB-01 (search by name/MPN/value/category) and LIB-12 (unified search across sources).
Output: search.rs module with search_components function using FTS5 queries.
- [x] **T04: Custom & JLCPCB Sources**
  - Implement custom library source for user-created libraries and JLCPCB API client for parts catalog search.

Purpose: Covers LIB-02 (organize by manufacturer/function), LIB-05 (JLCPCB import), LIB-06 (custom libraries), LIB-10 (search paths config).
Output: CustomSource and JLCPCBSource implementing LibrarySource trait.
- [x] **T05: LibraryManager Orchestrator**
  - Create LibraryManager as the unified orchestrator connecting all sources, search, and persistence.

Purpose: Single entry point for all library operations. Covers LIB-12 (unified search across sources) and wires together all prior plans.
Output: LibraryManager struct that application code uses for all library operations.
- [x] **T06: Metadata & Version Tracking**
  - Implement metadata viewing, footprint preview extraction, version tracking, and 3D model association.

Purpose: Covers LIB-03 (3D STEP models), LIB-07 (version tracking), LIB-08 (footprint preview), LIB-09 (metadata viewing).
Output: metadata.rs and preview.rs modules with version tracking, preview extraction, and model association.

## Files Likely Touched

- `crates/cypcb-library/Cargo.toml`
- `crates/cypcb-library/src/lib.rs`
- `crates/cypcb-library/src/models.rs`
- `crates/cypcb-library/src/schema.rs`
- `crates/cypcb-library/src/error.rs`
- `Cargo.toml`
- `crates/cypcb-library/src/sources/mod.rs`
- `crates/cypcb-library/src/sources/kicad.rs`
- `crates/cypcb-library/src/lib.rs`
- `crates/cypcb-library/Cargo.toml`
- `crates/cypcb-library/src/search.rs`
- `crates/cypcb-library/src/lib.rs`
- `crates/cypcb-library/src/sources/custom.rs`
- `crates/cypcb-library/src/sources/jlcpcb.rs`
- `crates/cypcb-library/src/sources/mod.rs`
- `crates/cypcb-library/Cargo.toml`
- `crates/cypcb-library/src/manager.rs`
- `crates/cypcb-library/src/lib.rs`
- `crates/cypcb-library/src/metadata.rs`
- `crates/cypcb-library/src/preview.rs`
- `crates/cypcb-library/src/lib.rs`
- `crates/cypcb-library/src/manager.rs`
