---
id: T02
parent: S02
milestone: M003
provides:
  - E2E test asserting componentCount > 0 and meshCount > 1 after loading blink.cypcb in 3D
  - E2E test validating all four geometry counters are valid numbers ≥ 0
  - E2E test proving re-toggle reconstructs identical geometry counts
  - Shared helpers loadBlink(), activate3D(), getGeometryCounts() in three-d-view.spec.ts
key_files:
  - viewer/e2e/three-d-view.spec.ts
key_decisions:
  - "Geometry assertions use debug surface counters, not pixel comparison — deterministic and fast"
  - "Re-toggle test compares all five counters (componentCount, meshCount, padCount, viaCount, traceSegmentCount) for full consistency proof"
patterns_established:
  - "activate3D() helper: click button → wait for active class → waitForFunction on __renderer3d.isActive → settle delay"
  - "getGeometryCounts() reads all counters in a single page.evaluate call — avoids multiple roundtrips"
observability_surfaces:
  - none (tests consume existing debug surface, no new surfaces added)
duration: 10m
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T02: E2E tests for 3D geometry verification

**Added 3 E2E tests proving the 3D renderer produces real geometry from blink.cypcb — component bodies, valid debug counters, and consistent re-toggle reconstruction.**

## What Happened

Extended `viewer/e2e/three-d-view.spec.ts` with a new `3D Geometry Verification` describe block containing three tests:

1. **"3D view renders component geometry after loading board"** — loads blink.cypcb via `__loadBoard`, toggles 3D, asserts `componentCount > 0` and `meshCount > 1`. This is the definitive regression test for the empty green board bug.

2. **"debug surface reports valid geometry counts"** — validates the debug surface contract: all four counters (`componentCount`, `traceSegmentCount`, `padCount`, `viaCount`) are `typeof 'number'` and `≥ 0`.

3. **"3D toggle preserves geometry on re-toggle"** — toggles 3D, captures all five counters, toggles back to 2D, toggles to 3D again, asserts all counts match. Proves `clearBoardGroup()` + re-init reconstructs geometry deterministically.

Extracted shared helpers (`loadBlink`, `activate3D`, `getGeometryCounts`) at the top of the file to keep tests focused on assertions.

## Verification

- `cd viewer && npx playwright test e2e/three-d-view.spec.ts` — **6 passed** (3 existing + 3 new)
- `cd viewer && npx playwright test e2e/performance.spec.ts` — **2 passed** (FPS at 60fps)
- `cd viewer && npx playwright test` — **52 passed**, zero failures (full E2E suite)
- `cd viewer && npx vitest run --reporter=verbose` — **63 unit tests passed**

## Diagnostics

Tests consume the existing `window.__renderer3d` debug surface. No new diagnostic surfaces added. Test failures will show the actual counter values in Playwright's expect output.

## Deviations

None.

## Known Issues

None.

## Files Created/Modified

- `viewer/e2e/three-d-view.spec.ts` — added 3 geometry verification tests and shared helpers (loadBlink, activate3D, getGeometryCounts)
