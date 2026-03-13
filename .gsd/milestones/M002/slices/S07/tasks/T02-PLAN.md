---
estimated_steps: 5
estimated_files: 8
---

# T02: Frontend lint + Vitest unit tests for pure modules

**Slice:** S07 — E2E Test Suite & Quality Gates
**Milestone:** M002

## Description

No frontend test or lint infrastructure exists. This task installs ESLint + Vitest, configures both, writes unit tests for the four pure-logic modules (viewport, hit-test, undo, url-state), and fixes any lint issues surfaced. These modules have zero DOM dependencies — they're pure math/data-structure code, ideal for fast unit tests that validate the test infrastructure works before Playwright E2E in T03.

## Steps

1. Install dev dependencies: `vitest`, `eslint`, `@typescript-eslint/eslint-plugin`, `@typescript-eslint/parser`, `typescript-eslint` (ESLint v9 flat config helper). Add `"test": "vitest run"`, `"lint": "eslint src/"` scripts to package.json.
2. Create `viewer/eslint.config.js` — ESLint v9 flat config for TypeScript. Enable recommended rules from `@typescript-eslint`. Exclude generated files and node_modules. Run `npx eslint src/` and fix all errors (likely: unused variables, missing return types on a few functions, any-typed params). Don't chase stylistic warnings that conflict with the existing codebase style — configure overrides where needed.
3. Create `viewer/vitest.config.ts` — extend from existing vite.config.ts, configure test include pattern `src/__tests__/**/*.test.ts`, set environment to `node` (pure functions, no DOM needed).
4. Write unit tests:
   - `viewer/src/__tests__/viewport.test.ts` — worldToScreen/screenToWorld roundtrip, zoomAtPoint preserves world-space cursor, pan offsets correctly, fitBoard produces viewport containing all board bounds
   - `viewer/src/__tests__/hit-test.test.ts` — hitTestTrace returns correct trace for point near segment, returns null for distant point, handles horizontal/vertical/diagonal segments, respects trace width
   - `viewer/src/__tests__/undo.test.ts` — push adds to stack, undo reverts, redo re-applies, undo past empty is no-op, redo past head is no-op, push after undo clears redo branch, capacity limit works
   - `viewer/src/__tests__/url-state.test.ts` — encodeViewState/decodeViewState roundtrip, handles edge values (zero, negative, very large), missing URL params return defaults
5. Run `npx vitest run` — verify ≥20 test cases pass. Run `npx eslint src/` — verify zero errors.

## Must-Haves

- [ ] ESLint v9 flat config created and passing on all viewer TypeScript
- [ ] Vitest configured and running
- [ ] ≥5 viewport tests (roundtrip, zoom, pan, fit)
- [ ] ≥4 hit-test tests (hit, miss, orientations, width)
- [ ] ≥7 undo tests (push, undo, redo, edge cases, capacity)
- [ ] ≥4 url-state tests (roundtrip, edge values, defaults)
- [ ] `npm run test` and `npm run lint` scripts work

## Verification

- `cd viewer && npx eslint src/` — exit 0, zero errors
- `cd viewer && npx vitest run` — all pass, ≥20 test cases
- `cd viewer && npx tsc --noEmit` — still passes (no type regressions)

## Inputs

- `viewer/src/viewport.ts` (118 lines) — pure math: worldToScreen, screenToWorld, zoomAtPoint, pan, fitBoard
- `viewer/src/hit-test.ts` (98 lines) — pure geometry: hitTestTrace with distance-to-segment
- `viewer/src/undo.ts` (308 lines) — UndoStack class with BoardCommand interface
- `viewer/src/url-state.ts` (39 lines) — URL encode/decode for view state
- `viewer/vite.config.ts` — existing Vite config to extend for Vitest

## Expected Output

- `viewer/eslint.config.js` — ESLint v9 flat config
- `viewer/vitest.config.ts` — Vitest config extending Vite
- `viewer/src/__tests__/viewport.test.ts` — viewport unit tests
- `viewer/src/__tests__/hit-test.test.ts` — hit-test unit tests
- `viewer/src/__tests__/undo.test.ts` — undo stack unit tests
- `viewer/src/__tests__/url-state.test.ts` — url-state unit tests
- `viewer/package.json` — updated with test/lint scripts and devDependencies
