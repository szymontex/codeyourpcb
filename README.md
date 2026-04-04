# CodeYourPCB

**Code-first PCB design.** Describe your circuit board in a simple DSL — get a deterministic, git-friendly, AI-editable design.

```cypcb
// blink.cypcb — 555 timer LED blink circuit
version 1

board blink {
    size 60mm x 40mm
    layers 2
}

component J1 connector "PIN-HDR-1x2" { value "5V PWR"; at 5mm, 20mm }
component U1 ic "SOIC-8" { value "NE555"; at 28mm, 20mm }
component R1 resistor "0402" { value "10k"; at 35mm, 30mm }
component C1 capacitor "1206" { value "10uF"; at 43mm, 20mm }
component LED1 led "0805" { value "RED"; at 55mm, 20mm }

net VCC [width 0.5mm  current 2A] { J1.1; U1.8; U1.4; R1.1 }
net GND { J1.2; U1.1; C1.2; LED1.K }

// Routed traces are saved in the file — deterministic, git-friendly
trace VCC {
    layer Top
    width 0.500000mm
    path 5mm,20mm -> 15mm,20mm -> 28mm,20mm -> 35mm,30mm
}
```

<p align="center">
  <img src="docs/images/editor-split.png" alt="Split view — code editor + live board preview" width="720">
</p>

Save the file, board updates instantly. No compile step, no refresh.

**New here?** [Syntax reference](docs/SYNTAX.md) | [Getting started](docs/user-guide/getting-started.md) | [Examples](examples/)

---

## Why?

Traditional PCB tools (KiCad, Altium, Eagle) are GUI-first. The project file is a binary/XML side-effect of clicking. This makes:

- **Git diffs** unreadable
- **AI assistance** impractical
- **Team review** painful
- **Automation** fragile

CodeYourPCB flips the model: the source file _is_ the design. Text in, board out.

## The LLM angle

The core idea: give AI coding assistants like Claude Code, Copilot, or Cursor the ability to **design real PCBs through declarative code** — the same way they write software.

Traditional PCB formats are opaque to LLMs. A KiCad `.kicad_pcb` file is thousands of lines of coordinate soup. No LLM can reason about that meaningfully.

`.cypcb` is different — the semantics are declarative and human-readable:

```cypcb
component U1 ic "SOIC-8" { value "NE555"; at 28mm, 20mm }
net VCC [width 0.5mm  current 2A] { U1.8; U1.4; R1.1 }

trace VCC {
    layer Top
    width 0.500000mm
    path 5mm,20mm -> 28mm,20mm -> 35mm,30mm
}
```

An LLM can generate this, review it, refactor it, and catch mistakes — just like source code. Even routed traces are human-readable `path` blocks that diff cleanly in git.

<p align="center">
  <img src="docs/images/jlcpcb-3d.png" alt="3D board view with JLCPCB parts search" width="720">
</p>

---

## Features

| Feature | Status |
|---------|--------|
| Live preview — save file, board updates instantly | Done |
| `.cypcb` DSL with Tree-sitter parser | Done |
| Professional 2D renderer (KiCad-style) | Done |
| 3D board viewer with KiCad-style copper, solder mask, round trace caps | Done |
| Interactive trace routing (45° constraint, obstacle dodge) | Done |
| Trace segment editing (drag segments & corners) | Done |
| **Trace persistence** — routed traces saved to `.cypcb` file, survive reload | Done |
| **Bidirectional sync** — edit trace code ↔ board updates in real-time | Done |
| **Net constraints** — `[width 0.5mm]`, `[current 2A]` per net | Done |
| **IPC-2221 auto-width** — routing auto-sets trace width from current rating | Done |
| **DRC: trace width vs current** — warns when trace too thin for current | Done |
| Design Rule Check (DRC) | Done |
| Gerber / Excellon / pick-and-place export | Done |
| Monaco editor with **context-aware completions** and syntax hints | Done |
| LSP (diagnostics, autocomplete, hover, go-to-definition) | Done |
| JLCPCB parts search & drag-and-drop placement | Done |
| Custom footprint definitions in DSL | Done |
| Project manager with templates & thumbnail persistence | Done |
| Dark / Light theme (WCAG AA) | Done |
| Web app | Done |
| Desktop app (Tauri v2, Win/Mac/Linux) | Done |
| KiCad component library import | Done |
| Share URL (viewport state) | Done |
| Copper pour / ground planes (zone fill with clearance) | Planned |
| Zone & keepout rendering (2D + 3D) | Planned |
| Autorouter (fundamental rewrite) | Planned |
| Differential pair routing | Planned |
| Impedance calculator | Planned |

<p align="center">
  <img src="docs/images/board-view.png" alt="555 timer blink circuit — board view with routed traces" width="720">
</p>
<p align="center">
  <img src="docs/images/project-manager.png" alt="Project manager — templates and recent projects" width="720">
</p>

---

## Quick Start

### Web (development)

```bash
# Prerequisites: Rust, Node.js 18+, wasm-pack
cargo install wasm-pack

# Clone and start
git clone https://github.com/szymontex/codeyourpcb.git
cd codeyourpcb/viewer
npm install
npm start          # builds WASM + starts dev server
```

Open http://localhost:5173 — pick a template or open a `.cypcb` file.

### Desktop app (Tauri)

```bash
# Linux prerequisites
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev pkg-config

npm run dev:desktop   # dev mode
npm run build:desktop # release installer
```

### CLI

```bash
cargo run -p cypcb-cli -- check examples/blink.cypcb   # DRC
cargo run -p cypcb-cli -- export examples/blink.cypcb  # Gerber + Excellon
```

### IDE workflow

Edit `.cypcb` files in your editor (VS Code, Neovim, etc.). Run `npm run dev:watch` in a terminal — the board preview updates on every save via WebSocket hot reload.

---

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Ctrl+E` | Toggle code editor |
| `Ctrl+S` | Save file |
| `Ctrl+O` | Open file / project manager |
| `Ctrl+J` | JLCPCB parts search |
| `Ctrl+Z` / `Ctrl+Shift+Z` | Undo / Redo |
| `F` | Fit board to view |
| `3` | Toggle 3D view |
| `R` / `Shift+R` | Rotate component CW / CCW |
| `?` | Keyboard shortcuts help |

During trace routing: `F` flip layer, `/` flip posture, `Q` toggle corner mode, `A` toggle angle snap.

---

## Project Structure

```
codeyourpcb/
├── crates/
│   ├── cypcb-core         # Types, coordinates, units
│   ├── cypcb-parser       # Tree-sitter grammar + AST
│   ├── cypcb-world        # ECS board state (hecs)
│   ├── cypcb-drc          # Design rule checks
│   ├── cypcb-autoroute    # A*-based autorouter (WIP)
│   ├── cypcb-router       # FreeRouting DSN bridge
│   ├── cypcb-calc         # Electrical calculations (IPC-2221)
│   ├── cypcb-export       # Gerber / Excellon / CPL export
│   ├── cypcb-kicad        # KiCad format import
│   ├── cypcb-library      # Component library (SQLite + FTS5)
│   ├── cypcb-lsp          # Language server (tower-lsp)
│   ├── cypcb-render       # Board engine (WASM)
│   ├── cypcb-rules        # Design rules & manufacturer presets
│   ├── cypcb-platform     # Native/web abstraction layer
│   ├── cypcb-watcher      # File system watcher
│   └── cypcb-cli          # CLI entry point
├── viewer/                # TypeScript frontend (Vite + Canvas 2D + Three.js)
├── src-tauri/             # Tauri v2 desktop shell
├── examples/              # Sample .cypcb files
└── docs/                  # User guide, syntax, API reference
```

---

## Documentation

- [Getting Started](docs/user-guide/getting-started.md)
- [Language Syntax](docs/SYNTAX.md)
- [Architecture](docs/architecture.md)
- [Project Structure](docs/user-guide/project-structure.md)
- [Library Management](docs/user-guide/library-management.md)
- [Platform Differences (Web vs Desktop)](docs/user-guide/platform-differences.md)
- [LSP Server](docs/api/lsp-server.md)
- [Contributing](CONTRIBUTING.md)

---

## Status

This project is **experimental**. The DSL, APIs, and file formats may change between versions.

**What works well:**
- Interactive routing (manual trace drawing, segment/corner editing, 45° constraints)
- Trace persistence — routed traces are saved as `path` blocks in the `.cypcb` file, survive reload with bit-exact determinism
- Net constraints with IPC-2221 auto-width and DRC current validation
- Context-aware Monaco editor with syntax snippets and live bidirectional sync
- 3D viewer with KiCad-style copper, solder mask, and round trace caps

**What's in progress:**
- Copper pour / zone fill (grammar + ECS exist, renderer not yet)
- Autorouter (fundamental rewrite needed)

PRs welcome — whether it's a bug fix, new feature, or just a typo.

---

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.
