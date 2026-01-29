# Milestone v1: Code-First PCB Design Tool

**Status:** ✅ SHIPPED 2026-01-29
**Phases:** 1, 2, 3, 4, 5, 7, 8
**Total Plans:** 51

## Overview

The v1 milestone delivers a fully functional code-first PCB design tool. Instead of clicking in a GUI and getting XML as a side effect, users write declarative code that defines components, connections, and constraints — the visual representation is computed from this source of truth. Designed for engineers who want git-friendly collaboration, AI/LLM-assisted editing, and deterministic builds.

**Core Achievement:** Proven that code-first PCB design is viable with a complete pipeline from DSL source → rendering → DRC → manufacturing files → autorouting → IDE integration.

## Phases

### Phase 1: Foundation

**Goal:** Working DSL parser that produces a valid board model
**Depends on:** None
**Plans:** 9 plans

Plans:
- [x] 01-01: Project setup (workspace, crates, dependencies)
- [x] 01-02: Core types (Nm, Point, Rect, Unit)
- [x] 01-03: Tree-sitter grammar definition
- [x] 01-04: ECS components for board model
- [x] 01-05: AST types and parser implementation
- [x] 01-06: BoardWorld and spatial indexing
- [x] 01-07: Footprint library (SMD + THT)
- [x] 01-08: AST-to-ECS synchronization
- [x] 01-09: CLI with parse/check commands

**Details:**
Custom Tree-sitter grammar for .cypcb files with board definition, component instantiation, and net connections. ECS-based BoardWorld with spatial indexing (R*-tree). Integer nanometer coordinates throughout (no floating-point). Basic SMD (0402-2512) and through-hole footprints.

**Requirements:** DSL-01–04, BRD-01–04, BRD-06, FTP-01–02, DEV-03

---

### Phase 2: Rendering

**Goal:** Minimal UI to verify board rendering with hot reload
**Depends on:** Phase 1
**Plans:** 9 plans

Plans:
- [x] 02-01: WASM crate setup (cypcb-render, BoardSnapshot)
- [x] 02-02: Frontend scaffolding (Vite, TypeScript, HTML)
- [x] 02-03: WASM build and integration (mock fallback)
- [x] 02-04: Canvas renderer with viewport transforms
- [x] 02-05: Interaction (zoom, pan, select, layer toggles)
- [x] 02-06: Hot reload (file watcher, WebSocket)
- [x] 02-07: Visual verification checkpoint
- [x] 02-08: [GAP CLOSURE] Fix WASM build (getrandom compatibility)
- [x] 02-09: [GAP CLOSURE] Enable real WASM integration

**Details:**
Web-based viewer with Canvas 2D rendering. Dual-mode architecture: native tree-sitter parsing in Rust and JavaScript parser → WASM for browser compatibility. Hot reload via chokidar file watcher and WebSocket. Viewport preservation across reloads. KiCad-style colors (red=top, blue=bottom).

**Requirements:** DSL-06, RND-01–04, DEV-04

**Gap Closure:** Initial WASM build failed due to getrandom/bevy_ecs incompatibility. Resolved with feature flags to exclude tree-sitter from WASM target. JS parser handles simple .cypcb syntax while WASM provides ECS-based board model.

---

### Phase 3: Validation

**Goal:** DRC prevents manufacturing-invalid designs
**Depends on:** Phase 1
**Plans:** 10 plans

Plans:
- [x] 03-01: DRC crate setup (types, traits, violation struct)
- [x] 03-02: IC footprints (SOIC, SOT, QFP families)
- [x] 03-03: Manufacturer presets (JLCPCB, PCBWay rules)
- [x] 03-04: Custom footprint DSL syntax and library registration
- [x] 03-05: Clearance checking rule (spatial index)
- [x] 03-06: Drill size, trace width, and connectivity rules
- [x] 03-07: DRC integration with rendering pipeline
- [x] 03-08: Violation display (markers, status bar, panel)
- [x] 03-09: Visual verification checkpoint
- [x] 03-10: Zones and keepouts (BRD-05)

**Details:**
Comprehensive DRC system with clearance checking (trace-trace, trace-pad), minimum trace width, drill size validation, and unconnected pin detection. Manufacturer presets for JLCPCB and PCBWay with file naming conventions. Real-time DRC feedback with visual markers, error badge, and violation panel with zoom-to-location.

**Requirements:** BRD-05, DRC-01–05, FTP-03–04

**Note:** Phase lacks formal VERIFICATION.md file but functionality is proven working through integration with downstream phases and human verification (03-09-SUMMARY.md).

---

### Phase 4: Export

**Goal:** Generate files manufacturers can use
**Depends on:** Phase 2, Phase 3
**Plans:** 7 plans

Plans:
- [x] 04-01: Export crate setup, coordinate conversion, apertures
- [x] 04-02: Gerber copper/mask/paste layer export
- [x] 04-03: Board outline and silkscreen export
- [x] 04-04: Excellon drill file export
- [x] 04-05: BOM and pick-and-place (CPL) export
- [x] 04-06: CLI export command and presets
- [x] 04-07: Visual verification checkpoint

**Details:**
Gerber X2 export with all layers (copper, mask, paste, silk, outline). Excellon drill files with METRIC,TZ format. BOM generation (CSV/JSON) with component grouping. Pick-and-place (CPL) CSV with coordinates in mm. Files verified with Ucamco Gerber viewer and JLCPCB DFM check.

**Requirements:** EXP-01–04, DEV-01

**Verification:** Human verification confirmed files work in external viewers and pass JLCPCB DFM check.

---

### Phase 5: Intelligence

**Goal:** Autorouting and professional IDE experience
**Depends on:** Phase 4
**Plans:** 11 plans

Plans:
- [x] 05-01: Trace/Via ECS components and DSL net constraints
- [x] 05-02: IPC-2221 trace width calculator (cypcb-calc)
- [x] 05-03: KiCad footprint import (cypcb-kicad)
- [x] 05-04: FreeRouting DSN export (cypcb-router)
- [x] 05-05: LSP server setup with hover and diagnostics
- [x] 05-06: FreeRouting SES import and CLI wrapper
- [x] 05-07: LSP completions and go-to-definition
- [x] 05-08: Trace and ratsnest rendering
- [x] 05-09: Autorouter UI integration (CLI, progress, cancel)
- [x] 05-10: Visual verification checkpoint
- [x] 05-11: [GAP CLOSURE] DSL syntax documentation

**Details:**
FreeRouting integration via CLI with DSN export and SES import. LSP server provides hover (component/net info), autocomplete (footprints, nets, components), go-to-definition, and real-time diagnostics (parse errors + DRC violations). KiCad .kicad_mod footprint import. IPC-2221 trace width calculator. Trace and ratsnest rendering in viewer.

**Requirements:** FTP-05, DEV-02, INT-01–03

**UAT Results:** 8 tests performed, 1 documentation gap found and closed. Gap was expectation mismatch on constraint syntax (`current 500mA` inside braces vs. square brackets before braces). Plan 05-11 created comprehensive docs/SYNTAX.md and updated examples.

---

### Phase 7: Navigation Controls

**Goal:** Alternative pan/zoom controls for laptops without middle-click
**Depends on:** Phase 2
**Plans:** 2 plans

Plans:
- [x] 07-01: Pointer Events multi-touch pan + touch-action CSS
- [x] 07-02: Cross-browser navigation verification checkpoint

**Details:**
Two-finger touchpad pan via Pointer Events API with drag counter pattern. Ctrl+LMB pan alternative. Pinch-to-zoom via browser-native gesture support. All existing navigation methods preserved (middle-click, scroll wheel). Verified cross-browser (Opera, Chrome, Firefox expected compatible via Pointer Events baseline support since July 2020).

**Requirements:** NAV-01–03

**Note:** Browser-specific gesture interpretation (Opera: two-finger = zoom, Chrome/Firefox: likely pan) is acceptable as Ctrl+LMB and middle-click provide consistent alternatives.

---

### Phase 8: File Picker

**Goal:** UI to load .cypcb and .ses files directly in the viewer
**Depends on:** Phase 5
**Plans:** 3 plans

Plans:
- [x] 08-01: File picker utilities and UI elements
- [x] 08-02: Integration with viewer (Open button, drag-drop)
- [x] 08-03: Human verification checkpoint

**Details:**
Pure client-side file loading using Browser File API. Open button triggers hidden input element, drag-and-drop with HTML5 drag-drop API and visual feedback. Extension-based dispatch (.cypcb → load_source, .ses → load_routes). Guard prevents loading .ses without board loaded first.

**Requirements:** FP-01–03

**UX Notes:** Error message visibility low, route loading with existing board shows "crooked paths" (visual confusion). Feature request for dedicated project browser UI noted for future improvements.

---

## Milestone Summary

**Phases:**
- Phase 1: Foundation (9 plans) — Core DSL parser and board model
- Phase 2: Rendering (9 plans) — Web viewer with hot reload
- Phase 3: Validation (10 plans) — DRC system with visual feedback
- Phase 4: Export (7 plans) — Manufacturing file generation
- Phase 5: Intelligence (11 plans) — Autorouting and LSP integration
- Phase 7: Navigation (2 plans) — Alternative pan/zoom controls
- Phase 8: File Picker (3 plans) — Load files directly in browser

**Key Decisions:**

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| External DSL (not embedded in Rust) | Full control over syntax, optimized for human/LLM readability | ✅ Successful - .cypcb files are clean and git-friendly |
| Tree-sitter for parsing | Incremental parsing, error tolerance, LSP support | ✅ Good - But required feature flags for WASM compatibility |
| Rust + WASM | Near-native performance, runs in browser and desktop, memory safe | ✅ Good - Dual-mode architecture works well |
| wgpu deferred, Canvas 2D for MVP | Simpler implementation for initial viewer | ✅ Good - Canvas 2D sufficient for 2D board view |
| FreeRouting for MVP autorouter | Proven, open source, allows custom later | ✅ Good - DSN/SES integration working with real .ses files |
| ECS architecture for board model | Composition over inheritance, cache-friendly, parallelizable | ✅ Good - Spatial queries and DRC benefit from ECS |
| Feature flags for WASM compatibility | Exclude tree-sitter from WASM, use JS parser instead | ✅ Good - Enables browser deployment without C compilation |
| Dual-mode parser (native + JS) | Native uses tree-sitter, WASM uses JS parser | ✅ Good - Both paths converge at BoardSnapshot interface |

**Issues Resolved:**

- WASM build incompatibility (getrandom/bevy_ecs) → Feature flags for conditional compilation
- Real WASM integration blocked by commented import → WasmPcbEngineAdapter bridge pattern
- Net constraint syntax confusion → Comprehensive docs/SYNTAX.md with examples
- Laptop touchpad navigation lacking → Two-finger pan via Pointer Events

**Technical Debt:**

- Phase 3 missing formal VERIFICATION.md file (functionality verified, documentation gap)
- Module/import system deferred to v2 (DSL-05)
- Net highlighting deferred to later (RND-05)
- Grid snapping deferred, display only implemented (RND-06)

**Architecture Quality:**

- Consistent integration patterns (all CLI commands use parse → sync → operate)
- Type safety maintained across Rust/JS boundary via serde
- Automatic DRC on every load/change
- Real E2E validation with actual .ses/.dsn files
- 35/35 requirements satisfied (100%)
- 8/8 E2E user flows complete

---

_For current project status, see .planning/ROADMAP.md (created for next milestone)_
