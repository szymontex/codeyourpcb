---
id: T03
parent: S04
milestone: M005
provides:
  - Detailed metric breakdown in variant panel rows (DRC violations, smoothness %, via count, total length in mm, crossings)
  - Two-line mini-card layout per variant (name+score header, metrics detail below)
key_files:
  - viewer/src/variant-panel.ts
key_files_not_in_repo:
  - viewer/src/variant-panel.ts - deleted by a9e8c7a, `refactor(viewer): delete the variant panel, which nothing could reach`
key_decisions:
  - total_length displayed in mm (divided by 1,000,000 from Nm)
  - smoothness displayed as percentage (multiplied by 100)
patterns_established:
  - Inline styles for variant panel sub-elements (no separate CSS file)
  - Two-line card layout with flex topLine for name+score, block metricsEl below
observability_surfaces:
  - ".variant-metrics" textContent contains pipe-separated DRC/Smooth/Vias/length/Cross fields
duration: 10m
verification_result: passed
completed_at: 2026-03-19
blocker_discovered: false
---

# T03: Enhance score panel with detailed metric breakdown

**Replaced terse `Xv · Yr` metrics with detailed DRC/smoothness/vias/length/crossings breakdown per variant row using two-line mini-card layout**

## What Happened

Refactored `showVariants()` in `variant-panel.ts` to display a rich metric breakdown for each variant instead of the previous terse via-count-and-route-count string. Each variant row is now a two-line mini-card: the top line shows the variant name and bold composite score in a flex row, and the second line shows a detailed metrics string with DRC violations, smoothness percentage, via count, total trace length (converted from Nm to mm), and crossings. The metrics line uses smaller font size (11px) and reduced opacity (0.7) to remain visually secondary to the composite score.

## Verification

- `npx tsc --noEmit` — zero TypeScript errors
- `npx vitest run` — 138 tests passed, 12 test files, 0 failures
- `npx vite build` — built successfully in ~24s
- Code review: `total_length` divides by 1,000,000 for mm conversion; `smoothness` multiplied by 100 for percentage display

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `npx tsc --noEmit` | 0 | ✅ pass | 4.6s |
| 2 | `npx vitest run` | 0 | ✅ pass | 4.6s |
| 3 | `npx vite build` | 0 | ✅ pass | 24.5s |

## Diagnostics

- **Inspect metrics text:** `document.querySelectorAll('.variant-metrics')` — each element's `textContent` shows `DRC: N | Smooth: N% | Vias: N | N.Nmm | Cross: N`
- **Inspect composite score:** `.variant-score` elements have `fontWeight: bold` and `fontSize: 14px`
- **Failure signal:** If score fields are `undefined`/`NaN`, the metrics string will contain `NaN` — visible in DOM inspection
- No new console log lines or debug surfaces — purely a presentation change

## Deviations

None — implemented exactly as planned.

## Known Issues

None.

## Files Created/Modified

- `viewer/src/variant-panel.ts` — enhanced `showVariants()` with two-line mini-card layout, detailed metric breakdown, and visually prominent composite score
- `.gsd/milestones/M005/slices/S04/tasks/T03-PLAN.md` — added Observability Impact section (pre-flight fix)
