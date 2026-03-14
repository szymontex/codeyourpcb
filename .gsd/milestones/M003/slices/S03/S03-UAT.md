# S03: Routing UX Upgrade — UAT

**Milestone:** M003
**Written:** 2026-03-13

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: All routing UX features are verified through 6 Playwright E2E tests using diagnostic surfaces (__routingState, __renderDiag, __viewport). The E2E tests exercise the full browser pipeline — Canvas rendering, keyboard events, mouse interaction, state machine transitions. Human visual verification is not required; the diagnostic surfaces expose all routing state that matters.

## Preconditions

- `cd viewer && npm run dev` running (or `npx playwright test` which starts its own server)
- Playwright installed (`npx playwright install chromium`)

## Smoke Test

Run `cd viewer && npx playwright test e2e/routing-ux.spec.ts` — 6/6 tests pass. This confirms routing lifecycle (start, complete, cancel), net highlighting, keyboard handlers (A, F, Escape), and state machine transitions all work.

## Test Cases

### 1. Start route on pad → routing mode active + net highlight set

1. Load routing-test.cypcb via `__loadBoard`
2. Click on R1 pad 1 position
3. **Expected:** `__routingState.mode === 'routing'`, `__routingState.netName === 'POWER'`, `__renderDiag.highlightedNet === 'POWER'`, `targetPadsCount > 0`

### 2. Complete route pad-to-pad → trace added + highlight cleared

1. Load routing-test.cypcb, start route on R1 pad 1
2. Click on R2 pad 1 (same POWER net)
3. **Expected:** `__routingState.mode === 'idle'`, `__renderDiag.highlightedNet === null`, trace count increased by 1

### 3. Cancel route with Escape → idle + highlight cleared

1. Start route on a pad
2. Press Escape
3. **Expected:** `__routingState.mode === 'idle'`, `__renderDiag.highlightedNet === null`, no trace added

### 4. Angle toggle with A key

1. Start route on a pad
2. Check `__routingState.angleSnapEnabled` (should be false — default OFF)
3. Press A
4. **Expected:** `__routingState.angleSnapEnabled === true`
5. Press A again
6. **Expected:** `__routingState.angleSnapEnabled === false`

### 5. Layer flip with F key

1. Start route on a pad
2. Note `__routingState.currentLayer`
3. Press F
4. **Expected:** `__routingState.currentLayer` toggled (Top↔Bottom)

### 6. Fixture loads with expected structure

1. Load routing-test.cypcb via `__loadBoard`
2. Read snapshot from `__pcbEngine`
3. **Expected:** 3 components (R1, R2, LED1), nets include POWER/SIGNAL/GROUND

## Edge Cases

### Route start outside any pad

1. Click on empty board area (not on a pad)
2. **Expected:** `__routingState.mode` remains `'idle'`, no routing started

### A key pressed while not routing

1. Ensure mode is idle
2. Press A
3. **Expected:** No state change, no error — routing guard prevents handler execution

### F key pressed while Monaco editor is focused

1. Focus the Monaco code editor
2. Press F
3. **Expected:** Character 'F' typed in editor, routing layer NOT flipped — editor guard prevents routing handler

## Failure Signals

- `npx playwright test e2e/routing-ux.spec.ts` reports any failure
- `__routingState.mode` stuck in 'routing' after Escape press
- `__renderDiag.highlightedNet` not null after route completion or cancellation
- `__routingState.targetPadsCount === 0` after starting a route on a pad that belongs to a multi-pad net

## Requirements Proved By This UAT

- None newly validated — routing UX upgrade is a quality improvement on existing validated "Manual trace editing" requirement

## Not Proven By This UAT

- Visual quality of snap indicator (pulsing circle appearance) — tested structurally, not visually
- Ratsnest emphasis visual appearance — verified via alpha/width params in code, not pixel comparison
- Performance under high pad count (500+ pads per net) — not tested with large boards
- Touch/stylus input for routing — only mouse tested

## Notes for Tester

- The known flake in `errors.spec.ts:102` is pre-existing and unrelated to routing. If it appears in a full suite run, ignore it.
- All E2E tests use diagnostic surfaces rather than pixel matching — they're deterministic and don't flake on headless rendering differences.
- The `routing-test.cypcb` fixture is intentionally simple (3 components, 3 nets) to keep tests fast and deterministic. Complex routing scenarios are covered by unit tests on the state machine.
