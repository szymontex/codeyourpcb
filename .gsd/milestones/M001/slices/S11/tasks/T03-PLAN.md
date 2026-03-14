# T03: Desktop Integration Module

**Slice:** S11 — **Milestone:** M001

## Description

Wire the frontend to respond to Tauri native menu events and IPC commands.

Purpose: Completes DESK-04 (window management) and DESK-05 (keyboard shortcuts) by connecting the native menu actions to frontend behavior.
Output: Desktop integration module that handles menu events, file operations via IPC, and detects Tauri environment.

## Must-Haves

- [ ] "Keyboard shortcuts Ctrl+S/Ctrl+O work in desktop mode"
- [ ] "Frontend responds to menu events from native menus"
- [ ] "Window management (minimize, maximize, fullscreen) works"

## Files

- `viewer/src/desktop.ts`
- `viewer/src/main.ts`
