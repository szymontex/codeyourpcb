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

# Architecture Patterns: v1.1 Foundation & Desktop Integration

**Domain:** Code-First PCB Design Tool (CodeYourPCB)
**Milestone:** v1.1 Foundation & Desktop
**Researched:** 2026-01-29
**Confidence:** HIGH (verified against Tauri 2.0, Monaco, KiCad library formats)

---

## Executive Summary

v1.1 adds four major subsystems to the existing v1.0 web viewer architecture:

1. **Library Management System** - Component/footprint library with KiCad compatibility
2. **Tauri Desktop Wrapper** - Native desktop shell with file system access
3. **Web Deployment** - Static site hosting for browser-only usage
4. **Monaco Editor** - In-app code editing with LSP integration

**Integration Strategy:** These features share the existing WASM core (cypcb-render) but diverge in execution environments:

- **Desktop mode** (Tauri): Full Rust backend, native file system, embedded Monaco editor
- **Web mode** (Static): WASM-only, browser File API, external editor via LSP

The architecture preserves the existing v1.0 hot reload development experience while enabling production desktop and web deployments.

---

## System Overview: v1.1 Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          DEPLOYMENT VARIANTS                              │
├─────────────────────────────────┬───────────────────────────────────────┤
│         DESKTOP (Tauri)         │           WEB (Static)                │
│                                 │                                       │
│  ┌──────────────────────────┐   │   ┌──────────────────────────────┐   │
│  │   Tauri Native Shell     │   │   │      Browser Window          │   │
│  │  ┌────────────────────┐  │   │   │  ┌────────────────────────┐  │   │
│  │  │   WebView (HTML)   │  │   │   │  │     Static HTML        │  │   │
│  │  │                    │  │   │   │  │                        │  │   │
│  │  │  ┌──────────────┐  │  │   │   │  │  ┌──────────────┐      │  │   │
│  │  │  │   Monaco     │  │  │   │   │  │  │  File Picker │      │  │   │
│  │  │  │   Editor     │  │  │   │   │  │  │  (browser)   │      │  │   │
│  │  │  └──────────────┘  │  │   │   │  │  └──────────────┘      │  │   │
│  │  │                    │  │   │   │  │                        │  │   │
│  │  │  ┌──────────────┐  │  │   │   │  │  ┌──────────────┐      │  │   │
│  │  │  │   Canvas     │  │  │   │   │  │  │   Canvas     │      │  │   │
│  │  │  │  Rendering   │  │  │   │   │  │  │  Rendering   │      │  │   │
│  │  │  └──────────────┘  │  │   │   │  │  └──────────────┘      │  │   │
│  │  │         ▲          │  │   │   │  │         ▲              │  │   │
│  │  └─────────┼──────────┘  │   │   │  └─────────┼──────────────┘  │   │
│  │            │             │   │   │            │                 │   │
│  │  ┌─────────▼──────────┐  │   │   │  ┌─────────▼──────────┐      │   │
│  │  │  WASM Core Engine  │  │   │   │  │  WASM Core Engine  │      │   │
│  │  │   (cypcb-render)   │  │   │   │  │   (cypcb-render)   │      │   │
│  │  └────────────────────┘  │   │   │  └────────────────────┘      │   │
│  │            ▲             │   │   │            ▲                 │   │
│  └────────────┼─────────────┘   │   └────────────┼─────────────────┘   │
│               │                 │                │ (limited)           │
│  ┌────────────▼─────────────┐   │   ┌────────────▼─────────────┐       │
│  │    Tauri IPC Commands    │   │   │   Browser File API       │       │
│  │  ┌────────────────────┐  │   │   │  (no backend access)     │       │
│  │  │ File System Access │  │   │   └──────────────────────────┘       │
│  │  │ Library Manager    │  │   │                                      │
│  │  │ Project Watcher    │  │   │   External LSP Server (optional):   │
│  │  │ Native Dialogs     │  │   │   ┌──────────────────────────────┐   │
│  │  └────────────────────┘  │   │   │   tower-lsp (cypcb-lsp)     │   │
│  │            ▲             │   │   │   (runs as separate process) │   │
│  │            │             │   │   └──────────────────────────────┘   │
│  │  ┌─────────▼──────────┐  │   │                                      │
│  │  │ Library Storage    │  │   │                                      │
│  │  │ ~/.codeyourpcb/    │  │   │                                      │
│  │  │   libs/            │  │   │                                      │
│  │  │   cache/           │  │   │                                      │
│  │  └────────────────────┘  │   │                                      │
│  └──────────────────────────┘   │                                      │
└─────────────────────────────────┴───────────────────────────────────────┘

                   SHARED DEVELOPMENT ENVIRONMENT
┌─────────────────────────────────────────────────────────────────────────┐
│                    Dev Server (viewer/server.ts)                        │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  WebSocket Server (port 4322)                                    │   │
│  │  - File watcher (chokidar)                                       │   │
│  │  - Hot reload broadcasts                                         │   │
│  │  - Route command proxy                                           │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│                              ▲                                          │
│                              │ WebSocket                                │
│                    ┌─────────▼──────────┐                               │
│                    │   Browser/Tauri    │                               │
│                    │  (development mode) │                               │
│                    └────────────────────┘                               │
└─────────────────────────────────────────────────────────────────────────┘
```

**Key Architectural Decisions:**

1. **WASM Core is Shared** - Both desktop and web modes use the same cypcb-render WASM module
2. **Environment-Specific Facades** - File access, library storage differ by deployment
3. **Monaco is Desktop-Only (v1.1)** - Web mode uses external editor + LSP initially
4. **Development Mode is Unified** - Same dev server works for both targets

---

## Component Integration Points

### 1. Library Management System

**Problem:** Users need to manage component libraries (symbols + footprints) with KiCad compatibility.

**Architecture Decision:** Create dedicated `cypcb-library` crate with dual storage backends.

#### Component Structure

```rust
// crates/cypcb-library/src/lib.rs

/// Library management with pluggable storage
pub struct LibraryManager {
    storage: Box<dyn LibraryStorage>,
    cache: ComponentCache,
}

/// Storage backend abstraction
pub trait LibraryStorage: Send + Sync {
    fn list_libraries(&self) -> Result<Vec<LibraryMetadata>>;
    fn get_component(&self, lib: &str, name: &str) -> Result<Component>;
    fn get_footprint(&self, lib: &str, name: &str) -> Result<Footprint>;
    fn add_library(&mut self, path: &Path) -> Result<LibraryId>;
}

/// Desktop implementation
pub struct FileSystemStorage {
    user_libs: PathBuf,  // ~/.codeyourpcb/libs/
    system_libs: Vec<PathBuf>,  // System-wide KiCad libs
}

/// Web implementation (future)
pub struct BrowserStorage {
    indexed_db: web_sys::IdbDatabase,
}

/// Component definition
pub struct Component {
    pub name: String,
    pub description: String,
    pub footprint_ref: FootprintRef,
    pub pins: Vec<Pin>,
    pub properties: HashMap<String, String>,
}

/// Footprint definition (KiCad-compatible)
pub struct Footprint {
    pub name: String,
    pub pads: Vec<Pad>,
    pub silkscreen: Vec<GraphicsElement>,
    pub courtyard: Polygon,
    pub model_3d: Option<PathBuf>,
}
```

#### Integration with Existing Crates

```
┌─────────────────┐     uses      ┌──────────────────┐
│  cypcb-parser   │──────────────▶│  cypcb-library   │
│  (DSL parsing)  │               │ (lib management)  │
└─────────────────┘               └──────────────────┘
        │                                   │
        │ creates                           │ provides
        │ entities                          │ definitions
        ▼                                   ▼
┌─────────────────┐               ┌──────────────────┐
│   cypcb-world   │──────────────▶│  cypcb-core      │
│  (ECS board)    │     uses      │  (shared types)  │
└─────────────────┘               └──────────────────┘
```

**Data Flow:**

1. User writes: `component R1 resistor("0805", "10k")`
2. Parser resolves `resistor` → calls `LibraryManager::get_component("built-in", "resistor")`
3. LibraryManager returns Component with footprint_ref → `"0805"`
4. Parser resolves footprint → calls `LibraryManager::get_footprint("built-in", "0805")`
5. ECS world spawns entity with Component + Footprint components

#### File Format: KiCad S-Expression

Use existing KiCad .kicad_mod format for footprints:

```scheme
(footprint "Resistor_SMD:R_0805_2012Metric" (version 20221018) (generator pcbnew)
  (layer "F.Cu")
  (attr smd)
  (fp_text reference "REF**" (at 0 -1.65) (layer "F.SilkS")
    (effects (font (size 1 1) (thickness 0.15)))
  )
  (fp_line (start -0.227064 -0.735) (end 0.227064 -0.735) (layer "F.SilkS"))
  (fp_line (start -0.227064 0.735) (end 0.227064 0.735) (layer "F.SilkS"))
  (pad "1" smd roundrect (at -0.9125 0) (size 1.025 1.4) (layers "F.Cu" "F.Paste" "F.Mask"))
  (pad "2" smd roundrect (at 0.9125 0) (size 1.025 1.4) (layers "F.Cu" "F.Paste" "F.Mask"))
)
```

**Parsing Strategy:** Use existing `cypcb-kicad` crate, extend with library directory scanning.

#### Storage Locations

**Desktop (Tauri):**
```
~/.codeyourpcb/
├── libs/                    # User libraries
│   ├── my-custom.pretty/   # Footprint library (KiCad format)
│   │   ├── SOIC-8.kicad_mod
│   │   └── QFN-32.kicad_mod
│   └── built-in.pretty/    # Bundled footprints
├── cache/                  # Parsed library cache
│   └── index.json
└── config.toml             # Library paths
```

**Web (Browser):**
```
IndexedDB: codeyourpcb
├── libraries/              # Object store
│   ├── {id: "built-in", ...}
│   └── {id: "user-1", ...}
└── footprints/             # Object store
    ├── {lib: "built-in", name: "0805", ...}
    └── ...
```

#### New Crate Structure

```
crates/cypcb-library/
├── Cargo.toml
├── src/
│   ├── lib.rs              # LibraryManager API
│   ├── storage/
│   │   ├── mod.rs
│   │   ├── filesystem.rs   # Desktop storage
│   │   └── browser.rs      # Web storage (feature-gated)
│   ├── component.rs        # Component types
│   ├── footprint.rs        # Footprint types
│   ├── parser.rs           # KiCad s-expr parsing (delegates to cypcb-kicad)
│   └── cache.rs            # In-memory cache
└── tests/
    └── kicad_compat.rs     # KiCad library import tests
```

**Dependencies:**
```toml
[dependencies]
cypcb-core = { workspace = true }
cypcb-kicad = { path = "../cypcb-kicad" }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }

# Desktop-only
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
notify = { workspace = true }  # Watch library directories

# Web-only
[target.'cfg(target_arch = "wasm32")'.dependencies]
web-sys = { version = "0.3", features = ["IdbDatabase"] }
wasm-bindgen-futures = "0.4"
```

---

### 2. Tauri Desktop Wrapper

**Problem:** Provide native desktop shell with file system access, native dialogs, and process management.

**Architecture Decision:** Tauri 2.0 as thin native shell around existing Vite + WASM frontend.

#### Tauri Integration Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Tauri Application                        │
│                                                             │
│  ┌────────────────────────────────────────────────────┐    │
│  │                 WebView (Frontend)                  │    │
│  │  ┌──────────────────────────────────────────────┐  │    │
│  │  │  Vite Dev Server (dev) / Static HTML (prod) │  │    │
│  │  │                                              │  │    │
│  │  │  ┌────────────────┐  ┌──────────────────┐   │  │    │
│  │  │  │ Monaco Editor  │  │ Canvas Renderer  │   │  │    │
│  │  │  └────────────────┘  └──────────────────┘   │  │    │
│  │  │                                              │  │    │
│  │  │         TypeScript/JavaScript                │  │    │
│  │  └──────────────────────────────────────────────┘  │    │
│  │                         │                           │    │
│  │                         │ invoke()                  │    │
│  │                         ▼                           │    │
│  │  ┌──────────────────────────────────────────────┐  │    │
│  │  │          IPC Bridge (JSON-RPC)               │  │    │
│  │  └──────────────────────────────────────────────┘  │    │
│  └────────────────────────────────────────────────────┘    │
│                         │                                   │
│                         │ Tauri Commands                    │
│                         ▼                                   │
│  ┌────────────────────────────────────────────────────┐    │
│  │              Rust Backend (src-tauri/)             │    │
│  │                                                     │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────┐ │    │
│  │  │ File Ops     │  │ Library Mgr  │  │ Watchers │ │    │
│  │  │ - open()     │  │ - list()     │  │ - .cypcb │ │    │
│  │  │ - save()     │  │ - import()   │  │ files    │ │    │
│  │  │ - dialog()   │  │ - search()   │  └──────────┘ │    │
│  │  └──────────────┘  └──────────────┘               │    │
│  │                                                     │    │
│  │  ┌──────────────────────────────────────────────┐  │    │
│  │  │         State Management (Mutex)             │  │    │
│  │  │  - Current project path                      │  │    │
│  │  │  - Library manager instance                  │  │    │
│  │  │  - File watchers                             │  │    │
│  │  └──────────────────────────────────────────────┘  │    │
│  │                         │                           │    │
│  └─────────────────────────┼───────────────────────────┘    │
│                            │                                │
│                            ▼                                │
│  ┌────────────────────────────────────────────────────┐    │
│  │          Native OS Services                        │    │
│  │  - File system (read/write)                        │    │
│  │  - Native dialogs (open/save)                      │    │
│  │  - Process spawning (autorouter)                   │    │
│  │  - Window management                               │    │
│  └────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

#### Tauri Commands

```rust
// src-tauri/src/commands.rs

use tauri::State;
use std::sync::Mutex;
use cypcb_library::LibraryManager;

/// Application state shared across commands
pub struct AppState {
    pub current_project: Mutex<Option<PathBuf>>,
    pub library_manager: Mutex<LibraryManager>,
}

/// Open file dialog and load .cypcb file
#[tauri::command]
async fn open_file(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ProjectData, String> {
    // Native file dialog
    let file_path = tauri::api::dialog::blocking::FileDialogBuilder::new()
        .add_filter("CodeYourPCB", &["cypcb"])
        .pick_file()
        .ok_or("No file selected")?;

    // Read file content
    let content = tokio::fs::read_to_string(&file_path)
        .await
        .map_err(|e| e.to_string())?;

    // Update state
    *state.current_project.lock().unwrap() = Some(file_path.clone());

    // Set up file watcher
    start_watcher(app.clone(), file_path.clone())?;

    Ok(ProjectData {
        path: file_path.to_string_lossy().to_string(),
        content,
    })
}

/// Save current project
#[tauri::command]
async fn save_file(
    content: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let path = state.current_project.lock().unwrap()
        .clone()
        .ok_or("No project open")?;

    tokio::fs::write(&path, content)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// List available component libraries
#[tauri::command]
fn list_libraries(state: State<'_, AppState>) -> Result<Vec<LibraryInfo>, String> {
    let manager = state.library_manager.lock().unwrap();
    manager.list_libraries()
        .map_err(|e| e.to_string())
}

/// Search for component in libraries
#[tauri::command]
fn search_component(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<ComponentInfo>, String> {
    let manager = state.library_manager.lock().unwrap();
    manager.search(query)
        .map_err(|e| e.to_string())
}

/// Import KiCad library
#[tauri::command]
async fn import_library(
    path: String,
    state: State<'_, AppState>,
) -> Result<LibraryId, String> {
    let mut manager = state.library_manager.lock().unwrap();
    manager.add_library(Path::new(&path))
        .map_err(|e| e.to_string())
}
```

#### File Watcher Integration

```rust
// src-tauri/src/watcher.rs

use notify::{Watcher, RecursiveMode, Event};
use tauri::{AppHandle, Manager};

/// Start watching .cypcb file for external changes
pub fn start_watcher(app: AppHandle, path: PathBuf) -> Result<(), String> {
    let app_clone = app.clone();

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
        if let Ok(event) = res {
            if event.kind.is_modify() {
                // Emit event to frontend
                app_clone.emit_all("file-changed", FileChangeEvent {
                    path: path.clone(),
                }).unwrap();
            }
        }
    }).map_err(|e| e.to_string())?;

    watcher.watch(&path, RecursiveMode::NonRecursive)
        .map_err(|e| e.to_string())?;

    // Store watcher in app state to prevent drop
    app.state::<Mutex<Option<notify::RecommendedWatcher>>>()
        .lock()
        .unwrap()
        .replace(watcher);

    Ok(())
}
```

#### Frontend Integration (TypeScript)

```typescript
// src/tauri.ts

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface ProjectData {
  path: string;
  content: string;
}

export interface LibraryInfo {
  id: string;
  name: string;
  path: string;
  component_count: number;
}

export async function openFile(): Promise<ProjectData> {
  return await invoke<ProjectData>('open_file');
}

export async function saveFile(content: string): Promise<void> {
  return await invoke('save_file', { content });
}

export async function listLibraries(): Promise<LibraryInfo[]> {
  return await invoke<LibraryInfo[]>('list_libraries');
}

export async function searchComponent(query: string): Promise<ComponentInfo[]> {
  return await invoke<ComponentInfo[]>('search_component', { query });
}

export async function importLibrary(path: string): Promise<string> {
  return await invoke<string>('import_library', { path });
}

// Listen for file changes
export function onFileChanged(callback: (path: string) => void) {
  return listen<{ path: string }>('file-changed', (event) => {
    callback(event.payload.path);
  });
}
```

#### Project Structure

```
src-tauri/
├── Cargo.toml
├── tauri.conf.json         # Tauri configuration
├── icons/                  # App icons
├── src/
│   ├── main.rs            # App initialization
│   ├── commands.rs        # Tauri command handlers
│   ├── watcher.rs         # File watching
│   └── state.rs           # Application state
└── capabilities/          # ACL permissions
    └── default.json
```

**Tauri Configuration:**

```json
// src-tauri/tauri.conf.json
{
  "$schema": "https://schema.tauri.app/config/2.0.0",
  "productName": "CodeYourPCB",
  "version": "1.1.0",
  "identifier": "com.codeyourpcb.app",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:5173",
    "frontendDist": "../viewer/dist"
  },
  "app": {
    "security": {
      "csp": "default-src 'self'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval';"
    }
  }
}
```

**Integration with Existing Dev Server:**

The existing `viewer/server.ts` WebSocket server continues to work in dev mode. Tauri's webview connects to `localhost:5173` (Vite) which connects to `localhost:4322` (WebSocket server).

In production, Tauri serves static files from `viewer/dist` directly, no WebSocket server needed.

---

### 3. Web Deployment

**Problem:** Enable browser-only usage without desktop app installation.

**Architecture Decision:** Static site deployment via Vite build, no backend required.

#### Deployment Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     CDN / Static Host                       │
│                  (Cloudflare Pages, Vercel)                 │
│                                                             │
│  ┌────────────────────────────────────────────────────┐    │
│  │                  index.html                        │    │
│  │  <script type="module" src="/assets/main-xyz.js">  │    │
│  └────────────────────────────────────────────────────┘    │
│                         │                                   │
│                         ▼                                   │
│  ┌────────────────────────────────────────────────────┐    │
│  │         Static Assets (Vite build output)          │    │
│  │  /assets/                                          │    │
│  │    main-xyz.js          (app bundle)               │    │
│  │    cypcb-render-abc.wasm (WASM core)               │    │
│  │    monaco-editor/        (Monaco assets)           │    │
│  │    styles-xyz.css                                  │    │
│  └────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                         │
                         │ HTTPS
                         ▼
              ┌──────────────────────┐
              │    User Browser      │
              │                      │
              │  ┌────────────────┐  │
              │  │ WASM Runtime   │  │
              │  └────────────────┘  │
              │  ┌────────────────┐  │
              │  │ File API       │  │
              │  │ (local files)  │  │
              │  └────────────────┘  │
              │  ┌────────────────┐  │
              │  │ IndexedDB      │  │
              │  │ (persistence)  │  │
              │  └────────────────┘  │
              └──────────────────────┘
```

#### Web-Specific Limitations

| Feature | Desktop (Tauri) | Web (Static) |
|---------|-----------------|--------------|
| **File Access** | Full native file system via Tauri commands | Browser File API only (user must pick files) |
| **Library Storage** | `~/.codeyourpcb/libs/` directory | IndexedDB (browser storage) |
| **File Watching** | Native file watcher (notify crate) | Not available (manual reload only) |
| **Monaco Editor** | Embedded in app | Future feature (v1.2+) |
| **LSP Server** | Can spawn cypcb-lsp process | External tower-lsp server via WebSocket |
| **Auto-routing** | Spawn FreeRouting.jar locally | Not available (requires backend) |

#### Build Configuration

```typescript
// vite.config.ts

import { defineConfig } from 'vite';

export default defineConfig({
  base: './',  // Relative paths for static deployment
  build: {
    target: 'esnext',
    outDir: 'dist',
    rollupOptions: {
      output: {
        manualChunks: {
          'monaco': ['monaco-editor'],  // Separate Monaco bundle
        },
      },
    },
  },
  optimizeDeps: {
    exclude: ['cypcb-render'],  // Don't pre-bundle WASM
  },
  worker: {
    format: 'es',
  },
});
```

#### Web-Specific File Picker

```typescript
// src/file-picker.ts

export async function openFileWeb(): Promise<{ name: string; content: string }> {
  // Browser File API
  const [fileHandle] = await window.showOpenFilePicker({
    types: [{
      description: 'CodeYourPCB Files',
      accept: { 'text/plain': ['.cypcb'] },
    }],
  });

  const file = await fileHandle.getFile();
  const content = await file.text();

  return {
    name: file.name,
    content,
  };
}

export async function saveFileWeb(content: string, suggestedName: string): Promise<void> {
  const handle = await window.showSaveFilePicker({
    suggestedName,
    types: [{
      description: 'CodeYourPCB Files',
      accept: { 'text/plain': ['.cypcb'] },
    }],
  });

  const writable = await handle.createWritable();
  await writable.write(content);
  await writable.close();
}
```

#### Deployment Options

**Recommended: Cloudflare Pages**

```yaml
# .github/workflows/deploy.yml
name: Deploy to Cloudflare Pages

on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
      - name: Install dependencies
        run: npm install
        working-directory: viewer
      - name: Build WASM
        run: ./build-wasm.sh
        working-directory: viewer
      - name: Build frontend
        run: npm run build
        working-directory: viewer
      - name: Deploy to Cloudflare Pages
        uses: cloudflare/wrangler-action@v3
        with:
          apiToken: ${{ secrets.CLOUDFLARE_API_TOKEN }}
          accountId: ${{ secrets.CLOUDFLARE_ACCOUNT_ID }}
          command: pages deploy viewer/dist --project-name=codeyourpcb
```

**Alternative: Vercel**

```json
// vercel.json
{
  "buildCommand": "npm run build",
  "outputDirectory": "viewer/dist",
  "framework": "vite"
}
```

**Alternative: GitHub Pages**

```yaml
# .github/workflows/gh-pages.yml
- name: Deploy to GitHub Pages
  uses: peaceiris/actions-gh-pages@v4
  with:
    github_token: ${{ secrets.GITHUB_TOKEN }}
    publish_dir: ./viewer/dist
```

#### Environment Detection

```typescript
// src/environment.ts

export const IS_TAURI = '__TAURI__' in window;
export const IS_WEB = !IS_TAURI;
export const IS_DEV = import.meta.env.DEV;

export async function openFile(): Promise<ProjectData> {
  if (IS_TAURI) {
    // Use Tauri command
    return await invoke<ProjectData>('open_file');
  } else {
    // Use browser File API
    return await openFileWeb();
  }
}
```

---

### 4. Monaco Editor Integration

**Problem:** Provide in-app code editing with syntax highlighting and LSP features.

**Architecture Decision:** Embed Monaco in desktop app, connect to existing tower-lsp server.

#### Monaco Integration Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                  Monaco Editor (Frontend)                   │
│                                                             │
│  ┌────────────────────────────────────────────────────┐    │
│  │         Monaco Editor Instance                     │    │
│  │  ┌──────────────────────────────────────────────┐  │    │
│  │  │  Text Model (.cypcb file content)            │  │    │
│  │  └──────────────────────────────────────────────┘  │    │
│  │  ┌──────────────────────────────────────────────┐  │    │
│  │  │  Language Configuration (cypcb)              │  │    │
│  │  │  - Syntax highlighting (TextMate grammar)    │  │    │
│  │  │  - Bracket matching                          │  │    │
│  │  │  - Comment patterns                          │  │    │
│  │  └──────────────────────────────────────────────┘  │    │
│  └────────────────────────────────────────────────────┘    │
│                         │                                   │
│                         │ LSP Protocol                      │
│                         ▼                                   │
│  ┌────────────────────────────────────────────────────┐    │
│  │      monaco-languageclient (LSP adapter)           │    │
│  └────────────────────────────────────────────────────┘    │
│                         │                                   │
│                         │ JSON-RPC over WebSocket/Worker    │
│                         ▼                                   │
└─────────────────────────┼───────────────────────────────────┘
                          │
         ┌────────────────┼────────────────┐
         │                │                │
         │ (Desktop)      │           (Web - future)
         ▼                ▼                ▼
┌──────────────────┐  ┌──────────────┐  ┌─────────────────┐
│  Tauri Command   │  │ Web Worker   │  │ External Server │
│  (spawn LSP)     │  │ (WASM LSP)   │  │  (WebSocket)    │
│                  │  │              │  │                 │
│  ┌────────────┐  │  │ ┌──────────┐ │  │ ┌─────────────┐ │
│  │ cypcb-lsp  │  │  │ │cypcb-lsp │ │  │ │  cypcb-lsp  │ │
│  │ (process)  │  │  │ │ (WASM)   │ │  │ │  (Node.js)  │ │
│  └────────────┘  │  │ └──────────┘ │  │ └─────────────┘ │
└──────────────────┘  └──────────────┘  └─────────────────┘
         │                │                    │
         └────────────────┴────────────────────┘
                          │
                          ▼
                 ┌────────────────────┐
                 │   LSP Server       │
                 │   (tower-lsp)      │
                 │                    │
                 │  - Completions     │
                 │  - Diagnostics     │
                 │  - Hover info      │
                 │  - Go to def       │
                 └────────────────────┘
```

#### Monaco Setup

```typescript
// src/editor/monaco-setup.ts

import * as monaco from 'monaco-editor';
import { buildWorkerDefinition } from 'monaco-editor-workers';

// Load Monaco workers
buildWorkerDefinition(
  '../node_modules/monaco-editor-workers/dist/workers',
  import.meta.url,
  false
);

// Register CodeYourPCB language
monaco.languages.register({
  id: 'cypcb',
  extensions: ['.cypcb'],
  aliases: ['CodeYourPCB', 'cypcb'],
});

// Basic syntax highlighting (TextMate grammar)
monaco.languages.setMonarchTokensProvider('cypcb', {
  keywords: [
    'board', 'component', 'net', 'trace', 'via', 'zone',
    'footprint', 'layer', 'stackup', 'rules',
  ],

  tokenizer: {
    root: [
      [/\b(board|component|net|trace|via)\b/, 'keyword'],
      [/@[a-zA-Z_]\w*/, 'annotation'],
      [/".*?"/, 'string'],
      [/\d+(\.\d+)?(mm|mil|in)/, 'number.unit'],
      [/\/\/.*$/, 'comment'],
    ],
  },
});

// Language configuration
monaco.languages.setLanguageConfiguration('cypcb', {
  comments: {
    lineComment: '//',
    blockComment: ['/*', '*/'],
  },
  brackets: [
    ['{', '}'],
    ['[', ']'],
    ['(', ')'],
  ],
  autoClosingPairs: [
    { open: '{', close: '}' },
    { open: '[', close: ']' },
    { open: '(', close: ')' },
    { open: '"', close: '"' },
  ],
});
```

#### LSP Client Integration

```typescript
// src/editor/lsp-client.ts

import {
  MonacoLanguageClient,
  CloseAction,
  ErrorAction,
  MessageTransports
} from 'monaco-languageclient';
import { toSocket, WebSocketMessageReader, WebSocketMessageWriter } from 'vscode-ws-jsonrpc';

export async function createLspClient(): Promise<MonacoLanguageClient> {
  // In Tauri: Connect to locally spawned LSP server
  const wsUrl = IS_TAURI
    ? 'ws://localhost:9257'  // Port from Tauri-spawned cypcb-lsp
    : 'ws://localhost:9257'; // External LSP server

  const webSocket = new WebSocket(wsUrl);

  await new Promise((resolve, reject) => {
    webSocket.onopen = resolve;
    webSocket.onerror = reject;
  });

  const socket = toSocket(webSocket);
  const reader = new WebSocketMessageReader(socket);
  const writer = new WebSocketMessageWriter(socket);

  const client = new MonacoLanguageClient({
    name: 'CodeYourPCB Language Client',
    clientOptions: {
      documentSelector: [{ language: 'cypcb' }],
      errorHandler: {
        error: () => ({ action: ErrorAction.Continue }),
        closed: () => ({ action: CloseAction.Restart }),
      },
    },
    connectionProvider: {
      get: () => Promise.resolve({ reader, writer }),
    },
  });

  await client.start();
  return client;
}
```

#### Tauri LSP Server Management

```rust
// src-tauri/src/lsp.rs

use std::process::{Command, Child};
use tauri::State;

pub struct LspServerHandle {
    process: Mutex<Option<Child>>,
}

/// Spawn cypcb-lsp server on localhost:9257
#[tauri::command]
pub fn start_lsp_server(state: State<'_, LspServerHandle>) -> Result<(), String> {
    let mut process = state.process.lock().unwrap();

    if process.is_some() {
        return Ok(()); // Already running
    }

    // Spawn LSP server binary (bundled with app)
    let child = Command::new("cypcb-lsp")
        .args(["--port", "9257"])
        .spawn()
        .map_err(|e| e.to_string())?;

    *process = Some(child);
    Ok(())
}

/// Stop LSP server on app exit
#[tauri::command]
pub fn stop_lsp_server(state: State<'_, LspServerHandle>) -> Result<(), String> {
    let mut process = state.process.lock().unwrap();

    if let Some(mut child) = process.take() {
        child.kill().map_err(|e| e.to_string())?;
    }

    Ok(())
}
```

#### Monaco Editor Component

```typescript
// src/components/Editor.tsx

import { useEffect, useRef } from 'react';
import * as monaco from 'monaco-editor';
import { createLspClient } from '../editor/lsp-client';

export function Editor({ value, onChange }: EditorProps) {
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    // Create editor instance
    const editor = monaco.editor.create(containerRef.current, {
      value,
      language: 'cypcb',
      theme: 'vs-dark',
      minimap: { enabled: false },
      fontSize: 14,
      lineNumbers: 'on',
      scrollBeyondLastLine: false,
    });

    editorRef.current = editor;

    // Connect to LSP server
    if (IS_TAURI) {
      // Start LSP server via Tauri command
      invoke('start_lsp_server').then(() => {
        createLspClient().catch(console.error);
      });
    }

    // Listen for content changes
    editor.onDidChangeModelContent(() => {
      onChange(editor.getValue());
    });

    return () => {
      editor.dispose();
      if (IS_TAURI) {
        invoke('stop_lsp_server');
      }
    };
  }, []);

  // Update editor when value changes externally
  useEffect(() => {
    if (editorRef.current && editorRef.current.getValue() !== value) {
      editorRef.current.setValue(value);
    }
  }, [value]);

  return <div ref={containerRef} style={{ height: '100%', width: '100%' }} />;
}
```

#### Web Worker LSP (Future)

For web deployment without external server, compile tower-lsp to WASM and run in Web Worker:

```typescript
// src/editor/lsp-worker.ts (future)

import { expose } from 'comlink';
import init, { LspServer } from 'cypcb-lsp-wasm';

let server: LspServer;

const api = {
  async initialize() {
    await init();
    server = new LspServer();
  },

  async handleRequest(method: string, params: any): Promise<any> {
    return server.handle_request(method, JSON.stringify(params));
  },
};

expose(api);
```

**Challenge:** tower-lsp uses Tokio, which doesn't compile to WASM. Workaround: Use wasm-bindgen-futures + custom async runtime or switch to pure async-std for WASM build.

---

## Data Flow: Complete Integration

### Scenario 1: Desktop - Open Project with Monaco

```
1. User clicks "Open" button
   │
   ▼
2. Tauri command: open_file()
   ├─> Native file dialog
   ├─> Read .cypcb file
   ├─> Start file watcher
   └─> Return { path, content }
   │
   ▼
3. Frontend receives ProjectData
   ├─> Load content into Monaco editor
   ├─> Pass content to WASM engine
   │   ├─> cypcb-render::load_source(content)
   │   ├─> Parse with cypcb-parser
   │   ├─> Resolve components via LibraryManager
   │   ├─> Build ECS world
   │   └─> Run DRC
   └─> Render canvas
   │
   ▼
4. Monaco connects to LSP server
   ├─> Tauri spawns cypcb-lsp process
   ├─> WebSocket connection on localhost:9257
   ├─> LSP client sends initialize request
   └─> Diagnostics/completions enabled
   │
   ▼
5. User edits in Monaco
   ├─> onChange event
   ├─> Pass updated content to WASM engine
   ├─> Incremental re-parse
   ├─> Update ECS world
   ├─> Re-run DRC
   └─> Re-render canvas
   │
   ▼
6. User adds component: component R1 resistor("0805")
   ├─> LSP provides completion for "resistor"
   ├─> Parser resolves via LibraryManager
   │   └─> Tauri reads ~/.codeyourpcb/libs/built-in.pretty/
   └─> Component appears on canvas
```

### Scenario 2: Web - Load File from Browser

```
1. User clicks "Open" button
   │
   ▼
2. Browser File Picker API
   ├─> window.showOpenFilePicker()
   └─> Return File object
   │
   ▼
3. Read file content
   ├─> file.text()
   └─> Load into WASM engine
       ├─> cypcb-render::load_source(content)
       └─> Render canvas
   │
   ▼
4. NO Monaco editor (v1.1)
   ├─> User must edit in external editor
   └─> Manual reload button to refresh
   │
   ▼
5. External LSP server (optional)
   ├─> User runs: cypcb-lsp --port 9257
   ├─> VS Code connects via extension
   └─> Browser connects via WebSocket
```

### Scenario 3: Development - Hot Reload

```
1. User edits .cypcb file in external editor
   │
   ▼
2. File system event
   ├─> Chokidar detects change (viewer/server.ts)
   └─> Read updated file content
   │
   ▼
3. WebSocket broadcast
   ├─> Send { type: 'reload', content, file }
   └─> Both desktop and web clients receive
   │
   ▼
4. Frontend processes reload
   ├─> Monaco updates content (if open in desktop)
   ├─> Pass to WASM engine
   ├─> Preserve viewport/selection
   └─> Re-render
```

---

## Build Order & Dependencies

### Phase Structure Recommendation

```
Level 0: Library Management Foundation
  └─> Create cypcb-library crate
      ├─> Define Component/Footprint types
      ├─> Implement KiCad parser (reuse cypcb-kicad)
      └─> FileSystemStorage backend

Level 1: Tauri Shell
  └─> Create src-tauri/ project
      ├─> Basic window setup
      ├─> File open/save commands
      ├─> Integrate LibraryManager
      └─> File watcher

Level 2: Monaco Integration
  └─> Add Monaco to frontend
      ├─> Language registration
      ├─> LSP client setup
      ├─> Tauri LSP spawning
      └─> Editor component

Level 3: Web Deployment
  └─> Static build configuration
      ├─> Vite config for CDN
      ├─> Environment detection
      ├─> Browser File API fallbacks
      └─> Deployment workflows
```

### Crate Dependency Graph (Updated)

```
Level 0 (No internal deps):
  cypcb-core          # Shared types

Level 1 (Depends on core):
  cypcb-parser        # DSL parsing
  cypcb-world         # ECS world
  cypcb-kicad         # KiCad format parsing

Level 2 (Depends on parser/world):
  cypcb-library       # NEW: Library management (uses cypcb-kicad)
  cypcb-drc           # DRC engine
  cypcb-export        # Export formats

Level 3 (Depends on library):
  cypcb-render        # WASM bindings (uses library for component resolution)
  cypcb-lsp           # LSP server (uses library for completions)

Level 4 (Application):
  src-tauri           # NEW: Desktop app (uses library, spawns LSP)
  viewer/             # Frontend (uses render WASM)
```

---

## Architectural Patterns

### Pattern 1: Environment-Specific Facades

**What:** Abstract platform differences behind common interfaces.

**Why:** Same frontend code works in desktop and web modes.

**Example:**

```typescript
// src/platform/file-system.ts

export interface FileSystem {
  openFile(): Promise<ProjectData>;
  saveFile(content: string): Promise<void>;
  watchFile(path: string, callback: () => void): void;
}

class TauriFileSystem implements FileSystem {
  async openFile(): Promise<ProjectData> {
    return await invoke('open_file');
  }
  // ... Tauri implementations
}

class BrowserFileSystem implements FileSystem {
  async openFile(): Promise<ProjectData> {
    return await openFileWeb();
  }
  // ... Browser API implementations
}

export const fs: FileSystem = IS_TAURI
  ? new TauriFileSystem()
  : new BrowserFileSystem();
```

### Pattern 2: Progressive Enhancement

**What:** Core functionality works everywhere, advanced features in capable environments.

**Why:** Web deployment doesn't block desktop-only features.

**Example:**

```typescript
// Feature detection
const features = {
  monacoEditor: IS_TAURI,
  fileWatcher: IS_TAURI,
  nativeDialogs: IS_TAURI,
  libraryImport: IS_TAURI,
  autoRouting: IS_TAURI,
};

// Conditional UI
{features.monacoEditor ? (
  <MonacoEditor />
) : (
  <ExternalEditorPrompt />
)}
```

### Pattern 3: Shared WASM Core

**What:** Same WASM module for desktop and web.

**Why:** Single source of truth, consistent behavior.

**Example:**

```rust
// crates/cypcb-render/src/lib.rs

#[wasm_bindgen]
pub struct PcbEngine {
    world: World,
    library: LibraryManager,
}

#[wasm_bindgen]
impl PcbEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let library = if cfg!(target_arch = "wasm32") {
            LibraryManager::new(Box::new(BrowserStorage::new()))
        } else {
            LibraryManager::new(Box::new(FileSystemStorage::new()))
        };

        Self {
            world: World::new(),
            library,
        }
    }
}
```

### Pattern 4: LSP as External Service

**What:** LSP server is separate process/service, not embedded.

**Why:** Supports both desktop (spawned) and web (remote) scenarios.

**Example:**

Desktop: Tauri spawns `cypcb-lsp` subprocess on localhost.
Web: User runs `cypcb-lsp --remote` or uses cloud-hosted instance.

Both connect via WebSocket using same protocol.

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Tauri-Specific Frontend Code

**What:** Using Tauri APIs directly in UI components.

**Why bad:** Breaks web deployment, hard to test.

**Instead:** Use facade pattern with environment detection.

### Anti-Pattern 2: Duplicating WASM Logic in Tauri

**What:** Implementing parser/DRC in Rust backend AND WASM.

**Why bad:** Code duplication, behavior divergence.

**Instead:** Use WASM core everywhere, Tauri only for I/O.

### Anti-Pattern 3: Blocking LSP Integration on Monaco

**What:** Waiting for Monaco to add LSP before shipping desktop.

**Why bad:** External editor + LSP already works (v1.0).

**Instead:** Monaco is enhancement, not requirement.

### Anti-Pattern 4: Requiring Backend for Web Deployment

**What:** Adding server-side rendering or API endpoints.

**Why bad:** Breaks static deployment, adds complexity.

**Instead:** Pure static site, use edge functions only if needed.

---

## Sources

**Tauri Integration:**
- [Tauri 2.0 Stable Release](https://v2.tauri.app/blog/tauri-20/)
- [Tauri Inter-Process Communication](https://v2.tauri.app/concept/inter-process-communication/)
- [Tauri State Management](https://v2.tauri.app/develop/state-management/)
- [Tauri File System Plugin](https://deepwiki.com/tauri-apps/tauri-plugin-fs/2.1-file-operations-system)

**Monaco Editor:**
- [Monaco Editor Integration with LSP](https://medium.com/@zsh-eng/integrating-lsp-with-the-monaco-code-editor-b054e9b5421f)
- [TypeFox monaco-languageclient](https://github.com/TypeFox/monaco-languageclient)
- [Tower-LSP Web Demo](https://github.com/silvanshade/tower-lsp-web-demo)

**Library Management:**
- [KiCad Footprint Library Format](https://dev-docs.kicad.org/en/file-formats/sexpr-footprint/index.html)
- [PCB Library Management Architecture](https://resources.altium.com/p/smart-architecture-successful-pcb-component-libraries)

**Web Deployment:**
- [Vite Static Site Deployment](https://vite.dev/guide/static-deploy)
- [WebAssembly Serverless on Edge](https://letket.com/high-performance-web-apps-in-2026-webassembly-webgpu-and-edge-architectures/)

---

*Architecture research for: v1.1 Foundation & Desktop Integration*
*Researched: 2026-01-29*

# Stack Research

**Domain:** Code-first PCB Design Tool (EDA)
**Researched:** 2026-01-21 (Updated 2026-01-29 for v1.1)
**Confidence:** HIGH

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Rust | 1.84+ | Core language | Memory safe, compiles to WASM, 30+ year longevity, used by Mozilla/Google/Microsoft |
| WebAssembly | 2.0 | Browser/portable runtime | W3C standard, near-native performance (8-10x faster than JS for compute), universal browser support |
| Tauri | 2.9+ | Desktop shell | 50% less RAM than Electron (~30MB vs ~200MB), <10MB bundle, Rust backend integration |
| Tree-sitter | 0.25 | DSL parser | Incremental parsing, error-tolerant, used by GitHub/Neovim/Zed, Rust native |
| wgpu | 24.0 | 2D/GPU rendering | WebGPU standard, cross-platform (Vulkan/Metal/DX12/WebGL), compute shaders for routing |
| Three.js | r170+ | 3D preview | Lightweight (168kB), massive ecosystem, WebGPU support coming |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| nalgebra | 0.33 | Linear algebra | Geometry calculations, transforms, matrix operations |
| glam | 0.29 | Fast 3D math | Hot rendering paths, SIMD-optimized |
| rstar | 0.12 | R*-tree spatial index | DRC collision detection, selection/picking |
| bevy_ecs | 0.15 | Entity Component System | Board data model, parallel queries |
| serde | 1.0 | Serialization | JSON, MessagePack, bincode support |
| tower-lsp | 0.20 | LSP framework | IDE integration (hover, completion, diagnostics) |
| thiserror | 2.0 | Library errors | Structured error types for parser/DRC |
| anyhow | 1.0 | Application errors | Error context and propagation |
| proptest | 1.5 | Property testing | Fuzzing parser, testing geometry algorithms |
| notify | 7.0 | File watching | Hot reload on .pcb file changes |
| gerber-types | latest | Gerber format | Export to manufacturing format |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| trunk | WASM bundler | `trunk serve` for dev, `trunk build --release` for production |
| wasm-pack | WASM packaging | Alternative to trunk, produces npm packages |
| cargo-watch | Auto-rebuild | `cargo watch -x check -x test` during development |
| criterion | Benchmarking | Performance regression testing for parser/DRC |
| insta | Snapshot testing | Gerber output stability, AST snapshots |

---

## v1.1 Stack Additions

*For: Library management, desktop packaging, web deployment, embedded editor*

### Library Management

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| serde_kicad_sexpr | 0.1+ | KiCad S-Expression parser | Serde-based KiCad v6+ footprint/.kicad_mod parsing with proper optional field handling |
| reqwest | 0.12+ | HTTP client (web) | WASM-compatible (via browser fetch), async/await, JSON support for API calls |
| web-sys | 0.3+ | Browser APIs (WASM) | Access to fetch API, IndexedDB, File API for web library management |
| tokio-rusqlite | 0.6+ | Async SQLite (desktop) | 100% safe Rust, async/await SQLite for desktop library cache |
| indexed_db_futures | 0.5+ | IndexedDB wrapper (web) | Async IndexedDB access for web library cache with automatic transaction rollback |

**Rationale:**
- **serde_kicad_sexpr** over manual parsing: Serde-based, handles KiCad's quirky S-expression format (struct names matter, special optional handling)
- **reqwest with WASM** over custom fetch: Battle-tested, but NOTE - web-sys + wasm-bindgen for direct fetch API is simpler for WASM (reqwest in WASM is overkill without full features like CORS credential control)
- **Split storage strategy**: SQLite for desktop (file system access, larger cache), IndexedDB for web (browser storage, offline-first)
- **tokio-rusqlite** over sqlx: Lighter weight, dedicated to SQLite, 100% safe Rust enforcement
- **indexed_db_futures** over raw web-sys: Removes JS callback pain, automatic transaction rollback on drop (safer default)

### 3D Model Handling

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| occt-import-js | 0.0.23 | STEP/IGES parser (web) | WASM-based OpenCascade, parses STEP to JSON for Three.js, client-side processing |
| three.js | r170+ | 3D rendering | Already in stack, handles GLB/GLTF + occt-import-js JSON output |

**Rationale:**
- **occt-import-js** over native STEP parsing: WASM memory limitations exist but acceptable for component-scale models (<5MB typical)
- KiCad footprints reference both .wrl (VRML) and .step files - prioritize STEP for accuracy, fall back to WRL for rendering
- Three.js already handles GLTF/GLB; occt-import-js converts STEP → JSON → Three.js geometry
- **LIMITATION:** Large assembly STEP files (>50MB) will struggle in browser; acceptable for component libraries

### Tauri Desktop Application

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| tauri | 2.9+ | Desktop framework | v2.0 stable released, supports Linux/macOS/Windows, framework-agnostic frontend |
| tauri-build | 2.9+ | Build-time codegen | Must match tauri version, generates Rust bindings for Tauri commands |

**Cargo.toml additions:**
```toml
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
tauri = { version = "2.9", features = ["protocol-asset"] }
serde = { version = "1.0", features = ["derive"] }
tokio-rusqlite = "0.6"

[build-dependencies]
tauri-build = { version = "2.9", features = [] }
```

**Rationale:**
- Tauri 2.x (latest 2.9.5) is stable and production-ready
- `protocol-asset` feature allows serving local files (footprint previews, 3D models)
- Desktop-only dependencies via `cfg(not(target_arch = "wasm32"))` keep WASM build clean
- tauri-build version MUST match tauri runtime version (semver compatibility critical)

### Web Deployment

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Cloudflare Pages | N/A | Static hosting | Unlimited bandwidth, WASM-friendly, fast edge network, free tier generous |
| Vite | 5.0+ | Build tool | Already using for viewer, handles WASM, code splitting, fast HMR |

**Alternatives Considered:**
- **Netlify**: Great DX, but bandwidth limits on free tier (100GB/mo vs Cloudflare's unlimited)
- **Vercel**: Excellent Next.js integration but not relevant here; similar bandwidth limits
- **GitHub Pages**: Free but slower edge network, no custom headers for WASM MIME types

**Rationale:**
- Cloudflare Pages wins for WASM apps: unlimited bandwidth, proper WASM MIME type handling, global CDN
- Vite already in stack (viewer/package.json), no new tooling needed
- Static site generation sufficient - no SSR needed for PCB tool
- **Deployment:** `vite build` → `wrangler pages deploy dist/`

### Monaco Editor Integration

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| monaco-editor | 0.55.1 | Code editor | VS Code editor core, 2000+ npm dependents, TypeScript defs, ESM build |
| monaco-editor (npm) | 0.55.1 | JavaScript integration | Install via npm, use ESM import (AMD deprecated) |

**Integration approach:**
```typescript
import * as monaco from 'monaco-editor';

// Create editor instance
const editor = monaco.editor.create(container, {
  value: initialCode,
  language: 'cypcb', // Custom language
  theme: 'vs-dark',  // Or 'vs' for light
  automaticLayout: true,
  minimap: { enabled: false },
});

// Register custom language
monaco.languages.register({ id: 'cypcb' });
monaco.languages.setMonarchTokensProvider('cypcb', {
  // Tokenizer rules - can leverage existing Tree-sitter grammar insights
});

// LSP integration via tower-lsp (existing)
// Use Language Server protocol over WebSocket or stdio
```

**Rationale:**
- Monaco over CodeMirror: Better TypeScript support, LSP integration precedent, VS Code familiarity
- Monaco over Ace: More actively maintained (Microsoft), better WASM story
- ESM build (not AMD): Modern, tree-shakeable, aligns with Vite
- **Integration with existing LSP:** tower-lsp already in stack; Monaco supports LSP via language client
- **Web vs Desktop:** Same Monaco code works in both Tauri (webview) and browser

**Dependencies:**
```json
{
  "dependencies": {
    "monaco-editor": "^0.55.1"
  }
}
```

**Custom Language Registration:**
- Use Monaco's Monarch tokenizer (declarative) OR
- Integrate Tree-sitter WASM grammar directly (more complex but consistent with LSP)
- LSP provides semantic tokens, diagnostics, autocomplete via existing tower-lsp server

### Dark Mode / Theme System

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| CSS Custom Properties | N/A | Theme variables | Native CSS, inherits across Shadow DOM, standard approach for 2026 |
| `prefers-color-scheme` | N/A | OS theme detection | CSS media query, automatic detection, no JS needed |
| Tauri theme API | 2.9+ | Desktop theme sync | `appWindow.theme()` and `onThemeChanged()` for OS integration |

**Implementation approach:**
```css
/* Define theme tokens */
:root {
  --bg-primary: light-dark(#ffffff, #1e1e1e);
  --fg-primary: light-dark(#000000, #d4d4d4);
  --accent: light-dark(#007acc, #4fc3f7);
}

/* Or fallback for older browsers */
:root {
  --bg-primary: #ffffff;
  --fg-primary: #000000;
}

@media (prefers-color-scheme: dark) {
  :root {
    --bg-primary: #1e1e1e;
    --fg-primary: #d4d4d4;
  }
}

/* Manual override */
[data-theme="dark"] {
  --bg-primary: #1e1e1e;
  --fg-primary: #d4d4d4;
}
```

**JavaScript theme toggle:**
```typescript
// Detect OS theme
const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;

// Listen for OS theme changes
window.matchMedia('(prefers-color-scheme: dark)')
  .addEventListener('change', (e) => {
    applyTheme(e.matches ? 'dark' : 'light');
  });

// Tauri: sync with OS (desktop only)
import { appWindow } from '@tauri-apps/api/window';
const theme = await appWindow.theme(); // 'dark' | 'light'
appWindow.onThemeChanged(({ payload: theme }) => {
  applyTheme(theme);
});
```

**Monaco Editor integration:**
```typescript
// Sync Monaco theme with app theme
monaco.editor.setTheme(isDark ? 'vs-dark' : 'vs');

// Custom theme definition
monaco.editor.defineTheme('cypcb-dark', {
  base: 'vs-dark',
  inherit: true,
  rules: [
    { token: 'component', foreground: '4fc3f7' },
    { token: 'net', foreground: 'ce9178' },
  ],
  colors: {
    'editor.background': '#1e1e1e',
  }
});
```

**Rationale:**
- CSS Custom Properties are the 2026 standard (Dropbox, Slack, Facebook use this approach)
- `light-dark()` CSS function (new in 2024-2025) simplifies implementation but older browser fallback needed
- `prefers-color-scheme` is universally supported, zero JS for automatic detection
- Tauri theme API provides OS integration for desktop without manual detection
- **Storage:** Use localStorage for manual theme override, default to `auto` (OS sync)

**No additional dependencies needed** - pure CSS + browser APIs + Tauri built-ins

---

## Installation

```toml
# Cargo.toml (v1.1 additions highlighted)

[package]
name = "cypcb"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
# Core
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Math & Geometry
nalgebra = "0.33"
glam = "0.29"
rstar = "0.12"

# ECS
bevy_ecs = "0.15"

# Parsing
tree-sitter = "0.25"

# Error handling
thiserror = "2.0"
anyhow = "1.0"

# Serialization
bincode = "1.3"
rmp-serde = "1.3"

# File watching
notify = "7.0"
notify-debouncer-full = "0.4"

# LSP
tower-lsp = "0.20"
lsp-types = "0.97"

# Async
tokio = { version = "1", features = ["full"] }

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# PCB Export
gerber-types = "0.1"

# ===== v1.1 ADDITIONS =====

# Library management
serde_kicad_sexpr = "0.1"

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
web-sys = { version = "0.3", features = [
    "CanvasRenderingContext2d",
    "HtmlCanvasElement",
    "Window",
    "Document",
    # v1.1: Library management
    "Request",
    "RequestInit",
    "RequestMode",
    "Response",
    "Headers",
    # v1.1: IndexedDB for web library cache
    "IdbFactory",
    "IdbDatabase",
    "IdbObjectStore",
    "IdbTransaction",
    "IdbRequest",
] }
serde-wasm-bindgen = "0.6"
console_error_panic_hook = "0.1"
# v1.1: IndexedDB wrapper
indexed_db_futures = "0.5"

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
tauri = { version = "2.9", features = ["protocol-asset"] }
# v1.1: Desktop library cache
tokio-rusqlite = "0.6"

[build-dependencies]
tauri-build = { version = "2.9", features = [] }

[dev-dependencies]
proptest = "1.5"
criterion = "0.5"
insta = { version = "1.40", features = ["json"] }

[profile.release]
lto = true
opt-level = 3

[profile.release-wasm]
inherits = "release"
opt-level = "s"  # Optimize for size
```

```json
// viewer/package.json (v1.1 additions)

{
  "name": "cypcb-viewer",
  "version": "0.1.0",
  "description": "CodeYourPCB viewer frontend",
  "type": "module",
  "scripts": {
    "start": "./start.sh",
    "build:wasm": "./build-wasm.sh",
    "dev": "vite",
    "dev:watch": "npx tsx server.ts",
    "build": "npm run build:wasm && tsc && vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "monaco-editor": "^0.55.1",
    "occt-import-js": "^0.0.23",
    "three": "^0.170.0"
  },
  "devDependencies": {
    "@types/ws": "^8.5.0",
    "@types/three": "^0.170.0",
    "chokidar": "^3.6.0",
    "tsx": "^4.0.0",
    "typescript": "^5.3.3",
    "vite": "^5.0.0",
    "ws": "^8.18.0"
  }
}
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Rust | C++ | Existing C++ codebase, need Altium/KiCad plugin integration |
| Tauri | Electron | Need Chrome DevTools debugging, complex native node modules |
| Tree-sitter | LALRPOP | Simpler grammar, don't need incremental parsing |
| wgpu | Canvas 2D | Very simple renders, don't need compute shaders |
| bevy_ecs | specs/hecs | Lighter weight, don't need Bevy's full ecosystem |
| nalgebra | cgmath | Legacy code compatibility (cgmath unmaintained) |
| bincode | MessagePack | Need human-readable cache files, cross-language interop |
| **v1.1 Alternatives:** |
| serde_kicad_sexpr | Manual parsing | Need KiCad v5 support (different format) or custom extensions |
| tokio-rusqlite | sqlx | Need multi-database support (Postgres/MySQL), compile-time query checking |
| indexed_db_futures | raw web-sys | Need fine-grained control over IndexedDB transactions |
| Monaco Editor | CodeMirror 6 | Need lighter bundle (<100kb), don't need LSP integration |
| Monaco Editor | Ace Editor | Legacy codebase already using Ace |
| occt-import-js | Native STEP parser | Desktop-only, need full CAD assembly support (>100MB files) |
| Cloudflare Pages | Netlify | Already invested in Netlify ecosystem, need form handling |
| Cloudflare Pages | Vercel | Using Next.js (not applicable here) |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| Electron | 200MB+ RAM, 80MB+ bundle, slow startup | Tauri 2.0 |
| cgmath | Unmaintained since 2021 | nalgebra or glam |
| nom (for DSL) | Not incremental, poor error recovery | Tree-sitter |
| OpenGL | Legacy, no compute shaders, poor WASM support | wgpu (WebGPU) |
| Custom autorouter (MVP) | Months of work for inferior results | FreeRouting (proven) |
| SQLite for board storage | Overkill, poor diff/merge | Custom binary + JSON |
| XML for file format | Verbose, poor human readability | Custom DSL |
| Floating-point coordinates | Precision issues, non-determinism | Integer nanometers (like KiCad) |
| **v1.1 Anti-Patterns:** |
| reqwest in WASM | Overkill, missing browser-specific features (credentials) | web-sys fetch API directly |
| Full KiCad library clone | 100+ MB download, sync nightmare | On-demand fetching with IndexedDB cache |
| Rust-native STEP parser | WASM binary bloat (>5MB), complex | occt-import-js (proven, 1.2MB) |
| Custom code editor | Reinventing wheel, months of work | Monaco Editor (VS Code proven) |
| Server-side library proxy | Deployment complexity, costs | Direct JLCPCB/KiCad API calls from client |
| Separate desktop/web codebases | Maintenance nightmare | Shared Rust core, platform-specific storage only |

## Stack Patterns by Variant

**If targeting web-only:**
- Skip Tauri entirely
- Use trunk for WASM bundling
- Consider Yew or Leptos for UI framework
- Use IndexedDB for library cache
- Use web-sys fetch API for library downloads

**If targeting desktop-only:**
- Can use native file dialogs via Tauri
- Consider egui for immediate-mode UI
- Can use native threads instead of web workers
- Use SQLite for library cache with full filesystem access
- Use reqwest (native, not WASM) for library downloads

**If needing 3D CAD integration:**
- Consider three-d crate for native 3D
- Or OpenCASCADE bindings via opencascade-sys
- STEP file export becomes important

**v1.1 Specific Patterns:**

**Library Management Architecture:**
- **Web:** IndexedDB cache → web-sys fetch → KiCad/JLCPCB API
- **Desktop:** SQLite cache → reqwest → KiCad/JLCPCB API + local filesystem scanning
- **Shared:** serde_kicad_sexpr parsing, common data structures

**Editor Integration:**
- **Both platforms:** Same Monaco editor (Tauri uses webview)
- **LSP:** tower-lsp server via WebSocket (web) or stdio (desktop)
- **Custom language:** Register 'cypcb' language with Monaco Monarch tokenizer

**3D Preview:**
- **Web:** occt-import-js (WASM) → Three.js
- **Desktop:** Same stack (Tauri webview runs same code)
- **Optimization:** Cache parsed STEP → JSON, avoid re-parsing

**Theme System:**
- **Web:** CSS custom properties + prefers-color-scheme
- **Desktop:** Same CSS + Tauri theme API for OS sync
- **Storage:** localStorage for manual override

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| bevy_ecs 0.15 | Rust 1.84+ | MSRV increased in 0.15 |
| wgpu 24.0 | naga 24.0 | Must match versions |
| tree-sitter 0.25 | tree-sitter-cli 0.25 | Grammar and runtime must match |
| Tauri 2.0 | @tauri-apps/api 2.0 | Frontend/backend versions must match |
| tower-lsp 0.20 | lsp-types 0.97 | Check compatibility on upgrade |
| **v1.1 Compatibility:** |
| tauri 2.9 | tauri-build 2.9 | **CRITICAL:** Must match exactly (semver compatible) |
| monaco-editor 0.55.1 | TypeScript 5.3+ | Type definitions require modern TS |
| occt-import-js 0.0.23 | three.js r170+ | JSON output compatible with Three.js geometry |
| indexed_db_futures 0.5 | web-sys 0.3 | Uses web-sys IDB bindings |
| tokio-rusqlite 0.6 | tokio 1.x | Async runtime compatibility |

## Integration Points (v1.1)

### Monaco Editor ↔ Tower-LSP
- Monaco uses Language Server Protocol client
- Connect via WebSocket (web) or stdio (desktop)
- Existing tower-lsp server provides diagnostics, completion, hover
- **Code:** Monaco language client → LSP over WebSocket → tower-lsp server

### Library Management ↔ Storage
- **Web path:** IndexedDB (indexed_db_futures) ← serde JSON ← serde_kicad_sexpr
- **Desktop path:** SQLite (tokio-rusqlite) ← serde JSON ← serde_kicad_sexpr
- **Shared:** KiCad S-expression parsing via serde_kicad_sexpr

### 3D Preview ↔ Library Management
- Library manager fetches .step file (via web-sys fetch or reqwest)
- occt-import-js parses STEP → JSON geometry
- Three.js renders JSON as mesh
- **Caching:** Store parsed JSON in IndexedDB/SQLite to avoid re-parsing

### Theme System ↔ Monaco Editor
- App theme changes trigger Monaco theme update
- `monaco.editor.setTheme('vs-dark' | 'vs')` or custom theme
- Tauri `onThemeChanged` event propagates to Monaco

### Tauri ↔ Web Shared Code
- Same Rust WASM core for parsing, validation, rendering
- Platform-specific: Storage (SQLite vs IndexedDB), fetch (reqwest vs web-sys)
- Tauri uses webview, so same HTML/CSS/JS/Monaco code

## Performance Considerations (v1.1)

### Library Cache Strategy
- **Cache key:** Library source + component ID + version
- **Cache invalidation:** 7-day TTL for JLCPCB parts (inventory changes), 30-day for KiCad (stable)
- **Size limits:** IndexedDB ~50MB quota (browser), SQLite unlimited (desktop)
- **Prefetching:** Cache popular components on first launch

### STEP File Parsing
- **occt-import-js limitations:** Large files (>50MB) may fail due to WASM memory
- **Mitigation:** Limit component 3D models to <5MB (typical), warn on large files
- **Caching:** Parse once, store JSON geometry, reuse across sessions

### Monaco Editor Performance
- **Bundle size:** ~5MB uncompressed, ~1.5MB gzipped
- **Loading strategy:** Lazy load Monaco on first editor open (code splitting)
- **Web workers:** Monaco runs language features in workers (non-blocking)

### IndexedDB Performance
- **Read latency:** ~10ms for cached footprint
- **Write latency:** ~50ms for new footprint
- **Bulk operations:** Use transactions for multiple writes (batch insertions)

## Build Configuration (v1.1)

### Vite Configuration for Monaco
```typescript
// vite.config.ts
import { defineConfig } from 'vite';

export default defineConfig({
  optimizeDeps: {
    include: ['monaco-editor'],
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          monaco: ['monaco-editor'],
        },
      },
    },
  },
});
```

### Tauri Configuration
```json
// src-tauri/tauri.conf.json
{
  "build": {
    "beforeBuildCommand": "npm run build",
    "beforeDevCommand": "npm run dev",
    "devPath": "http://localhost:5173",
    "distDir": "../dist"
  },
  "tauri": {
    "allowlist": {
      "fs": {
        "scope": ["$APPLOCALDATA/*", "$APPDATA/*"]
      },
      "http": {
        "scope": ["https://api.jlcpcb.com/*", "https://gitlab.com/kicad/*"]
      }
    },
    "windows": [
      {
        "theme": "auto"
      }
    ]
  }
}
```

### WASM Optimization
```toml
[profile.release-wasm]
inherits = "release"
opt-level = "s"  # Size optimization for WASM
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

## Sources

- Brainstorm research session (extensive benchmarks gathered)
- [WebAssembly 3.0 Rust vs C++ Benchmarks](https://markaicode.com/webassembly-3-performance-rust-cpp-benchmarks-2025/)
- [Tauri vs Electron 2025](https://codeology.co.nz/articles/tauri-vs-electron-2025-desktop-development.html)
- [wgpu Documentation](https://wgpu.rs/)
- [Tree-sitter GitHub](https://github.com/tree-sitter/tree-sitter)
- [KiCad Developer Docs](https://dev-docs.kicad.org/)

**v1.1 Sources:**
- [Tauri 2.0 Official Documentation](https://v2.tauri.app/)
- [Tauri 2.0 Stable Release](https://v2.tauri.app/blog/tauri-20/)
- [Tauri Core Releases](https://v2.tauri.app/release/)
- [Monaco Editor Repository](https://github.com/microsoft/monaco-editor)
- [Monaco Editor npm Package](https://www.npmjs.com/package/monaco-editor)
- [occt-import-js Repository](https://github.com/kovacsv/occt-import-js)
- [occt-import-js npm Package](https://www.npmjs.com/package/occt-import-js)
- [OCCT STEP Viewer Web](https://github.com/Roadinforest/occt-step-viewer-web)
- [KiCad Footprint Format Documentation](https://dev-docs.kicad.org/en/file-formats/sexpr-footprint/index.html)
- [KiCad S-Expression Format](https://dev-docs.kicad.org/en/file-formats/sexpr-intro/)
- [KiCad Footprint 3D Model Requirements](https://klc.kicad.org/footprint/f9/f9.3.html)
- [serde_kicad_sexpr Repository](https://github.com/kicad-rs/serde_kicad_sexpr)
- [tokio-rusqlite crate](https://crates.io/crates/tokio-rusqlite)
- [indexed_db_futures crate](https://crates.io/crates/indexed_db_futures)
- [wasm-bindgen Guide: Fetch Example](https://rustwasm.github.io/docs/wasm-bindgen/examples/fetch.html)
- [JLCPCB API Platform](https://api.jlcpcb.com/)
- [Cloudflare vs Vercel vs Netlify 2026](https://dev.to/dataformathub/cloudflare-vs-vercel-vs-netlify-the-truth-about-edge-performance-2026-50h0)
- [Dark Mode with CSS Custom Properties](https://css-irl.info/quick-and-easy-dark-mode-with-css-custom-properties/)
- [Dark Mode in Web Components 2026](https://dev.to/stuffbreaker/dark-mode-in-web-components-is-about-to-get-awesome-4i14)
- [Tauri Dark Mode Implementation](https://dev.to/rain9/tauri-4-get-the-theme-switching-function-fixed-21po)

---
*Stack research for: Code-first PCB Design Tool*
*Original research: 2026-01-21*
*v1.1 additions: 2026-01-29*

# Feature Research

**Domain:** Code-first PCB Design Tool (EDA)
**Researched:** 2026-01-21
**Confidence:** HIGH

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels broken.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Component placement | Can't design PCB without placing parts | MEDIUM | Need footprint library support |
| Net connections | Defining what connects to what is the whole point | LOW | Core of the DSL |
| Multi-layer support | Even simple boards are 2-layer | MEDIUM | Stackup definition |
| Design Rule Check (DRC) | Every EDA tool has this | HIGH | Clearance, width, drill rules |
| Board outline definition | Manufacturing requires it | LOW | Polygon/shape definition |
| Gerber export | Universal manufacturing format | MEDIUM | Multiple layers, drill files |
| 2D board view | Must see what you're designing | MEDIUM | Top/bottom copper, silk, mask |
| Undo/redo | Expected in any editor | MEDIUM | Command pattern, history |
| Zoom/pan | Basic navigation | LOW | Standard canvas interactions |
| Grid snapping | Alignment is critical | LOW | Configurable grid sizes |
| Net highlighting | See where connections go | LOW | Visual feedback on selection |
| Component rotation | 0/90/180/270 at minimum | LOW | Transform in DSL |

### Differentiators (Competitive Advantage)

Features that set this code-first approach apart.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Git-friendly file format** | Teams can collaborate, diff, merge, review | HIGH | Core value proposition |
| **LLM/AI editable** | "Claude, move this trace" | MEDIUM | Clear DSL = AI can edit |
| **Deterministic builds** | Same file = same output, always | HIGH | Must avoid floating-point randomness |
| **Hot reload** | Edit file → see changes instantly | MEDIUM | File watch + incremental parse |
| **LSP/IDE integration** | Autocomplete, hover, go-to-definition | HIGH | Makes code-first practical |
| **Electrical-aware constraints** | System knows signal types, not just geometry | HIGH | crosstalk_sensitive, high_speed |
| **Declarative modules** | Reusable circuit blocks | MEDIUM | Import/compose patterns |
| **CI/CD testable** | Run DRC in pipeline, fail on violations | LOW | CLI interface |
| **Constraint-based routing** | Say what, not how | HIGH | Autorouter with constraints |
| **AI hints in syntax** | @ai-hint comments for LLM context | LOW | Comment convention |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Real-time collaboration | "Like Figma for PCB" | Complexity explosion, conflict resolution hell | Git-based async workflow |
| Schematic-driven layout | Traditional EDA workflow | Couples two complex systems, harder to maintain | Unified DSL for both (later) |
| Built-in component marketplace | Convenience | Licensing complexity, hosting costs, curation burden | Import from existing (KiCad, etc.) |
| Automatic schematic generation | Derive schematic from code | Hard to make readable, loses design intent | Optional separate schematic file |
| Unlimited undo history | Users want infinite undo | Memory explosion on complex boards | Configurable limit (100-1000) |
| Visual-first mode | Some users prefer GUI | Dilutes code-first value prop | Code-first with visual feedback |
| Manufacturing integration | One-click order | Business complexity, liability, certification | Export files, user orders |

## Feature Dependencies

```
[Parser] ─────────────────────┐
    │                         │
    ▼                         │
[Board Model] ───────────────┬┴──────────────┐
    │                        │               │
    ▼                        ▼               ▼
[Renderer 2D]          [DRC Engine]    [LSP Server]
    │                        │
    ▼                        ▼
[3D Preview]          [Autorouter]
                            │
                            ▼
                     [Gerber Export]
```

### Dependency Notes

- **Renderer requires Board Model:** Can't draw what doesn't exist
- **DRC requires Board Model:** Checks operate on board state
- **LSP requires Parser:** Needs AST for hover/completion
- **Autorouter requires DRC:** Must validate routes against rules
- **3D Preview requires 2D Renderer:** Shares component geometry
- **Gerber Export requires DRC passing:** Don't export invalid designs

## MVP Definition

### Launch With (v1)

Minimum viable product — what's needed to validate the code-first concept.

- [ ] **Custom DSL parser** — The language IS the product
- [ ] **Board model with components and nets** — Core data structure
- [ ] **2D board view renderer** — Must see results
- [ ] **Hot reload** — Edit-see cycle must be fast
- [ ] **Basic DRC (clearance, width)** — Prevent obvious errors
- [ ] **Gerber export** — Must be manufacturable
- [ ] **Simple footprint support** — At least basic SMD/through-hole
- [ ] **CLI interface** — For CI/CD integration

### Add After Validation (v1.x)

Features to add once core is working.

- [ ] **LSP server** — When users want IDE integration
- [ ] **Autorouter integration (FreeRouting)** — When manual routing gets tedious
- [ ] **3D preview** — When users want to check mechanical fit
- [ ] **Undo/redo** — When editing becomes complex
- [ ] **KiCad footprint import** — When component variety matters
- [ ] **Multi-board projects** — When users have complex systems

### Future Consideration (v2+)

Features to defer until product-market fit is established.

- [ ] **Custom autorouter (GPU-accelerated)** — After proving concept with FreeRouting
- [ ] **Ngspice simulation integration** — When users need electrical verification
- [ ] **Schematic view** — When users want traditional EDA workflow option
- [ ] **WASM plugin system** — When extension ecosystem is needed
- [ ] **IPC-2581 export** — When manufacturers request it
- [ ] **Impedance calculator integration** — When high-speed design is common

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| DSL parser | HIGH | HIGH | **P0** |
| Board model | HIGH | MEDIUM | **P0** |
| 2D renderer | HIGH | MEDIUM | **P0** |
| Hot reload | HIGH | LOW | **P0** |
| Gerber export | HIGH | MEDIUM | **P0** |
| Basic DRC | HIGH | MEDIUM | **P1** |
| Footprint support | HIGH | MEDIUM | **P1** |
| CLI interface | MEDIUM | LOW | **P1** |
| LSP server | HIGH | HIGH | **P2** |
| Autorouter | HIGH | LOW (FreeRouting) | **P2** |
| 3D preview | MEDIUM | MEDIUM | **P2** |
| Undo/redo | MEDIUM | MEDIUM | **P2** |
| KiCad import | MEDIUM | MEDIUM | **P2** |
| Plugin system | MEDIUM | HIGH | **P3** |
| Simulation | MEDIUM | HIGH | **P3** |

**Priority key:**
- P0: Must have for MVP launch
- P1: Required for usable product
- P2: Should have, add post-launch
- P3: Nice to have, future consideration

## Competitor Feature Analysis

| Feature | KiCad | Eagle | Altium | EasyEDA | **CodeYourPCB** |
|---------|-------|-------|--------|---------|-----------------|
| File format | S-expr (text) | XML | Binary | Cloud | **Custom DSL** |
| Git-friendly | Partial | Poor | Poor | N/A | **Excellent** |
| AI editable | Poor | Poor | Poor | Poor | **Excellent** |
| Learning curve | High | Medium | High | Low | **Medium*** |
| Autorouter | External | Built-in | Built-in | Built-in | External (MVP) |
| Simulation | Basic | Basic | Advanced | Basic | External (ngspice) |
| Price | Free | $$ | $$$$ | Free/$ | **Free/Open** |
| Collaboration | Manual merge | Poor | PDM | Cloud | **Git native** |

*Learning curve for programmers is low; for traditional EDA users is higher initially.

## Sources

- KiCad user feedback and feature requests
- Eagle/Altium marketing materials and documentation
- tscircuit project (similar code-first approach)
- JITX marketing (commercial code-first EDA)
- Brainstorm session requirements discussion

---
*Feature research for: Code-first PCB Design Tool*
*Researched: 2026-01-21*

# Domain Pitfalls: Code-First PCB Design Tool

**Domain:** Code-first PCB/EDA design tool
**Researched:** 2026-01-21
**Confidence:** MEDIUM (mix of authoritative sources and community experience)

---

## Critical Pitfalls

Mistakes that cause rewrites, fundamental breakage, or project failure.

### Pitfall 1: DSL Syntax Lock-in

**What goes wrong:** Early DSL design decisions become permanent because users write code against them. Syntax mistakes discovered later cannot be fixed without breaking all existing designs.

**Why it happens:**
- Rushing to "something that works" without considering evolution
- Not understanding the PCB domain deeply enough before designing syntax
- Copying syntax from other DSLs without considering PCB-specific needs
- "More ways to do it" seems friendly but creates maintenance burden

**Consequences:**
- Permanent technical debt in the language itself
- Users must learn multiple syntax variants
- Documentation becomes cluttered with deprecated patterns
- LLMs trained on old syntax produce incompatible code

**Prevention:**
1. **Version the DSL from day one** - Include `version: 1` in every file
2. **Start minimal** - Fewer keywords are easier to evolve. "One way to do it + escape hatch" (expose raw values when DSL doesn't cover use case)
3. **Dogfood extensively** - Design real boards with the DSL before freezing syntax
4. **Reserve keywords** - Reserve likely-needed keywords even if not implemented
5. **Study prior art deeply** - KDL, S-expressions (KiCad), JITX, SKiDL syntax choices and their tradeoffs

**Detection (warning signs):**
- Developers asking "can we also support X syntax for the same thing?"
- Documentation showing multiple ways to express identical intent
- Users confused about which variant to use
- Discussions about "the right way" to express something

**Phase mapping:** Phase 1 (Foundation) - get grammar design right before any code depends on it

**Sources:**
- [Martin Fowler DSL Q&A](https://martinfowler.com/bliki/DslQandA.html) - "Having more than one way to do something is not a virtue, it's a curse"
- [DSL Evolution InfoQ](https://www.infoq.com/articles/dsl-evolution/) - Versioning strategies

---

### Pitfall 2: Floating-Point Geometry Errors

**What goes wrong:** Using floating-point numbers (f32/f64) for PCB coordinates leads to cumulative precision errors. Two traces that should connect don't. DRC reports false violations or misses real ones. Gerber output has micro-gaps.

**Why it happens:**
- Floating-point is the default in most languages
- Errors are small initially, only manifest at scale
- Different operations accumulate errors differently
- Operations far from origin have worse precision

**Consequences:**
- Non-deterministic builds (same source file produces slightly different outputs)
- Manufacturing defects from micro-gaps in Gerber output
- DRC inconsistencies (pass on one run, fail on another)
- Debug nightmare - issues appear/disappear based on operation order

**Prevention:**
1. **Use integer nanometers internally** - KiCad uses 32-bit signed integers with 1nm resolution (supports boards up to ~2.14m)
2. **Convert at boundaries only** - Parse mm/mils from user input to internal integers, convert back only for display/export
3. **Avoid coordinate system mismatches** - KiCad's pixel-style (Y-down) vs Gerber's Cartesian (Y-up) causes mirrored placements if not handled correctly
4. **Test with extreme coordinates** - Generate test cases at board corners, with many sequential operations
5. **Snap to grid after operations** - Prevents drift accumulation

**Detection:**
- Unit tests comparing geometry operations show non-zero deltas
- Same design produces different Gerber checksums on different runs
- DRC results are non-deterministic
- Visual inspection shows micro-gaps at high zoom

**Phase mapping:** Phase 1 (Foundation) - core data model must use integers from the start

**Sources:**
- [KiCad Coordinate System](https://forum.kicad.info/t/coordinate-system-grid-and-origins-in-the-pcb-editor/24535) - "Internal measurement resolution is 1 nanometer, stored as 32-bit integers"
- [Mitigating Floating Point Errors in Computational Geometry](https://medium.com/@moiserushanika2006/mitigating-floating-point-errors-in-computational-geometry-algorithms-a62525da45ef)

---

### Pitfall 3: Gerber Generation Edge Cases

**What goes wrong:** Gerber export works for simple cases but fails silently or produces unmanufacturable output for complex designs. Manufacturing house rejects files or produces wrong boards.

**Why it happens:**
- Testing only with simple boards
- Not understanding Gerber format nuances (RS-274X vs X2, aperture handling)
- Drill file coordinate system mismatches
- Large copper pours generating vector fills instead of contours

**Consequences:**
- Manufacturing delays (files rejected, require manual fixes)
- Wrong boards produced (expensive prototype wasted)
- User trust destroyed (tool "works" until it matters)
- Support burden from debugging manufacturing issues

**Prevention:**
1. **Use Gerber X2 (not 274D or even plain 274X)** - Modern standard with metadata
2. **Test against Gerber viewers** - gerbv, KiCad Gerber viewer, manufacturer's viewer
3. **Verify drill/Gerber alignment** - Units and zero-suppression must match between Excellon drill and Gerber files
4. **Generate flash pads, not vector pads** - Vector pads slow manufacturing, may cause errors
5. **Always include board outline** - "Most common mistake" per multiple manufacturers
6. **Use contour fills, not vector fills** - Large copper areas with 1-2mil vectors become too large for plotters
7. **Test with multiple manufacturers' DFM tools** - JLCPCB, PCBWay, OSH Park all have free checks

**Detection:**
- Gerber viewer shows visual anomalies (gaps, jagged edges)
- File sizes suspiciously large (vector fill issue)
- Manufacturer DFM tool reports errors
- Drill holes visually offset from pads in viewer

**Phase mapping:** Phase 2 (Core Features) - Gerber export must be battle-tested before release

**Sources:**
- [Common Gerber File Issues - Bittele](https://www.7pcb.com/blog/common-gerber-issues-how-to-fix-them)
- [Common Problems with Gerber Files - Sierra Circuits](https://www.protoexpress.com/blog/common-problems-associated-with-gerber-files/)
- [Gerber Files - Bay Area Circuits](https://bayareacircuits.com/common-problems-with-gerber-files-and-how-to-avoid-them/)

---

### Pitfall 4: Autorouter Non-Determinism

**What goes wrong:** Running the autorouter twice on the same design produces different results. Version control becomes meaningless. "Works on my machine" for routing.

**Why it happens:**
- Random seeds not controlled
- Floating-point accumulation in cost functions
- Order-dependent data structures (hash maps with non-deterministic iteration)
- External autorouter (FreeRouting) may have own non-determinism

**Consequences:**
- Git history becomes noise (every commit changes routes even if design unchanged)
- LLM-assisted editing becomes unreliable (can't predict routing outcome)
- Debugging impossible (can't reproduce the specific routing that failed DRC)
- Violates core value proposition ("same file = same output")

**Prevention:**
1. **Seed all randomness explicitly** - Accept seed as parameter, default to hash of design
2. **Use deterministic data structures** - BTreeMap not HashMap, sorted iteration
3. **Integer arithmetic for cost functions** - Avoid floating-point in optimization loops
4. **Cache and version routing results** - Store solved routes with content hash of inputs
5. **Document FreeRouting determinism** - If using external router, understand its guarantees (may need patches)
6. **Include routing in test suite** - Same inputs must produce byte-identical outputs

**Detection:**
- Running router twice produces different `.session` files
- Git shows routing changes when only components changed
- Users report "it routed fine yesterday but fails today"
- Route quality varies between runs

**Phase mapping:** Phase 3 (Intelligence) - autorouter integration must address this upfront

**Sources:**
- [Why PCB Autorouting Remains Broken](https://autocuro.com/blog/why-pcb-autorouting-remains-broken)
- [tscircuit autorouting repo](https://github.com/tscircuit/autorouting) - "CBS algorithm's predictability and determinism"

---

### Pitfall 5: DRC Performance Cliff

**What goes wrong:** DRC works fine for 50-component boards but takes minutes or hangs completely for 500-component boards. Real-time DRC becomes unusable, users disable it, ship broken designs.

**Why it happens:**
- Naive O(n^2) algorithms (check every object pair)
- No spatial indexing
- Checking entire board on every edit
- Copper pour recalculation on every change

**Consequences:**
- Users disable DRC, ship boards with violations
- IDE/editor becomes sluggish
- Large designs become impractical
- Competitive disadvantage vs tools with fast DRC

**Prevention:**
1. **Spatial indexing from day one** - R*-tree (rstar crate) for all geometry queries
2. **Incremental DRC** - Only check objects affected by the edit
3. **Zone-based checking** - Divide board into zones, parallelize
4. **Tiered DRC** - Fast subset for real-time, full check on save/export
5. **Profile with realistic boards** - 500+ components, dense routing, large copper pours
6. **GPU acceleration for geometry** - Modern DRC research uses GPU for intersection tests

**Detection:**
- DRC time grows non-linearly with component count
- Profiler shows geometry functions dominating
- Users complaining about lag after adding copper pours
- Memory usage spikes during DRC

**Phase mapping:** Phase 2 (Core Features) - basic DRC must scale; Phase 5 (Advanced) - GPU acceleration

**Sources:**
- [PDRC: GPU-Accelerated DRC](http://www.cse.cuhk.edu.hk/~byu/papers/C219-DAC2024-PDRC.pdf) - Bentley-Ottmann variants, R-tree optimization
- [EasyEDA DRC](https://docs.easyeda.com/en/PCB/Design-Rule-Check/) - Real-time DRC approach

---

### Pitfall 6: File Format Breaking Changes Without Migration

**What goes wrong:** File format changes break existing designs. Users can't open their old projects. Or worse, projects open but with subtle corruption.

**Why it happens:**
- "We'll add migration later" (you won't)
- Not versioning files from the start
- Changing semantics without changing syntax
- Incomplete migration (handles 80% of cases, corrupts 20%)

**Consequences:**
- Users lose work (or think they did)
- Trust destroyed instantly
- Support nightmare
- Fork pressure (users stick to old version)

**Prevention:**
1. **Version in every file** - `version: 1` on line 1, mandatory
2. **Never remove, only deprecate** - Old syntax continues to parse
3. **Write migration with every breaking change** - Automated upgrade path
4. **Round-trip tests** - Parse old format, save new format, verify no data loss
5. **Keep old version parsers** - Can always read old files
6. **Warn on save, not on load** - "Saving will upgrade to v2 format. Continue?"

**Detection:**
- Old test fixtures start failing mysteriously
- Users report "file won't open" after updating
- Diff shows unexpected changes in unchanged sections
- Migration test suite incomplete or missing

**Phase mapping:** Phase 1 (Foundation) - versioning must be in grammar from start

**Sources:**
- [KiCad File Compatibility](https://forum.kicad.info/t/backward-and-forward-compatibility/45234) - "Major releases almost always come with changes to file formats"
- [Go Backward Compatibility](https://go.dev/blog/compat) - Language evolution lessons

---

## v1.1 Integration Pitfalls

Critical mistakes when adding library management, Tauri desktop, web deployment, and Monaco editor to the existing v1.0 system.

### Pitfall 7: Library Namespace Conflicts Without Resolution Strategy

**What goes wrong:** Multiple library sources (KiCad, JLCPCB, custom) contain components with identical names but different implementations. User imports "R_0805" from three different sources, gets silent overwrites or unpredictable behavior. Designs reference the wrong footprint, leading to manufacturing failures caught only at prototype stage.

**Why it happens:** Each library ecosystem (KiCad, JLCPCB, SnapEDA) uses its own naming conventions. Developers assume "last write wins" or "merge on import" without considering that footprints with identical names may have different pad layouts, silkscreen, or 3D models. The "Footprint Release God" pattern from professional teams (one person approves all additions) doesn't translate to multi-source consumption.

**How to avoid:**
- Implement namespace-prefixed imports: `kicad::R_0805`, `jlcpcb::R_0805`, `custom::R_0805`
- Store library source metadata with each imported footprint
- Detect conflicts at import time and require explicit user resolution
- Create a conflict resolution UI showing side-by-side comparison of duplicate footprints
- Version control the library index with source metadata

**Warning signs:**
- Import logs show "replaced existing footprint" messages without user confirmation
- Users report "board looks different after re-importing libraries"
- Manufacturing errors where pad sizes don't match expected values
- No library source indicated in component metadata

**Phase to address:** v1.1 Phase 1 (Library Management Foundation) must implement namespace system. Phase 2 (Library Integration) must build conflict resolution UI.

**Sources:**
- [Managing PCB Footprint Libraries with Altium 365](https://resources.altium.com/p/managing-pcb-footprint-libraries-with-altium-365)
- [Centralized Component Libraries Best Practices](https://resources.altium.com/p/centralized-component-libraries-best-practices-hardware-teams)

---

### Pitfall 8: Desktop/Web Feature Parity Drift Without Abstraction Layer

**What goes wrong:** Desktop Tauri build gets native file system access, while web version can't read/write files. Developers implement file operations using Tauri's `fs` plugin directly in business logic. Web deployment breaks completely. Codebase diverges into two separate implementations with different bugs, making maintenance hell.

**Why it happens:** Tauri is "not meant for web platform" (GitHub #11347). `window.__TAURI_INTERNALS__` is undefined in web contexts, causing runtime errors. Developers take the path of least resistance: `if (window.__TAURI__) { /* desktop code */ } else { /* web fallback */ }` scattered throughout codebase. Research shows 800% increase in React code duplication since 2023.

**How to avoid:**
- Create platform abstraction layer at project start: `FileSystem`, `Dialog`, `Menu` interfaces
- Desktop implementation uses Tauri APIs (`@tauri-apps/plugin-fs`)
- Web implementation uses File System Access API (with fallback to input/download)
- Use Vite's `TAURI_ENV_PLATFORM` for build-time conditional compilation, not runtime checks
- Tree-shake unused platform code via Vite's dead code elimination
- Write integration tests for both platforms in CI

**Warning signs:**
- Direct imports of `@tauri-apps/api` in business logic (not abstraction layer)
- Runtime platform detection (`if (window.__TAURI__)`) instead of build-time
- Features working in desktop but "not implemented" stubs in web
- Complaints that "web version is always behind desktop"
- Bundle size includes both Tauri and web APIs

**Phase to address:** v1.1 Phase 3 (Tauri Desktop Foundation) must establish abstraction layer BEFORE implementing any Tauri-specific features. Phase 4 (Web Deployment) validates abstraction works.

**Sources:**
- [Tauri IPC Issue #11347](https://github.com/tauri-apps/tauri/issues/11347)
- [Code Duplication: Best Tools 2026](https://www.codeant.ai/blogs/stop-code-duplication-developers-guide)
- [Tauri Environment Variables](https://v2.tauri.app/reference/environment-variables/)

---

### Pitfall 9: Monaco Editor Bundle Size Explosion (4MB+ Initial Load)

**What goes wrong:** Developer imports Monaco Editor with default configuration. Initial bundle size jumps from ~500KB to 4.5MB. Web deployment becomes unusable on slow connections. Even with gzip, first meaningful paint takes 10+ seconds on 3G. Users abandon the app before the editor loads.

**Why it happens:** Monaco Editor includes support for 40+ languages by default, each with its own parser, syntax highlighter, and web worker. The `monaco-editor-webpack-plugin` was archived in November 2023, leaving developers without clear optimization path. Developers miss that "workers feature includes language web workers, and if not set you will have to provide them manually or accept a heavy performance penalty."

**How to avoid:**
- Use `vite-plugin-monaco-editor` with explicit language worker configuration
- For .cypcb files, only include: `['editorWorkerService']` (no language-specific workers needed)
- Implement custom syntax highlighting via Tree-sitter (already in codebase for LSP)
- Lazy load Monaco: `const Monaco = lazy(() => import('./MonacoEditor'))`
- Code split Monaco bundle separately: Vite automatically chunks at dynamic import boundaries
- Compress workers with Brotli (50% size reduction according to WASM optimization docs)
- Set performance budget in CI: fail build if Monaco chunk exceeds 500KB compressed
- Consider CDN loader for Monaco workers instead of bundling

**Warning signs:**
- Initial JavaScript bundle exceeds 2MB uncompressed
- Monaco included in main bundle instead of async chunk
- All language workers loading (CSS, HTML, JSON, TypeScript) when only custom language needed
- No dynamic imports for Monaco components
- Build time increases by 30+ seconds due to Monaco processing
- Network tab shows 40+ worker files loading on editor mount

**Phase to address:** v1.1 Phase 5 (Monaco Editor Integration) must implement lazy loading and worker optimization from day one. Phase 6 (Performance) validates bundle budgets.

**Sources:**
- [Monaco Editor Bundle Size Issue #3518](https://github.com/microsoft/monaco-editor/issues/3518)
- [vite-plugin-monaco-editor](https://github.com/vdesjs/vite-plugin-monaco-editor)
- [Understanding Monaco's web worker architecture](https://app.studyraid.com/en/read/15534/540352/understanding-monacos-web-worker-architecture)

---

### Pitfall 10: Dark Mode Theme Inconsistency Across Surfaces

**What goes wrong:** Developer implements dark mode for application UI using CSS custom properties. Monaco Editor, embedded PDFs, 3D previews, and external library components remain light-themed. Result is jarring "flashbang" effect when switching tabs. User-generated content (imported footprints with hardcoded colors) doesn't respect theme. Application feels unpolished and "stitched together."

**Why it happens:** Each subsystem has its own theming API: Monaco uses `setTheme()`, Canvas/wgpu needs shader recompilation, Three.js scene background, CSS custom properties. Developer implements theme toggle that only updates CSS vars. JavaScript executes after CSS, causing "flash of incorrect theme" (FOIT). Third-party embedded media (footprint previews) have varying backgrounds that clash with dark mode.

**How to avoid:**
- Create central theme manager that coordinates all subsystems:
  ```typescript
  ThemeManager.setTheme('dark', {
    updateCSS: () => document.documentElement.dataset.theme = 'dark',
    updateMonaco: () => monaco.editor.setTheme('vs-dark'),
    updateCanvas: () => renderer.setBackgroundColor(0x1e1e1e),
    updateThreeJS: () => scene.background = new THREE.Color(0x1e1e1e)
  })
  ```
- Save theme preference to localStorage before applying to prevent FOIT
- Inject theme CSS in `<head>` before body content loads
- Define color palette as design tokens shared across all systems
- Test all UI states in both themes (hover, active, disabled, error)
- For user-generated content, apply CSS filters or overlays to blend with theme
- Validate theme consistency with automated screenshot tests

**Warning signs:**
- Theme toggle only updates CSS variables, not Monaco/Canvas/Three.js
- White flash visible when page loads in dark mode
- User preferences not persisting across sessions
- Some UI components use hardcoded colors instead of theme tokens
- Third-party components (library preview cards) have light backgrounds in dark mode
- Accessibility contrast ratios fail in one theme but not the other

**Phase to address:** v1.1 Phase 2 (Dark Mode & UI Polish) must establish theme architecture. All subsequent phases must validate theme consistency for new features.

**Sources:**
- [Dark Mode: Users Think About It and Issues to Avoid](https://www.nngroup.com/articles/dark-mode-users-issues/)
- [A Complete Guide to Dark Mode on the Web](https://css-tricks.com/a-complete-guide-to-dark-mode-on-the-web/)

---

### Pitfall 11: File System API Mismatches Breaking Project Persistence

**What goes wrong:** Desktop Tauri build saves projects using native file system (`@tauri-apps/plugin-fs`). Users expect "Open Recent" menu, file watchers, and auto-save. Web version uses File System Access API (Chrome) or falls back to download/upload pattern (Firefox). Recent files list breaks, auto-save downloads 30 files, file watchers don't work. Users lose work because "Save" behavior is unpredictable across platforms.

**Why it happens:** Developer assumes Tauri's file system semantics apply everywhere. Tauri allows "path traversal prevention" but assumes full file system access to project directories. Web's File System Access API requires explicit user permission per directory, no persistent access across sessions (except with permission prompts). Firefox/Safari don't support File System Access API at all, requiring completely different code path.

**How to avoid:**
- Design file persistence API around most restricted platform (web), then enhance for desktop:
  ```typescript
  interface ProjectPersistence {
    // Works everywhere (download/upload pattern)
    exportProject(): Promise<Blob>
    importProject(file: File): Promise<void>

    // Enhanced for platforms with persistent access
    saveToFileSystem?(): Promise<void>
    enableAutoSave?(): Promise<void>
  }
  ```
- Web: Use IndexedDB for auto-save, export to download when user clicks "Save"
- Desktop: Use native file system with file watchers and auto-save
- Show platform-appropriate UI: "Download" button on web, "Save" on desktop
- Warn users on web that "unsaved changes" means "not downloaded"
- Test with all browser combinations: Chrome (FS Access API), Firefox (download), Safari

**Warning signs:**
- "Open Recent" menu exists in Tauri but not web
- Auto-save triggers downloads in browser
- No warning when closing browser tab with unsaved changes
- Code assumes persistent file handles work everywhere
- File watchers attempted in web context (always fails)
- Error handling missing for permission denials

**Phase to address:** v1.1 Phase 3 (Tauri Desktop Foundation) must NOT assume native file semantics are universal. Phase 4 (Web Deployment) must validate degraded-but-functional persistence.

**Sources:**
- [Tauri File System Plugin](https://v2.tauri.app/plugin/file-system/)
- [Tauri Discussion #6941: Detect desktop mode](https://github.com/tauri-apps/tauri/discussions/6941)

---

### Pitfall 12: Library Version Drift Without Dependency Locking

**What goes wrong:** Team imports KiCad library version 6.0.7. Three months later, new developer clones project, imports "latest" KiCad library (now 6.0.10). Footprints have subtle changes (pad sizes adjusted for manufacturability). DRC passes on old version, fails on new version. Manufacturing files generated from different library versions are incompatible. Git shows no diff because library isn't version-controlled.

**Why it happens:** Industry best practice is "work with local copy, push to centralized repo with version control" but this assumes single source of truth. CodeYourPCB wants to support multiple library sources (KiCad, JLCPCB, custom). Developer implements "import from URL" feature without capturing version/commit hash. Users assume "R_0805 is R_0805" universally when in reality standards evolve (IPC-7351C released in 2022 changed footprint calculations).

**How to avoid:**
- Lock library dependencies in project manifest (similar to package.json):
  ```json
  {
    "libraries": {
      "kicad-official": {
        "source": "https://gitlab.com/kicad/libraries/kicad-footprints",
        "version": "6.0.7",
        "commit": "abc123def456",
        "imported": "2026-01-15"
      }
    },
    "components": {
      "R1": { "footprint": "kicad-official::R_0805", "library_version": "6.0.7" }
    }
  }
  ```
- Version control the imported footprints directory, not just references
- Warn when component references library version different from project lock
- Provide `library update` command that shows diff before updating
- Document library provenance: source URL, import date, commit hash, checksums

**Warning signs:**
- Project file references footprints by name only, no version info
- Library import doesn't record source metadata
- Different developers get different DRC results on same design
- "Works on my machine" syndrome for PCB validation
- No way to reproduce historical builds (manufacturability regressions)

**Phase to address:** v1.1 Phase 1 (Library Management Foundation) must implement versioned library manifest. Phase 7 (Documentation) must document library update workflow.

**Sources:**
- [Best practice for version controlling PCB design](https://www.embeddedrelated.com/thread/9643/best-practice-for-version-controlling-of-a-pcb-design)
- [PCB Footprint Creation Guidelines](https://www.ultralibrarian.com/2024/02/13/pcb-footprint-creation-guidelines-avoid-redundant-library-demands-ulc/)

---

## Technical Debt Patterns

Mistakes that don't break things immediately but accumulate into larger problems.

### Pattern 1: Hardcoded Manufacturing Assumptions

**What goes wrong:** Tool assumes all manufacturers have same capabilities. Users can't specify their manufacturer's constraints. Designs pass DRC but are rejected by fab.

**Why it happens:**
- Using "typical" values from one manufacturer
- Not exposing constraint customization
- Assuming "smaller is always harder" (not always true)

**Prevention:**
- Make DRC rules data-driven from manufacturer capability files
- Provide presets for common manufacturers (JLCPCB, PCBWay, OSH Park)
- Allow custom constraint profiles
- Validate against manufacturer's stated capabilities, not guesses

**Phase mapping:** Phase 2 (Core Features) - DRC constraint system design

---

### Pattern 2: Units Confusion

**What goes wrong:** Mix of mm, mils, inches throughout codebase. Conversion errors in calculations. User specifies 10mm, gets 10mil trace.

**Why it happens:**
- Different PCB conventions (US uses mils, metric uses mm)
- Copy-pasting code without checking units
- No type safety on dimensional values

**Prevention:**
- Single internal unit (nanometers as integers)
- Type-safe dimensions: `struct Millimeters(i64)`, `struct Mils(i64)`
- Explicit conversion functions, no bare number arithmetic
- Display units configurable per-user, stored values always canonical

**Phase mapping:** Phase 1 (Foundation) - data model types

---

### Pattern 3: Monolithic Component Model

**What goes wrong:** Component = one giant struct with all possible fields. Simple resistor carries baggage for BGA-specific fields. Hard to extend for new component types.

**Why it happens:**
- Started with simple components, added fields as needed
- Fear of "too many types"
- Not using ECS or composition patterns

**Prevention:**
- ECS architecture for component model (brainstorm.md already plans this)
- Composition: `Position + Footprint + NetConnections + OptionalFields`
- Components are entities with attached component-data, not inheritance hierarchies

**Phase mapping:** Phase 1 (Foundation) - data model architecture

---

### Pattern 4: Schematic-Layout Desync

**What goes wrong:** For code-first tools, the "schematic" view and "layout" view show different information. Net connections don't match. Users trust the wrong view.

**Why it happens:**
- Schematic and layout are separate rendering paths
- No single source of truth enforced
- Round-trip sync is genuinely hard

**Prevention:**
- Single authoritative data model that both views query
- No separate "schematic netlist" and "layout netlist"
- Test: modify data model, verify both views update consistently
- Consider: schematic view is optional/generated, not authoritative

**Phase mapping:** Phase 4 (Full Experience) - when adding schematic view

**Sources:**
- [SKiDL Discussion on Schematic Generation](https://github.com/devbisme/skidl/discussions/129) - "JitX has a Sr Software engineer dedicated to just schematic generation so it's got to be a hard problem"

---

### Pattern 5: Platform Abstraction Shortcuts (v1.1)

**What goes wrong:** Skip platform abstraction layer, use `if (window.__TAURI__)` checks scattered throughout business logic. Desktop and web codebases diverge with 800% code duplication. Feature parity becomes impossible to maintain.

**Why it happens:** Abstraction feels like overhead early on. Runtime checks seem simpler than build-time configuration. Pressure to ship features quickly leads to copy-paste solutions for each platform.

**Prevention:**
- Never allow direct Tauri API imports in business logic (enforce with linter)
- Create abstraction layer before implementing first platform-specific feature
- Use build-time conditional compilation (Vite's `TAURI_ENV_PLATFORM`) not runtime checks
- Write integration tests that run on both platforms
- Code review checklist: "Does this code work on both web and desktop?"

**Phase mapping:** v1.1 Phase 3 (Tauri Desktop) - establish abstraction BEFORE feature work

---

### Pattern 6: Monaco Bundle Bloat Through Lazy Configuration (v1.1)

**What goes wrong:** Include Monaco with default configuration "to get it working." Bundle size jumps to 4MB. "We'll optimize later" becomes "users complain about slow load times." Web deployment becomes unusable for users on slow connections.

**Why it happens:** Monaco optimization requires understanding workers, language services, and code splitting. Default configuration includes 40+ languages. Developer focuses on functionality first, considers performance "later." Performance debt harder to pay down after features ship.

**Prevention:**
- Configure `vite-plugin-monaco-editor` with minimal workers from day one
- Set performance budget in CI before Monaco integration
- Lazy load Monaco only when user clicks "edit code"
- Use Tree-sitter for syntax highlighting (already in codebase)
- Measure bundle size impact before merging Monaco PR

**Phase mapping:** v1.1 Phase 5 (Monaco Integration) - optimization is not optional

---

## Performance Traps

Patterns that seem fine but kill performance at scale.

### Trap 1: Recalculating Copper Pours on Every Edit

**What goes wrong:** Moving a component triggers full copper pour recalculation. Board with multiple pours becomes unusable in real-time editing.

**Why it happens:**
- Pour geometry depends on component positions
- Naive implementation: any change = full recalc
- Polygon operations are expensive

**Prevention:**
- Lazy pour evaluation (mark dirty, recalc on demand)
- Zone-based dirty tracking (only recalc pours in affected zone)
- Background pour calculation with preview
- Cache pour geometry with invalidation

---

### Trap 2: String Comparisons for Net Matching

**What goes wrong:** Net connectivity checks use string comparison for net names. Performance degrades O(n * string_length) and is allocation-heavy.

**Why it happens:**
- Net names are strings in the source file
- Easy to compare `net1.name == net2.name`
- Works fine for small designs

**Prevention:**
- Intern net names to numeric IDs at parse time
- Compare IDs (integer comparison) not names
- Only convert back to strings for display

---

### Trap 3: Full Board Re-render on Viewport Change

**What goes wrong:** Panning or zooming renders entire board. Large boards drop to single-digit FPS during navigation.

**Why it happens:**
- Simple render loop: "for each object, draw"
- No culling, no level-of-detail
- Canvas/WebGL state thrashing

**Prevention:**
- Viewport culling (only render visible objects using spatial index)
- Level-of-detail rendering (simplify geometry when zoomed out)
- Batch similar draw calls
- Dirty rectangle tracking (only redraw changed regions)

---

### Trap 4: Loading All Footprint Thumbnails Eagerly (v1.1)

**What goes wrong:** Smooth experience with 10 components, gradually slower imports as library grows. Once library hits 500+ footprints, library browser becomes unusable with 10+ second load times.

**Why it happens:** Rendering footprint thumbnails is expensive (SVG parsing, canvas rendering). Loading all thumbnails upfront seems simple. Performance acceptable during initial development with small test library.

**Prevention:**
- Virtualize library browser (only render visible rows)
- Lazy-load thumbnails on scroll with Intersection Observer
- Pre-generate thumbnail sprites for common components
- Cache rendered thumbnails in IndexedDB

**Phase mapping:** v1.1 Phase 1 (Library Management) - virtualization from day one

**Sources:**
- [High-Performance Web Apps in 2026](https://letket.com/high-performance-web-apps-in-2026-webassembly-webgpu-and-edge-architectures/)

---

### Trap 5: Synchronous File Writes on Every Edit (v1.1)

**What goes wrong:** Instant feedback for small files, but files >10MB cause UI freezes. Rapid editing (10+ edits/second) triggers excessive I/O, degrading performance.

**Why it happens:** Auto-save feature implemented naively without debouncing. Each edit immediately writes to disk. Works fine during initial testing with small example projects.

**Prevention:**
- Debounce auto-save (5 second delay)
- Use optimistic UI (show changes immediately, persist asynchronously)
- Write to temp location first, atomic rename on success
- IndexedDB for web (async by default), native fs for desktop

**Phase mapping:** v1.1 Phase 4 (Web Deployment) - persistence strategy design

---

## UX Pitfalls

Design decisions that hurt user adoption.

### Pitfall 1: "Code-First" Means "No Visual Feedback"

**What goes wrong:** Tool requires writing code with no visual preview. Users can't see what they're building. Traditional EDA users bounce immediately.

**Why it happens:**
- "Code-first" interpreted as "code-only"
- Visual preview is hard, deferred
- Underestimating importance of visual feedback for spatial tasks

**Consequences:**
- Steep learning curve
- Users can't validate their understanding
- No immediate gratification
- Adoption limited to command-line enthusiasts

**Prevention:**
- Hot-reload visual preview is MVP, not "nice to have"
- "Code-first" means code is source of truth, not that code is only interface
- Preview updates on every file save (or faster with file watching)

**Phase mapping:** Phase 1 (Foundation) - file watching and basic renderer are core MVP

---

### Pitfall 2: Component Library Chicken-and-Egg

**What goes wrong:** Tool requires components, but has no library. Users must create every footprint from scratch. Barrier to first successful design is weeks of work.

**Why it happens:**
- "We'll build the library later"
- Underestimating library importance
- Not realizing users won't create their own

**Consequences:**
- Users can't build anything practical
- First experience is frustration
- Abandoned after "hello world" attempt

**Prevention:**
- Import KiCad footprint libraries from day one (PROJECT.md already plans this)
- Ship with curated set of common components (0805, QFP, SOT-23, etc.)
- Make footprint import/creation as easy as possible
- Provide procedural footprint generation for common patterns

**Phase mapping:** Phase 5 (Advanced) - but basic import earlier

---

### Pitfall 3: Error Messages Without Location

**What goes wrong:** "Invalid net connection" with no file/line information. Users can't find the problem in a 500-line design file.

**Why it happens:**
- Errors generated after parsing loses source location
- Not threading source spans through compilation
- Easier to just print the error message

**Prevention:**
- Every AST node carries source span
- Errors include file:line:column
- Provide code snippet context in error messages
- LSP diagnostics with precise ranges (Phase 4)

**Phase mapping:** Phase 1 (Foundation) - Tree-sitter preserves locations; maintain them through pipeline

---

### Pitfall 4: Learning HDL-Like Syntax as a PCB Designer

**What goes wrong:** Target users are PCB designers, not programmers. HDL-style syntax with functions, loops, and imports is unfamiliar. Users give up before understanding the paradigm.

**Why it happens:**
- Tool built by programmers for programmers
- Assuming PCB designers want to learn programming
- Not providing gradual on-ramp

**Consequences:**
- Adoption limited to software engineers doing hobby electronics
- Professional PCB designers stick with traditional tools
- Market constrained unnecessarily

**Prevention:**
- Simplest designs require minimal syntax (just component placement)
- Advanced features (loops, functions) are opt-in for power users
- Excellent error messages guide users to correct syntax
- Examples for every common task
- AI-assisted editing lowers barrier (user describes intent, AI writes syntax)

**Sources:**
- [HDL Learning Curve Challenges](https://www.sciencedirect.com/science/article/abs/pii/S0950584923000502) - "HDLs have a steep learning curve for beginners"
- [PCB HDL Adoption Challenges](https://ducky64.github.io/HATRA20_PCB_HDLs.pdf) - "While some learning curve is inevitable, flattening it as much as possible is necessary"

---

### Pitfall 5: Modal "Importing Library" Dialog Blocking UI (v1.1)

**What goes wrong:** User can't continue working while waiting for 50MB library download. Modal dialog blocks entire UI. User frustration as they're forced to watch progress bar instead of working.

**Why it happens:** Simple implementation uses modal dialog with progress bar. Synchronous download easier to implement than background task. Seems acceptable during testing with small libraries.

**Prevention:**
- Background import with notification system
- Allow continued editing during library download
- Show progress in status bar, not modal dialog
- Queue multiple library imports concurrently

**Phase mapping:** v1.1 Phase 1 (Library Management) - background tasks from start

**Sources:**
- [Dark Mode: Users Think About It and Issues](https://www.nngroup.com/articles/dark-mode-users-issues/) - Discusses interruption patterns

---

### Pitfall 6: Theme Toggle Without Preference Persistence (v1.1)

**What goes wrong:** User sets dark mode, refreshes page, back to light mode. Eye strain from unexpected light theme. User has to toggle dark mode on every visit.

**Why it happens:** Theme toggle updates UI state but doesn't persist to localStorage. Developer forgets that web apps don't automatically preserve state. No respect for `prefers-color-scheme` media query.

**Prevention:**
- Save theme preference to localStorage immediately on toggle
- Load theme preference before first paint (prevent FOIT)
- Respect `prefers-color-scheme` as default
- Apply theme synchronously in `<head>` before body loads

**Phase mapping:** v1.1 Phase 2 (Dark Mode) - persistence is day one requirement

**Sources:**
- [A Complete Guide to Dark Mode on the Web](https://css-tricks.com/a-complete-guide-to-dark-mode-on-the-web/)

---

### Pitfall 7: Desktop and Web Versions Behave Differently (v1.1)

**What goes wrong:** User expects "File → Open" on web but it doesn't exist. Desktop has "Open Recent" menu, web doesn't. Confusion about platform differences. Support burden from "this feature doesn't work" reports.

**Why it happens:** Platform capabilities differ (native file system vs browser security model). Developer implements features where possible, leaving gaps elsewhere. No unified design for cross-platform workflows.

**Prevention:**
- Keep workflows similar across platforms with appropriate labels: "Open" vs "Import from file"
- Show platform-appropriate UI but consistent mental model
- Document platform differences in help system
- Use same keyboard shortcuts where possible
- Graceful degradation, not "feature missing" error messages

**Phase mapping:** v1.1 Phase 4 (Web Deployment) - cross-platform UX design

---

### Pitfall 8: Auto-Save Download Spam on Web (v1.1)

**What goes wrong:** User makes 10 edits, gets 10 download prompts. Disables auto-save to stop spam. Loses work when browser crashes. Bad experience drives users away from web version.

**Why it happens:** Desktop auto-save pattern (write to file system) naively ported to web. Web's security model prevents silent file writes. Each auto-save triggers download dialog.

**Prevention:**
- Auto-save to IndexedDB silently (no download prompt)
- Show "export to file" action in menu (user-initiated)
- Warn on tab close if IndexedDB has unsaved changes
- Desktop gets real auto-save, web gets background persistence
- Clear messaging: "Auto-save keeps your work safe in browser storage"

**Phase mapping:** v1.1 Phase 4 (Web Deployment) - persistence UX design

**Sources:**
- [Tauri Discussion #6941: Detect desktop mode](https://github.com/tauri-apps/tauri/discussions/6941)

---

## "Looks Done But Isn't" Checklist

Features that appear complete but have subtle gaps.

### Gerber Export
- [ ] Board outline included (separate file or in drill file)
- [ ] Drill file units match Gerber units
- [ ] Drill file zero-suppression consistent
- [ ] Flash pads, not vector pads for SMD
- [ ] Contour fills, not vector fills for copper pours
- [ ] Aperture list is single, not multiple
- [ ] Tested with gerbv, KiCad viewer, manufacturer DFM
- [ ] Edge cuts geometry actually in file (KiCad bug reference)
- [ ] Coordinate system matches (Y-up vs Y-down handled)

### Autorouter Integration
- [ ] Results are deterministic (same input = same output)
- [ ] Routing respects all design rules
- [ ] Via placement follows design rules
- [ ] Differential pairs handled correctly
- [ ] Length matching constraints respected
- [ ] Error reporting is actionable (not just "routing failed")
- [ ] Partial routing success doesn't corrupt board

### DRC Implementation
- [ ] Scales to 1000+ components without hanging
- [ ] Clearance checking handles all object types (trace-trace, trace-pad, pad-pad, trace-zone)
- [ ] Net-aware (same-net objects don't violate clearance)
- [ ] Via annular ring checking
- [ ] Drill-to-copper clearance
- [ ] Silk-to-pad clearance
- [ ] Thermal relief verification for plane connections
- [ ] Min trace width per net class
- [ ] Differential pair spacing and skew
- [ ] Results are deterministic

### Component Library Import
- [ ] KiCad S-expression footprints parse correctly
- [ ] 3D models referenced (even if not rendered yet)
- [ ] Pin names preserved
- [ ] Pad types (SMD, TH, NPTH) handled
- [ ] Custom pad shapes supported
- [ ] Courtyard/silkscreen layers imported
- [ ] Units conversion correct (KiCad uses mm)

### DSL Parser
- [ ] Error recovery (partial parse of invalid file)
- [ ] Source locations preserved through to error messages
- [ ] Comments preserved for round-trip (if editing support planned)
- [ ] Unicode handling (component names, user strings)
- [ ] Large file performance (1000+ lines)
- [ ] Incremental parsing works

### File Format
- [ ] Version number in every file
- [ ] Forward migration path defined
- [ ] Round-trip test (parse-save-parse produces identical result)
- [ ] Handles missing optional fields (backward compat)
- [ ] Warns on unknown fields (forward compat)
- [ ] Special characters in strings escaped correctly

### v1.1 Integration Checklist

#### Library Management
- [ ] Namespace-prefixed imports prevent conflicts
- [ ] Library source metadata stored with footprints
- [ ] Conflict detection shows side-by-side comparison
- [ ] Version locking in project manifest
- [ ] Library update shows diff before applying

#### Platform Abstraction (Tauri/Web)
- [ ] No direct Tauri API imports in business logic
- [ ] FileSystem, Dialog, Menu interfaces abstract platform
- [ ] Build-time conditional compilation (not runtime checks)
- [ ] Integration tests run on both platforms
- [ ] Feature parity validated in CI

#### Monaco Integration
- [ ] Monaco chunk <500KB compressed
- [ ] Lazy loaded with dynamic import
- [ ] Only editorWorkerService included (no language workers)
- [ ] Tree-sitter used for syntax highlighting
- [ ] Performance budget enforced in CI

#### Dark Mode
- [ ] Theme manager coordinates CSS, Monaco, Canvas, Three.js
- [ ] No FOIT (flash of incorrect theme)
- [ ] Preference persists to localStorage
- [ ] Respects prefers-color-scheme
- [ ] All UI states tested in both themes

#### File Persistence
- [ ] Web: IndexedDB auto-save, export to download
- [ ] Desktop: Native file system with watchers
- [ ] Platform-appropriate UI (Download vs Save)
- [ ] Warning on tab close for unsaved web changes
- [ ] Graceful degradation documented

#### Performance
- [ ] Footprint thumbnails lazy-loaded with Intersection Observer
- [ ] Library browser virtualized (only visible rows)
- [ ] Auto-save debounced (5s delay)
- [ ] Bundle size budgets enforced

---

## Phase-Specific Warnings

| Phase | Likely Pitfall | Mitigation |
|-------|---------------|------------|
| Phase 1: Foundation | DSL syntax lock-in | Minimal grammar, version from start |
| Phase 1: Foundation | Floating-point in data model | Integer nanometers, convert at edges |
| Phase 2: Core Features | DRC performance cliff | Spatial indexing from day one |
| Phase 2: Core Features | Gerber edge cases | Test against multiple viewers/manufacturers |
| Phase 3: Intelligence | Autorouter non-determinism | Seed randomness, deterministic data structures |
| Phase 4: Full Experience | Schematic-layout desync | Single source of truth |
| Phase 5: Advanced | Component library chicken-egg | KiCad import as MVP priority |
| All Phases | File format breaking changes | Version in files, migration tests |

### v1.1 Phase-Specific Warnings

| Phase | Likely Pitfall | Mitigation |
|-------|---------------|------------|
| v1.1 Phase 1: Library Management | Namespace conflicts | Prefix imports, detect conflicts at import time |
| v1.1 Phase 1: Library Management | Version drift | Lock library versions in manifest |
| v1.1 Phase 1: Library Management | Eager thumbnail loading | Virtualize browser, lazy-load on scroll |
| v1.1 Phase 2: Dark Mode | Theme inconsistency | Central theme manager for all surfaces |
| v1.1 Phase 2: Dark Mode | FOIT on page load | Apply theme before body renders |
| v1.1 Phase 3: Tauri Desktop | Feature parity drift | Abstraction layer before Tauri features |
| v1.1 Phase 3: Tauri Desktop | Runtime platform checks | Build-time conditional compilation |
| v1.1 Phase 4: Web Deployment | File system mismatch | IndexedDB persistence, export to download |
| v1.1 Phase 4: Web Deployment | Auto-save download spam | Silent IndexedDB save, manual export |
| v1.1 Phase 5: Monaco Integration | Bundle size explosion | vite-plugin-monaco-editor, lazy load, minimal workers |
| v1.1 Phase 5: Monaco Integration | Worker misconfiguration | Test Network tab for 404s |

---

## Sources

### DSL Design
- [Martin Fowler DSL Q&A](https://martinfowler.com/bliki/DslQandA.html)
- [DSL Evolution InfoQ](https://www.infoq.com/articles/dsl-evolution/)
- [DSL Best Practices LinkedIn](https://www.linkedin.com/advice/0/how-do-you-evolve-dsl-java-without-breaking-backward-compatibility)
- [Tonsky DSL Design](https://tonsky.me/blog/dsl/)

### PCB/EDA Specific
- [KiCad Coordinate System](https://forum.kicad.info/t/coordinate-system-grid-and-origins-in-the-pcb-editor/24535)
- [KiCad File Compatibility](https://forum.kicad.info/t/backward-and-forward-compatibility/45234)
- [Common Gerber Issues - Bittele](https://www.7pcb.com/blog/common-gerber-issues-how-to-fix-them)
- [Common Gerber Problems - Sierra Circuits](https://www.protoexpress.com/blog/common-problems-associated-with-gerber-files/)
- [PCB Design Mistakes - Cadence](https://resources.pcb.cadence.com/blog/2025-common-pcb-design-mistakes)
- [Why PCB Autorouting Remains Broken](https://autocuro.com/blog/why-pcb-autorouting-remains-broken)

### Code-First PCB Tools
- [SKiDL Discussions](https://github.com/devbisme/skidl/discussions/)
- [JITX Documentation](https://docs.jitx.com/)
- [tscircuit](https://tscircuit.com/)
- [PCB HDL Research Paper](https://ducky64.github.io/HATRA20_PCB_HDLs.pdf)

### Numerical/Geometric
- [Mitigating Floating Point Errors - Medium](https://medium.com/@moiserushanika2006/mitigating-floating-point-errors-in-computational-geometry-algorithms-a62525da45ef)
- [Floating Point Precision - LinkedIn](https://www.linkedin.com/advice/1/how-can-you-prevent-floating-point-errors-computational-hcmoe)

### Performance
- [PDRC: GPU-Accelerated DRC - CUHK](http://www.cse.cuhk.edu.hk/~byu/papers/C219-DAC2024-PDRC.pdf)
- [Routing Algorithms - Wikipedia](https://en.wikipedia.org/wiki/Routing_(electronic_design_automation))

### v1.1 Integration Sources

**Tauri:**
- [Tauri 2.0 Stable Release](https://v2.tauri.app/blog/tauri-20/)
- [Tauri File System Plugin](https://v2.tauri.app/plugin/file-system/)
- [Tauri IPC Architecture](https://v2.tauri.app/concept/inter-process-communication/)
- [Tauri Environment Variables](https://v2.tauri.app/reference/environment-variables/)
- [GitHub Issue #11347: HTTP plugin fallback](https://github.com/tauri-apps/tauri/issues/11347)
- [GitHub Discussion #6941: Detect desktop mode](https://github.com/tauri-apps/tauri/discussions/6941)

**Monaco Editor:**
- [Monaco Bundle Size Issue #97](https://github.com/microsoft/monaco-editor-webpack-plugin/issues/97)
- [Monaco Issue #3518: Import adds ALL to bundle](https://github.com/microsoft/monaco-editor/issues/3518)
- [vite-plugin-monaco-editor](https://github.com/vdesjs/vite-plugin-monaco-editor)
- [Understanding Monaco's web worker architecture](https://app.studyraid.com/en/read/15534/540352/understanding-monacos-web-worker-architecture)
- [Configuring Monaco workers](https://app.studyraid.com/en/read/15534/540353/configuring-monaco-workers-for-optimal-performance)

**Library Management:**
- [Managing PCB Footprint Libraries - Altium 365](https://resources.altium.com/p/managing-pcb-footprint-libraries-with-altium-365)
- [Centralized Component Libraries Best Practices](https://resources.altium.com/p/centralized-component-libraries-best-practices-hardware-teams)
- [PCB Footprint Creation Guidelines - Ultra Librarian](https://www.ultralibrarian.com/2024/02/13/pcb-footprint-creation-guidelines-avoid-redundant-library-demands-ulc/)
- [Best practice for version controlling PCB design](https://www.embeddedrelated.com/thread/9643/best-practice-for-version-controlling-of-a-pcb-design)

**Web Performance:**
- [The State of WebAssembly 2025-2026](https://platform.uno/blog/the-state-of-webassembly-2025-2026/)
- [High-Performance Web Apps in 2026](https://letket.com/high-performance-web-apps-in-2026-webassembly-webgpu-and-edge-architectures/)
- [Optimizing WASM Binary Size](https://book.leptos.dev/deployment/binary_size.html)
- [Code-splitting and minimal edge latency](https://www.fastly.com/blog/code-splitting-and-minimal-edge-latency-the-perfect-match)

**Dark Mode:**
- [Dark Mode: Users Issues - NN/G](https://www.nngroup.com/articles/dark-mode-users-issues/)
- [Complete Guide to Dark Mode - CSS-Tricks](https://css-tricks.com/a-complete-guide-to-dark-mode-on-the-web/)
- [Dark Side of Dark Mode - Vareweb](https://vareweb.com/blog/the-dark-side-of-dark-mode-in-web-design/)

**Code Quality:**
- [Code Duplication Best Tools 2026](https://www.codeant.ai/blogs/stop-code-duplication-developers-guide)
- [Code Rot Vs Code Gen 2025-2026](https://fullstacktechies.com/code-rot-vs-code-gen-ai-react-strategy/)
- [DRY Principle in AI-Generated Code](https://www.faros.ai/blog/ai-generated-code-and-the-dry-principle)

---
*Pitfalls research for: CodeYourPCB (general domain + v1.1 integration challenges)*
*Updated: 2026-01-29 for v1.1 milestone*

# atopile vs CodeYourPCB — porównanie
*Analiza: 2026-03-08*

## Co atopile ma lepiej niż my

### Język i semantyka DSL
- **Constraint solver** — `assert resistor.resistance within 10kohm +/- 10%` i kompilator sam dobiera części z LCSC. My: zero.
- **System modułów** — pełna hierarchia importów, typy dziedziczą. My: brak.
- **Typowane interfejsy** — `I2C`, `SPI`, `USB`, `ElectricPower` jako pierwsze klasy języka. My: sieci to stringi.
- **Jednostki fizyczne w języku** — `10kohm`, `3.3V`, `100nF` to typy, nie stringi. Kompilator sprawdza dimensionalnie.
- **For-loop po komponentach** — `for r in resistors: r.resistance = 10k`. My: zero.
- **Auto-picking z LCSC** — pisze specyfikację, dostaje realne części z katalogu dostawcy.

### Ekosystem
- **Package registry** — `packages.atopile.io`, community modules, `ato install`
- **VS Code extension** opublikowana w marketplace — jeden klik install
- **MCP server** — `ato mcp`, AI może zarządzać projektem przez protokół
- **Discord + community** — żywa społeczność, lata przewagi

---

## Co my mamy lepiej niż atopile

### Kluczowa różnica strukturalna: atopile to NAKŁADKA na KiCad, my jesteśmy ZAMIENNIKIEM całego stacku

- **Standalone — nie wymaga KiCad** — atopile kompiluje do `.kicad_pcb` i otwiera KiCad do layoutu. My jesteśmy w pełni samodzielni. Bez KiCad atopile jest bezużyteczne.
- **Własny viewer/renderer** — atopile nie ma wbudowanego widoku PCB. My mamy pełny 2D canvas z warstwami, padami, ratsnest, DRC markers.
- **Wbudowany autorouter** — integracja z FreeRouting (DSN/SES). atopile routing = ręcznie w KiCad.
- **Własny DRC** — atopile nie ma własnego DRC, robi to KiCad. My mamy pełny clearance + width + drill + connectivity check.
- **Gerber X2 export natywny** — my generujemy Gerbers bezpośrednio. atopile musi przez KiCad CLI.
- **Web-first / WASM** — działa w przeglądarce bez instalacji. atopile: tylko desktop, wymaga Python + KiCad.
- **Tauri desktop app** — instalowany natywny app, <10MB. atopile: pip install + oddzielny KiCad.
- **Embedded Monaco editor** — edycja + podgląd w jednym oknie przeglądarki.
- **Share URL** — link do designu bezpośrednio w przeglądarce.
- **Trace width calculator (IPC-2221)** — wbudowany.
- **Prostszy onboarding** — atopile wymaga: Python + pip + KiCad + extension. My: otwórz URL.

---

## Podział rynku

| | atopile | CodeYourPCB |
|---|---|---|
| Dla kogo | Inżynier który i tak używa KiCad, chce lepszy schematic capture | Programista który nie chce instalować KiCad w ogóle |
| Layout | W KiCad (GUI) | W naszym viewerze + FreeRouting |
| Routing | Ręcznie w KiCad | FreeRouting (automatyczny) |
| Działa bez instalacji | ❌ | ✅ (przeglądarka) |
| DSL power | Wyższy (constraints, units, modules) | Prostszy, ale kompletny end-to-end |
| Standalone | ❌ | ✅ |

**Wniosek:** Jesteśmy jedynym code-first PCB toolem który jest w pełni samodzielny i działa w przeglądarce. atopile to nakładka na KiCad. My jesteśmy zamiennikiem całego stacku.

# Feature Research - v1.1 Foundation & Desktop

**Milestone:** v1.1 Foundation & Desktop
**Researched:** 2026-01-29
**Confidence:** HIGH

## Feature Landscape

This research focuses on NEW features for v1.1: library management, desktop application, web deployment, and embedded code editor.

### Table Stakes (Users Expect These)

Features users assume exist in professional PCB tools and modern desktop applications.

#### Component Library Management

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Library search | Users need to find components quickly | MEDIUM | Search by name, MPN, value, category |
| Library organization | Users expect logical structure | MEDIUM | By manufacturer, function, custom categories |
| 3D model association | Modern PCB tools show 3D models | MEDIUM | STEP file linking, preview |
| Multiple library sources | KiCad, JLCPCB, custom libs are standard | HIGH | Multi-format import, unified interface |
| Library version control | Footprints change, users need history | MEDIUM | Track library updates, rollback capability |
| Footprint preview | See component before use | LOW | Render footprint in library browser |
| Component metadata | Datasheet links, specs, lifecycle status | LOW | Display in component details panel |
| Library path management | Users have libs in different locations | MEDIUM | Configurable search paths, auto-discovery |

#### Desktop Application

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Native file dialogs | Desktop apps use OS dialogs | LOW | Tauri plugin-dialog provides this |
| Application menus | File/Edit/View standard pattern | LOW | Platform-specific menu bars |
| Window management | Minimize, maximize, fullscreen | LOW | Tauri handles automatically |
| Native notifications | Desktop apps notify users | LOW | DRC completion, export success |
| Installation/updates | Install once, update easily | MEDIUM | MSI/DMG/AppImage packaging |
| Keyboard shortcuts | Ctrl+S, Ctrl+Z expected | LOW | Accelerator key bindings |
| System tray integration | Background running option | LOW | Tauri system tray plugin |
| Multi-window support | Separate editor/viewer windows | MEDIUM | Tauri multi-window API |

#### Web Deployment

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Fast initial load | Users abandon slow sites | HIGH | Code splitting, lazy loading WASM |
| Responsive design | Works on tablets, large screens | MEDIUM | Mobile already works, scale up |
| Browser file access | Open/save local files | LOW | File System Access API |
| Shareable URLs | Share designs via link | MEDIUM | URL-based project loading |
| Offline support | Work without internet | MEDIUM | Service workers, IndexedDB cache |
| HTTPS hosting | Required for PWA features | LOW | Netlify/Vercel handle this |
| Cross-browser support | Chrome, Firefox, Safari, Edge | MEDIUM | WebGPU fallback to WebGL |

#### Embedded Code Editor

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Syntax highlighting | Every code editor has this | MEDIUM | Monaco/Tree-sitter integration |
| Auto-completion | Expected in modern editors | HIGH | LSP integration for context-aware |
| Error highlighting | See syntax errors inline | MEDIUM | Diagnostic display from LSP |
| Line numbers | Standard editor feature | LOW | Monaco provides by default |
| Code folding | Collapse sections | LOW | Based on language structure |
| Find/replace | Basic editing requirement | LOW | Monaco built-in |
| Undo/redo | Expected in any editor | LOW | Monaco handles automatically |
| Multi-cursor editing | Power user feature, now standard | LOW | Monaco provides this |

### Differentiators (Competitive Advantage)

Features that set CodeYourPCB apart from traditional EDA tools.

#### Library Management Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **"Idiot-proof" auto-organization** | Drop libs anywhere, app organizes them | HIGH | AI-powered lib detection and categorization |
| **Multi-source unified search** | Search KiCad + JLCPCB + custom in one query | MEDIUM | Unified index across sources |
| **Supply chain integration** | See stock, pricing, lifecycle status | MEDIUM | API integration with suppliers |
| **Git-friendly library format** | Library changes are version-controlled | LOW | Text-based lib definitions |
| **Component recommendation** | "Similar to X" suggestions | MEDIUM | Based on footprint similarity, usage |
| **Automatic 3D model fetching** | Find and download models automatically | MEDIUM | Integration with 3D model databases |

#### Desktop App Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Tiny bundle size (<10MB)** | Download/install faster than competitors | LOW | Tauri advantage over Electron |
| **Low memory footprint** | Run on older machines | LOW | Rust + OS WebView advantage |
| **Cross-platform consistency** | Same UX on Windows/Mac/Linux | MEDIUM | Tauri handles platform differences |
| **Fast startup (<1s)** | No waiting for Electron/JVM | LOW | Rust native binary |
| **CLI + GUI in one binary** | Developers can script, non-devs can click | MEDIUM | Tauri commands exposed to CLI |

#### Web Deployment Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **No-install sharing** | "Try my design" via URL | MEDIUM | Full viewer in browser |
| **Progressive enhancement** | Works offline after first load | MEDIUM | PWA service worker caching |
| **Instant updates** | No user action needed | LOW | Static site deployment |
| **URL-based projects** | Share exact state via link | MEDIUM | Encode project in URL params |
| **Edge deployment** | 10-20ms global response | LOW | Cloudflare/Vercel Edge |

#### Embedded Editor Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Integrated LSP** | Same autocomplete as VS Code | MEDIUM | Reuse existing LSP server |
| **Live DRC feedback** | See violations as you type | HIGH | Incremental parsing + DRC |
| **AI assistant integration** | "Fix this trace" inline | MEDIUM | LLM API with context injection |
| **Side-by-side preview** | Code on left, board on right | LOW | Layout component arrangement |
| **Error recovery** | Keep working with syntax errors | MEDIUM | Tree-sitter error tolerance |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems for v1.1.

#### Library Management Anti-Features

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Built-in footprint editor** | "Make custom footprints in-app" | Complex GUI, KiCad already excellent | Use KiCad editor, import result |
| **Component marketplace** | "Download parts" | Hosting costs, curation, liability | Integrate existing sources (KiCad, SnapEDA) |
| **Automatic library updates** | "Stay current" | Breaking changes, user surprise | Manual update with changelog review |
| **Cloud library sync** | "Access anywhere" | Privacy, vendor lock-in | Git-based sync, user controls |
| **Library analytics** | "Most used components" | Privacy invasion, complexity | Local-only usage tracking |

#### Desktop App Anti-Features

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Multi-document interface (MDI)** | "Open many projects" | Confusing, crashes affect all | Multiple windows, each isolated |
| **Builtin terminal** | "Run commands in app" | OS terminal is better | External terminal, good integration |
| **Custom window decorations** | "Looks unique" | Platform inconsistency, accessibility | Use native OS window chrome |
| **Splash screen** | "Looks professional" | Slower perceived startup | Fast startup instead |
| **Auto-update without user consent** | "Stay current" | User control, bandwidth surprise | Notify, user initiates update |

#### Web Deployment Anti-Features

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Backend server** | "User accounts" | Hosting costs, maintenance, security | Static site + local storage |
| **Real-time collaboration** | "Like Figma" | Complexity explosion, conflicts | Git-based async workflow |
| **Mobile-first design** | "Touch interface" | Compromises desktop UX | Desktop-first, mobile for viewing |
| **Heavy animations** | "Looks polished" | Performance cost, accessibility | Subtle, purposeful animations |
| **Analytics/tracking** | "Know usage" | Privacy invasion | Optional, opt-in telemetry |

#### Embedded Editor Anti-Features

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **VIM/Emacs bindings** | "I use VIM" | Maintenance burden, incomplete | Use external editor + hot reload |
| **Multiple themes** | "Personalization" | Maintenance, testing overhead | Dark + Light only, system preference |
| **Extensions/plugins** | "Like VS Code" | Security, stability, complexity | Built-in features, external editor option |
| **Git integration** | "Commit from editor" | UI complexity, external tools better | External git client |
| **Minimap** | "Like VS Code" | Memory cost for DSL files | Monaco minimap is 5MB+ overhead |

## Feature Dependencies

### v1.1 Dependency Graph

```
[Existing v1.0 Foundation]
    │
    ├─────────────────────┬──────────────────┬─────────────────┐
    │                     │                  │                 │
    ▼                     ▼                  ▼                 ▼
[Library System]    [Desktop App]    [Web Deploy]    [Embedded Editor]
    │                     │                  │                 │
    ├─► Search Index      ├─► Menus          ├─► PWA Cache     ├─► Monaco Editor
    ├─► 3D Models         ├─► File Dialogs   ├─► Service Wkr   ├─► LSP Client
    ├─► Metadata DB       ├─► Packaging      ├─► URL Routing   ├─► Syntax Theme
    └─► Multi-source      └─► Auto-update    └─► Edge Deploy   └─► Live Preview
```

### Dependency Notes

- **Library System is independent:** Can build without other v1.1 features
- **Desktop App requires Library System:** Desktop needs component selection
- **Embedded Editor requires LSP (v1.0):** Reuses existing language server
- **Web Deploy requires build system:** Separate from desktop packaging
- **Dark Mode affects all:** Theme system must work in editor, viewer, dialogs

### Cross-Feature Dependencies

| Feature A | Depends On | Feature B | Reason |
|-----------|------------|-----------|--------|
| Embedded Editor | → | Library System | Component autocomplete from library |
| Desktop Dialogs | → | Library System | "Add component" dialog needs library |
| Web Deployment | → | Embedded Editor | Browser users need code editing |
| Dark Mode | → | All features | Consistent theme across app |
| 3D Models | → | Library System | Models associated with library components |

## MVP Definition for v1.1

### Launch With (v1.1)

Features needed to deliver "professional desktop experience."

#### Library Management
- [x] **Multi-source library support** — KiCad + JLCPCB + custom
- [x] **Search and filtering** — Find components by name, MPN, category
- [x] **3D model association** — Link STEP files to footprints
- [x] **Library path configuration** — User-specified library locations
- [x] **Footprint preview** — Visual confirmation before use

#### Desktop Application
- [x] **Tauri packaging** — Native installers for Win/Mac/Linux
- [x] **Native file dialogs** — OS-native open/save dialogs
- [x] **Application menus** — Standard File/Edit/View menus
- [x] **Dark mode theme** — System preference support
- [x] **Keyboard shortcuts** — Standard accelerators

#### Web Deployment
- [x] **Static site hosting** — Netlify/Vercel deployment
- [x] **Fast WASM loading** — Optimized bundle size
- [x] **Browser file access** — File System Access API
- [x] **Responsive layout** — Works on tablets and desktops
- [x] **HTTPS by default** — SSL for all deployments

#### Embedded Code Editor
- [x] **Monaco integration** — VS Code editor embedded
- [x] **Syntax highlighting** — .cypcb language support
- [x] **LSP integration** — Autocomplete, hover, diagnostics
- [x] **Side-by-side view** — Code left, board right
- [x] **Error highlighting** — Inline error display

### Add After v1.1 Launch (v1.2)

Features to defer until foundation is solid.

#### Library Management
- [ ] **Supply chain integration** — Stock, pricing, lifecycle
- [ ] **Component recommendations** — "Similar to X" suggestions
- [ ] **Auto 3D model fetching** — Download from databases
- [ ] **Library version control** — Track updates, rollback

#### Desktop Application
- [ ] **Auto-update system** — Background update checks
- [ ] **Multi-window support** — Separate editor/viewer windows
- [ ] **System tray integration** — Background running
- [ ] **Native notifications** — DRC complete, export ready

#### Web Deployment
- [ ] **Offline PWA support** — Service worker caching
- [ ] **URL-based projects** — Share via link with state
- [ ] **Edge deployment** — Global CDN for 10-20ms response
- [ ] **Cross-browser testing** — Firefox, Safari validation

#### Embedded Editor
- [ ] **Live DRC feedback** — See violations as you type
- [ ] **AI assistant integration** — Inline LLM help
- [ ] **Error recovery UI** — Suggestions for syntax errors
- [ ] **Code snippets** — Common patterns library

### Future Consideration (v2+)

Advanced features requiring more research.

- [ ] **AI-powered library organization** — Automatic categorization
- [ ] **CLI + GUI unified binary** — Script the desktop app
- [ ] **Component datasheet viewer** — Built-in PDF display
- [ ] **Library conflict resolution** — Handle duplicate components
- [ ] **Custom editor themes** — Beyond dark/light
- [ ] **Multi-language LSP** — Support other DSLs

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Technical Risk | Priority |
|---------|------------|---------------------|----------------|----------|
| **Library Management** |
| Multi-source support | HIGH | MEDIUM | LOW | **P0** |
| Search/filter | HIGH | MEDIUM | LOW | **P0** |
| 3D model linking | MEDIUM | LOW | LOW | **P1** |
| Supply chain data | HIGH | HIGH | MEDIUM | **P2** |
| Auto organization | MEDIUM | HIGH | MEDIUM | **P3** |
| **Desktop Application** |
| Tauri packaging | HIGH | MEDIUM | LOW | **P0** |
| Native dialogs | HIGH | LOW | LOW | **P0** |
| App menus | HIGH | LOW | LOW | **P0** |
| Dark mode | MEDIUM | MEDIUM | LOW | **P1** |
| Auto-update | MEDIUM | MEDIUM | MEDIUM | **P2** |
| **Web Deployment** |
| Static hosting | HIGH | LOW | LOW | **P0** |
| Fast WASM load | HIGH | MEDIUM | LOW | **P0** |
| File System API | HIGH | LOW | MEDIUM | **P1** |
| PWA offline | MEDIUM | MEDIUM | LOW | **P2** |
| Edge deploy | LOW | LOW | LOW | **P3** |
| **Embedded Editor** |
| Monaco integration | HIGH | MEDIUM | LOW | **P0** |
| Syntax highlighting | HIGH | LOW | LOW | **P0** |
| LSP integration | HIGH | LOW | LOW | **P0** |
| Live DRC | MEDIUM | HIGH | HIGH | **P2** |
| AI assistant | LOW | HIGH | HIGH | **P3** |

**Priority key:**
- P0: Must have for v1.1 launch
- P1: Should have, adds polish
- P2: Nice to have, add in v1.2
- P3: Future consideration

## Competitor Feature Analysis

### Library Management

| Feature | KiCad | Altium | EasyEDA | **CodeYourPCB v1.1** |
|---------|-------|--------|---------|---------------------|
| Library sources | KiCad | Altium | LCSC | **KiCad + JLCPCB + Custom** |
| Organization | Manual folders | Database | Cloud | **Auto-detect + manual** |
| 3D models | Manual link | Integrated | Some | **Manual link (v1.1)** |
| Search | Basic | Advanced | Good | **Full-text + filters** |
| Supply chain | Plugin | Native | LCSC only | **Deferred to v1.2** |
| Version control | Manual | Vault | Cloud | **Git-native** |

### Desktop Application

| Feature | KiCad | Eagle | Altium | **CodeYourPCB v1.1** |
|---------|-------|-------|--------|---------------------|
| Platform | Win/Mac/Linux | Win/Mac/Linux | Win only | **Win/Mac/Linux** |
| Bundle size | 300MB+ | 150MB+ | 2GB+ | **<10MB** |
| Memory usage | 200MB+ | 150MB+ | 500MB+ | **30-40MB** |
| Startup time | 3-5s | 2-3s | 5-10s | **<1s** |
| Native menus | Yes | Yes | Yes | **Yes** |
| Dark mode | Yes | Partial | Yes | **Yes** |

### Web Deployment

| Feature | EasyEDA | Flux.ai | **CodeYourPCB v1.1** |
|---------|---------|---------|---------------------|
| Web access | Cloud-only | Cloud-only | **Static + optional cloud** |
| Offline work | No | No | **Yes (PWA in v1.2)** |
| Installation | None | None | **Optional desktop** |
| Performance | Good | Good | **Excellent (WASM)** |
| Privacy | Cloud | Cloud | **Local-first** |
| Sharing | Built-in | Built-in | **URL-based (v1.2)** |

### Embedded Code Editor

| Feature | Text Editor | External IDE | **CodeYourPCB v1.1** |
|---------|-------------|--------------|---------------------|
| Syntax highlight | Manual | Via plugin | **Built-in** |
| Autocomplete | None | Full LSP | **Full LSP** |
| Live preview | None | Via viewer | **Side-by-side** |
| Error checking | None | Via LSP | **Inline** |
| Learning curve | Low | High | **Medium** |
| Integration | Manual reload | Hot reload | **Instant** |

## Complexity Assessment

### Implementation Complexity by Category

| Category | Overall Complexity | Key Challenges |
|----------|-------------------|----------------|
| **Library Management** | MEDIUM-HIGH | Multi-source unification, metadata management |
| **Desktop Application** | LOW-MEDIUM | Tauri handles most platform complexity |
| **Web Deployment** | LOW | Static site hosting is straightforward |
| **Embedded Editor** | MEDIUM | Monaco integration, LSP WebSocket bridge |
| **Dark Mode** | LOW-MEDIUM | Consistent theming across all components |
| **3D Models** | MEDIUM | File association, preview rendering |

### Risk Factors

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Monaco bundle size | MEDIUM | HIGH | Use dynamic imports, lazy load |
| Multi-source lib conflicts | HIGH | MEDIUM | Clear precedence rules, user override |
| Cross-platform packaging | LOW | HIGH | Tauri CI templates, test on all platforms |
| WASM loading time | MEDIUM | HIGH | Code splitting, streaming compilation |
| LSP WebSocket reliability | LOW | MEDIUM | Fallback to polling, clear error states |
| 3D model file sizes | MEDIUM | MEDIUM | Lazy loading, optional download |

## User Stories

### Library Management

**As a PCB designer, I want to:**
- Search for "0603 resistor" and find it across all my libraries
- Import KiCad footprint libraries without manual conversion
- See a 3D preview of a component before placing it
- Know which components are in stock at JLCPCB
- Organize my custom components separately from vendor libs

### Desktop Application

**As a user, I want to:**
- Install CodeYourPCB like any other desktop app
- Use File > Open instead of typing file paths
- Have dark mode match my system preference
- Press Ctrl+S to save (not think about it)
- Start the app in under a second

### Web Deployment

**As a developer, I want to:**
- Share my PCB design via URL for code review
- Work on a design on my laptop, then my desktop
- Have teammates view my board without installing anything
- Deploy updates by pushing to git
- Not pay for server hosting

### Embedded Code Editor

**As a code-first user, I want to:**
- Edit .cypcb files without switching to external editor
- See autocomplete suggestions for component names
- Have errors highlighted as I type
- See the board update as I edit code
- Use the same LSP as my VS Code setup

## Sources

### Library Management
- [PCB Component Library Comparison](https://www.ultralibrarian.com/2026/01/22/pcb-component-library-comparison/) - UltraLibrarian, 2026
- [PCB Library Management Guide](https://www.embedded-consultants.com/blog/pcb-library-management/) - Embedded Consultants
- [KiCad Library Conventions](https://klc.kicad.org/) - KiCad EDA
- [Component Library Best Practices](https://www.ultralibrarian.com/2023/04/25/component-library-best-practices-explained-ulc/) - UltraLibrarian
- [3D CAD Model Library and OrCAD X](https://resources.pcb.cadence.com/blog/2025-integrating-3d-cad-model-library-orcad-x) - Cadence, 2025

### Desktop Application (Tauri)
- [Tauri v2.0 Official Documentation](https://tauri.app/) - Tauri Contributors, © 2026
- [Tauri vs Electron Performance](https://www.gethopp.app/blog/tauri-vs-electron) - Hopp
- [Tauri Dialog Plugin](https://v2.tauri.app/plugin/dialog/) - Tauri v2
- [Window Menu | Tauri](https://v2.tauri.app/learn/window-menu/) - Tauri v2
- [tauri-ui Templates](https://github.com/agmmnn/tauri-ui) - Community project

### Web Deployment
- [Netlify](https://www.netlify.com/) - Modern web platform
- [PWA 2.0 + Edge Runtime 2026](https://www.zignuts.com/blog/pwa-2-0-edge-runtime-full-stack-2026) - Zignuts
- [Next.js 16 PWA Offline Support](https://blog.logrocket.com/nextjs-16-pwa-offline-support) - LogRocket
- [Progressive Web Apps Offline Guide](https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/Guides/Offline_and_background_operation) - MDN

### Embedded Code Editor
- [Monaco Editor vs CodeMirror Comparison](https://agenthicks.com/research/codemirror-vs-monaco-editor-comparison) - PARA Garden
- [Migrating Monaco to CodeMirror](https://sourcegraph.com/blog/migrating-monaco-codemirror) - Sourcegraph
- [monaco-languageclient 10.6.0](https://github.com/TypeFox/monaco-languageclient) - TypeFox, released Jan 14, 2026
- [LSP and Monaco Integration](https://github.com/eclipse-theia/theia/wiki/LSP-and-Monaco-Integration) - Eclipse Theia
- [CodeMirror Autocompletion](https://codemirror.net/examples/autocompletion/) - CodeMirror docs

### Performance Optimization
- [WebAssembly State 2025-2026](https://platform.uno/blog/the-state-of-webassembly-2025-2026/) - Platform.uno
- [WASM Performance Optimization](https://betterstack.com/community/guides/scaling-nodejs/webassembly-web-apps/) - Better Stack
- [Advanced WASM Performance](https://dev.to/rikinptl/advanced-webassembly-performance-optimization-pushing-the-limits-of-web-performance-4ke0) - DEV Community

### Dark Mode & Theming
- [Dark Mode Best Practices 2026](https://www.tech-rz.com/blog/dark-mode-design-best-practices-in-2026/) - Tech-RZ
- [Dark Mode UI Design Best Practices](https://www.designstudiouiux.com/blog/dark-mode-ui-design-best-practices/) - Design Studio
- [Dark Mode Done Right 2026](https://medium.com/@social_7132/dark-mode-done-right-best-practices-for-2026-c223a4b92417) - Medium, Nov 2025
- [Tailwind Dark Mode](https://tailwindcss.com/docs/dark-mode) - Tailwind CSS

### UX Best Practices
- [Filter UX Design Patterns](https://www.pencilandpaper.io/articles/ux-pattern-analysis-enterprise-filtering) - Pencil & Paper
- [Search UX Best Practices 2026](https://www.designrush.com/best-designs/websites/trends/search-ux-best-practices) - DesignRush
- [Common UX Mistakes](https://www.eleken.co/blog-posts/bad-ux-examples) - Eleken
- [14 Common UX Design Mistakes](https://contentsquare.com/guides/ux-design/mistakes/) - Contentsquare

**Confidence Assessment:**
- **Library Management:** HIGH - Well-established patterns in PCB/CAD industry
- **Desktop Application:** HIGH - Tauri documentation is current (© 2026), proven patterns
- **Web Deployment:** HIGH - PWA and static hosting are mature technologies
- **Embedded Editor:** HIGH - Monaco and LSP integration well-documented
- **Overall:** HIGH - All technologies have production examples and clear documentation

---
*Feature research for: CodeYourPCB v1.1 Foundation & Desktop*
*Researched: 2026-01-29*
*Next: Use this to inform phase structure and requirements definition*