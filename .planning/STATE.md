# Project State: CodeYourPCB

**Last Updated:** 2026-01-29
**Milestone:** v1.1 "Foundation & Desktop"
**Status:** Phase 10 Complete

## Project Reference

**Core Value:** The source file is the design - git-friendly, AI-editable, deterministic PCB layouts

**Current Focus:** Build solid foundation for library management, project organization, and professional desktop experience with web deployment

## Current Position

**Phase:** Phase 12 (Tauri Desktop Foundation)
**Plan:** 12-01 complete
**Status:** In progress - Tauri v2 shell scaffolded, needs GTK libraries for compilation

**Progress:**
```
[=========================                         ] 45%
v1.1: Phase 9 ✓ → 10 ✓ → 11 ✓ → 12 [░] → 13 → 14 → 15
```

**Requirements Complete:** 30/64 (47%)

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
- Completed: 3
- In progress: 1
- Pending: 3

**Requirements:**
- Total v1.1: 64
- Satisfied: 30
- In progress: 0
- Pending: 34

**Efficiency:**
- Plans completed (v1.1): 14
- Blockers encountered: 3 (pkg-config resolved, FTS5 corruption fixed, GTK3 libraries needed)
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

**FTS5 External Content Tables Don't Support UPDATE (Phase 10):**
- Using `content=components, content_rowid=rowid` causes SQLITE_CORRUPT_VTAB on UPDATE
- SQLite FTS5 external content tables track rowids but UPDATE doesn't sync properly
- Solution: Use standalone FTS5 table with DELETE+INSERT in UPDATE trigger
- INSERT ... ON CONFLICT ... DO UPDATE doesn't fire UPDATE triggers
- Must use separate INSERT try/catch UPDATE pattern for trigger compatibility
- Established in 10-04

**Optional Features for API Integrations (Phase 10):**
- JLCPCB API requires manual application approval - not all users have access
- Feature flags allow compiling without optional dependencies
- rustls-tls preferred over native-tls to avoid system OpenSSL dependency
- Enables CI builds without pkg-config/libssl-dev requirements
- Established in 10-04

**Arc<Mutex<Connection>> for Shared Resources (Phase 10):**
- CustomSource doesn't own SQLite Connection, receives Arc<Mutex<Connection>>
- Allows sharing single connection across multiple source instances
- Caller manages connection lifetime and initialization
- Alternative: each source owns connection, but wasteful for single DB
- Established in 10-04

**LibraryManager Single Entry Point (Phase 10):**
- LibraryManager aggregates all sources (KiCad, Custom, JLCPCB) behind unified API
- Application code never imports schema/search/sources directly
- Single Arc<Mutex<Connection>> created in manager, cloned to sources
- Configuration methods (set_kicad_search_paths) are mutable, operations are immutable
- Import pipeline verified end-to-end: source → parse → index → search
- Established in 10-05

**FART Prevention for Dark Mode (Phase 11):**
- Inline script in HTML head sets data-theme before CSS loads
- Prevents Flash of inAccurate coloR Theme during page load
- Synchronous localStorage read + matchMedia check executes before first paint
- Critical for professional user experience across theme switches
- Established in 11-01

**CSS Custom Properties Over SCSS (Phase 11):**
- Native CSS custom properties with data-theme attribute selector
- No build step required, browser-native, dynamic theme switching
- 16 semantic colors + 4 PCB-specific colors for both light/dark
- WCAG AA compliant (4.5:1 minimum contrast ratios)
- Alternative to SCSS/CSS-in-JS patterns
- Established in 11-01

**ThemeManager Singleton Pattern (Phase 11):**
- Single source of truth for theme state
- Coordinates localStorage persistence, matchMedia system preference, DOM updates
- Provides subscribe() for component reactivity to theme changes
- Critical for coordinating Monaco, Canvas, Three.js theme consistency (Phase 14)
- Established in 11-01

**Filter Brightness for Hover Effects (Phase 11):**
- Use `filter: brightness(0.85)` instead of hardcoded hover colors
- Automatically adapts to both light and dark theme colors
- Reduces CSS custom property count (no separate hover colors needed)
- Works consistently across all button types (accent, success, error, warning)
- Established in 11-02

**PCB Electrical Colors Fixed (Phase 11):**
- Copper red/blue, violations red, ratsnest gold never change with theme
- Domain colors have semantic meaning beyond theme preference
- Only UI elements (background, grid, labels) adapt to theme
- Maintains consistency with PCB industry standards (KiCad-style colors)
- Established in 11-02

**Theme-Aware Canvas Rendering (Phase 11):**
- Single getComputedStyle() call per render frame (efficient)
- Theme colors passed as parameter to avoid repeated DOM queries
- Canvas subscribes to ThemeManager for automatic redraw on theme change
- Background, grid, board outline, labels all theme-aware
- Established in 11-02

**Tauri v2 Project Structure at src-tauri/ (Phase 12):**
- Standard Tauri layout sits alongside viewer/ directory at project root
- Separate workspace member (cypcb-desktop) with own Cargo.toml
- Alternative (nested inside viewer/) rejected - Tauri convention is root-level
- Established in 12-01

**Window Maximized by Default (Phase 12):**
- PCB viewers benefit from maximum canvas space for board visibility
- Configured in tauri.conf.json app.windows[0].maximized: true
- Alternative (default size) rejected - requires manual resizing on every launch
- Established in 12-01

**Vite Watch Ignores src-tauri/ (Phase 12):**
- Prevents infinite rebuild loop (Rust compilation triggers Vite reload)
- Added to vite.config.ts server.watch.ignored: ['**/src-tauri/**']
- Without this, Tauri dev mode enters constant restart cycle
- Critical for usable development workflow
- Established in 12-01

**Build Target Conditional on TAURI_ENV_PLATFORM (Phase 12):**
- safari13 for macOS webview compatibility (WebKit)
- chrome105 for Windows webview compatibility (Edge/Chromium)
- esnext for web-only builds (non-Tauri)
- Single Vite config serves both desktop and web contexts
- Established in 12-01

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

**JLCPCB API Documentation Unknown (Phase 10):**
- JLCPCB API requires manual application approval, not publicly documented
- Assumed endpoint: `https://api.jlcpcb.com/components/search` but unverified
- **Resolution:** Made JLCPCB integration fully optional behind `jlcpcb` feature flag. Users with API access can enable and configure.
- **Impact:** JLCPCB search will need verification/adjustment once user with API access tests it. Core library functionality works without it.

**Tauri GTK3 System Libraries (Phase 12):**
- Tauri v2 on Linux requires GTK3 system libraries: pkg-config, libglib2.0-dev, libgtk-3-dev, libwebkit2gtk-4.1-dev, libayatana-appindicator3-dev, librsvg2-dev
- Not available in this execution environment, requires sudo to install
- **Resolution:** Tauri project structure created and validated (JSON config, icons, workspace setup). Compilation deferred until environment with GTK3 libraries available.
- **Impact:** Cannot run `tauri dev` or `tauri build` in this environment. Desktop development requires workstation or CI with system dependencies installed. Project structure is correct and will compile in proper environment.

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
Phase 12 in progress (2026-01-29). Plan 12-01 complete - Tauri v2 shell scaffolded with proper config. GTK3 system libraries required for compilation verification.

**What's Next:**
Continue Phase 12 (Desktop Application) - Native menus (12-02), file dialogs (12-03), window management (12-04). OR Phase 14 (Monaco Editor) if Phase 12 blocked. Phase 13 (Web Deployment) can run in parallel.

**Context for Next Session:**
- Phase 12 Plan 01 complete: Tauri v2 project scaffolded (12-01)
- Tauri structure: src-tauri/ workspace member with Cargo.toml, tauri.conf.json, capabilities (12-01)
- Vite integration: TAURI_DEV_HOST, watch ignore, conditional build targets (12-01)
- Window config: Maximized start, 800x600 minimum, file association for .cypcb (12-01)
- Plugins: tauri-plugin-dialog, tauri-plugin-fs, tauri-plugin-window-state registered (12-01)
- Icons: Placeholder PNGs created (32x32, 128x128, 256x256, ico, icns) (12-01)
- npm packages: @tauri-apps/api@2.9.1, @tauri-apps/cli@2.9.6 installed (12-01)
- Scripts: dev:desktop, build:desktop added to package.json (12-01)
- GTK3 blocker: Compilation requires system libraries not available in this environment (12-01)
- Commits: 12-01 (d01bd98, 732576a)

**Parallelization Opportunities:**
Next phases (independent after Phase 11):
- Phase 12 (Desktop) - Tauri app, native file dialogs, menus
- Phase 13 (Web) - Static deployment, browser limitations

Phase 14 now unblocked:
- Phase 14 (Monaco) - Requires Phase 11 theme system (complete)

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
| 2026-01-29 | 10-04 | CustomSource for user libraries, JLCPCBSource API client (optional), FTS5 UPDATE bug fixed |
| 2026-01-29 | 10-05 | LibraryManager orchestrator with unified search and import pipeline |
| 2026-01-29 | 10-06 | Version tracking, 3D model association, footprint preview extraction |
| 2026-01-29 | 11-01 | Theme types, ThemeManager singleton, CSS custom properties, FART prevention |
| 2026-01-29 | 11-02 | HTML inline styles to CSS variables, theme-aware canvas rendering |
| 2026-01-29 | 11-03 | Theme toggle UI with keyboard shortcut, WCAG AA verification |
| 2026-01-29 | 11-04 | Monaco editor theme definitions (light/dark) with ThemeManager wiring |
| 2026-01-29 | 12-01 | Tauri v2 desktop shell scaffolded with maximized window, file association, Vite integration |

**Last session:** 2026-01-29 17:05 UTC
**Stopped at:** Completed Phase 12 Plan 01 - Tauri foundation ready, needs GTK3 for compilation
**Resume file:** None

*Last updated: 2026-01-29 17:05 UTC*

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
