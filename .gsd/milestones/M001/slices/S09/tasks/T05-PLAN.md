# T05: LibraryManager Orchestrator

**Slice:** S09 — **Milestone:** M001

## Description

Create LibraryManager as the unified orchestrator connecting all sources, search, and persistence.

Purpose: Single entry point for all library operations. Covers LIB-12 (unified search across sources) and wires together all prior plans.
Output: LibraryManager struct that application code uses for all library operations.

## Must-Haves

- [ ] "LibraryManager provides single entry point for all library operations"
- [ ] "Unified search queries all indexed sources simultaneously"
- [ ] "Library import indexes components into FTS5 for instant search"
- [ ] "User can configure search paths for KiCad libraries"

## Files

- `crates/cypcb-library/src/manager.rs`
- `crates/cypcb-library/src/lib.rs`
