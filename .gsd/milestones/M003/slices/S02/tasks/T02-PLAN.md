# T02: E2E tests for 3D geometry verification

## Description

Extend the existing 3D view E2E tests to verify that the 3D renderer produces real geometry — not an empty green board. Uses the enriched `__renderer3d` debug surface (componentCount, meshCount, etc.) to assert geometry presence without fragile pixel comparison.

## Steps

1. **Add "3D view renders component geometry" test** — In `viewer/e2e/three-d-view.spec.ts`, add a test that: loads the app, waits for Ready, uses `page.evaluate(() => (window as any).__loadBoard(...))` with blink.cypcb source to ensure a board with components is loaded, clicks 3D button, waits for `__renderer3d.isActive`, then asserts `componentCount > 0` and `meshCount > 1`.

2. **Add "debug surface reports geometry counts" test** — Toggle 3D, verify that `componentCount`, `traceSegmentCount`, `padCount`, `viaCount` are all numbers (typeof === 'number') and ≥ 0. This validates the debug surface contract for downstream consumers.

3. **Add "3D toggle preserves geometry on re-toggle" test** — Toggle to 3D, capture componentCount and meshCount, toggle back to 2D, toggle to 3D again, verify counts match the first toggle (proves re-init reconstructs geometry correctly).

4. **Run full test suite** — Verify all new and existing tests pass: three-d-view.spec.ts, performance.spec.ts, and any other E2E tests.

## Must-Haves

- Test asserting `componentCount > 0` after loading blink.cypcb and toggling 3D
- Test asserting all four geometry counters are valid numbers
- Test asserting re-toggle produces consistent geometry counts
- All existing 3D toggle and dispose tests unchanged and passing
- Performance test (FPS ≥ 30) still passes

## Verification

- `cd viewer && npx playwright test e2e/three-d-view.spec.ts` — all tests pass
- `cd viewer && npx playwright test e2e/performance.spec.ts` — FPS test passes
- `cd viewer && npx playwright test` — full E2E suite passes

## Inputs

- T01 completed: body dimensions fixed, debug surface enriched with geometry counts
- Existing `__loadBoard()` function on window (from S01) for loading board source in tests
- blink.cypcb board has components (R1, R2, C1, LED1, U1 etc.)

## Expected Output

- `viewer/e2e/three-d-view.spec.ts` — extended with 3 new tests verifying 3D geometry
- All tests pass headless, proving the empty board bug is fixed and 3D renders real content
