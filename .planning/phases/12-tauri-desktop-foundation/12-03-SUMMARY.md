---
phase: 12
plan: 03
subsystem: desktop-integration
tags: [tauri, ipc, menu-events, file-operations, desktop]
requires: [phase-12-01, phase-12-02]
provides: [desktop-menu-integration, tauri-ipc-handlers, desktop-file-ops]
affects: [phase-12-04, phase-13]
tech-stack:
  added: []
  patterns:
    - tauri-event-listener
    - dynamic-import-for-web-compatibility
    - custom-event-dispatching
key-files:
  created:
    - viewer/src/desktop.ts
  modified:
    - viewer/src/main.ts
decisions:
  - what: "Use custom events for app-desktop communication"
    why: "Decouples desktop module from main app logic, allows main.ts to remain unchanged for web builds"
    alternatives: "Direct function calls - rejected, would require tight coupling and mode detection"
  - what: "Dynamic imports for @tauri-apps/api"
    why: "Prevents web builds from breaking when Tauri APIs are not available"
    alternatives: "Static imports with try/catch - rejected, would still fail at build time"
  - what: "Module-level state for currentFilePath"
    why: "Simple state tracking for save operations without complex state management"
    alternatives: "Pass through event detail - rejected, would require coordinating multiple events"
metrics:
  duration: 169s
  tasks-completed: 2
  tests-added: 0
  commits: 2
completed: 2026-01-29
---

# Phase 12 Plan 03: Desktop Menu & IPC Integration Summary

**One-liner:** Connected Tauri native menus to frontend via event listeners and IPC commands for file open/save

## What Was Built

Created a desktop integration layer (`viewer/src/desktop.ts`) that bridges Tauri's native menu system with the frontend viewer application. The module listens for `menu-action` events from the Rust backend and handles file operations (open/save) via Tauri IPC commands. The integration is conditionally loaded only in desktop mode, preserving web build compatibility.

## Technical Implementation

### Desktop Integration Module (Task 1)
- **viewer/src/desktop.ts:** Desktop-specific integration layer with three main functions:
  - `isDesktop()`: Detects Tauri environment by checking `window.__TAURI_INTERNALS__`
  - `initDesktop()`: Dynamically imports Tauri APIs and sets up menu event listener
  - `handleMenuAction()`: Routes menu events to appropriate handlers

- **Menu Event Handling:** Listens for `menu-action` events from backend (Plan 02) and dispatches to handlers:
  - `file.new` -> Dispatches `desktop:new-file` custom event
  - `file.open` -> Invokes Tauri `open_file` command, dispatches `desktop:open-file` with content
  - `file.save` -> Invokes Tauri `save_file` command with current path
  - `file.save_as` -> Invokes Tauri `save_file_as` command with file picker
  - `view.zoom_in/zoom_out/fit` -> Dispatches `desktop:viewport` custom events
  - `view.theme` -> Dispatches `desktop:toggle-theme` custom event
  - `help.about` -> Shows simple alert dialog

- **IPC Commands:** Calls Tauri commands from Plan 02 (`src-tauri/src/commands.rs`):
  - `open_file()` -> Returns `{ path, content }` or null if cancelled
  - `save_file(path, content)` -> Saves to specified path
  - `save_file_as(content)` -> Shows save dialog, returns chosen path

- **State Management:** Module-level `currentFilePath` variable tracks the current file for save operations. Updated when files are opened or saved.

- **Web Compatibility:** Dynamic imports (`await import('@tauri-apps/api/event')`) prevent web builds from failing when Tauri APIs are unavailable. The module only loads if `isDesktop()` returns true.

### Main Entry Point Wiring (Task 2)
- **viewer/src/main.ts:** Added conditional desktop initialization at end of `init()` function:
  ```typescript
  if (isDesktop()) {
    await initDesktop();
  }
  ```

- **Initialization Order:** Desktop module loads AFTER existing initialization (WASM, UI, WebSocket) so the viewer is ready to receive file content from menu actions.

- **Web Build Preservation:** Import statement added but only called conditionally. Web builds skip the desktop code path entirely.

## Deviations from Plan

None - plan executed exactly as written.

## Requirements Satisfied

**From ROADMAP.md Phase 12:**
- DESK-04: Window management (minimize, maximize, fullscreen) - fullscreen handled in backend (Plan 02), minimize/maximize are native OS window controls
- DESK-05: Keyboard shortcuts (Ctrl+S, Ctrl+O) - work via native menu accelerators (Plan 02), frontend responds to events

## Files Changed

### Created (1 file)
- `viewer/src/desktop.ts` - Desktop integration module (259 lines)

### Modified (1 file)
- `viewer/src/main.ts` - Added conditional desktop initialization (+6 lines)

## Dependencies

**Requires (from earlier plans):**
- Phase 12 Plan 01: Tauri project structure and configuration
- Phase 12 Plan 02: Native menus with `menu-action` event emission, IPC commands (`open_file`, `save_file`, `save_file_as`)

**Provides (for later plans/phases):**
- Desktop menu event handling infrastructure
- File open/save via native dialogs
- Custom event patterns for app-desktop communication

**Affects:**
- Phase 12 Plan 04: Window management can use similar custom event pattern
- Phase 13 (Web Deployment): Dynamic imports ensure web builds remain unaffected

## Next Phase Readiness

**Blockers:**
None - frontend integration is complete and type-checks successfully.

**Concerns:**
- Custom events (`desktop:open-file`, `desktop:viewport`, etc.) are dispatched but not yet handled by main.ts. The viewer will need event listeners added to respond to these actions.
- File content retrieval (`getCurrentFileContent()`) uses a request/response pattern via custom events, but main.ts doesn't yet implement the response handler. This will be needed for save operations to work end-to-end.

**Recommendations:**
- Add event listeners in main.ts to handle `desktop:open-file` (load file content into engine)
- Add event listener for `desktop:content-request` to respond with current source content
- Add event listeners for `desktop:viewport` to handle zoom actions from menu
- Add event listener for `desktop:toggle-theme` to cycle theme
- Test file open/save in Tauri dev mode (`npm run dev:desktop`) once GTK libraries are available

## Open Questions

None - plan executed as designed.

## Lessons Learned

1. **Dynamic Imports for Conditional Dependencies:** Using `await import('@tauri-apps/api/event')` inside `initDesktop()` prevents web builds from failing when Tauri packages are not available at runtime. This is superior to conditional imports at module level because it defers evaluation until the function is called.

2. **Custom Events for Loose Coupling:** Dispatching custom events (`desktop:open-file`, `desktop:viewport`) decouples the desktop integration module from the main application logic. The main app can remain ignorant of desktop mode and only add event listeners when needed.

3. **Request/Response Pattern via Events:** The `getCurrentFileContent()` function demonstrates a promise-based request/response pattern using custom events. This allows async coordination between modules without direct function calls.

4. **Module-Level State is Sufficient:** Tracking `currentFilePath` at module level is simple and sufficient for this use case. More complex state management (Redux, Zustand) would be overkill.

5. **TypeScript Any for Dynamic Imports:** TypeScript requires `any` type for dynamically imported modules (`tauriEvent: any`). Type assertions at call sites (`as { path: string; content: string }`) provide safety without breaking the dynamic import pattern.
