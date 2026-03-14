# T01: Tauri Project Scaffold

**Slice:** S11 — **Milestone:** M001

## Description

Scaffold the Tauri v2 desktop shell alongside the existing viewer frontend.

Purpose: Creates the foundational project structure so all subsequent plans can build native desktop features on top.
Output: Compilable Tauri project that wraps the existing Vite viewer in a native window, starting maximized.

## Must-Haves

- [ ] "Application window opens when running tauri dev"
- [ ] "Window starts maximized per configuration"
- [ ] "Vite dev server integrates with Tauri without rebuild loops"

## Files

- `Cargo.toml`
- `src-tauri/Cargo.toml`
- `src-tauri/build.rs`
- `src-tauri/tauri.conf.json`
- `src-tauri/capabilities/default.json`
- `src-tauri/src/main.rs`
- `src-tauri/src/lib.rs`
- `viewer/vite.config.ts`
- `viewer/package.json`
