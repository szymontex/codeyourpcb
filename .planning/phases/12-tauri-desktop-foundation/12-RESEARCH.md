# Phase 12: Tauri Desktop Foundation - Research

**Researched:** 2026-01-29
**Domain:** Tauri v2 desktop application wrapping existing Vite/WASM viewer
**Confidence:** HIGH

## Summary

Phase 12 wraps the existing `viewer/` Vite frontend in a Tauri v2 desktop shell, adding native OS menus, file dialogs, keyboard shortcuts, file association for `.cypcb`, and platform installers. The existing codebase already has strong foundations: `cypcb-platform` provides a Platform facade with FileSystem, Dialog, Storage, and Menu data models. The viewer uses Vite on port 4321 with a WASM engine (`cypcb-render`).

Tauri v2 is the clear choice -- it uses the OS webview (no bundled Chromium), producing ~3-10MB installers vs Electron's 100MB+, with ~30-40MB idle memory vs Electron's 200MB+, and sub-second startup. The `src-tauri/` directory sits alongside `viewer/` in the project root. Tauri's Rust backend connects naturally to the existing Rust crate ecosystem.

**Primary recommendation:** Add `src-tauri/` at project root with Tauri v2. The Tauri backend converts the existing `MenuBar` data model into native Tauri menus, uses `tauri::api::dialog` for file operations (replacing rfd in desktop context), and configures bundler for `.cypcb` file association. The existing viewer frontend is served as-is by Tauri's webview.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tauri | 2.x | Desktop shell, window management, IPC | Official framework, 3-10MB bundles, uses OS webview |
| tauri-build | 2.x | Build integration | Required by Tauri |
| @tauri-apps/cli | 2.x | Dev/build tooling | `tauri dev`, `tauri build` commands |
| @tauri-apps/api | 2.x | JS-to-Rust IPC from frontend | Type-safe frontend-backend communication |

### Supporting (Tauri plugins)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tauri-plugin-updater | 2.x | Auto-update mechanism | DESK-07: check for and install updates |
| tauri-plugin-global-shortcut | 2.x | Global keyboard shortcuts | DESK-05: if shortcuts need to work when app is not focused |
| tauri-plugin-dialog | 2.x | Native file/message dialogs | DESK-01/02: file open/save dialogs |
| tauri-plugin-fs | 2.x | File system access from JS | Reading/writing .cypcb files from frontend |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Tauri | Electron | 10x larger bundles (100MB+), 5x more memory, ships Chromium |
| Tauri | Wails (Go) | Project is Rust -- Tauri is native fit, Wails requires Go |
| tauri-plugin-dialog | rfd (already in project) | Tauri has its own dialog system that integrates better with the window lifecycle; rfd still useful for non-Tauri desktop builds |

### Installation

```bash
# In project root (alongside viewer/)
npm install --save-dev @tauri-apps/cli@latest
npm install @tauri-apps/api@latest
npm install @tauri-apps/plugin-dialog @tauri-apps/plugin-fs

# In src-tauri/Cargo.toml
# tauri = { version = "2", features = ["devtools"] }
# tauri-plugin-dialog = "2"
# tauri-plugin-fs = "2"
# tauri-plugin-updater = "2"
```

## Architecture Patterns

### Recommended Project Structure
```
codeyourpcb/
├── viewer/                    # Existing frontend (Vite + TS + WASM)
│   ├── index.html
│   ├── src/
│   ├── pkg/                   # WASM output
│   ├── vite.config.ts
│   └── package.json
├── src-tauri/                 # NEW: Tauri desktop shell
│   ├── Cargo.toml             # Depends on tauri + workspace crates
│   ├── tauri.conf.json        # App config, bundler, file associations
│   ├── build.rs               # tauri_build::build()
│   ├── capabilities/
│   │   └── default.json       # Security permissions
│   ├── icons/                 # App icons (PNG/ICO/ICNS)
│   └── src/
│       ├── main.rs            # Desktop entry point
│       └── lib.rs             # Menu setup, IPC commands, event handling
├── crates/
│   ├── cypcb-platform/        # Existing platform abstractions
│   └── ...
└── Cargo.toml                 # Workspace root (add src-tauri to members)
```

### Pattern 1: Menu Data Model to Tauri Native Menu
**What:** Convert the existing `cypcb_platform::MenuBar` declarative data model into Tauri's native menu API at app startup.
**When to use:** App initialization in `setup()` callback.
**Example:**
```rust
// Source: https://v2.tauri.app/learn/window-menu/
use tauri::menu::{MenuBuilder, SubmenuBuilder, MenuItem};
use cypcb_platform::{MenuBar, Menu, MenuItem as PlatformMenuItem};

fn build_tauri_menu(app: &tauri::App, menu_bar: &MenuBar) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let mut builder = MenuBuilder::new(app);
    for menu in &menu_bar.items {
        let submenu = build_submenu(app, menu)?;
        builder = builder.item(&submenu);
    }
    builder.build()
}

fn build_submenu(app: &tauri::App, menu: &Menu) -> tauri::Result<tauri::menu::Submenu<tauri::Wry>> {
    let mut sub = SubmenuBuilder::new(app, &menu.label);
    for item in &menu.items {
        match item {
            PlatformMenuItem::Action { id, label, shortcut, enabled } => {
                let mi = MenuItem::with_id(app, id, label, *enabled, shortcut.as_deref())?;
                sub = sub.item(&mi);
            }
            PlatformMenuItem::Separator => {
                sub = sub.separator();
            }
            PlatformMenuItem::Submenu(nested) => {
                let nested_sub = build_submenu(app, nested)?;
                sub = sub.item(&nested_sub);
            }
        }
    }
    sub.build()
}
```

### Pattern 2: IPC Commands for File Operations
**What:** Tauri commands invoked from JavaScript for file open/save that use native dialogs.
**When to use:** When frontend needs to open/save files via native OS dialogs.
**Example:**
```rust
// Tauri command callable from JavaScript
#[tauri::command]
async fn open_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let file = app.dialog()
        .file()
        .add_filter("CodeYourPCB", &["cypcb"])
        .blocking_pick_file();
    match file {
        Some(path) => {
            let content = std::fs::read_to_string(path.path)?;
            Ok(Some(content))
        }
        None => Ok(None),
    }
}
```

### Pattern 3: Menu Event to Frontend Communication
**What:** Menu clicks in Rust emit events to the JS frontend via Tauri's event system.
**When to use:** All menu actions that affect the UI.
**Example:**
```rust
app.on_menu_event(move |app_handle, event| {
    match event.id().0.as_str() {
        "file.open" => {
            // Emit to frontend
            app_handle.emit("menu-action", "file.open").unwrap();
        }
        "file.save" => {
            app_handle.emit("menu-action", "file.save").unwrap();
        }
        _ => {}
    }
});
```

### Pattern 4: tauri.conf.json for Vite Integration
**What:** Configure Tauri to use the existing viewer Vite setup.
**Example:**
```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "CodeYourPCB",
  "version": "0.1.0",
  "identifier": "com.codeyourpcb.app",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:4321",
    "frontendDist": "../viewer/dist"
  },
  "app": {
    "windows": [
      {
        "title": "CodeYourPCB",
        "maximized": true,
        "minWidth": 800,
        "minHeight": 600
      }
    ]
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/icon.png", "icons/icon.icns", "icons/icon.ico"],
    "fileAssociations": [
      {
        "ext": ["cypcb"],
        "mimeType": "application/x-codeyourpcb",
        "description": "CodeYourPCB Design File"
      }
    ]
  }
}
```

### Anti-Patterns to Avoid
- **Putting all logic in Rust backend:** Keep rendering and UI state in the frontend (JS/WASM). Use Tauri backend only for OS integration (menus, dialogs, file I/O, window management).
- **Using rfd inside Tauri:** Tauri has its own dialog system via `tauri-plugin-dialog`. Using rfd separately can conflict with the Tauri event loop and window focus.
- **Bundling the WASM engine via Tauri resources:** The WASM module should continue being served by Vite as a frontend asset, not as a Tauri resource. Tauri's webview loads it like any other web asset.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Auto-updates | Custom download+replace logic | `tauri-plugin-updater` | Code signing, platform-specific installers, rollback |
| Native file dialogs | Custom GTK/Win32/Cocoa bindings | `tauri-plugin-dialog` | Cross-platform, integrated with Tauri window lifecycle |
| App installers | Shell scripts, makefiles | `tauri build` (NSIS/WiX for Windows, DMG for macOS, AppImage/deb for Linux) | Handles code signing, file associations, uninstaller |
| Global shortcuts | Raw keycode listeners | `tauri-plugin-global-shortcut` or menu accelerators | Cross-platform modifier key handling (Cmd vs Ctrl) |
| Window state persistence | localStorage for window bounds | `tauri-plugin-window-state` | Saves/restores position, size, maximized state across launches |
| App icons | Manual format conversion | `tauri icon` CLI command | Generates all required sizes/formats from single source |

**Key insight:** Tauri's plugin ecosystem handles most OS integration needs. The Tauri CLI (`tauri build`) produces platform-specific installers automatically -- no custom packaging scripts needed.

## Common Pitfalls

### Pitfall 1: Vite watching src-tauri causes infinite rebuild loop
**What goes wrong:** Vite detects changes in `src-tauri/` (Rust compilation output) and triggers frontend hot reload, which triggers another Tauri rebuild.
**Why it happens:** Default Vite watch config includes all project directories.
**How to avoid:** Add `watch: { ignored: ['**/src-tauri/**'] }` to vite.config.ts server options.
**Warning signs:** App constantly restarting during development.

### Pitfall 2: WebView version differences across OS
**What goes wrong:** CSS/JS features work in development (Chrome-based) but fail on macOS (WebKit/Safari) or older Windows (EdgeHTML).
**Why it happens:** Tauri uses system webview: WebKit on macOS/Linux, WebView2 (Edge/Chromium) on Windows.
**How to avoid:** Set Vite build target to `safari13` for macOS compatibility (already handled by Tauri env vars). Test on all target platforms. Avoid bleeding-edge CSS/JS features.
**Warning signs:** UI looks different or broken on macOS vs Windows.

### Pitfall 3: Forgetting Tauri security capabilities
**What goes wrong:** IPC commands fail silently or with permission errors.
**Why it happens:** Tauri v2 has a capability-based security model. Commands must be explicitly allowed in `capabilities/default.json`.
**How to avoid:** Add all required plugin permissions to capabilities file during setup.
**Warning signs:** Console errors about denied permissions.

### Pitfall 4: File association not working on installed app
**What goes wrong:** Double-clicking `.cypcb` files doesn't open the app after installation.
**Why it happens:** File associations only register during installer execution, not during development. Also requires handling the file path argument in the app startup.
**How to avoid:** Handle `tauri::RunEvent::Opened { urls }` for file-opened events. Test with actual installed builds, not `tauri dev`.
**Warning signs:** File association works in registry/plist but app doesn't receive the file path.

### Pitfall 5: Bundle size bloat from debug symbols
**What goes wrong:** Release build is 50MB+ instead of target <10MB.
**Why it happens:** Debug symbols and unoptimized Rust compilation.
**How to avoid:** Ensure `src-tauri/Cargo.toml` has `[profile.release]` with `strip = true`, `lto = true`, `opt-level = "s"` or `"z"`.
**Warning signs:** Built binary is much larger than expected.

### Pitfall 6: Existing dev server (server.ts) conflicts with Tauri dev
**What goes wrong:** Two dev servers running, WebSocket connections confused.
**Why it happens:** The existing `viewer/server.ts` spawns Vite and adds a WebSocket server for hot reload. Tauri also spawns Vite via `beforeDevCommand`.
**How to avoid:** For Tauri desktop dev, use `tauri dev` which handles Vite. The `server.ts` is for web-only development. May need separate npm scripts: `dev:web` (existing) and `dev:desktop` (Tauri).
**Warning signs:** Port conflicts, duplicate Vite instances.

## Code Examples

### Tauri Desktop Entry Point (src-tauri/src/main.rs)
```rust
// Source: https://v2.tauri.app/start/project-structure/
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    app_lib::run();
}
```

### Tauri App Setup with Menus (src-tauri/src/lib.rs)
```rust
// Source: https://v2.tauri.app/learn/window-menu/
use cypcb_platform::{MenuBar, Menu, MenuItem as PlatformMenuItem};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // Build menu from platform data model
            let menu_bar = create_app_menu();
            let tauri_menu = build_tauri_menu(app, &menu_bar)?;
            app.set_menu(tauri_menu)?;

            // Handle menu events
            app.on_menu_event(|app_handle, event| {
                handle_menu_event(app_handle, &event);
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![open_file, save_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn create_app_menu() -> MenuBar {
    MenuBar::new()
        .add_menu(
            Menu::new("File")
                .add_item(PlatformMenuItem::action("file.new", "New").with_shortcut("CmdOrCtrl+N"))
                .add_item(PlatformMenuItem::action("file.open", "Open...").with_shortcut("CmdOrCtrl+O"))
                .separator()
                .add_item(PlatformMenuItem::action("file.save", "Save").with_shortcut("CmdOrCtrl+S"))
                .add_item(PlatformMenuItem::action("file.save_as", "Save As...").with_shortcut("CmdOrCtrl+Shift+S"))
                .separator()
                .add_item(PlatformMenuItem::action("file.quit", "Quit").with_shortcut("CmdOrCtrl+Q"))
        )
        .add_menu(
            Menu::new("Edit")
                .add_item(PlatformMenuItem::action("edit.undo", "Undo").with_shortcut("CmdOrCtrl+Z"))
                .add_item(PlatformMenuItem::action("edit.redo", "Redo").with_shortcut("CmdOrCtrl+Shift+Z"))
        )
        .add_menu(
            Menu::new("View")
                .add_item(PlatformMenuItem::action("view.zoom_in", "Zoom In").with_shortcut("CmdOrCtrl+="))
                .add_item(PlatformMenuItem::action("view.zoom_out", "Zoom Out").with_shortcut("CmdOrCtrl+-"))
                .add_item(PlatformMenuItem::action("view.fit", "Fit to Window").with_shortcut("CmdOrCtrl+0"))
                .separator()
                .add_item(PlatformMenuItem::action("view.fullscreen", "Toggle Fullscreen").with_shortcut("F11"))
        )
        .add_menu(
            Menu::new("Help")
                .add_item(PlatformMenuItem::action("help.about", "About CodeYourPCB"))
        )
}
```

### Vite Config Updates for Tauri Compatibility
```typescript
// viewer/vite.config.ts additions
export default defineConfig({
  server: {
    port: 4321,
    host: process.env.TAURI_DEV_HOST || '0.0.0.0',
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === 'windows'
      ? 'chrome105'
      : 'safari13',
    minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});
```

### Release Profile for Small Bundles
```toml
# src-tauri/Cargo.toml
[profile.release]
strip = true
lto = true
opt-level = "s"
codegen-units = 1
panic = "abort"
```

### File Association Handling
```rust
// Handle files opened via OS file association (double-click .cypcb)
.build(tauri::generate_context!())
.expect("error while running tauri application")
// The RunEvent::Opened event fires when a file is double-clicked
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Tauri v1 menus (builder pattern in main) | Tauri v2 `MenuBuilder`/`SubmenuBuilder` with IDs | Oct 2024 (Tauri 2.0 stable) | New menu API, must use v2 patterns |
| Tauri v1 `tauri.conf.json` format | Tauri v2 new schema (`$schema: schema.tauri.app/config/2`) | Oct 2024 | Different config structure, `frontendDist` replaces `distDir` |
| Manual update server | `tauri-plugin-updater` 2.x | Oct 2024 | First-class plugin, requires signing keys |
| Tauri v1 `allowlist` security | Tauri v2 `capabilities/` directory | Oct 2024 | More granular, file-based permissions |

**Deprecated/outdated:**
- Tauri v1 API: Do NOT reference v1 docs. v2 has different config, menu API, and plugin system.
- `distDir` config key: Replaced by `frontendDist` in v2.
- `allowlist` in tauri.conf.json: Replaced by capabilities directory in v2.

## Open Questions

1. **CmdOrCtrl shortcut format in MenuBar**
   - What we know: Tauri uses `CmdOrCtrl+S` format for cross-platform shortcuts. The existing `MenuBar` model stores shortcuts as plain strings like `"Ctrl+S"`.
   - What's unclear: Whether to update the MenuBar data model to use `CmdOrCtrl` format, or translate at the Tauri rendering layer.
   - Recommendation: Translate at the Tauri layer. The MenuBar model should remain platform-agnostic. The comment in `menu.rs` already says "Platform-specific rendering code will convert to native format."

2. **WebSocket dev server coexistence**
   - What we know: `server.ts` runs a WebSocket server on port 4322 for hot reload + routing. Tauri dev mode spawns Vite separately.
   - What's unclear: Whether the WS-based routing integration needs to work in Tauri mode, or if Tauri commands should replace it.
   - Recommendation: In desktop mode, use Tauri IPC commands for routing (spawn CLI binary directly from Rust backend). Keep `server.ts` for web-only development.

3. **Window state persistence plugin**
   - What we know: `tauri-plugin-window-state` can save/restore window position and size.
   - What's unclear: Whether this is a must-have for Phase 12 or can be deferred.
   - Recommendation: Include it -- it's a single plugin add and significantly improves desktop UX.

## Sources

### Primary (HIGH confidence)
- [Tauri v2 Official Docs - Project Structure](https://v2.tauri.app/start/project-structure/) - project layout, config files
- [Tauri v2 Official Docs - Window Menu](https://v2.tauri.app/learn/window-menu/) - menu API, Rust examples
- [Tauri v2 Official Docs - Vite Integration](https://v2.tauri.app/start/frontend/vite/) - Vite config for Tauri
- [Tauri v2 Official Docs - Global Shortcut Plugin](https://v2.tauri.app/plugin/global-shortcut/) - keyboard shortcuts
- [Tauri v2 Official Docs - Updater Plugin](https://v2.tauri.app/plugin/updater/) - auto-update mechanism
- [Tauri v2 Configuration Reference](https://v2.tauri.app/reference/config/) - full config schema
- [Tauri v2 Config Schema](https://schema.tauri.app/config/2) - JSON schema for tauri.conf.json

### Secondary (MEDIUM confidence)
- [DoltHub: Electron vs Tauri](https://www.dolthub.com/blog/2025-11-13-electron-vs-tauri/) - real-world size/memory comparisons
- [Hopp Blog: Tauri vs Electron](https://www.gethopp.app/blog/tauri-vs-electron) - bundle size, memory benchmarks
- [CrabNebula: Tauri Auto-Updates Guide](https://docs.crabnebula.dev/cloud/guides/auto-updates-tauri/) - updater setup details

### Tertiary (LOW confidence)
- None -- all findings verified with official Tauri v2 documentation.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Tauri v2 is stable, well-documented, exact version known
- Architecture: HIGH - Project structure well-defined, integration path clear with existing Vite viewer
- Pitfalls: HIGH - Common issues documented in official docs and community reports

**Research date:** 2026-01-29
**Valid until:** 2026-03-29 (Tauri v2 is stable, unlikely to change significantly)
