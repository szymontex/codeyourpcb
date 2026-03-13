---
estimated_steps: 4
estimated_files: 10
---

# T04: PCB knowledge base documentation and competitor DRC analysis

**Slice:** S01 — PCB Knowledge Base & Design Rules
**Milestone:** M002

## Description

Create the structured `docs/pcb-knowledge/` markdown knowledge base covering IPC standards, manufacturer capabilities, signal integrity, thermal management, trace geometry best practices, and competitor DRC architecture analysis. This is the human-readable reference that future slices, contributors, and the AI agent itself will consult when making design decisions.

Each document covers a specific topic area with practical rules, tables, formulas, and sources. The competitor analysis captures architectural patterns from KiCad, Horizon EDA, and atopile — design patterns only, no copied code. Every document includes attribution and source links.

## Steps

1. Create `docs/pcb-knowledge/README.md` as the knowledge base index with a table of contents linking all topic docs and competitor analyses. Brief project context explaining this is the reference for `cypcb-rules` implementation. Create `docs/pcb-knowledge/ipc-standards.md` covering IPC-2221 (clearance/trace width/thermal formulas with the actual equations), IPC-2141 (impedance control — microstrip and stripline formulas), IPC-2222 (rigid board stackup, via aspect ratios). Include tables of key parameter values. Note accuracy limitations (paywall, approximations).

2. Create `docs/pcb-knowledge/manufacturer-capabilities.md` with full comparison table of all supported manufacturers (JLCPCB 2/4/6L standard and advanced, PCBWay, OSHPark 2/4L). Include min trace, clearance, drill, via, annular ring, silk width, edge clearance, copper weight, board thickness, and special capabilities (blind/buried vias, impedance control, flex). Every value sourced with URL and date. Create `docs/pcb-knowledge/trace-geometry.md` covering via fanout patterns, differential pair routing, length matching, teardrops, copper balancing, and trace tapering best practices.

3. Create `docs/pcb-knowledge/signal-integrity.md` covering the signal classification taxonomy (Digital/HighSpeed/Analog/Power/Differential) with routing rules per class — trace widths, clearances, layer preferences, shielding, guard traces, return path integrity. Include impedance targets for common interfaces (USB: 90Ω differential, Ethernet: 100Ω differential, DDR: 50Ω single-ended). Create `docs/pcb-knowledge/thermal-management.md` covering IPC-2221 current derating, copper pour thermal relief design, via stitching for thermal transfer, thermal pad patterns, and power plane design guidelines.

4. Create `docs/pcb-knowledge/competitors/README.md` with overview of analyzed competitors and what patterns we adopted. Create `docs/pcb-knowledge/competitors/kicad-drc.md` analyzing KiCad's DRC architecture — individual test organization, net class system, conditional rules, DRC marker system. Create `docs/pcb-knowledge/competitors/horizon-eda.md` analyzing Horizon EDA's JSON rule configuration and rule_match concept. Create `docs/pcb-knowledge/competitors/atopile-constraints.md` analyzing atopile's constraint solver approach vs. post-hoc DRC checking. Each doc notes: what patterns are relevant to us, what we adopted, license considerations.

## Must-Haves

- [ ] `docs/pcb-knowledge/README.md` has table of contents linking all docs
- [ ] IPC standards doc includes actual formulas (IPC-2221 trace width, clearance tables)
- [ ] Manufacturer capabilities doc has comparison table with source URLs and dates
- [ ] Signal integrity doc covers all 5 signal classes with routing rules
- [ ] Thermal management doc covers current derating and thermal relief
- [ ] Trace geometry doc covers differential pairs and via fanout
- [ ] 3 competitor analysis docs with architectural pattern descriptions (no copied code)
- [ ] Every doc has substantive content (not stubs) — minimum 100 lines each for main topics
- [ ] License/attribution notes in competitor analysis docs

## Verification

- `find docs/pcb-knowledge -name '*.md' | wc -l` >= 10
- `wc -l docs/pcb-knowledge/ipc-standards.md` >= 100 lines
- `wc -l docs/pcb-knowledge/manufacturer-capabilities.md` >= 100 lines
- `wc -l docs/pcb-knowledge/signal-integrity.md` >= 100 lines
- `grep -l 'Source:' docs/pcb-knowledge/manufacturer-capabilities.md` confirms sources present
- `grep -l 'License' docs/pcb-knowledge/competitors/*.md` confirms attribution in competitor docs
- `head -30 docs/pcb-knowledge/README.md` shows working table of contents

## Observability Impact

- Signals added/changed: None — documentation only
- How a future agent inspects this: `find docs/pcb-knowledge -name '*.md'` lists all docs, `cat docs/pcb-knowledge/README.md` for navigation
- Failure state exposed: None — static documentation

## Inputs

- S01 research findings: manufacturer specs, IPC standards, competitor analysis notes
- `crates/cypcb-rules/` — the implemented types that these docs describe and contextualize
- `/workspace/competitors/` — previously analyzed competitor source patterns (referenced but not copied)

## Expected Output

- `docs/pcb-knowledge/README.md` — knowledge base index
- `docs/pcb-knowledge/ipc-standards.md` — IPC-2221, IPC-2141, IPC-2222 reference
- `docs/pcb-knowledge/manufacturer-capabilities.md` — manufacturer comparison with sources
- `docs/pcb-knowledge/signal-integrity.md` — signal classification and routing rules
- `docs/pcb-knowledge/thermal-management.md` — current derating and thermal design
- `docs/pcb-knowledge/trace-geometry.md` — via fanout, differential pairs, best practices
- `docs/pcb-knowledge/competitors/README.md` — competitor analysis overview
- `docs/pcb-knowledge/competitors/kicad-drc.md` — KiCad DRC architecture patterns
- `docs/pcb-knowledge/competitors/horizon-eda.md` — Horizon EDA rule system patterns
- `docs/pcb-knowledge/competitors/atopile-constraints.md` — atopile constraint solver patterns
