# M001: CodeYourPCB v1.0 + v1.1 — Full Stack PCB Design Tool

**Vision:** A code-first PCB design tool where you write declarative code that defines components, connections, and constraints — the visual representation is computed from this source of truth. Designed for engineers who want git-friendly collaboration, AI/LLM-assisted editing, and deterministic builds. v1.0 delivered the core pipeline (DSL → rendering → DRC → manufacturing export → autorouting → IDE integration). v1.1 extended it with desktop application, library management, embedded editor, dark mode, and web deployment.
**Version:** v1.1

## Success Criteria

- Users can write .cypcb files and see boards rendered in a web viewer with hot reload
- DRC catches manufacturing-invalid designs before export
- Manufacturing files (Gerber X2, Excellon, BOM, CPL) pass JLCPCB verification
- FreeRouting autorouter integration works end-to-end
- LSP server provides full IDE experience (hover, completion, diagnostics, goto)
- Desktop application installs and runs natively via Tauri
- Web application loads fast, works across browsers, supports file access
- Multi-source library management with search across KiCad, JLCPCB, and custom sources
- Dark/light theme system meets WCAG AA contrast requirements
- Monaco editor provides syntax highlighting and live preview

## Slices

- [x] **S01: Foundation** `risk:medium` `depends:[]`
  > After this: Working DSL parser that produces a valid board model with ECS components and spatial indexing
- [x] **S02: Rendering** `risk:medium` `depends:[S01]`
  > After this: Web viewer with Canvas 2D rendering, zoom/pan/selection, hot reload, and WASM integration
- [x] **S03: Validation** `risk:medium` `depends:[S02]`
  > After this: Comprehensive DRC system with clearance, trace width, drill size, and connectivity rules
- [x] **S04: Export** `risk:medium` `depends:[S03]`
  > After this: Manufacturing file export (Gerber X2, Excellon, BOM, CPL) verified with JLCPCB
- [x] **S05: Intelligence** `risk:medium` `depends:[S04]`
  > After this: FreeRouting autorouter integration, LSP server, trace/via support, and net constraints
- [x] **S06: Navigation Controls** `risk:medium` `depends:[S05]`
  > After this: Two-finger touchpad panning, Ctrl+click pan, and pinch-to-zoom for laptop users
- [x] **S07: File Picker** `risk:medium` `depends:[S06]`
  > After this: File picker with drag-and-drop support for loading .cypcb and .ses files
- [x] **S08: Platform Abstraction Layer** `risk:medium` `depends:[S07]`
  > After this: cypcb-platform crate with FileSystem, Dialog, Storage, and Menu traits for desktop/web
- [x] **S09: Library Management Foundation** `risk:medium` `depends:[S08]`
  > After this: Multi-source library management with KiCad import, FTS5 search, and unified interface
- [x] **S10: Dark Mode & UI Polish** `risk:medium` `depends:[S09]`
  > After this: Theme system with CSS custom properties, ThemeManager, FART prevention, WCAG AA compliance
- [x] **S11: Tauri Desktop Foundation** `risk:medium` `depends:[S10]`
  > After this: Tauri v2 desktop application with native menus, file dialogs, and installer
- [x] **S12: Web Deployment** `risk:medium` `depends:[S11]`
  > After this: Production web deployment with optimized WASM, File System Access API, and Cloudflare Pages
- [x] **S13: Monaco Editor Integration** `risk:medium` `depends:[S12]`
  > After this: Embedded Monaco editor with .cypcb syntax highlighting, LSP bridge, and live preview
- [x] **S14: Documentation & Polish** `risk:medium` `depends:[S13]`
  > After this: Comprehensive user guide, API docs, example walkthroughs, and contributing guide
- [x] **S15: Web Verification & Polish** `risk:medium` `depends:[S14]`
  > After this: WASM bundling verified in production, Share URL feature enabled, deployment confirmed
