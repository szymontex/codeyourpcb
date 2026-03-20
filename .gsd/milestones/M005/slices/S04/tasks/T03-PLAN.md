---
estimated_steps: 5
estimated_files: 2
---

# T03: Enhance score panel with detailed metric breakdown

**Slice:** S04 — Variant Generation & Tuning via Worker
**Milestone:** M005

## Description

The current `showVariants()` in `variant-panel.ts` shows only three pieces of information per variant: the name, the composite score, and a terse `{via_count}v · {routes.length}r` string. The roadmap requires "score panel shows ranked results" with a meaningful metric breakdown. Users need to see DRC violations, smoothness percentage, via count, total trace length, and crossings to understand *why* one variant is better.

This task enhances the score panel display without changing any behavior — purely a presentation improvement.

## Steps

1. **Replace the terse metrics string** in `showVariants()` (in `viewer/src/variant-panel.ts`). Currently:
   ```typescript
   metricsEl.textContent = `${v.score.via_count}v · ${v.routes.length}r`;
   ```
   Replace with a detailed breakdown:
   ```typescript
   // Convert total_length from Nm to mm for display
   const lengthMm = (v.score.total_length / 1_000_000).toFixed(1);
   const smoothPct = (v.score.smoothness * 100).toFixed(0);
   metricsEl.textContent = `DRC: ${v.score.drc_violations} | Smooth: ${smoothPct}% | Vias: ${v.score.via_count} | ${lengthMm}mm | Cross: ${v.score.crossings}`;
   ```

2. **Add a CSS class** for the metrics detail line. Add `.variant-metrics` styling to ensure the text wraps cleanly and is visually secondary to the composite score. Check if `viewer/src/variant-panel.css` exists; if not, add inline styles via the element's `style` property:
   ```typescript
   metricsEl.style.fontSize = '11px';
   metricsEl.style.opacity = '0.7';
   metricsEl.style.display = 'block';
   metricsEl.style.marginTop = '2px';
   ```

3. **Make composite score visually prominent**. The `scoreEl` already shows the composite number. Ensure it stands out:
   ```typescript
   scoreEl.style.fontWeight = 'bold';
   scoreEl.style.fontSize = '14px';
   ```

4. **Update the variant row layout** to accommodate the two-line layout (name + score on top, metrics detail below). Change the row structure from horizontal single-line to a mini-card:
   ```typescript
   const topLine = document.createElement('div');
   topLine.style.display = 'flex';
   topLine.style.justifyContent = 'space-between';
   topLine.style.alignItems = 'center';
   topLine.appendChild(nameEl);
   topLine.appendChild(scoreEl);
   row.appendChild(topLine);
   row.appendChild(metricsEl);
   ```

5. **Type-check and test**: Run `npx tsc --noEmit` and `npx vitest run` to verify no regressions.

## Must-Haves

- [ ] Each variant row shows composite score prominently
- [ ] Each variant row shows a detail line with: DRC violations, smoothness %, via count, total length in mm, crossings
- [ ] total_length correctly converted from Nm to mm (divide by 1,000,000)
- [ ] smoothness correctly displayed as percentage (multiply by 100)
- [ ] Metrics detail line is visually secondary (smaller font, slightly dimmed)
- [ ] `npx tsc --noEmit` passes

## Verification

- `npx tsc --noEmit` — zero TypeScript errors
- `npx vitest run` — all tests pass (no regressions)
- Visual: variant rows show "DRC: N | Smooth: N% | Vias: N | N.Nmm | Cross: N" below the name+score line

## Inputs

- `viewer/src/variant-panel.ts` — current `showVariants()` with terse metrics display
- `VariantData.score` shape: `{ total_length, via_count, drc_violations, smoothness, crossings, layer_balance, composite }` — all numbers

## Observability Impact

- **Changed signal:** `.variant-metrics` text content changes from terse `Xv · Yr` to detailed `DRC: N | Smooth: N% | Vias: N | N.Nmm | Cross: N` — any test or agent inspecting this text must use the new format.
- **Inspection:** `document.querySelectorAll('.variant-metrics')` returns elements whose `textContent` contains pipe-separated metric fields. The composite score remains in `.variant-score`.
- **Failure visibility:** If `total_length` or `smoothness` are `NaN` or `undefined` on the `VariantData.score` object, the metrics line will display `NaN` — visible in the DOM as a debugging cue.
- **No new console logs or debug surfaces** — this is a pure presentation change on existing data.

## Expected Output

- `viewer/src/variant-panel.ts` — enhanced `showVariants()` with detailed metric breakdown per variant row, visually organized as name+score header with metrics detail line below
