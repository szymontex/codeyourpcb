# Project Research Summary

**Project:** CodeYourPCB v1.1 Foundation & Desktop
**Domain:** Code-first PCB Design Tool (EDA)
**Researched:** 2026-01-29
**Confidence:** HIGH

## Executive Summary

CodeYourPCB v1.1 adds professional desktop capabilities and library management to the existing v1.0 web viewer foundation. The recommended approach builds on proven technologies: Tauri 2.0 for native desktop packaging (50% less RAM than Electron, <10MB bundle), Monaco Editor for in-app code editing with existing LSP integration, and a dual-storage library system supporting KiCad and JLCPCB component sources. The architecture preserves the existing WASM core while adding environment-specific facades for file system and library storage.

The critical success factor is establishing a platform abstraction layer BEFORE implementing Tauri-specific features. Research shows 800% code duplication risk when developers scatter runtime platform checks throughout business logic. The solution is build-time conditional compilation with shared interfaces for FileSystem, Dialog, and Library storage. Desktop uses native file system + SQLite, web uses File System Access API + IndexedDB, but both expose identical APIs to application code.

Key risks center on integration complexity: library namespace conflicts between multiple sources, Monaco bundle size explosion (4MB+ if misconfigured), and desktop/web feature parity drift. Mitigation strategies are well-established: namespace-prefixed library imports, lazy-loaded Monaco with minimal workers, and progressive enhancement where desktop adds capabilities without breaking web. The foundation is solid—v1.0 already shipped with parser, DRC, LSP, and FreeRouting integration—so v1.1 focuses on delivery mechanisms and developer experience.

## Key Findings

### Recommended Stack

**Core v1.1 additions:** Tauri 2.9 for desktop shell, Monaco Editor 0.55 for embedded editing, and serde_kicad_sexpr for library parsing. The stack maintains the existing Rust/WASM core while adding platform-specific integrations. Research confirms Tauri 2.x is production-ready with framework-agnostic frontend support, and Monaco is the industry standard for LSP-integrated code editors.

**Core technologies (v1.1 additions):**
- **Tauri 2.9:** Desktop framework — 50% less RAM than Electron, supports Linux/macOS/Windows, native file dialogs and process spawning
- **Monaco Editor 0.55.1:** Code editor — VS Code core, TypeScript support, LSP integration precedent, works in Tauri webview
- **serde_kicad_sexpr:** KiCad parser — Serde-based S-expression parsing for .kicad_mod footprints with proper optional field handling
- **tokio-rusqlite 0.6:** Desktop storage — 100% safe Rust async SQLite for library cache with filesystem access
- **indexed_db_futures 0.5:** Web storage — Async IndexedDB wrapper for browser library cache with automatic transaction rollback
- **occt-import-js 0.0.23:** 3D STEP parser — WASM OpenCascade for client-side STEP file parsing to Three.js geometry

**Supporting infrastructure:**
- **Cloudflare Pages:** Web deployment — unlimited bandwidth, WASM-friendly, fast edge network
- **Vite 5.0+:** Build tool — already in stack, handles WASM code splitting and Monaco bundling
- **CSS Custom Properties:** Dark mode — native browser support, inherits across Shadow DOM, Tauri theme API for OS sync

### Expected Features

**Must have (table stakes):**
- **Multi-source library support** — KiCad + JLCPCB + custom libraries with unified search
- **Library organization** — By manufacturer, function, custom categories with auto-detection
- **3D model association** — STEP file linking with preview rendering
- **Footprint preview** — Visual confirmation before component placement
- **Native file dialogs** — OS-native open/save dialogs (desktop only)
- **Application menus** — Standard File/Edit/View platform menus
- **Dark mode theme** — System preference support across all UI surfaces
- **Monaco integration** — VS Code editor embedded with .cypcb syntax highlighting
- **LSP integration** — Autocomplete, hover, diagnostics from existing tower-lsp server
- **Static site hosting** — Fast WASM loading with responsive layout

**Should have (competitive advantage):**
- **Multi-source unified search** — Search across KiCad + JLCPCB + custom in single query
- **Git-friendly library format** — Text-based lib definitions for version control
- **Tiny bundle size** — <10MB desktop installer vs 300MB+ for KiCad/Eagle
- **Fast startup** — <1s launch time vs 3-5s for competitors
- **No-install web sharing** — Full viewer via URL with no installation
- **Side-by-side code/board** — Live preview as you edit code

**Defer (v1.2+):**
- **Supply chain integration** — Stock, pricing, lifecycle status from suppliers
- **Component recommendations** — "Similar to X" suggestions based on usage
- **Auto 3D model fetching** — Download from databases automatically
- **PWA offline support** — Service worker caching for web version
- **Live DRC feedback** — See violations as you type (performance intensive)
- **AI assistant integration** — Inline LLM help for code-first editing

### Architecture Approach

v1.1 extends the existing WASM core with environment-specific facades. The cypcb-render WASM module remains shared between desktop and web, with platform differences abstracted behind LibraryStorage, FileSystem, and Dialog interfaces. Desktop Tauri build gets FileSystemStorage with SQLite cache and native process spawning. Web build gets BrowserStorage with IndexedDB cache and File System Access API. Monaco Editor runs in both (Tauri uses webview), connecting to tower-lsp server via WebSocket (desktop spawns process, web connects to external).

**Major components:**
1. **cypcb-library crate** — Component/footprint management with pluggable storage backends (FileSystemStorage for desktop, BrowserStorage for web)
2. **Tauri desktop wrapper** — Native shell with IPC commands for file operations, library management, LSP server spawning, and file watching
3. **Monaco integration** — Embedded editor with custom language registration, Tree-sitter syntax highlighting, and LSP client via monaco-languageclient
4. **Platform abstraction layer** — FileSystem, Dialog, Menu interfaces with build-time conditional compilation (TAURI_ENV_PLATFORM) to prevent code duplication
5. **Library storage system** — Dual backends sharing serde_kicad_sexpr parser: ~/.codeyourpcb/libs/ + SQLite for desktop, IndexedDB for web

**Key integration points:**
- Monaco ↔ Tower-LSP: Language client connects via WebSocket, Tauri spawns cypcb-lsp process, web uses external server
- Library Management ↔ Storage: Shared parsing logic, platform-specific persistence (SQLite vs IndexedDB)
- Theme System ↔ All surfaces: Central ThemeManager coordinates CSS, Monaco, Canvas, Three.js background colors

### Critical Pitfalls

Research identified 12 critical pitfalls specific to v1.1 integration, beyond general EDA domain risks.

1. **Library Namespace Conflicts** — Multiple sources (KiCad, JLCPCB, custom) contain identically-named footprints with different implementations. Silent overwrites cause manufacturing failures. **Mitigation:** Namespace-prefixed imports (kicad::R_0805), conflict detection UI, library source metadata storage.

2. **Desktop/Web Feature Parity Drift** — Direct Tauri API usage in business logic breaks web deployment, causing 800% code duplication. **Mitigation:** Platform abstraction layer established BEFORE Tauri features, build-time conditional compilation, integration tests on both platforms.

3. **Monaco Bundle Size Explosion** — Default Monaco configuration includes 40+ language workers, jumping bundle from 500KB to 4.5MB. **Mitigation:** vite-plugin-monaco-editor with minimal workers, lazy loading, Tree-sitter for syntax highlighting.

4. **Dark Mode Inconsistency** — CSS dark mode works but Monaco/Canvas/Three.js remain light-themed, causing jarring "flashbang" effect. **Mitigation:** Central ThemeManager coordinating all subsystems, theme persistence to localStorage, prefers-color-scheme support.

5. **File System API Mismatches** — Desktop assumes persistent file access, web requires per-session permissions. Auto-save triggers download spam on web. **Mitigation:** Design for most restricted platform (web), enhance for desktop; IndexedDB auto-save on web, native file system on desktop.

## Implications for Roadmap

Based on research, v1.1 should be structured around four parallel capability streams with a foundational abstraction phase.

### Phase 0: Platform Abstraction Layer
**Rationale:** Must establish before ANY platform-specific features. Research shows 800% code duplication when skipped.
**Delivers:** FileSystem, Dialog, Menu, LibraryStorage interfaces with desktop and web implementations
**Avoids:** Desktop/web feature parity drift (Pitfall 2)
**Technology:** Build-time conditional compilation via Vite's TAURI_ENV_PLATFORM

### Phase 1: Library Management Foundation
**Rationale:** Library system is independent and foundational. Desktop and web both need component selection.
**Delivers:** cypcb-library crate with KiCad S-expression parsing, namespace-prefixed imports, dual storage backends
**Addresses:** Multi-source library support, library organization, footprint preview (table stakes)
**Avoids:** Library namespace conflicts (Pitfall 1), library version drift (Pitfall 12)
**Technology:** serde_kicad_sexpr for parsing, tokio-rusqlite (desktop), indexed_db_futures (web)

### Phase 2: Dark Mode & UI Polish
**Rationale:** Theme system must work before adding Monaco and complex UI. Sets visual foundation.
**Delivers:** Central ThemeManager, CSS custom properties, localStorage persistence, prefers-color-scheme support
**Addresses:** Dark mode theme (table stakes), system preference sync
**Avoids:** Dark mode inconsistency (Pitfall 4), theme toggle without persistence (Pitfall 6)
**Technology:** CSS custom properties, Tauri theme API, light-dark() CSS function

### Phase 3: Tauri Desktop Foundation
**Rationale:** Desktop shell provides native file system, process spawning, and packaging. Builds on abstraction layer.
**Delivers:** Tauri 2.0 project structure, native file dialogs, application menus, file watchers, LSP server spawning
**Addresses:** Native file dialogs, application menus, keyboard shortcuts (table stakes)
**Avoids:** File system API mismatches (Pitfall 5), desktop/web behavioral differences (Pitfall 7)
**Technology:** Tauri 2.9 with protocol-asset feature, platform abstraction from Phase 0

### Phase 4: Web Deployment
**Rationale:** Validates platform abstraction works. Web is most restricted platform, so building for web ensures desktop doesn't assume too much.
**Delivers:** Vite static build, File System Access API integration, IndexedDB persistence, deployment workflows
**Addresses:** Static site hosting, fast WASM loading, browser file access (table stakes)
**Avoids:** Auto-save download spam (Pitfall 8), backend server dependency
**Technology:** Vite build config, Cloudflare Pages deployment, File System Access API

### Phase 5: Monaco Editor Integration
**Rationale:** Depends on theme system (Phase 2) and LSP infrastructure (v1.0). Performance-critical, must optimize from start.
**Delivers:** Monaco editor component, custom language registration, LSP client, lazy loading with code splitting
**Addresses:** Monaco integration, syntax highlighting, LSP integration, side-by-side view (table stakes)
**Avoids:** Monaco bundle size explosion (Pitfall 3), worker misconfiguration
**Technology:** Monaco 0.55.1, vite-plugin-monaco-editor, monaco-languageclient, Tree-sitter for syntax

### Phase 6: Library Integration & 3D Preview
**Rationale:** Combines library management (Phase 1) with 3D rendering. Non-blocking for core functionality.
**Delivers:** 3D model association, STEP file parsing, Three.js integration, library browser with thumbnails
**Addresses:** 3D model association, footprint preview (table stakes)
**Avoids:** Eager thumbnail loading (Pitfall 4 from performance traps)
**Technology:** occt-import-js for STEP parsing, Three.js for rendering, Intersection Observer for lazy loading

### Phase 7: Documentation & Polish
**Rationale:** Final phase ensures features are discoverable and platform differences are clear.
**Delivers:** User documentation, platform comparison guide, library update workflow, integration examples
**Addresses:** User onboarding, cross-platform workflows
**Avoids:** UX confusion about desktop vs web capabilities

### Phase Ordering Rationale

- **Phase 0 first:** Platform abstraction MUST come before platform-specific features to prevent code duplication
- **Library Management early:** Independent system needed by both desktop and web, no platform dependencies
- **Dark Mode before Monaco:** Theme system must coordinate all surfaces; easier to integrate Monaco into existing theme than retrofit
- **Desktop before Web:** Desktop is superset of web capabilities; building desktop first reveals what needs abstraction for web
- **Monaco after theme + desktop:** Depends on both theme coordination and LSP server spawning
- **3D Preview last:** Non-blocking enhancement, combines multiple prior systems

### Research Flags

**Phases needing deeper research during planning:**
- **Phase 1 (Library Management):** KiCad S-expression format edge cases (optional fields, coordinate systems), library conflict resolution UX design
- **Phase 6 (3D Preview):** STEP file size limits in WASM, Three.js geometry caching strategies, occt-import-js memory management

**Phases with standard patterns (skip research-phase):**
- **Phase 0 (Platform Abstraction):** Well-documented pattern in Tauri ecosystem, Vite env variable usage is standard
- **Phase 2 (Dark Mode):** CSS custom properties and prefers-color-scheme are mature web standards
- **Phase 3 (Tauri Desktop):** Official Tauri 2.0 documentation is comprehensive and current (© 2026)
- **Phase 4 (Web Deployment):** Vite static deployment and Cloudflare Pages are well-documented
- **Phase 5 (Monaco Integration):** monaco-languageclient is proven solution, LSP integration patterns established

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Tauri 2.0 stable, Monaco well-documented, libraries proven in production |
| Features | HIGH | Table stakes validated against KiCad/Altium/EasyEDA, differentiators based on existing v1.0 architecture |
| Architecture | HIGH | Extension of proven v1.0 WASM core, platform abstraction is established pattern |
| Pitfalls | HIGH | v1.1 integration pitfalls sourced from Tauri/Monaco GitHub issues, library management from Altium best practices |

**Overall confidence:** HIGH

All technologies have production examples and official documentation current to 2026. v1.1 builds on proven v1.0 foundation rather than starting from scratch. Integration risks are well-documented with clear mitigation strategies.

### Gaps to Address

**Library conflict resolution UX:** Research identifies the problem (namespace conflicts) and high-level solution (namespace prefixing, conflict detection), but specific UI patterns need validation during Phase 1 planning. Recommendation: Study Altium 365 library management UI as reference.

**Monaco worker configuration for .cypcb language:** Research shows minimal workers needed (editorWorkerService only), but integration with existing Tree-sitter grammar needs validation. Recommendation: Prototype during Phase 5 planning with bundle size measurement.

**Cross-platform keyboard shortcuts:** Research doesn't address Cmd vs Ctrl differences (macOS vs Windows/Linux). Recommendation: Use Tauri's keyboard shortcut registration which handles platform differences automatically.

**3D model caching strategy:** occt-import-js parses STEP to JSON, but optimal caching location (IndexedDB vs memory vs file system) unclear. Recommendation: Benchmark during Phase 6 planning with representative STEP files.

**File watcher behavior in web context:** Desktop uses notify crate for file watching, but web has no equivalent. Research recommends manual reload only, but doesn't address multi-tab synchronization. Recommendation: Consider BroadcastChannel API for web tab sync during Phase 4 planning.

## Sources

### Primary (HIGH confidence)
- **Tauri 2.0 Official Documentation** (v2.tauri.app) — Desktop shell architecture, IPC patterns, file system plugin, theme API
- **Monaco Editor Repository** (github.com/microsoft/monaco-editor) — Editor integration, language registration, worker architecture
- **KiCad Developer Docs** (dev-docs.kicad.org) — S-expression format, footprint structure, 3D model requirements
- **STACK.md research** (2026-01-29) — Technology choices with version compatibility matrix
- **FEATURES-v1.1.md research** (2026-01-29) — Feature prioritization with competitor analysis
- **ARCHITECTURE.md research** (2026-01-29) — Integration patterns with data flow diagrams

### Secondary (MEDIUM confidence)
- **GitHub Issues** (Tauri #11347, Monaco #3518) — Known integration challenges and workarounds
- **Altium 365 Library Management** — Professional library organization patterns
- **vite-plugin-monaco-editor** — Bundle optimization strategies
- **Code duplication research** (codeant.ai) — 800% duplication stat from React study
- **PITFALLS.md research** (2026-01-29) — v1.1 integration pitfalls with sources

### Tertiary (LOW confidence)
- **Community forum discussions** (KiCad forums, Tauri Discord) — Anecdotal experiences, needs validation during implementation

---
*Research completed: 2026-01-29*
*Ready for roadmap: yes*
