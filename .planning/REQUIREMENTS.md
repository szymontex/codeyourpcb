# Requirements: CodeYourPCB v1.1

**Defined:** 2026-01-29
**Milestone:** v1.1 "Foundation & Desktop"
**Core Value:** The source file is the design - git-friendly, AI-editable, deterministic

## v1.1 Requirements

Requirements for v1.1 Foundation & Desktop milestone. Each maps to roadmap phases.

### Platform Abstraction

- [x] **PLAT-01**: Build-time conditional compilation separates web and desktop implementations
- [x] **PLAT-02**: FileSystem trait abstracts file operations (native FS vs File System Access API)
- [x] **PLAT-03**: Dialog trait abstracts file dialogs (Tauri vs browser)
- [x] **PLAT-04**: Menu trait abstracts application menus (Tauri vs HTML)
- [x] **PLAT-05**: Storage trait abstracts persistence (SQLite vs IndexedDB)

### Library Management

- [x] **LIB-01**: User can search component libraries by name, MPN, value, category
- [x] **LIB-02**: User can organize libraries by manufacturer, function, custom categories
- [x] **LIB-03**: User can associate 3D STEP models with components
- [x] **LIB-04**: User can import libraries from KiCad (.kicad_mod, .pretty folders)
- [x] **LIB-05**: User can import libraries from JLCPCB (API integration)
- [x] **LIB-06**: User can create custom component libraries
- [x] **LIB-07**: User can track library versions and rollback changes
- [x] **LIB-08**: User can preview footprints before adding to board
- [x] **LIB-09**: User can view component metadata (datasheet links, specs, lifecycle)
- [x] **LIB-10**: User can configure library search paths
- [x] **LIB-11**: System auto-organizes dropped library folders ("idiot-proof")
- [x] **LIB-12**: User can search across all library sources in unified interface

### Desktop Application

- [ ] **DESK-01**: User can open files via native OS file dialog
- [ ] **DESK-02**: User can save files via native OS file dialog
- [ ] **DESK-03**: Application has native menu bar (File/Edit/View/Help)
- [ ] **DESK-04**: User can minimize, maximize, fullscreen application window
- [ ] **DESK-05**: User can use keyboard shortcuts (Ctrl+S, Ctrl+O, Ctrl+Z, etc.)
- [ ] **DESK-06**: Application installs via platform-specific installer (MSI/DMG/AppImage)
- [ ] **DESK-07**: Application can check for and install updates
- [ ] **DESK-08**: Desktop bundle size is <10MB (vs Electron 100MB+)
- [ ] **DESK-09**: Desktop memory footprint is <50MB idle (vs Electron 200MB+)
- [ ] **DESK-10**: Desktop application starts in <1 second

### Web Deployment

- [ ] **WEB-01**: Web application loads in <3 seconds on 3G connection
- [ ] **WEB-02**: Web application is responsive (works on tablet and desktop)
- [ ] **WEB-03**: Web application is served over HTTPS
- [ ] **WEB-04**: Web application works in Chrome, Firefox, Safari, Edge
- [ ] **WEB-05**: User can open local files via File System Access API
- [ ] **WEB-06**: User can save local files via File System Access API
- [ ] **WEB-07**: User can share designs via URL
- [ ] **WEB-08**: Shared URLs load project state from URL parameters
- [ ] **WEB-09**: Web deployment uses global CDN (Cloudflare Pages/Vercel)

### Embedded Editor

- [ ] **EDIT-01**: Editor provides syntax highlighting for .cypcb files
- [ ] **EDIT-02**: Editor provides auto-completion via LSP integration
- [ ] **EDIT-03**: Editor highlights syntax and semantic errors inline
- [ ] **EDIT-04**: Editor displays line numbers
- [ ] **EDIT-05**: Editor supports code folding for blocks
- [ ] **EDIT-06**: Editor has find/replace functionality
- [ ] **EDIT-07**: Editor supports undo/redo operations
- [ ] **EDIT-08**: Editor supports multi-cursor editing
- [ ] **EDIT-09**: Editor connects to existing tower-lsp server
- [ ] **EDIT-10**: Editor and board viewer display side-by-side

### Dark Mode & UI Polish

- [x] **UI-01**: Application supports dark mode theme
- [x] **UI-02**: Application supports light mode theme
- [x] **UI-03**: Application respects OS theme preference (auto dark/light)
- [x] **UI-04**: User can manually toggle between dark and light modes
- [x] **UI-05**: Theme applies consistently to editor, viewer, dialogs, menus
- [x] **UI-06**: Dark mode meets 4.5:1 contrast ratio (WCAG AA)
- [x] **UI-07**: Light mode meets 4.5:1 contrast ratio (WCAG AA)
- [x] **UI-08**: Monaco editor theme syncs with application theme
- [x] **UI-09**: Canvas renderer theme syncs with application theme

### Documentation

- [ ] **DOC-01**: User guide explains how to create .cypcb files
- [ ] **DOC-02**: User guide explains library management (import, organize, search)
- [ ] **DOC-03**: User guide explains desktop vs web feature differences
- [ ] **DOC-04**: User guide explains project structure and file organization
- [ ] **DOC-05**: User guide includes example projects with comments
- [ ] **DOC-06**: API documentation covers LSP server usage
- [ ] **DOC-07**: API documentation covers library file formats
- [ ] **DOC-08**: Contributing guide explains development setup
- [ ] **DOC-09**: Contributing guide explains architecture and codebase structure

## Future Requirements (v1.2+)

Deferred to future milestones.

### Advanced Library

- **LIB-ADV-01**: Supply chain integration (stock, pricing, lifecycle status from APIs)
- **LIB-ADV-02**: Component recommendations ("similar to X")
- **LIB-ADV-03**: Automatic 3D model fetching from databases
- **LIB-ADV-04**: Library conflict resolution UI (side-by-side comparison)

### Advanced Desktop

- **DESK-ADV-01**: Native OS notifications (DRC completion, export success)
- **DESK-ADV-02**: System tray integration (background running)
- **DESK-ADV-03**: Multi-window support (separate editor/viewer windows)

### Advanced Web

- **WEB-ADV-01**: PWA offline support (service workers, IndexedDB cache)
- **WEB-ADV-02**: Progressive enhancement (works offline after first load)
- **WEB-ADV-03**: Multi-tab sync via BroadcastChannel API

### Advanced Editor

- **EDIT-ADV-01**: Live DRC feedback (violations as you type)
- **EDIT-ADV-02**: AI assistant integration (LLM inline suggestions)
- **EDIT-ADV-03**: Error recovery (continue working with syntax errors)

### 3D Preview (v1.2)

- **3D-01**: User can view 3D preview of board assembly
- **3D-02**: System parses STEP files for component 3D models
- **3D-03**: 3D viewer supports zoom, pan, rotate navigation
- **3D-04**: 3D viewer shows component placement and orientation
- **3D-05**: 3D viewer shows board outline and thickness

### Undo/Redo System

- **UNDO-01**: User can undo editing operations
- **UNDO-02**: User can redo editing operations
- **UNDO-03**: Undo history persists across sessions
- **UNDO-04**: Command pattern supports complex object graphs

## Out of Scope

Explicitly excluded from v1.1. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Built-in footprint editor | Use KiCad editor, import result (KiCad is excellent at this) |
| Component marketplace | Integration with existing sources better than building marketplace |
| Automatic library updates | Breaking changes risk, manual update with changelog better |
| Cloud library sync | Privacy concerns, git-based sync gives user control |
| Real-time collaboration | Git-based async workflow is the model |
| Mobile app | Desktop/web first, mobile adds complexity without core value |
| Backend server | Static site + local storage avoids hosting costs and maintenance |
| VIM/Emacs bindings | Maintenance burden, use external editor + hot reload instead |
| Multiple editor themes | Dark + Light sufficient, reduces maintenance |
| Custom window decorations | Platform consistency and accessibility more important |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| PLAT-01 | Phase 9 | Complete |
| PLAT-02 | Phase 9 | Complete |
| PLAT-03 | Phase 9 | Complete |
| PLAT-04 | Phase 9 | Complete |
| PLAT-05 | Phase 9 | Complete |
| LIB-01 | Phase 10 | Pending |
| LIB-02 | Phase 10 | Pending |
| LIB-03 | Phase 10 | Pending |
| LIB-04 | Phase 10 | Pending |
| LIB-05 | Phase 10 | Pending |
| LIB-06 | Phase 10 | Pending |
| LIB-07 | Phase 10 | Pending |
| LIB-08 | Phase 10 | Pending |
| LIB-09 | Phase 10 | Pending |
| LIB-10 | Phase 10 | Pending |
| LIB-11 | Phase 10 | Pending |
| LIB-12 | Phase 10 | Pending |
| UI-01 | Phase 11 | Complete |
| UI-02 | Phase 11 | Complete |
| UI-03 | Phase 11 | Complete |
| UI-04 | Phase 11 | Complete |
| UI-05 | Phase 11 | Complete |
| UI-06 | Phase 11 | Complete |
| UI-07 | Phase 11 | Complete |
| UI-08 | Phase 11 | Complete |
| UI-09 | Phase 11 | Complete |
| DESK-01 | Phase 12 | Pending |
| DESK-02 | Phase 12 | Pending |
| DESK-03 | Phase 12 | Pending |
| DESK-04 | Phase 12 | Pending |
| DESK-05 | Phase 12 | Pending |
| DESK-06 | Phase 12 | Pending |
| DESK-07 | Phase 12 | Pending |
| DESK-08 | Phase 12 | Pending |
| DESK-09 | Phase 12 | Pending |
| DESK-10 | Phase 12 | Pending |
| WEB-01 | Phase 13 | Pending |
| WEB-02 | Phase 13 | Pending |
| WEB-03 | Phase 13 | Pending |
| WEB-04 | Phase 13 | Pending |
| WEB-05 | Phase 13 | Pending |
| WEB-06 | Phase 13 | Pending |
| WEB-07 | Phase 13 | Pending |
| WEB-08 | Phase 13 | Pending |
| WEB-09 | Phase 13 | Pending |
| EDIT-01 | Phase 14 | Pending |
| EDIT-02 | Phase 14 | Pending |
| EDIT-03 | Phase 14 | Pending |
| EDIT-04 | Phase 14 | Pending |
| EDIT-05 | Phase 14 | Pending |
| EDIT-06 | Phase 14 | Pending |
| EDIT-07 | Phase 14 | Pending |
| EDIT-08 | Phase 14 | Pending |
| EDIT-09 | Phase 14 | Pending |
| EDIT-10 | Phase 14 | Pending |
| DOC-01 | Phase 15 | Pending |
| DOC-02 | Phase 15 | Pending |
| DOC-03 | Phase 15 | Pending |
| DOC-04 | Phase 15 | Pending |
| DOC-05 | Phase 15 | Pending |
| DOC-06 | Phase 15 | Pending |
| DOC-07 | Phase 15 | Pending |
| DOC-08 | Phase 15 | Pending |
| DOC-09 | Phase 15 | Pending |

**Coverage:**
- v1.1 requirements: 64 total
- Mapped to phases: 64 (100%)
- Unmapped: 0

---
*Requirements defined: 2026-01-29*
*Last updated: 2026-01-29 (Phase 9 complete: PLAT-01 through PLAT-05)*
