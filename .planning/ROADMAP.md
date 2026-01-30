# Roadmap: CodeYourPCB v1.1

**Milestone:** v1.1 "Foundation & Desktop"
**Goal:** Build a solid foundation for library management, project organization, and professional desktop experience with web deployment
**Created:** 2026-01-29
**Phases:** 7 (Phase 9-15)
**Requirements:** 64 total

## Overview

v1.1 extends the proven v1.0 web viewer foundation with professional desktop capabilities and comprehensive library management. The roadmap establishes platform abstractions first to prevent code duplication, then builds library management, theming, and deployment infrastructure before integrating the Monaco editor. This approach ensures desktop and web builds share a common core while enabling platform-specific enhancements.

## Phase 9: Platform Abstraction Layer

**Goal:** Establish build-time conditional compilation that enables desktop and web to share business logic while using platform-specific implementations

**Dependencies:** None (foundation phase)

**Requirements:** PLAT-01, PLAT-02, PLAT-03, PLAT-04, PLAT-05 (5 total)

**Success Criteria:**
1. FileSystem trait abstracts file operations with identical API for native FS and File System Access API
2. Dialog trait abstracts file dialogs with identical API for Tauri and browser
3. Menu trait abstracts application menus with identical API for Tauri and HTML
4. Storage trait abstracts persistence with identical API for SQLite and IndexedDB
5. Build compiles successfully for both web and desktop targets using conditional features

**Plans:** 3 plans

Plans:
- [x] 09-01-PLAN.md — Create cypcb-platform crate with FileSystem trait and native/web implementations
- [x] 09-02-PLAN.md — Dialog wrapper and Storage trait with SQLite/localStorage backends
- [x] 09-03-PLAN.md — Menu data model and Platform facade with dual-target verification

**Status:** Complete
**Completed:** 2026-01-29

## Phase 10: Library Management Foundation

**Goal:** Users can search, organize, and preview components from multiple library sources through a unified interface

**Dependencies:** Phase 9 (requires Storage abstraction)

**Requirements:** LIB-01, LIB-02, LIB-03, LIB-04, LIB-05, LIB-06, LIB-07, LIB-08, LIB-09, LIB-10, LIB-11, LIB-12 (12 total)

**Success Criteria:**
1. User can search across KiCad, JLCPCB, and custom libraries simultaneously with unified results
2. User can import KiCad .pretty folders and system auto-organizes with namespace prefixing
3. User can create custom component libraries and organize by manufacturer or function
4. User can preview footprints visually before adding to board
5. User can associate 3D STEP models with components and view metadata (datasheet, specs)

**Plans:** 6 plans

Plans:
- [x] 10-01-PLAN.md — Create cypcb-library crate with models, schema, and error types
- [x] 10-02-PLAN.md — KiCad .kicad_mod S-expression parser and .pretty folder importer
- [x] 10-03-PLAN.md — FTS5 full-text search engine with BM25 ranking
- [x] 10-04-PLAN.md — Custom library source and optional JLCPCB API client
- [x] 10-05-PLAN.md — LibraryManager orchestrator with unified search
- [x] 10-06-PLAN.md — Metadata, version tracking, footprint preview, and 3D model association

**Status:** Complete
**Completed:** 2026-01-29

## Phase 11: Dark Mode & UI Polish

**Goal:** Application provides consistent, accessible dark and light themes across all UI surfaces

**Dependencies:** None (independent of other phases)

**Requirements:** UI-01, UI-02, UI-03, UI-04, UI-05, UI-06, UI-07, UI-08, UI-09 (9 total)

**Success Criteria:**
1. Application detects OS theme preference and applies appropriate theme automatically
2. User can manually toggle between dark and light modes with state persisted
3. Theme applies consistently across editor, viewer, dialogs, and menus
4. Dark mode meets WCAG AA contrast requirements (4.5:1 minimum)
5. Light mode meets WCAG AA contrast requirements (4.5:1 minimum)

**Plans:** 4 plans

Plans:
- [x] 11-01-PLAN.md — Theme system foundation: CSS custom properties, ThemeManager, FART prevention
- [x] 11-02-PLAN.md — Migrate all UI surfaces to CSS variables and make canvas renderer theme-aware
- [x] 11-03-PLAN.md — Theme toggle UI, keyboard shortcut, WCAG AA verification and polish
- [x] 11-04-PLAN.md — Monaco editor theme definitions (infrastructure for Phase 14)

**Status:** Complete
**Completed:** 2026-01-29

## Phase 12: Tauri Desktop Foundation

**Goal:** Users can run CodeYourPCB as a native desktop application with OS integration

**Dependencies:** Phase 9 (uses platform abstractions), Phase 11 (theme system)

**Requirements:** DESK-01, DESK-02, DESK-03, DESK-04, DESK-05, DESK-06, DESK-07, DESK-08, DESK-09, DESK-10 (10 total)

**Success Criteria:**
1. User can open and save files via native OS file dialogs
2. Application has native menu bar with standard File/Edit/View/Help menus
3. Application installs via platform-specific installer (MSI/DMG/AppImage) with <10MB bundle size
4. Application starts in less than 1 second and uses <50MB memory idle
5. User can use standard keyboard shortcuts (Ctrl+S, Ctrl+O, Ctrl+Z) that work cross-platform

**Plans:** 5 plans

Plans:
- [x] 12-01-PLAN.md — Scaffold Tauri v2 project structure with config, workspace integration, and Vite compatibility
- [x] 12-02-PLAN.md — Native menu bar from platform MenuBar data model and file open/save IPC commands
- [x] 12-03-PLAN.md — Frontend desktop integration module wiring menu events to viewer actions
- [x] 12-04-PLAN.md — Installer config, file association, updater plugin, and performance verification
- [x] 12-05-PLAN.md — Wire desktop menu events to viewer engine (gap closure)

**Status:** Complete
**Completed:** 2026-01-30

## Phase 13: Web Deployment

**Goal:** Users can access CodeYourPCB via browser without installation, with fast loading and file access

**Dependencies:** Phase 9 (uses platform abstractions), Phase 11 (theme system)

**Requirements:** WEB-01, WEB-02, WEB-03, WEB-04, WEB-05, WEB-06, WEB-07, WEB-08, WEB-09 (9 total)

**Success Criteria:**
1. Web application loads in less than 3 seconds on 3G connection
2. Web application works in Chrome, Firefox, Safari, and Edge
3. User can open and save local files via File System Access API
4. User can share designs via URL with project state loaded from URL parameters
5. Web deployment uses global CDN with HTTPS serving

**Status:** Pending

## Phase 14: Monaco Editor Integration

**Goal:** Users can edit .cypcb files in an embedded editor with syntax highlighting and LSP features

**Dependencies:** Phase 11 (theme system), Phase 12 or 13 (LSP server spawning or connection)

**Requirements:** EDIT-01, EDIT-02, EDIT-03, EDIT-04, EDIT-05, EDIT-06, EDIT-07, EDIT-08, EDIT-09, EDIT-10 (10 total)

**Success Criteria:**
1. Editor provides syntax highlighting for .cypcb files using Tree-sitter grammar
2. Editor connects to existing tower-lsp server and provides autocomplete and hover
3. Editor displays syntax and semantic errors inline as user types
4. Editor supports standard code editing features (find/replace, undo/redo, multi-cursor)
5. Editor and board viewer display side-by-side with live preview on file changes

**Status:** Pending

## Phase 15: Documentation & Polish

**Goal:** Users have comprehensive documentation explaining features, workflows, and platform differences

**Dependencies:** All previous phases (documents completed features)

**Requirements:** DOC-01, DOC-02, DOC-03, DOC-04, DOC-05, DOC-06, DOC-07, DOC-08, DOC-09 (9 total)

**Success Criteria:**
1. User guide explains .cypcb file syntax with annotated examples
2. User guide explains library management workflows (import, organize, search)
3. User guide clearly documents desktop vs web feature differences
4. API documentation covers LSP server usage and library file formats
5. Contributing guide explains development setup and architecture

**Status:** Pending

## Progress Tracking

| Phase | Requirements | Status | Completion |
|-------|-------------|--------|------------|
| 9 - Platform Abstraction | 5 | Complete | 100% |
| 10 - Library Management | 12 | Complete | 100% |
| 11 - Dark Mode & UI Polish | 9 | Complete | 100% |
| 12 - Tauri Desktop | 10 | Complete | 100% |
| 13 - Web Deployment | 9 | Pending | 0% |
| 14 - Monaco Editor | 10 | Pending | 0% |
| 15 - Documentation | 9 | Pending | 0% |

**Overall:** 36/64 requirements complete (56%)

## Critical Path

The critical path for v1.1 is:

1. Phase 9 (Platform Abstraction) - Must complete before any platform-specific features
2. Phase 10 (Library Management) - Independent, can run parallel to phases 11-13
3. Phase 11 (Dark Mode) - Should complete before Monaco integration
4. Phase 12 OR 13 (Desktop or Web) - Interchangeable, both use abstractions from Phase 9
5. Phase 14 (Monaco Editor) - Requires theme system and LSP infrastructure
6. Phase 15 (Documentation) - Final polish after features complete

Phases 10, 11, and 12/13 can be developed in parallel after Phase 9 completes.

---
*Last updated: 2026-01-29*
