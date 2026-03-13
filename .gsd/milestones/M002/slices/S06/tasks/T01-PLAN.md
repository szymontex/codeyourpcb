---
estimated_steps: 4
estimated_files: 1
---

# T01: Competition Feature Matrix

**Slice:** S06 — Competition Feature Parity & UI Polish
**Milestone:** M002

## Description

Research and document a comprehensive feature matrix comparing CodeYourPCB against 9 EDA tools (atopile, KiCad, Altium Designer, Cadence Allegro, Cadence OrCAD, Autodesk EAGLE, EasyEDA, Flux.ai, diodeinc/pcb) across 11 feature categories. Use cloned repos for open-source tools, web research for commercial. Produce a structured markdown document with honest parity assessment that informs remaining S06 work and feeds into S07/S08 backlog.

## Steps

1. Audit open-source competitors from cloned repos: atopile (`/workspace/competitors/atopile/`), diodeinc/pcb (`/workspace/competitors/pcb/`), and existing KiCad analysis in `docs/pcb-knowledge/competitors/`. Extract feature lists across all 11 categories.
2. Research commercial tools (Altium, Allegro, OrCAD, EAGLE, EasyEDA, Flux.ai) via web search — official feature pages, manuals, and tutorial references. Focus on feature presence/absence, not deep implementation details.
3. Catalogue CodeYourPCB's current capabilities by reviewing existing crates, viewer features, DSL constructs, and export formats. Be honest about what's implemented vs stubbed.
4. Write `docs/competition-feature-matrix.md` with: executive summary, per-category comparison tables (rows=features, columns=tools), parity status icons (✅ parity, 🔶 partial, ❌ missing, 🚀 advantage), and a prioritized gap list for S07/S08.

## Must-Haves

- [ ] All 9 competitor tools represented
- [ ] 11 categories covered: DSL/schematic, PCB layout editing, autorouter, DRC, 3D viewer, export formats, library management, collaboration, platform support, pricing, extensibility
- [ ] Honest self-assessment — no inflated parity claims
- [ ] Prioritized gap list identifying features to address in S07/S08

## Verification

- `test -f docs/competition-feature-matrix.md` — file exists
- `grep -c "atopile\|KiCad\|Altium\|Allegro\|OrCAD\|EAGLE\|EasyEDA\|Flux\|diodeinc" docs/competition-feature-matrix.md` — all 9 tools mentioned
- Document has at least 11 category headings

## Inputs

- `/workspace/competitors/atopile/` — cloned atopile repo
- `/workspace/competitors/pcb/` — cloned diodeinc/pcb repo
- `docs/pcb-knowledge/competitors/` — existing KiCad/competitor analysis
- Web research for commercial tools

## Expected Output

- `docs/competition-feature-matrix.md` — comprehensive feature matrix with parity assessment and gap list
