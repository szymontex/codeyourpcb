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
