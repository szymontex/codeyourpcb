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
