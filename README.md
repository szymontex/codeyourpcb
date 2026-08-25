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

// Routed traces saved in the file — survives reload, diffs cleanly
trace VCC {
    layer Top
    width 0.5mm
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
    width 0.5mm
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
| `.cypcb` DSL, read by a hand-written Rust parser | Done — the default since the C parser stopped reaching WASM; `--features tree-sitter-parser` builds the grammar instead and a differential test reads every example with both and compares the ASTs. `docs/one-parser.md` has the measurement behind the choice |
| Live preview — save file, board updates instantly | Done |
| 2D renderer (KiCad-style) | Done |
| 3D viewer — extruded copper, solder mask, round trace caps | Done |
| Interactive trace routing (45° constraint, obstacle dodge) | Done |
| Trace segment & corner editing (drag to reshape) | Done |
| Trace persistence — routed traces saved as `path` blocks, survive reload | Done |
| Bidirectional sync — edit trace code ↔ board updates in real-time | Done |
| Net constraints — `[width 0.5mm]`, `[current 2A]` per net | Done |
| IPC-2221 auto-width from current rating | Done |
| DRC — every rule in the registry: `grep -o 'Box::new(rules::[A-Za-z]*' crates/cypcb-drc/src/lib.rs` names them, because a list written here goes stale the next time one is added | Done — `grep -c 'Box::new(rules::' crates/cypcb-drc/src/lib.rs` counts them, because a number written here goes stale the next time one is added; `cypcb check` prints copper-on-copper apart from a gap under spec, gives an orphaned plane its size and corners, and says when a rule's number is this tool's own rather than the fab's |
| DRC — silkscreen clearance | Done — a Rust rule checks a footprint's own artwork against every other part's pads on the same side |
| Gerber / Excellon / BOM / pick-and-place export | Done — with a Gerber job file (`<board>-job.gbrjob`) at the root of the output, naming every Gerber and drill file, what each one is, its format and, for the images, its polarity - plus board size, layer count and, when the design declares a stackup, the material stackup and board thickness. Each file states its own function in its header and the job file reads it back, so the two cannot disagree |
| Monaco editor with context-aware completions | Done |
| JLCPCB parts search & placement | Done |
| Custom footprint definitions in DSL | Done |
| Project manager with templates | Done |
| Dark / Light theme | Done |
| Web app (WASM) | Done |
| Desktop app (Tauri v2) | Builds on macOS and Windows; the Linux build needs the GTK/webkit dev packages listed under Quick Start. Nothing here runs in CI, because there is none |
| Autorouter — PathFinder negotiated congestion, multi-layer | Routes the benchmark boards complete; **the toolbar button is hidden** while routing quality is worked on |
| KiCad component library import | Done |
| KiCad `.kicad_pcb` import | Done — `cypcb parse-kicad`, used by the routing benchmarks. Reads KiCad 5 through 10, including the tableless net form KiCad 10 introduced, where a pad names its net directly instead of pointing into a table |
| KiCad `.kicad_pcb` import to the language | Done — `cypcb from-kicad <board>.kicad_pcb` writes a `.cypcb` design: the board, a declared outline when it is not a plain rectangle, a `footprint` definition per part KiCad names, one `component` each with its value, placement, rotation and side, a `net` block per net, and the routed copper. The command re-reads what it wrote before reporting success. What a part *is* comes from its reference designator prefix, because KiCad does not record it, and `generic` where the prefix says nothing. A copper pour comes back as a `zone` block with the net and the layer it was poured on, named after that net where KiCad named it - `zone GND` bare and `zone "VBUS+"` quoted, because the language takes both; a board whose pads are named rather than numbered - a USB-C receptacle's A1, B4, S1 - comes back carrying those names, and a net called `VBUS+` or `D-` comes back quoted, because the language takes both and renaming either would move pins onto the wrong nets |
| KiCad `.kicad_pcb` export | Done — `cypcb to-kicad --preset <fab>` writes the outline, a footprint per part with its pads, nets and the face it is soldered to, every trace and via, the net list, every copper pour and keepout, the board thickness the design states, and a legend per part - its own artwork when the footprint has any, its courtyard outline when it does not, the same rule the Gerber writer follows. Free silkscreen text and 3D models are not written; KiCad fills its own defaults for what a file leaves out. With `--preset` it also writes the `.kicad_pro` beside the board, because that is where KiCad keeps design rules - a board file stating them is a board file pcbnew refuses to open. Open the board through that file and KiCad's own DRC measures it against the fab the design was checked for |
| Copper pour / ground planes | Done — a zone is filled against the copper on its layer, with the fab's clearance to foreign copper and thermal spokes to its own pads. The same geometry reaches the Gerbers, the viewer and the checker, which reports two planes shorted together and copper the fill left connected to nothing. See `examples/pour-island.cypcb` |
| Module system — reusable circuit blocks | Done — `module` defines one, `use M as N at 10mm, 5mm` places it, modules nest. See `examples/v2-modules.cypcb` |
| `import` — block libraries across files | Done — resolved relative to the importing file; modules, footprints and interfaces cross, a design's own board and parts do not. See `examples/v2-imports.cypcb` |
| `assert` — the design's own claims, checked | Done — `board.*`, `<part>.value` and `<net>.current/width/clearance`; anything else is reported as not checked rather than skipped |
| Parts engine — auto component picking from JLCPCB/LCSC | Partly — a design names the part with `lcsc "C7593"`, the viewer fetches that part's real footprint, and the bill of materials carries it in JLCPCB's own `LCSC Part #` column. Picking a part for you is planned |
| Two-sided assembly | Partly — `side bottom` on a component flips it onto the back of the board: its copper, solder mask, paste and silkscreen all move with it and the pick-and-place list says `Bottom`. The browser draws its legend in the bottom silkscreen colour and its pads on the bottom copper |
| Schematic generation from `.cypcb` | Planned |
| Differential pairs | Declared and checked — `diffpair USB { USB_DP USB_DM }`, and the checker measures the skew between the halves against the fab's length-match tolerance. Routing them alongside each other, and the gap between them, are still planned |
| CI/CD in GitHub Actions | Not planned — the quality gate is `scripts/quality-gate.sh`, run locally |

Routing quality on the six bundled KiCad benchmarks. The numbers move whenever
the router or the checker changes, so they are not written down here - this
prints them:

```bash
cargo test --release -p cypcb-autoroute --test benchmark_validation \
    -- benchmark_all_fixtures_drc --ignored --nocapture
```

Each line reads `N routes, V violations against R, S shorts against T, U
unrouted`, where `R` and `T` are ratchets: the values the board is not allowed
to exceed, declared in `DRC_RATCHETS` in that test and lowered as the router
improves. Every board routes to completion - `U` is 0 on all six, and that is
asserted, not observed.

A table of these numbers lived here and every row of it had gone stale: it
listed three of the six boards and figures the checker had since moved twice,
once by measuring copper instead of part bodies and once by seeing footprints
on imported boards at all. `docs/TRACKER.md` records each change with what it
moved.

`cypcb route` routes the board eleven ways and keeps the best,
because no one setting wins everywhere - measured across the six benchmark
boards, the winner differs between them. It costs roughly eleven times the wall
clock of a single run and buys, on `examples/blink.cypcb`, 5 violations with 3 shorts
against 9 with 6. `--fast` routes once when the wait matters more than the
board.

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

Open http://localhost:4321 — pick a template or open a `.cypcb` file.

To reach that server from another machine, name the host it will answer to:
`CYPCB_DEV_HOSTS=board.example.lan npm start`. Vite refuses a `Host` header it
was not told about, and the list is empty by default, so without this you get a
bare 403 from your own dev server.

### Desktop app (Tauri)

```bash
# Linux prerequisites
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev pkg-config

npm run dev:desktop   # dev mode
npm run build:desktop # release installer
```

### CLI

```bash
cargo run -p cypcb-cli -- check examples/blink.cypcb          # DRC, exit 1 on violations
cargo run -p cypcb-cli -- export examples/blink.cypcb -o out    # refuses a shorted board unless --force
cargo run -p cypcb-cli -- route examples/blink.cypcb --variants   # route, keep the best
cargo run -p cypcb-cli -- score examples/blink.routed.cypcb   # quality metrics as JSON
cargo run -p cypcb-cli -- export examples/blink.cypcb         # 14 manufacturing files
cargo run -p cypcb-cli -- parse examples/blink.cypcb          # the board, as JSON
cargo run -p cypcb-cli -- parse examples/blink.cypcb -o ast    # the AST instead
cargo run -p cypcb-cli -- parse-kicad tests/fixtures/benchmark/led_blink.kicad_pcb   # a KiCad board
cargo run -p cypcb-cli -- to-kicad examples/blink.cypcb       # the design as a KiCad board
cargo run -p cypcb-cli -- from-kicad tests/fixtures/benchmark/led_blink.kicad_pcb   # a KiCad board as a design
cargo run -p cypcb-cli -- watch examples/blink.cypcb          # check again on every save
```

`check`, `route` and `score` all take `--preset`, and an unknown
name prints the list. They use the same rules and agree on the same board:
`examples/blink.routed.cypcb` is 4 violations to `check --preset pcbway` and 4
to `score --preset pcbway`.

On an **unrouted** file they will not agree, and that is not a disagreement
about the rules. `score` routes the board before measuring it, so it reports
what its own routing came to - `examples/blink.cypcb` is 24 violations to
`check` and 9 to `score`, because the second one laid copper first.

`--preset` means two things, though, and the lists are not the same length.
`check`, `route` and `score` take a **design-rule** preset - what a house can
etch - and know eleven: `jlcpcb_standard_2layer`, `jlcpcb_standard_4layer`,
the two advanced variants, `pcbway_standard`, `oshpark_2layer`,
`oshpark_4layer`, `ipc_class1`, `ipc_class2`, `ipc_class3` and `prototype`,
with `jlcpcb`, `oshpark`, `pcbway` and `ipc1`..`ipc3` as short forms that name
what they resolved to. `export` takes a **file convention** preset - what a
house wants the Gerbers called and in what coordinate format - and only
`jlcpcb` and `pcbway` have been written down. So a board can be checked
against OSHPark and not yet exported for it; `cypcb export --house oshpark`
says exactly that.

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
│   ├── cypcb-parser       # Rust reader (default) + Tree-sitter grammar
│   ├── cypcb-world        # ECS board state (bevy_ecs)
│   ├── cypcb-drc          # Design rule checks
│   ├── cypcb-autoroute    # A*-based autorouter (WIP)
│   ├── cypcb-router       # Route application, and the DSN/SES bridge
│   ├── cypcb-calc         # Electrical calculations (IPC-2221)
│   ├── cypcb-export       # Gerber / Excellon / CPL export
│   ├── cypcb-kicad        # KiCad format import and export
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

## Landscape

CodeYourPCB sits in the emerging "PCB as code" space alongside a few other tools:

- **[JITX](https://www.jitx.com/)** — commercial, most mature code-first EDA. Constraint-driven routing, signal integrity, VS Code integration. Proprietary.
- **[atopile](https://atopile.io/)** — open-source `.ato` language with compiler, reusable modules, component picking. Uses KiCad for layout. MIT.
- **[tscircuit](https://tscircuit.com/)** — open-source React/TypeScript PCB framework. Very LLM-friendly (TypeScript is natural for models). MIT.

Where CodeYourPCB differs: own rendering engine (no KiCad dependency), runs in browser via WASM, interactive routing built-in, traces persist as readable DSL code. Where it's behind: no schematic capture, smaller component library.

---

## Documentation

- [Getting Started](docs/user-guide/getting-started.md)
- [Language Syntax](docs/SYNTAX.md)
- [Architecture](docs/architecture.md)
- [Routing](docs/routing.md) - what the autorouter does, the settings that pay, and the seventeen measured and dropped
- [One parser](docs/one-parser.md) - why the language is read twice, what each way of fixing that costs, and which one was chosen
- [Project Structure](docs/user-guide/project-structure.md)
- [Library Management](docs/user-guide/library-management.md)
- [Platform Differences (Web vs Desktop)](docs/user-guide/platform-differences.md)
- [Language server](docs/language-server.md) - the `cypcb-lsp` binary: what it answers, what it does not, and how to point an editor at it
- [Editor features in the browser](docs/api/lsp-server.md) - the WASM bridge that gives the built-in editor its diagnostics, without a server process
- [Contributing](CONTRIBUTING.md)

---

## Status

Experimental. The DSL and file format may change between versions.

The core loop works: write `.cypcb` → see board → route traces → save → reload
→ export. Traces are deterministic and a saved board reads back as the board
it was, which is checked end to end: routed copper reaches the Gerbers, the
cut path matches the outline the source declares, the mask opens over every
pad and the pick-and-place names the side each part is assembled on.

A copper pour is filled by the engine and drawn from it: the viewer sends the
zones it parsed and gets back the rectangles a fabricator receives, so the
screen cannot disagree with the Gerber. Keepouts are outlined, and copper a
pour left stranded is outlined too, because an island looks exactly like the
rest of the plane.

The main gap is the autorouter: its toolbar button stays hidden while routing
quality is worked on. `interface` is no longer one - a module that claims an
interface nobody defined, or claims one and does not expose all of its pins, is
reported by name.

PRs welcome.

---

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.
