# Competition Feature Matrix

**CodeYourPCB vs. 9 EDA Tools — Comprehensive Feature Comparison**

*Last updated: 2026-08-26. The CodeYourPCB column is measured; see **Verification** at the foot of this file. The other nine columns are read from vendor documentation and are not re-checked here.*

---

## Executive Summary

CodeYourPCB occupies a unique niche: **the only code-first, standalone, browser-native PCB design tool**. Our closest competitors either require KiCad as a backend (atopile, diodeinc/pcb) or are GUI-first tools that don't support code-driven design (KiCad, Altium, Allegro, EasyEDA). Flux.ai is the nearest overlap — browser-based with AI — but is GUI-first and subscription-only.

**Our strongest differentiators:**
- 🚀 Fully standalone — no KiCad dependency
- 🚀 Browser-native (WASM) + Tauri desktop, zero install
- 🚀 Integrated code editor (Monaco) + live PCB preview
- 🚀 Built-in autorouter - an in-house grid A*, no Java to install (FreeRouting stays available through DSN/SES)
- 🚀 Share-by-URL for instant collaboration

**Our biggest gaps (honest assessment):**
- ❌ No constraint solver / component auto-selection
- ❌ No SPICE simulation
- ❌ No package registry / community ecosystem
- ❌ Limited component library - an LCSC part number written on a component reaches the BOM, but nothing resolves a part number to a footprint
- ❌ No STEP or 3D model export, no IPC-2581, no ODB++
- ❌ No real-time multi-user collaboration
- ❌ No schematic capture (code-only entry)

---

## Tool Overview

| Tool | Type | License | Primary Language | KiCad Dependency |
|------|------|---------|-----------------|------------------|
| **CodeYourPCB** | Code-first EDA | Proprietary | Rust + TypeScript | None (standalone) |
| **atopile** | Code-first hardware HDL | MIT | Python | Yes (layout + DRC) |
| **diodeinc/pcb** | Code-first CLI + Zener DSL | Proprietary | Rust | Yes (layout) |
| **KiCad** | Full GUI EDA suite | GPL-3.0 | C++ | N/A (is KiCad) |
| **Altium Designer** | Enterprise GUI EDA | Commercial | Delphi/C++ | None |
| **Cadence Allegro** | Enterprise GUI EDA | Commercial | C++ | None |
| **Cadence OrCAD** | Mid-range GUI EDA | Commercial | C++ | None |
| **Autodesk EAGLE** | GUI EDA (sunset June 2026) | Commercial | C++ | None |
| **EasyEDA** | Browser-based GUI EDA | Freemium | JavaScript | None |
| **Flux.ai** | AI-powered browser EDA | Subscription | TypeScript | None |

---

## 1. DSL / Schematic Entry

| Feature | CodeYourPCB | atopile | diodeinc/pcb | KiCad | Altium | Allegro | OrCAD | EAGLE | EasyEDA | Flux.ai |
|---------|:-----------:|:-------:|:------------:|:-----:|:------:|:-------:|:-----:|:-----:|:-------:|:-------:|
| Code-based design entry | ✅ `.cypcb` DSL | ✅ `.ato` language | ✅ `.zen` (Starlark) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | 🔶 AI prompts |
| GUI schematic capture | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Module / hierarchy system | ✅ `module` + `use` | ✅ Full inheritance | ✅ Modules + imports | ✅ Hierarchical sheets | ✅ Multi-sheet | ✅ | ✅ | ✅ | ✅ | ✅ |
| Typed interfaces (I2C, SPI) | ✅ `interface`, enforced | ✅ First-class types | ✅ Typed nets | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Physical units in DSL | ✅ mm, mil, oz | ✅ `10kohm`, `3.3V` | ✅ `1kohm`, `0402` | N/A | N/A | N/A | N/A | N/A | N/A | N/A |
| Constraint assertions | ✅ `assert` | ✅ `assert within` | ✅ Checks + properties | ✅ Custom DRC rules | ✅ Constraint manager | ✅ ECSets | ✅ | 🔶 | 🔶 | 🔶 |
| LSP / IDE support | ✅ Hover, completion, go-to-definition, diagnostics | ✅ VS Code extension | ✅ Starlark LSP | ❌ | ❌ | ❌ | ❌ | ❌ | N/A | N/A |
| Embedded code editor | ✅ Monaco | ❌ (VS Code extension) | ❌ (CLI) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |

**Parity assessment:** Our DSL is simpler but complete end-to-end. atopile and diodeinc/pcb have richer language features (modules, types, units) but depend on KiCad for layout. v2 DSL features (modules, interfaces, constraints) are in progress and will close the gap significantly.

---

## 2. PCB Layout Editing

| Feature | CodeYourPCB | atopile | diodeinc/pcb | KiCad | Altium | Allegro | OrCAD | EAGLE | EasyEDA | Flux.ai |
|---------|:-----------:|:-------:|:------------:|:-----:|:------:|:-------:|:-----:|:-----:|:-------:|:-------:|
| Interactive PCB editor | ✅ Canvas viewer | ❌ (KiCad) | ❌ (KiCad) | ✅ Full | ✅ Full | ✅ Full | ✅ Full | ✅ Full | ✅ Full | ✅ Full |
| Component placement | ✅ Code-defined | ❌ (KiCad) | ❌ (KiCad) | ✅ GUI | ✅ GUI | ✅ GUI | ✅ GUI | ✅ GUI | ✅ GUI | ✅ AI-assisted |
| Interactive trace routing | 🔶 Click-to-route | ❌ (KiCad) | ❌ (KiCad) | ✅ Push & shove | ✅ Push & shove | ✅ Advanced | ✅ | ✅ | ✅ Push & shove | ✅ AI + manual |
| Ratsnest display | ✅ | ❌ (KiCad) | ❌ (KiCad) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Layer visibility toggle | ✅ | N/A | N/A | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Copper pour / zones | ✅ `zone`, islands flagged | ❌ (KiCad) | ❌ (KiCad) | ✅ Zone manager | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Differential pair routing | ❌ | ❌ (KiCad) | ❌ | ✅ | ✅ | ✅ | 🔶 Pro only | ✅ | 🔶 Pro only | ✅ |
| Length matching | 🔶 Diff-pair skew checked | ❌ | ❌ | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | 🔶 |
| Multi-layer support | ✅ 2 and 4-layer, blind/buried | ❌ (KiCad) | ❌ (KiCad) | ✅ Up to 32 | ✅ Up to 64 | ✅ Unlimited | ✅ Up to 32 | ✅ Up to 16 | ✅ Up to 34 | ✅ Up to 8 |

**Parity assessment:** Our interactive editing is functional but minimal compared to mature GUI tools. We're code-first by design — interactive routing is supplementary. The key gap is copper pour/zones and differential pair routing.

---

## 3. Autorouter

| Feature | CodeYourPCB | atopile | diodeinc/pcb | KiCad | Altium | Allegro | OrCAD | EAGLE | EasyEDA | Flux.ai |
|---------|:-----------:|:-------:|:------------:|:-----:|:------:|:-------:|:-----:|:-----:|:-------:|:-------:|
| Built-in autorouter | ✅ In-house A* (FreeRouting optional) | ❌ | ❌ | 🔶 External only | ✅ ActiveRoute | ✅ | ❌ Standard | ✅ | ✅ | ✅ AI auto-layout |
| Grid-based pathfinding | ✅ A* with cost model | N/A | N/A | N/A | ✅ | ✅ | N/A | ✅ | ✅ | ✅ ML-based |
| Signal class awareness | ✅ | N/A | N/A | ✅ Net classes | ✅ Net classes | ✅ ECSets | ✅ | 🔶 | 🔶 | ✅ |
| Post-route optimization | ✅ Via minimization | N/A | N/A | ❌ | ✅ | ✅ | ❌ | 🔶 | 🔶 | ✅ |

**Parity assessment:** 🚀 Our integrated autorouter is a significant advantage over atopile and diodeinc/pcb (both require manual routing in KiCad). We're competitive with mid-range tools but lack the sophistication of Altium/Allegro constraint-driven routing.

---

## 4. Design Rule Check (DRC)

| Feature | CodeYourPCB | atopile | diodeinc/pcb | KiCad | Altium | Allegro | OrCAD | EAGLE | EasyEDA | Flux.ai |
|---------|:-----------:|:-------:|:------------:|:-----:|:------:|:-------:|:-----:|:-----:|:-------:|:-------:|
| Built-in DRC engine | ✅ `cypcb-drc` | ❌ (KiCad) | 🔶 Diagnostics | ✅ ~40 rule types | ✅ Comprehensive | ✅ Advanced | ✅ | ✅ | ✅ | ✅ |
| Clearance check | ✅ | N/A | 🔶 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Trace width check | ✅ | N/A | 🔶 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Drill size check | ✅ | N/A | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Annular ring check | ✅ | N/A | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Connectivity check | ✅ | N/A | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Conditional rules | ❌ | ❌ | ❌ | ✅ Expression-based | ✅ Constraint manager | ✅ ECSets | ❌ | ❌ | ❌ | ❌ |
| Incremental DRC | 🔶 run_drc_incremental | N/A | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | 🔶 | 🔶 |
| DRC markers on canvas | ✅ | N/A | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |

**Parity assessment:** Our DRC is solid for a code-first tool — clearance, width, drill, annular ring, connectivity, and edge clearance are all implemented. Missing: silk clearance, hole-to-hole, conditional rules, and severity levels. Competitive with EAGLE/EasyEDA, behind KiCad/Altium/Allegro in breadth.

---

## 5. 3D Viewer

| Feature | CodeYourPCB | atopile | diodeinc/pcb | KiCad | Altium | Allegro | OrCAD | EAGLE | EasyEDA | Flux.ai |
|---------|:-----------:|:-------:|:------------:|:-----:|:------:|:-------:|:-----:|:-----:|:-------:|:-------:|
| 3D board visualization | ✅ Three.js | ❌ (KiCad) | ❌ | ✅ OpenCascade | ✅ Native | 🔶 Via Allegro 3D | ❌ | 🔶 | ✅ WebGL | ✅ Custom 3D |
| Component 3D models | 🔶 Generic bodies, GLTF when assigned | N/A | ❌ | ✅ STEP/VRML | ✅ STEP | ✅ | ❌ | 🔶 | ✅ | ✅ |
| Layer stack-up view | ✅ Copper layers | N/A | ❌ | ✅ | ✅ | ✅ | 🔶 | 🔶 | ✅ | ✅ |
| STEP/3D model export | ❌ | ❌ | ❌ | ✅ STEP/VRML/BREP | ✅ STEP/Parasolid | ✅ | ❌ | ❌ | ❌ | ❌ |
| MCAD integration | ❌ | ❌ | ❌ | 🔶 | ✅ SOLIDWORKS/Creo | ✅ | ❌ | 🔶 Fusion 360 | ❌ | ❌ |

**Parity assessment:** Our 3D viewer is functional with copper layers, pads, vias, traces, and generic component bodies rendered in Three.js. We're ahead of atopile/diodeinc/pcb (no viewer) but behind KiCad/Altium (proper STEP models). STEP export is a gap.

---

## 6. Export Formats

| Feature | CodeYourPCB | atopile | diodeinc/pcb | KiCad | Altium | Allegro | OrCAD | EAGLE | EasyEDA | Flux.ai |
|---------|:-----------:|:-------:|:------------:|:-----:|:------:|:-------:|:-----:|:-----:|:-------:|:-------:|
| Gerber X2 | ✅ Native | ❌ (KiCad CLI) | ❌ (KiCad) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Excellon drill | ✅ | ❌ (KiCad) | ❌ (KiCad) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| BOM (CSV/JSON) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Pick & Place (CPL) | ✅ | ❌ (KiCad) | ❌ (KiCad) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| IPC-2581 | ❌ | ❌ | ✅ `ipc2581` crate | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| ODB++ | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| PDF/SVG output | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| DSN/SES (FreeRouting) | ✅ | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| KiCad format import | ✅ `cypcb-kicad` | ✅ Bi-directional | ✅ `pcb-kicad` | N/A | ✅ | 🔶 | 🔶 | ✅ | ✅ | 🔶 |

**Parity assessment:** 🚀 Native Gerber X2 + Excellon + BOM + CPL without requiring KiCad is a significant advantage over atopile/diodeinc. Gaps: IPC-2581, ODB++, PDF/SVG output.

---

## 7. Library Management

| Feature | CodeYourPCB | atopile | diodeinc/pcb | KiCad | Altium | Allegro | OrCAD | EAGLE | EasyEDA | Flux.ai |
|---------|:-----------:|:-------:|:------------:|:-----:|:------:|:-------:|:-----:|:-----:|:-------:|:-------:|
| Built-in component library | 🔶 Basic footprints | ✅ Package registry | ✅ Stdlib + API | ✅ Thousands | ✅ Octopart/Vault | ✅ | ✅ | ✅ | ✅ 700K+ | ✅ 750K+ |
| Community package registry | ❌ | ✅ packages.atopile.io | ❌ | ✅ KiCad libraries | ✅ Altium 365 | ❌ | ❌ | ❌ | ✅ OSHWLab | ✅ Community |
| LCSC/Mouser integration | 🔶 LCSC part in the BOM | ✅ LCSC auto-pick | ✅ Diode API | ❌ | ✅ Octopart | ❌ | ❌ | ❌ | ✅ LCSC + JLCPCB | ✅ Multi-supplier |
| Custom footprint creation | ✅ Code-defined | ✅ | ✅ | ✅ GUI editor | ✅ IPC wizard | ✅ | ✅ | ✅ | ✅ | ✅ |
| 3D model assignment | ✅ JLCPCB parts, drawn by refdes | ❌ | ❌ | ✅ STEP/VRML | ✅ | ✅ | ❌ | 🔶 | ✅ | ✅ |

**Parity assessment:** ❌ This is our weakest category. We have basic footprint support but no component catalog, no supplier integration, and no community registry. EasyEDA (700K+) and Flux.ai (750K+) have massive libraries. atopile has LCSC auto-picking. This is a critical gap for adoption.

---

## 8. Collaboration

| Feature | CodeYourPCB | atopile | diodeinc/pcb | KiCad | Altium | Allegro | OrCAD | EAGLE | EasyEDA | Flux.ai |
|---------|:-----------:|:-------:|:------------:|:-----:|:------:|:-------:|:-----:|:-----:|:-------:|:-------:|
| Share-by-URL | 🚀 Yes | ❌ | ❌ | ❌ | ✅ Altium 365 | ❌ | ❌ | ❌ | ✅ | ✅ |
| Real-time multi-user | ❌ | ❌ | ❌ | ❌ | ✅ PCB CoDesign | ❌ | ❌ | ❌ | ✅ | ✅ |
| Version control friendly | ✅ Text-based DSL | ✅ Text-based | ✅ Text-based | 🔶 S-expr text | ❌ Binary | ❌ Binary | ❌ Binary | ❌ Binary | ❌ Cloud-only | 🔶 Cloud VCS |
| Git workflow | ✅ | ✅ | ✅ | 🔶 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| MCP server (AI agents) | ❌ | ✅ `ato mcp` | ✅ `pcb-mcp` | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ Copilot |

**Parity assessment:** 🚀 Share-by-URL and git-friendly text DSL are genuine advantages. Code-first tools (us, atopile, diodeinc) are inherently better for version control. Gap: no real-time multi-user editing, no MCP server for AI integration.

---

## 9. Platform Support

| Feature | CodeYourPCB | atopile | diodeinc/pcb | KiCad | Altium | Allegro | OrCAD | EAGLE | EasyEDA | Flux.ai |
|---------|:-----------:|:-------:|:------------:|:-----:|:------:|:-------:|:-----:|:-----:|:-------:|:-------:|
| Browser (zero install) | 🚀 WASM viewer | ❌ | 🔶 WASM (pcb-zen-wasm) | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | ✅ |
| Desktop app | ✅ Tauri, 8.9MiB binary | ❌ (Python CLI) | ✅ Native CLI | ✅ Native | ✅ Native | ✅ Native | ✅ Native | ✅ Native | ✅ Electron | ❌ Browser only |
| CLI | ✅ `cypcb` | ✅ `ato` | ✅ `pcb` | ✅ `kicad-cli` | 🔶 Limited | 🔶 | 🔶 | ❌ | ❌ | ❌ |
| Windows | ✅ | ✅ (WSL) | ✅ (WSL) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| macOS | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ |
| Linux | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ | ✅ |

**Parity assessment:** 🚀 Browser + Tauri + CLI is the most versatile platform story of any code-first tool. Only EasyEDA and Flux.ai also run in browsers, but they're not code-first. KiCad/Altium/Allegro are desktop-only.

---

## 10. Pricing

| Tool | Free Tier | Paid | Model |
|------|-----------|------|-------|
| **CodeYourPCB** | ✅ Open / free viewer | TBD | Local-first |
| **atopile** | ✅ MIT open source | Free | Open source |
| **diodeinc/pcb** | 🔶 Source available | TBD | CLI tool |
| **KiCad** | ✅ Fully free | Free | Open source (GPL-3.0) |
| **Altium Designer** | ❌ | ~$4,000–$12,000/yr | Subscription/perpetual |
| **Cadence Allegro** | ❌ | ~$4,000+/yr | Subscription/perpetual |
| **Cadence OrCAD** | 🔶 Academic free | ~$1,280+/yr | Subscription/perpetual |
| **EAGLE** | ❌ (sunset June 2026) | Included with Fusion 360 (~$545/yr) | Subscription |
| **EasyEDA** | ✅ Free (ad-supported) | $20–$40/mo Pro | Freemium |
| **Flux.ai** | 🔶 Free trial | $20–$158/mo | Subscription + ACU usage |

**Parity assessment:** We compete well on pricing with the free/open-source tier. atopile and KiCad are free. Commercial tools range from $545/yr (EAGLE/Fusion) to $12,000/yr (Altium/Allegro).

---

## 11. Extensibility

| Feature | CodeYourPCB | atopile | diodeinc/pcb | KiCad | Altium | Allegro | OrCAD | EAGLE | EasyEDA | Flux.ai |
|---------|:-----------:|:-------:|:------------:|:-----:|:------:|:-------:|:-----:|:-----:|:-------:|:-------:|
| Plugin / extension API | ❌ | 🔶 Python modules | 🔶 Starlark extensible | ✅ Python API | ✅ Delphi scripts | ✅ SKILL language | 🔶 | ✅ ULP scripts | ❌ | ❌ |
| SPICE simulation | ❌ | ❌ | ✅ `pcb-sim` crate | ✅ Ngspice | ✅ Keysight SI/PI | ✅ Sigrity | 🔶 PSpice | ✅ | ✅ Ngspice | ✅ |
| Signal integrity analysis | ❌ | ❌ | ❌ | 🔶 | ✅ Keysight | ✅ Sigrity | ❌ | ❌ | ❌ | 🔶 |
| Trace width calculator | ✅ IPC-2221 | ❌ | ❌ | ❌ | 🔶 | 🔶 | ❌ | ❌ | ❌ | ❌ |
| API / headless mode | ✅ `PcbEngine`: load, drc, route, export | ✅ Python API | ✅ Rust crates | ✅ kicad-cli | 🔶 | 🔶 | ❌ | ❌ | ❌ | ✅ |
| AI / LLM integration | ❌ | ✅ MCP server | ✅ MCP server | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ Copilot + agents |

**Parity assessment:** We have good headless/API capabilities (WASM, CLI) and a unique trace width calculator. Gaps: no simulation, no plugin system, no AI integration. Flux.ai leads on AI features; KiCad/Altium/Allegro lead on plugin ecosystems.

---

## Parity Summary Heatmap

| Category | vs atopile | vs diodeinc | vs KiCad | vs Altium | vs Allegro | vs OrCAD | vs EAGLE | vs EasyEDA | vs Flux.ai |
|----------|:---------:|:-----------:|:--------:|:---------:|:----------:|:--------:|:--------:|:----------:|:----------:|
| DSL/Schematic | 🔶 | 🔶 | 🚀 | 🚀 | 🚀 | 🚀 | 🚀 | 🚀 | 🚀 |
| PCB Layout | 🚀 | 🚀 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Autorouter | 🚀 | 🚀 | 🔶 | ❌ | ❌ | 🔶 | 🔶 | 🔶 | 🔶 |
| DRC | 🚀 | 🚀 | 🔶 | ❌ | ❌ | 🔶 | 🔶 | 🔶 | 🔶 |
| 3D Viewer | 🚀 | 🚀 | ❌ | ❌ | ❌ | 🚀 | 🚀 | 🔶 | 🔶 |
| Export | 🚀 | 🔶 | 🔶 | ❌ | ❌ | 🔶 | 🔶 | 🔶 | 🔶 |
| Library Mgmt | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| Collaboration | 🚀 | 🚀 | 🚀 | 🔶 | 🚀 | 🚀 | 🚀 | 🔶 | ❌ |
| Platform | 🚀 | 🔶 | 🚀 | 🚀 | 🚀 | 🚀 | 🚀 | 🔶 | 🔶 |
| Pricing | 🔶 | 🔶 | ❌ | 🚀 | 🚀 | 🚀 | 🚀 | 🔶 | 🚀 |
| Extensibility | 🔶 | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |

Legend: 🚀 = CodeYourPCB advantage | ✅ = parity | 🔶 = partial | ❌ = competitor advantage

---

## Prioritized Gap List for S07/S08

### Priority 1 — Critical for Adoption (S07)

1. **Component library expansion** — Integrate with at least one supplier API (LCSC or Mouser) for real-time component search, footprint download, and BOM costing. This is the single biggest adoption blocker.
2. **v2 DSL: Modules + typed interfaces** — Complete the module system with `import`, hierarchical composition, and typed interface validation. Required to match atopile/diodeinc language power.
3. **Copper pour / zone fill** — Required for any real 2+ layer design. Ground planes are table stakes.

### Priority 2 — Competitive Parity (S07/S08)

4. **PDF/SVG export** — Manufacturing documentation output. Straightforward addition to the export pipeline.
5. **IPC-2581 export** — Modern manufacturing interchange format. diodeinc already has this.
6. **MCP server** — AI agent integration. Both atopile and diodeinc have this. Enables AI-assisted design workflows.
7. **v2 DSL: Physical units + constraint assertions** — Type-safe `10kohm` values and `assert within` checks. Close the language gap with atopile.

### Priority 3 — Differentiation (S08+)

8. **SPICE simulation** — Circuit simulation integrated with the viewer. Would leapfrog atopile/diodeinc (neither has simulation).
9. **Real-time collaboration** — WebSocket-based multi-user editing on shared designs.
10. **Plugin/extension API** — Allow community-contributed DRC rules, export formats, and DSL extensions.
11. **STEP model export** — 3D model output for MCAD integration.
12. **Differential pair routing** — Required for high-speed designs. Currently blocks use in USB/HDMI/DDR projects.

### Not Prioritized (Out of Scope)

- GUI schematic capture — We are code-first by design. Adding a schematic editor would dilute our identity.
- Full constraint solver — atopile's component auto-selection is impressive but architecturally different from our approach. We focus on layout verification, not component selection.
- Signal integrity analysis — Enterprise feature; requires significant investment. Target if/when enterprise customers appear.

---

## Methodology

- **Open-source tools** (atopile, diodeinc/pcb, KiCad): Analyzed from cloned repositories, source code inspection, and official documentation.
- **Commercial tools** (Altium, Allegro, OrCAD, EAGLE): Analyzed from official feature pages, documentation, release notes, and industry reviews.
- **CodeYourPCB**: Audited from actual crate structure, viewer source, DSL examples, and WASM API surface.
- **Honesty policy**: Features are marked as present only if they are implemented and functional, not planned or stubbed.

---

## Verification

The CodeYourPCB column is the only one this repository can measure, and every
claim in it that changed on 2026-08-26 was changed against a command. The rows
about the language are guarded by
`cargo test -p cypcb-cli --test the_matrix_is_honest_about_us`, which fails if
a cell is downgraded or if the construct behind it stops working.

| Claim | Command |
|---|---|
| modules, interfaces, assertions, pours | `./target/release/cypcb check examples/{v2-modules,v2-interfaces,v2-constraints,pour-island}.cypcb` |
| physical units | `crates/cypcb-parser/src/reader.rs` reads `mm`, `mil` and `oz`; a bare number is millimetres |
| 4-layer boards | `examples/four-layer.cypcb`, `examples/blind-via.cypcb`, `tests/fixtures/benchmark/multi_ic.kicad_pcb` |
| in-house autorouter | `./target/release/cypcb route --help` - `--in-house` is what a run does anyway; D1 closed on 2026-08-09 in favour of it |
| LCSC in the BOM | `crates/cypcb-export/src/bom/csv.rs` writes the `LCSC Part #` column |
| what export does **not** write | `./target/release/cypcb export --dry-run examples/blink.cypcb` - Gerber, Excellon, BOM, CPL and a job file, and nothing else |
| what the language server answers | `cargo test -p cypcb-lsp --test the_manual_matches_the_server` - the manual and the server's `initialize` result are held to each other in both directions. Hover, completion and go-to-definition are what it advertises; references, rename, formatting and semantic tokens are not implemented |
| what the 3D view draws | A part placed from the JLCPCB panel registers its EasyEDA model uuid: `viewer/src/main.ts` calls `register3DModel(pkg, footprint.modelUuid)`, `viewer/src/wasm.ts` hands it to the engine (replaying whatever arrived before the engine existed), the snapshot carries it as `model_3d`, and `renderer3d.ts` fetches the OBJ and replaces the placeholder mesh named for that component's refdes. Nothing assigns a model from the language |
| what the browser API exposes | `cargo test -p cypcb-cli --test the_matrix_is_honest_about_us` holds the row against `crates/cypcb-render/src/lib.rs`, where `PcbEngine`'s `#[wasm_bindgen]` methods are declared. `Browser (zero install)` is the one row here nobody can test from a command line: it is a claim about how the thing is delivered, not about what it does |
| the desktop binary's size | `cargo build --release -p cypcb-desktop && ls -l target/release/cypcb-desktop` -> **9,333,064 bytes** on x86_64 Linux, 2026-08-26. That is the executable; a packaged bundle carries more |

Last verified: 2026-08-26.
