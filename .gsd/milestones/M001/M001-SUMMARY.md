---
id: M001
title: "CodeYourPCB v1.0 + v1.1 — Full Stack PCB Design Tool"
status: complete
started: 2026-01-21
completed: 2026-02-21
slices_completed: 15
tasks_completed: 81
requirements_validated: 64
---

# M001: CodeYourPCB v1.0 + v1.1 — Full Stack PCB Design Tool

## What Was Delivered

A fully standalone, code-first PCB design tool — from DSL source file to manufactured board — running in the browser and as a native desktop app. No KiCad dependency. No external GUI. Write `.cypcb` code, see the board, run DRC, export Gerbers, autoroute, ship.

### v1.0 (Shipped 2026-01-29)

Core pipeline: DSL → parser → ECS board model → 2D renderer → DRC → manufacturing export → autorouter → LSP.

- **Tree-sitter DSL parser** with custom grammar, error recovery, source spans
- **ECS board model** (bevy_ecs) with R*-tree spatial indexing, integer nanometer coordinates
- **Web viewer** with Canvas 2D rendering, zoom/pan/selection, layer toggles, hot reload
- **DRC system** — clearance, trace width, drill size, connectivity, zones/keepouts
- **Manufacturing export** — Gerber X2, Excellon drill, BOM, pick-and-place (JLCPCB verified)
- **FreeRouting integration** — DSN export, SES import, autorouting CLI, trace rendering, ratsnest
- **LSP server** — hover, completion, go-to-definition, real-time diagnostics
- **Touchpad navigation** and **file picker with drag-drop**

### v1.1 (Shipped 2026-02-21)

Professional desktop + web deployment on top of v1.0 core.

- **Platform abstraction layer** — FileSystem, Dialog, Storage, Menu traits with desktop/web implementations
- **Multi-source library management** — KiCad .kicad_mod import, FTS5 search, JLCPCB API, custom libraries, 3D model association
- **Dark mode** — CSS custom properties, ThemeManager, FART prevention, WCAG AA compliant
- **Tauri v2 desktop app** — native menus, file dialogs, installer, <10MB bundle
- **Web deployment** — Cloudflare Pages, File System Access API, URL sharing, optimized WASM (264KB)
- **Monaco editor** — syntax highlighting, LSP bridge, live preview, draggable split layout
- **Documentation** — user guide, API docs, examples, contributing guide

## Stats

- **Codebase:** ~126K lines (35K Rust, 90K TypeScript/JS, 900 CSS/HTML)
- **14 Rust crates** in workspace
- **15 slices**, **81 tasks** completed
- **64 requirements** validated
- **v1.0:** 8 days (2026-01-21 → 2026-01-29), 7 phases, 51 plans
- **v1.1:** 23 days (2026-01-29 → 2026-02-21), 8 phases, 30 plans

## Key Architectural Decisions

- Integer nanometers (i64) for all coordinates — no floating-point precision issues
- ECS composition over inheritance for board model
- Tree-sitter for incremental, error-tolerant parsing
- Platform facade pattern — application code never imports platform-specific types
- WASM bridge over WebSocket LSP for web mode
- Editor as single source of truth when visible
- Viewport-only URL sharing (board content via git, not URLs)

## Known Limitations

- Phase 3 (Validation) missing formal VERIFICATION.md (functionality working)
- Module/import system deferred
- Grid snapping deferred (grid display works)
- Net highlighting deferred
- JLCPCB API client unverified (requires manual API approval)
- Tauri compilation requires GTK3 system libraries on Linux
- Browser verification of WASM deployment deferred (human_needed)

## What's Next

Priority features for v1.2+ based on competitive analysis (atopile, diodeinc/pcb):
- Constraint solver (`assert R1.resistance within 10kohm +/- 10%`)
- Module/import system with typed interfaces
- Physical units in language (`10kohm`, `3.3V`, `100nF`)
- Package registry
- VS Code extension
- Auto-picking from supplier catalogs
- 3D board preview
- Undo/redo system
