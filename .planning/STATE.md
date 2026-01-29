# Project State: CodeYourPCB

**Last Updated:** 2026-01-29
**Milestone:** v1.1 "Foundation & Desktop"
**Status:** Phase 9 Complete

## Project Reference

**Core Value:** The source file is the design - git-friendly, AI-editable, deterministic PCB layouts

**Current Focus:** Build solid foundation for library management, project organization, and professional desktop experience with web deployment

## Current Position

**Phase:** Phase 10 (Library Management Foundation)
**Plan:** 3 of 6 complete
**Status:** Plan 10-03 complete - FTS5 search engine implemented

**Progress:**
```
[=========                                         ] 19%
v1.1: Phase 9 ✓ → 10 (3/6) → 11 → 12 → 13 → 14 → 15
```

**Requirements Complete:** 12/64 (18.8%)

**Requirements Coverage:** 64/64 mapped to phases (100%)

## Milestone Context

**Previous Milestone:** v1.0 MVP (shipped 2026-01-29)
- 32,440 lines of code (30,005 Rust + 2,435 TypeScript)
- 7 phases completed (Phase 1-8), 51 plans executed
- 35/35 v1 requirements satisfied
- Delivered: DSL parser, ECS board model, web viewer, DRC, manufacturing export, FreeRouting integration, LSP server, touchpad navigation, file picker

**Current Milestone:** v1.1 Foundation & Desktop
- 7 phases planned (Phase 9-15)
- 64 requirements across 7 categories
- Extends v1.0 with desktop app, library management, embedded editor, dark mode

## Performance Metrics

**Phases:**
- Total planned (v1.1): 7
- Completed: 1
- In progress: 1
- Pending: 5

**Requirements:**
- Total v1.1: 64
- Satisfied: 8
- In progress: 3
- Pending: 53

**Efficiency:**
- Plans completed (v1.1): 4
- Blockers encountered: 1 (pkg-config requirement - resolved with optional feature)
- Revisions needed: 0

## Accumulated Context

### Key Decisions

**Phase Numbering Continues from v1.0:**
- v1.0 completed Phase 1-8
- v1.1 starts at Phase 9 to maintain continuity
- Phase numbers reflect overall project progression

**Platform Abstraction First (Phase 9):**
- Research shows 800% code duplication risk when platform checks scatter through business logic
- Solution: Build-time conditional compilation with shared interfaces for FileSystem, Dialog, Menu, Storage
- Desktop uses native FS + SQLite, web uses File System Access API + IndexedDB
- Both expose identical APIs to application code

**Library Management Foundation (Phase 10):**
- Multi-source library support: KiCad + JLCPCB + custom libraries
- Namespace-prefixed imports prevent conflicts (kicad::R_0805 vs jlcpcb::R_0805)
- Dual storage backends share parsing logic, platform-specific persistence
- Can run parallel to Phase 11 after Phase 9 completes

**Theme System Before Monaco (Phase 11):**
- Central ThemeManager must coordinate CSS, Monaco, Canvas, Three.js
- Easier to integrate Monaco into existing theme than retrofit
- Prevents dark mode inconsistency (jarring "flashbang" effect)
- Can run parallel to Phase 10 after Phase 9 completes

**Desktop Before Web (Phase 12 before 13):**
- Desktop is superset of web capabilities
- Building desktop first reveals what needs abstraction for web
- Validates platform abstraction layer works
- Phase 12 and 13 can technically run in parallel after Phase 9

**Monaco After Infrastructure (Phase 14):**
- Depends on theme system (Phase 11) and LSP infrastructure
- Performance-critical, must optimize from start
- Bundle size risk: 4MB+ if misconfigured, need minimal workers
- Must wait for Phase 11 completion

**Documentation Last (Phase 15):**
- Documents completed features from all previous phases
- Final polish and user onboarding materials
- Must wait for all feature phases to complete

**WASM Constraints (Phase 9):**
- WASM is single-threaded, so platform abstractions use `#[async_trait(?Send)]`
- FileHandle traits can't require Send+Sync bounds
- Design for most restricted platform (WASM) to ensure compatibility
- Established in 09-01, applies to all future platform abstractions

**Menu as Data Model (Phase 9):**
- Menu is declarative data structure, NOT a trait abstraction
- Tauri native menus and HTML menus are fundamentally different rendering paradigms
- Data model (MenuBar/Menu/MenuItem) can be serialized and rendered by either platform
- Rendering deferred to Phase 12 (Desktop native menus) and Phase 13 (Web HTML menus)
- Established in 09-03

**Platform Facade Pattern (Phase 9):**
- Platform struct is single entry point for all platform services
- Application code imports only Platform, never platform-specific types
- Accessor methods (fs(), dialog(), storage()) return concrete types behind cfg attributes
- Prevents platform checks from scattering through business logic (800% duplication prevention)
- Established in 09-03

**Namespace-Prefixed Components (Phase 10):**
- ComponentId with source::name format prevents conflicts across library sources
- kicad::R_0805 vs jlcpcb::R_0805 are distinct components
- Composite UNIQUE constraint (source, name) enforces per-source uniqueness
- Display trait shows full_name() in UI (e.g., "kicad::R_0805")
- Established in 10-01

**Dual Metadata Storage (Phase 10):**
- Individual columns (description, manufacturer, mpn, etc.) enable SQL WHERE and FTS5 indexing
- metadata_json TEXT column preserves full ComponentMetadata as JSON for extensibility
- Balances queryability with source-specific field flexibility
- Deserialization required when reading components (minimal overhead)
- Established in 10-01

**SQLite FTS5 for Component Search (Phase 10):**
- FTS5 sufficient for component library scale (<1M components)
- BM25 ranking provides relevance scoring (lower/more negative = better match)
- Automatic sync via INSERT/UPDATE/DELETE triggers (no manual index management)
- Upgrade path to Tantivy if search performance becomes bottleneck
- Established in 10-01

**FTS5 BM25 Negative Scores (Phase 10):**
- BM25 scores are NEGATIVE in SQLite FTS5 (implementation detail)
- Lower (more negative) = better match
- ORDER BY rank ASC gives best matches first
- Different from most search engines (usually positive scores, higher = better)
- Established in 10-03

**Dynamic SQL with Parameterized Filters (Phase 10):**
- Build SQL conditionally based on which SearchFilters are set
- Convert Vec<String> to Vec<&dyn rusqlite::ToSql> for parameter passing
- Supports any combination of filters without code duplication
- Field names validated against whitelist to prevent SQL injection
- Established in 10-03

**Manual S-Expression Tree Walking (Phase 10):**
- KiCad S-expressions have variable structure incompatible with Serde derive macros
- Navigate lexpr::Value Cons cells (Lisp-style linked lists) with pattern matching
- Recursive field search traverses entire tree for nested structures
- More flexible and maintainable than custom Serde deserializer
- Established in 10-02

**LibrarySource Trait Pattern (Phase 10):**
- Common interface for KiCad, JLCPCB, and custom library sources
- Blocking I/O design (runs in spawn_blocking, not async)
- source_name, list_libraries, import_library methods
- Enables multi-source library aggregation in future manager
- Established in 10-02

### Active TODOs

- [x] Plan Phase 9: Platform Abstraction Layer (completed)
- [x] Validate all platform abstractions compile for both targets (09-01 complete)
- [x] Complete remaining Phase 9 plans (09-01, 09-02, 09-03 complete)
- [ ] Complete final Phase 9 plan (09-04)
- [ ] Set up continuous integration for dual-target builds

### Known Blockers

**Linux File Dialogs (Phase 9):**
- Native file dialogs on Linux require pkg-config and system libraries (gtk3-dev/wayland-dev)
- Not available in CI containerized environments
- **Resolution:** Made rfd optional via `native-dialogs` feature. FileSystem returns NotSupported error without feature. Production builds enable feature when dependencies available.
- **Impact:** CI can compile and test without system dependencies. Desktop builds need manual dependency installation.

### Research Notes

**From SUMMARY.md (2026-01-29):**
- Platform abstraction is critical success factor (800% duplication prevention)
- Library namespace conflicts need conflict detection UI
- Monaco bundle size explosion risk (vite-plugin-monaco-editor with minimal workers)
- Dark mode inconsistency across subsystems (central ThemeManager solution)
- File System API mismatches between desktop and web (design for most restricted platform)

**Phases needing deeper research during planning:**
- Phase 10: KiCad S-expression format edge cases, library conflict resolution UX
- Phase 14: Monaco worker configuration for .cypcb, Tree-sitter integration

**Phases with standard patterns (skip research-phase):**
- Phase 9: Well-documented Tauri ecosystem pattern
- Phase 11: CSS custom properties are mature web standard
- Phase 12: Tauri 2.0 documentation is comprehensive
- Phase 13: Vite static deployment is well-documented

### v1.0 Tech Debt

**From v1.0 completion:**
- Phase 3 (Validation) missing formal VERIFICATION.md file (functionality working)
- Module/import system deferred to v2
- Grid snapping deferred (grid display works)
- Net highlighting deferred

**Not blocking v1.1:** All v1.0 features functional, documentation gaps acceptable

## Session Continuity

**Where We Are:**
Phase 10 Plan 03 complete (2026-01-29). FTS5 search engine implemented with BM25 ranking, field-specific queries, and optional filters. search_components function provides the backend for all component discovery features. 2 additional LIB requirements satisfied (LIB-01 search, LIB-12 unified search). Critical bug fixed: SearchFilters::default() now returns limit=50 instead of 0.

**What's Next:**
Continue Phase 10 with Plan 04 (Search Manager), Plan 05 (Library UI), or Plan 06 (Integration). Plan 02 (KiCad Parser) already complete. Plans can run in parallel.

**Context for Next Session:**
- Library foundation complete: ComponentId, Component, ComponentMetadata models ready (Plan 01)
- SQLite schema with libraries, components, components_fts tables initialized (Plan 01)
- FTS5 automatic sync via triggers (no manual index management) (Plan 01)
- CRUD operations: insert_library, insert_component, insert_components_batch, get_component (Plan 01)
- KiCad .kicad_mod parser implemented with LibrarySource trait (Plan 02)
- Search engine complete: search_components, search_by_field, rebuild_index, component_count (Plan 03)
- All library code uses parameterized queries (SQL injection prevention)
- 12+ comprehensive tests verify schema, CRUD, search, KiCad parsing

**Parallelization Opportunities:**
Within Phase 10 (after Plans 01-03):
- Plan 04 (Search Manager) - High-level API wrapping search_components
- Plan 05 (Library UI) - Component browser and search interface
- Plan 06 (Integration) - Wire library management into main application

Other phases (independent):
- Phase 11 (Dark Mode) - Independent of library management
- Phase 12 (Desktop) - Independent of library management
- Phase 13 (Web) - Independent of library management

After Phase 11 completes:
- Phase 14 (Monaco) requires Phase 11 theme system

After all feature phases complete:
- Phase 15 (Documentation) documents everything

---
*State initialized for v1.1: 2026-01-29*

## Recent Activity

| Date | Plan | Summary |
|------|------|---------|
| 2026-01-29 | 09-01 | Created cypcb-platform crate with FileSystem abstraction (native + WASM) |
| 2026-01-29 | 09-02 | Implemented Dialog wrapper (rfd) and Storage trait (SQLite + localStorage) |
| 2026-01-29 | 09-03 | Added Menu data model and Platform facade for unified service access |
| 2026-01-29 | 10-01 | Created cypcb-library crate with data models, SQLite schema, FTS5 search foundation |
| 2026-01-29 | 10-02 | Implemented KiCad .kicad_mod parser with LibrarySource trait and auto-organize folders |
| 2026-01-29 | 10-03 | Implemented FTS5 search engine with BM25 ranking and optional filters |

**Last session:** 2026-01-29 10:42 UTC
**Stopped at:** Completed 10-03 execution
**Resume file:** None

*Last updated: 2026-01-29 10:42 UTC*

**Storage Strategy (Phase 9):**
- Native: SQLite via rusqlite for structured key-value storage with table namespacing
- Web: localStorage for v1.1 (sufficient for settings/preferences, ~5MB quota)
- IndexedDB upgrade path documented for Phase 10 when library storage needs >5MB
- Trait abstraction allows swapping backends without API changes
- Established in 09-02

**Dialog Limitations (Phase 9):**
- rfd requires GUI system libraries on Linux (GTK/Wayland)
- Made optional via desktop feature to support CI/headless builds
- Folder picker not supported in browsers (API limitation, not implementation)
- Established in 09-02
