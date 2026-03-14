# S11: Tauri Desktop Foundation

**Goal:** Scaffold the Tauri v2 desktop shell alongside the existing viewer frontend.
**Demo:** Scaffold the Tauri v2 desktop shell alongside the existing viewer frontend.

## Must-Haves


## Tasks

- [x] **T01: Tauri Project Scaffold**
  - Scaffold the Tauri v2 desktop shell alongside the existing viewer frontend.

Purpose: Creates the foundational project structure so all subsequent plans can build native desktop features on top.
Output: Compilable Tauri project that wraps the existing Vite viewer in a native window, starting maximized.
- [x] **T02: Native Menu Bar & File IPC**
  - Add native menus and file dialog IPC commands to the Tauri desktop shell.

Purpose: Enables DESK-01 (open files), DESK-02 (save files), and DESK-03 (native menu bar) requirements.
Output: Native menu bar with File/Edit/View/Help menus, and Tauri IPC commands for file open/save that the frontend can invoke.
- [x] **T03: Desktop Integration Module**
  - Wire the frontend to respond to Tauri native menu events and IPC commands.

Purpose: Completes DESK-04 (window management) and DESK-05 (keyboard shortcuts) by connecting the native menu actions to frontend behavior.
Output: Desktop integration module that handles menu events, file operations via IPC, and detects Tauri environment.
- [x] **T04: Installer & File Association**
  - Configure installers, file associations, auto-updater, and verify performance targets.

Purpose: Completes DESK-06 (installers), DESK-07 (updates), DESK-08 (bundle size), DESK-09 (memory), DESK-10 (startup time).
Output: Build configuration producing platform installers with .cypcb file association, updater plugin, and optimized release profile.
- [x] **T05: Desktop Menu Event Wiring** `est:1min`
  - Wire desktop menu events to the viewer engine by adding event listeners in main.ts.

Purpose: desktop.ts dispatches custom events (desktop:open-file, desktop:content-request, desktop:viewport, desktop:toggle-theme, desktop:new-file) but main.ts has no listeners for them. Without these listeners, native menu actions have no effect on the viewer.

Output: main.ts handles all desktop custom events, completing the end-to-end menu-to-viewer pipeline.

## Files Likely Touched

- `Cargo.toml`
- `src-tauri/Cargo.toml`
- `src-tauri/build.rs`
- `src-tauri/tauri.conf.json`
- `src-tauri/capabilities/default.json`
- `src-tauri/src/main.rs`
- `src-tauri/src/lib.rs`
- `viewer/vite.config.ts`
- `viewer/package.json`
- `src-tauri/src/lib.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/menu.rs`
- `viewer/src/desktop.ts`
- `viewer/src/main.ts`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`
- `src-tauri/src/lib.rs`
- `viewer/src/main.ts`
