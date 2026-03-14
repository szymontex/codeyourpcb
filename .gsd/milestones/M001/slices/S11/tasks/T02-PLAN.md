# T02: Native Menu Bar & File IPC

**Slice:** S11 — **Milestone:** M001

## Description

Add native menus and file dialog IPC commands to the Tauri desktop shell.

Purpose: Enables DESK-01 (open files), DESK-02 (save files), and DESK-03 (native menu bar) requirements.
Output: Native menu bar with File/Edit/View/Help menus, and Tauri IPC commands for file open/save that the frontend can invoke.

## Must-Haves

- [ ] "User can open .cypcb files via native OS file dialog"
- [ ] "User can save files via native OS file dialog"
- [ ] "Application has native File/Edit/View/Help menu bar"
- [ ] "Menu actions emit events to the frontend"

## Files

- `src-tauri/src/lib.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/menu.rs`
