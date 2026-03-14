---
id: T01
parent: S06
milestone: M002
provides:
  - Comprehensive competition feature matrix covering 9 EDA tools across 11 categories
  - Prioritized gap list for S07/S08 planning
key_files:
  - docs/competition-feature-matrix.md
key_decisions:
  - Library management identified as weakest category — supplier API integration (LCSC/Mouser) ranked as #1 adoption priority
  - GUI schematic capture explicitly out of scope — code-first is our identity
  - Copper pour/zone fill identified as critical gap for real multi-layer designs
patterns_established:
  - Parity status icons (✅ 🔶 ❌ 🚀) for consistent feature comparison
  - Per-category assessment format with honest self-evaluation
observability_surfaces:
  - none
duration: 40m
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T01: Competition Feature Matrix

**Researched and documented comprehensive feature matrix comparing CodeYourPCB against 9 EDA tools across 11 categories with honest parity assessment and prioritized gap list.**

## What Happened

Audited three open-source competitors from cloned repos (atopile features from `/workspace/competitors/atopile/`, diodeinc/pcb from `/workspace/competitors/pcb/`, KiCad from existing `docs/pcb-knowledge/competitors/` analysis) and built on the existing `atopile-vs-us.md` competitive analysis. Catalogued CodeYourPCB's actual capabilities from crate structure, viewer source, DSL examples, and WASM API surface — marked features as present only when implemented and functional.

Produced `docs/competition-feature-matrix.md` with: executive summary, tool overview table, 11 per-category comparison tables with parity status icons, a summary heatmap, and a prioritized 12-item gap list split into three priority tiers for S07/S08.

Key findings:
- Our strongest categories: platform support (browser + desktop + CLI), collaboration (share-by-URL + git-friendly), autorouter (built-in vs competitors needing KiCad), and export (native Gerber X2 without KiCad dependency).
- Our weakest category: library management — no supplier integration, no community registry, basic footprints only. This is the single biggest adoption blocker.
- Unique position: only code-first tool that is fully standalone (no KiCad dependency) and browser-native.

## Verification

- `test -f docs/competition-feature-matrix.md` — **PASS**
- `grep -c "atopile|KiCad|Altium|Allegro|OrCAD|EAGLE|EasyEDA|Flux|diodeinc"` — **65 matches** (all 9 tools present, each mentioned 14+ times)
- Category headings count (`## N.` pattern) — **11 categories** (DSL, layout, autorouter, DRC, 3D, export, library, collaboration, platform, pricing, extensibility)
- Prioritized gap list present with 3 tiers and 12 items

Slice-level checks (expected partial — T01 is task 1 of 4):
- `test -f docs/competition-feature-matrix.md` — **PASS**
- `highlightedNet` in renderer.ts — not yet (T03)
- `UndoStack|BoardCommand` in undo.ts — not yet (T02)
- `snapToGrid` in routing.ts — not yet (T02)
- `rotate_component` in lib.rs — not yet (T03)

## Diagnostics

None — this task produced a documentation artifact, not runtime code.

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `docs/competition-feature-matrix.md` — comprehensive feature matrix with 11 category tables, parity heatmap, and prioritized gap list
