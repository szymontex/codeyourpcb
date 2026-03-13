---
estimated_steps: 4
estimated_files: 5
---

# T03: E2E tests and full-suite verification

**Slice:** S04 — UI Architecture — Toolbar, View Menu & Settings
**Milestone:** M003

## Description

Write E2E tests proving the UI restructuring works end-to-end: View menu toggles, Preferences modal, unit switching, grid visibility separation from grid snap, settings persistence across reload. Update existing E2E tests whose selectors changed due to moved toolbar elements. Run the full test suite to verify zero regressions.

## Steps

1. Write `viewer/e2e/ui-architecture.spec.ts` — Test cases:
   - **Toolbar structure**: essential buttons visible (Editor, Undo, Redo, Fit, View, 3D, Theme, Prefs, Open), layer checkboxes NOT in toolbar
   - **View menu opens/closes**: click View button → dropdown visible, click again or press Escape → closes, click outside → closes
   - **Layer toggle via View menu**: open View menu, uncheck Top layer, verify rendering changes (via __renderDiag or snapshot), recheck
   - **Grid visibility toggle**: open View menu, toggle grid visible off, verify grid not drawn (check __renderDiag or visual), toggle on
   - **Preferences modal**: click Prefs button → modal opens, change unit to 'mil', close modal, verify coords display shows mil suffix
   - **Unit switching persistence**: set unit to mil, reload page, verify unit still mil (read from __settings or check coords display)
   - **Settings persist across reload**: change a layer color in Preferences, reload, verify color persists
   - **Grid visibility vs grid snap separation**: grid visible toggle doesn't affect routing snap; grid snap in Preferences doesn't affect grid visibility

2. Update `viewer/e2e/app-load.spec.ts` — Change `#layer-top` / `#layer-bottom` selectors to find them inside the View dropdown instead of toolbar (or update the check to verify View button exists).

3. Update `viewer/e2e/board-interaction.spec.ts` and `viewer/e2e/reliability.spec.ts` — Update all references to `#layer-top`, `#layer-bottom`, `#layer-ratsnest`, `#grid-snap` to access them through the View menu (click View button first to open dropdown, then interact with the control).

4. Run full test suite and fix any failures: `cd viewer && npx vitest run && npx playwright test`. Iterate until all pass.

## Must-Haves

- [ ] New E2E test file `ui-architecture.spec.ts` with ≥6 test cases
- [ ] Existing E2E tests updated for moved selectors — no hardcoded references to toolbar-level `#layer-top` etc.
- [ ] Full vitest suite passes
- [ ] Full Playwright suite passes (including pre-existing tests)
- [ ] TypeScript compiles clean

## Verification

- `cd viewer && npx vitest run` — all unit tests pass
- `cd viewer && npx playwright test e2e/ui-architecture.spec.ts` — new E2E tests pass
- `cd viewer && npx playwright test` — full suite passes (zero regressions beyond known pre-existing flake in errors.spec.ts:102)
- `cd viewer && npx tsc --noEmit` — zero errors

## Inputs

- `viewer/src/settings.ts` — T01 settings module with `window.__settings`
- `viewer/src/units.ts` — T01 unit system
- `viewer/index.html` — T02 restructured toolbar, View dropdown, Preferences modal
- `viewer/src/main.ts` — T02 wiring
- `viewer/e2e/app-load.spec.ts` — existing tests referencing `#layer-top`, `#layer-bottom`
- `viewer/e2e/board-interaction.spec.ts` — existing tests referencing layer checkboxes
- `viewer/e2e/reliability.spec.ts` — existing tests referencing layer/ratsnest checkboxes

## Expected Output

- `viewer/e2e/ui-architecture.spec.ts` — NEW: 6+ E2E tests for View menu, Preferences, units, persistence
- `viewer/e2e/app-load.spec.ts` — MODIFIED: updated selectors
- `viewer/e2e/board-interaction.spec.ts` — MODIFIED: updated selectors (open View menu before toggling layers)
- `viewer/e2e/reliability.spec.ts` — MODIFIED: updated selectors
