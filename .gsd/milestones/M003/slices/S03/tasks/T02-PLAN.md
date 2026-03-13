---
estimated_steps: 5
estimated_files: 2
---

# T02: E2E routing tests with test fixture

**Slice:** S03 — Routing UX Upgrade
**Milestone:** M003

## Description

Create a deterministic routing test fixture (routing-test.cypcb) and write Playwright E2E tests exercising the full routing UX flow in headless Chromium. Tests verify via diagnostic surfaces (`__routingState`, `__renderDiag`, `__pcbEngine`), not pixel comparison — consistent with the established testing pattern from S01.

This task must run after T01 because it exercises the features T01 implements. E2E tests are the slice's primary proof: the milestone success criteria says "Routing flow verified by E2E test."

## Steps

1. **Create routing-test.cypcb fixture** — Write `viewer/e2e/fixtures/routing-test.cypcb` with 3 components (e.g., 2 resistors + 1 LED) and 3 nets connecting them. Use known component positions with generous spacing so pad clicks are reliable. Verify the fixture loads correctly via `__loadBoard` before writing tests.

2. **Write start/complete routing E2E test** — Load routing-test.cypcb via `__loadBoard`. Use `page.evaluate()` to get pad screen coordinates from snapshot component/pad positions + viewport worldToScreen transform. Click a pad to start routing → assert `__routingState.mode === 'routing'` and `__routingState.netName` is correct. Assert `__renderDiag.highlightedNet` matches the net. Click the target pad → assert `__routingState.mode === 'idle'` and `__renderDiag.highlightedNet` is null. Verify trace count increased.

3. **Write cancel routing E2E test** — Start route on a pad, verify routing mode, press Escape, assert mode returns to idle and highlightedNet is cleared. Verify trace count unchanged.

4. **Write keyboard toggle E2E tests** — Start route, press `A`, assert `__routingState.angleSnapEnabled === true`. Press `A` again, assert it's `false`. Press `F`, assert layer flipped in `__routingState`.

5. **Run full test suite** — Run `npx playwright test e2e/routing-ux.spec.ts` to verify all pass. Run `npx playwright test` to verify no regressions in existing tests. Run `npx vitest run` to confirm unit tests still pass.

## Must-Haves

- [ ] routing-test.cypcb fixture with ≥3 components and ≥3 nets at known positions
- [ ] E2E test: start route on pad → routing mode active + correct net + highlight set
- [ ] E2E test: complete route pad-to-pad → trace added + highlight cleared + idle mode
- [ ] E2E test: cancel route with Escape → idle mode + highlight cleared + no trace added
- [ ] E2E test: angle toggle with A key → angleSnapEnabled flips
- [ ] All tests use diagnostic surfaces, not pixel comparison
- [ ] Full Playwright suite passes (minus known errors.spec.ts:102 flake)

## Verification

- `cd viewer && npx playwright test e2e/routing-ux.spec.ts` — ≥5 tests pass
- `cd viewer && npx playwright test` — full suite passes (known flake excluded)
- `cd viewer && npx vitest run` — all unit tests still pass

## Inputs

- `viewer/src/routing.ts` — T01's extended state machine with __routingState diagnostic
- `viewer/src/main.ts` — T01's __loadBoard, __renderDiag, __routingState exposure
- `viewer/e2e/renderer-quality.spec.ts` — existing E2E patterns (page.evaluate for diagnostics, __loadBoard usage)
- S01 E2E patterns — diagnostic-driven assertions, no pixel comparison

## Expected Output

- `viewer/e2e/fixtures/routing-test.cypcb` — deterministic test fixture with 3 components, 3 nets
- `viewer/e2e/routing-ux.spec.ts` — ≥5 E2E tests covering start/complete/cancel/highlight/toggle routing flows
