# CodeYourPCB

**Code-first PCB design.** Describe your circuit board in a simple DSL — get a deterministic, git-friendly, AI-editable design.

```
// blink.cypcb
version 1

board blink {
    size 50mm x 30mm
    layers 2
}

component LED1 led "0805" { value "RED"; at 25mm, 15mm }
component R1 resistor "0402" { value "330"; at 15mm, 15mm }

net VCC [current 20mA] { R1.1 }
net LED_ANODE { R1.2; LED1.A }
net GND { LED1.K }
```

Save the file → board updates instantly. No compile step, no refresh.
Autorouted, DRC validated, Gerber export ready.

**New here?** → [Syntax reference](docs/SYNTAX.md) · [Getting started](docs/user-guide/getting-started.md) · [Examples](examples/)

Use your favorite IDE to edit `.cypcb` files — the board updates live on every save. Or use the built-in web editor.

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

`.cypcb` is different. The semantics are declarative and human-readable:

```
component U1 ic "SOIC-8" { value "NE555"; at 28mm, 20mm }
net VCC { U1.8; U1.4; R1.1 }
net GND  { U1.1; C1.2 }
```

An LLM can generate this, review it, refactor it, and catch mistakes — just like source code. The autorouter handles the geometry. The LLM handles the intent.

This makes CodeYourPCB a natural interface between AI agents and hardware design.

---

## Features

| Feature | Status |
|---------|--------|
| Live preview — save file, board updates instantly | ✅ |
| `.cypcb` DSL with Tree-sitter parser | ✅ |
| Canvas renderer (WebGL via WASM) | ✅ |
| Design Rule Check (DRC) | ✅ |
| FreeRouting autorouter integration | ✅ |
| Gerber / DSN / netlist export | ✅ |
| Monaco editor with syntax highlighting | ✅ |
| LSP (diagnostics, autocomplete, hover) | ✅ |
| Dark mode (WCAG AA) | ✅ |
| Web app (Cloudflare Pages) | ✅ |
| Desktop app (Tauri v2, Win/Mac/Linux) | ✅ |
| KiCad component library import | ✅ |
| JLCPCB library integration | ✅ |
| Share URL (viewport state) | ✅ |

---

## Quick Start

### Desktop (development build)

```bash
# Prerequisites: Rust, Node.js 18+, wasm-pack
cargo install wasm-pack

# Build WASM engine
cd viewer
wasm-pack build ../crates/cypcb-render --target web --out-dir src/wasm

# Run dev server
npm install
npm run dev
```

### Desktop app (Tauri)

```bash
# Linux prerequisites
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev pkg-config

npm run tauri dev   # dev mode
npm run tauri build # release installer (MSI / DMG / AppImage)
```

### CLI

```bash
cargo run -p cypcb-cli -- check examples/blink.cypcb   # DRC
cargo run -p cypcb-cli -- export examples/blink.cypcb  # Gerber
```

---

## Project Structure

```
codeyourpcb/
├── crates/
│   ├── cypcb-core        # Types, coordinates, units
│   ├── cypcb-parser      # Tree-sitter grammar + AST
│   ├── cypcb-world       # ECS board state (Bevy ECS)
│   ├── cypcb-drc         # Design rule checks
│   ├── cypcb-calc        # Electrical calculations
│   ├── cypcb-router      # FreeRouting bridge
│   ├── cypcb-export      # Gerber / DSN / netlist output
│   ├── cypcb-kicad       # KiCad format import
│   ├── cypcb-library     # Component library (SQLite + FTS5)
│   ├── cypcb-lsp         # Language server (tower-lsp)
│   ├── cypcb-render      # WebGL renderer (WASM)
│   ├── cypcb-platform    # Native/web abstraction layer
│   ├── cypcb-watcher     # File system watcher
│   └── cypcb-cli         # CLI entry point
├── viewer/               # TypeScript frontend (Vite)
├── src-tauri/            # Tauri desktop shell
├── examples/             # Sample .cypcb files
└── docs/                 # User guide & API reference
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

This project is **experimental**. The DSL, APIs, and file formats may change between versions. Use at your own risk.

All PRs are welcome — whether it's a bug fix, new feature, or just a typo. Open a PR and we'll figure it out together.

---

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.
