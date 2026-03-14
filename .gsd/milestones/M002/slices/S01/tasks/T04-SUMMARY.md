---
id: T04
parent: S01
milestone: M002
provides:
  - docs/pcb-knowledge/ structured knowledge base with 10 markdown documents
  - IPC-2221/2141/2222 standards reference with formulas, tables, and accuracy notes
  - Manufacturer comparison table with all 7 presets sourced with URLs and dates
  - Signal integrity guide covering all 5 SignalClass variants with routing rules
  - Competitor DRC analysis of KiCad, Horizon EDA/pcb, and atopile
key_files:
  - docs/pcb-knowledge/README.md
  - docs/pcb-knowledge/ipc-standards.md
  - docs/pcb-knowledge/manufacturer-capabilities.md
  - docs/pcb-knowledge/signal-integrity.md
  - docs/pcb-knowledge/thermal-management.md
  - docs/pcb-knowledge/trace-geometry.md
  - docs/pcb-knowledge/competitors/README.md
  - docs/pcb-knowledge/competitors/kicad-drc.md
  - docs/pcb-knowledge/competitors/horizon-eda.md
  - docs/pcb-knowledge/competitors/atopile-constraints.md
key_decisions:
  - Competitor analysis covers architectural patterns only — no code copied, license attribution in every doc
  - IPC formulas sourced from widely-published summaries with explicit accuracy limitation notes (standards are paywalled)
  - Manufacturer data pulled directly from implemented preset values in cypcb-rules to ensure docs match code
patterns_established:
  - Knowledge base cross-references Rust types (DesignConstraints fields, SignalClass variants, ViolationKind) for 1:1 doc-code mapping
  - Each manufacturer section includes source URL, verified date, and preset mapping to code
observability_surfaces:
  - "find docs/pcb-knowledge -name '*.md'" lists all docs
  - "cat docs/pcb-knowledge/README.md" for navigation
duration: ~25min
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T04: PCB knowledge base documentation and competitor DRC analysis

**Created 10-document structured knowledge base covering IPC standards, manufacturer capabilities, signal integrity, thermal management, trace geometry, and 3 competitor DRC architecture analyses.**

## What Happened

Built the complete `docs/pcb-knowledge/` knowledge base as specified in the task plan:

1. **README.md** — index with table of contents linking all 9 topic docs, project context, accuracy limitations note
2. **ipc-standards.md** (306 lines) — IPC-2221 clearance tables, trace width formula (I = k × ΔT^0.44 × A^0.725), current capacity reference tables, board class comparison, IPC-2141 impedance formulas (microstrip, stripline, differential), IPC-2222 stackup configs and annular ring requirements
3. **manufacturer-capabilities.md** (136 lines) — full comparison table of all 7 presets (JLCPCB std/adv 2L/4L, PCBWay, OSHPark 2L/4L) with every constraint value sourced from the actual preset code, detailed notes per manufacturer, "how to add a new manufacturer" guide
4. **trace-geometry.md** (276 lines) — via fanout patterns, differential pair routing rules, length matching techniques with bus-specific tolerances, teardrops, copper balancing, acid traps, routing layer strategy for 2/4/6-layer boards
5. **signal-integrity.md** (236 lines) — all 5 SignalClass variants with constraint tables and routing guidelines, impedance targets for 15 common interfaces, return path integrity rules, crosstalk mitigation (3W/5W rules)
6. **thermal-management.md** (274 lines) — IPC-2221 current derating formula and tables (1oz/2oz, internal/external), thermal relief design with spoke configurations, via stitching for thermal transfer with resistance calculations, QFN thermal pad patterns, power plane design and decoupling strategy
7. **competitors/README.md** — overview of what we analyzed, what we adopted, what we didn't
8. **competitors/kicad-drc.md** — test provider pattern, net class system, conditional rules, DRC markers, violation type mapping to our ViolationKind
9. **competitors/horizon-eda.md** — unified diagnostics pipeline, pass-based filtering, compact rendering, severity classification, Starlark configuration
10. **competitors/atopile-constraints.md** — constraint solver vs. post-hoc DRC, default override pattern (has_default_constraint), staged design checks, comparison table

All values in the manufacturer table were verified against the actual Rust preset implementations to ensure docs match code.

## Verification

All task-level checks pass:
- `find docs/pcb-knowledge -name '*.md' | wc -l` = 10 ✅
- `wc -l docs/pcb-knowledge/ipc-standards.md` = 306 (≥ 100) ✅
- `wc -l docs/pcb-knowledge/manufacturer-capabilities.md` = 136 (≥ 100) ✅
- `wc -l docs/pcb-knowledge/signal-integrity.md` = 236 (≥ 100) ✅
- `grep -l 'Source:' docs/pcb-knowledge/manufacturer-capabilities.md` confirms sources ✅
- `grep -l 'License' docs/pcb-knowledge/competitors/*.md` confirms attribution in all 3 ✅
- `head -30 docs/pcb-knowledge/README.md` shows working TOC ✅

Slice-level checks:
- `cargo test -p cypcb-rules` — 8 doc tests pass ✅ (no changes to code)
- `cargo test -p cypcb-drc` — 23 pass ✅ (no changes to code)
- `cargo build -p cypcb-rules -p cypcb-drc` — compiles ✅
- `test -d docs/pcb-knowledge && test -f docs/pcb-knowledge/README.md` — PASS ✅
- `cargo clippy -p cypcb-rules -- -D warnings` — clean ✅
- `cargo build --workspace` — pre-existing failure in gdk-sys (system dependency for GUI crate, not related)

## Diagnostics

- `find docs/pcb-knowledge -name '*.md'` lists all knowledge base docs
- `cat docs/pcb-knowledge/README.md` for navigation and table of contents
- Static documentation — no runtime behavior, no failure modes

## Deviations

None.

## Known Issues

- `cargo build --workspace` fails due to missing `gdk-sys` system library — pre-existing, unrelated to this task or slice

## Files Created/Modified

- `docs/pcb-knowledge/README.md` — knowledge base index with TOC
- `docs/pcb-knowledge/ipc-standards.md` — IPC-2221, IPC-2141, IPC-2222 reference
- `docs/pcb-knowledge/manufacturer-capabilities.md` — manufacturer comparison with sources
- `docs/pcb-knowledge/signal-integrity.md` — signal classification and routing rules
- `docs/pcb-knowledge/thermal-management.md` — current derating and thermal design
- `docs/pcb-knowledge/trace-geometry.md` — via fanout, differential pairs, best practices
- `docs/pcb-knowledge/competitors/README.md` — competitor analysis overview
- `docs/pcb-knowledge/competitors/kicad-drc.md` — KiCad DRC architecture patterns
- `docs/pcb-knowledge/competitors/horizon-eda.md` — Horizon EDA diagnostics patterns
- `docs/pcb-knowledge/competitors/atopile-constraints.md` — atopile constraint solver patterns
