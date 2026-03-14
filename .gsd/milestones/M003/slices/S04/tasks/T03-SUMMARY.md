---
id: T03
parent: S04
milestone: M003
provides:
  - 15 new E2E tests in ui-architecture.spec.ts covering View menu, Preferences modal, persistence, unit switching, grid visibility vs snap separation
  - All existing E2E tests updated for View-dropdown selectors — zero hardcoded toolbar-level layer references
key_files:
  - viewer/e2e/ui-architecture.spec.ts
  - viewer/e2e/app-load.spec.ts
  - viewer/e2e/board-interaction.spec.ts
  - viewer/e2e/renderer-quality.spec.ts
key_decisions:
  - reliability.spec.ts URL state tests left unchanged — they use page.evaluate(getElementById) which works regardless of DOM nesting, no View menu interaction needed
  - Persistence tests use page.evaluate to clear localStorage instead of addInitScript, since addInitScript re-executes on reload and would destroy the settings being tested
patterns_established:
  - View-menu-first pattern for E2E: tests that interact with layer/grid checkboxes open the View dropdown first via page.click('#view-menu-btn')
  - Helper function openViewMenu(page) encapsulates dropdown-open idempotency check
observability_surfaces:
  - window.__settings used by E2E tests to verify settings state without DOM scraping
duration: 1 context window
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T03: E2E tests and full-suite verification

**Wrote 15 E2E tests for S04 UI restructuring, updated 4 existing test files for View-dropdown selectors, full suite passes (73/73).**

## What Happened

Created `viewer/e2e/ui-architecture.spec.ts` with 15 test cases across 4 describe blocks: Toolbar Structure (2), View Menu (5), Preferences Modal (5), Persistence (3). Tests cover: essential buttons visible, layer checkboxes hidden in dropdown, View menu open/close via click/Escape/outside-click, layer toggle state, grid visibility toggle with __settings verification, Preferences open/close via X/Escape/backdrop, unit switching to mil, layer color change, unit persistence across reload, color persistence across reload, and grid visibility vs grid snap independence.

Updated 4 existing test files: `app-load.spec.ts` (replaced `#layer-top`/`#layer-bottom` visibility checks with `#view-menu-btn`/`#prefs-btn` and dropdown-hidden assertion), `board-interaction.spec.ts` (added `openViewMenu()` helper before all layer checkbox interactions), `renderer-quality.spec.ts` (added View menu open before layer toggle in performance test), `reliability.spec.ts` (no changes needed — getElementById works regardless of DOM position).

## Verification

- `npx tsc --noEmit` — zero errors
- `npx vitest run` — 109 unit tests passed (9 files)
- `npx playwright test e2e/ui-architecture.spec.ts` — 15/15 passed
- `npx playwright test` — 73/73 passed, zero failures

## Diagnostics

- `window.__settings` in browser console exposes current settings for debugging test assertions
- Persistence tests validate via __settings after reload, not DOM state, making them resilient to UI changes

## Deviations

- `renderer-quality.spec.ts` also needed updating (not listed in task plan) — its performance test toggled `#layer-top` directly
- Persistence tests initially used `addInitScript` to clear localStorage, which re-ran on reload and cleared the settings under test. Fixed by using `page.evaluate` for the initial clear instead.

## Known Issues

None.

## Files Created/Modified

- `viewer/e2e/ui-architecture.spec.ts` — NEW: 15 E2E tests for View menu, Preferences, persistence, unit switching
- `viewer/e2e/app-load.spec.ts` — MODIFIED: updated toolbar element checks for new structure
- `viewer/e2e/board-interaction.spec.ts` — MODIFIED: layer toggles open View menu first
- `viewer/e2e/renderer-quality.spec.ts` — MODIFIED: performance test opens View menu before layer toggle
