---
id: T02
parent: S07
milestone: M002
provides:
  - ESLint v9 flat config for viewer TypeScript (zero errors)
  - Vitest configured with 40 passing unit tests across 4 pure modules
  - npm run test and npm run lint scripts
key_files:
  - viewer/eslint.config.js
  - viewer/vitest.config.ts
  - viewer/src/__tests__/viewport.test.ts
  - viewer/src/__tests__/hit-test.test.ts
  - viewer/src/__tests__/undo.test.ts
  - viewer/src/__tests__/url-state.test.ts
key_decisions:
  - "Disabled @typescript-eslint/no-explicit-any: codebase uses `any` sparingly for WASM interop and window debug surfaces — enforcing this would require significant refactoring unrelated to this task"
  - "Disabled @typescript-eslint/no-this-alias: renderer3d.ts uses `const self = this` in debug surface getters for closure capture, which is the correct pattern"
  - "Set Vitest environment to node: all tested modules are pure math/data-structures with zero DOM deps"
patterns_established:
  - "Test files live in viewer/src/__tests__/*.test.ts"
  - "Mock BoardCommand objects with vi.fn() tracking execute/undo calls via a shared log array"
  - "url-state tests mock window.location.search via Object.defineProperty on globalThis"
  - "ESLint ignores _ prefixed vars/args/caught errors (argsIgnorePattern, varsIgnorePattern, caughtErrorsIgnorePattern)"
observability_surfaces:
  - none
duration: 1 step
verification_result: passed
completed_at: 2026-03-13
blocker_discovered: false
---

# T02: Frontend lint + Vitest unit tests for pure modules

**Installed ESLint v10 + Vitest, fixed 5 lint errors, wrote 40 unit tests for viewport, hit-test, undo, and url-state modules — all passing.**

## What Happened

Installed eslint (v10), typescript-eslint, and vitest as dev deps. Created ESLint v9-style flat config with typescript-eslint recommended rules and project-specific overrides. Initial lint found 5 errors: unused type import in lsp-bridge, stale @ts-ignore, prefer-const, unused catch var, and this-alias in debug surface. Fixed all five.

Created vitest.config.ts extending the existing vite.config.ts. Wrote unit tests for all four pure modules:
- **viewport** (14 tests): worldToScreen/screenToWorld roundtrip, Y-axis flip, off-center viewport, zoomAtPoint cursor preservation + scale clamping, pan direction + magnitude, fitBoard centering + containment + zero guard, resizeViewport
- **hit-test** (7 tests): horizontal/vertical/diagonal segment hits, miss for distant point, trace width affecting hit radius, null snapshot, empty traces
- **undo** (10 tests): empty state, push+execute, undo, redo, undo-past-empty no-op, redo-past-head no-op, push-after-undo clears redo, capacity limit at 100, multi-step sequence, clear
- **url-state** (9 tests): encode typical state, single layer, integer rounding, zero values, roundtrip, large coordinates, missing layer returns null, defaults for missing params, negative zoom

## Verification

- `cd viewer && npx eslint src/` — exit 0, zero errors ✅
- `cd viewer && npx vitest run` — 4 files, 40 tests, all pass ✅
- `cd viewer && npx tsc --noEmit` — clean, no type regressions ✅
- `cargo fmt --check` — zero diffs (slice-level) ✅
- `cargo clippy --workspace --exclude cypcb-cli --exclude cypcb-desktop -- -D warnings` — zero warnings (slice-level) ✅
- `playwright test` — not yet set up (T03) ⏳
- `quality-gate.sh` — not yet created (T04) ⏳

## Diagnostics

None — this is test/lint infrastructure with no runtime behavior.

## Deviations

- ESLint installed as v10 (latest) rather than v9 — still uses flat config, fully compatible with typescript-eslint v8
- Removed stale `@ts-ignore` comment on shareBtn entirely rather than converting to `@ts-expect-error` — the TS error it suppressed no longer exists, so `@ts-expect-error` itself became a TS error

## Known Issues

None.

## Files Created/Modified

- `viewer/eslint.config.js` — ESLint v9 flat config with typescript-eslint recommended + project overrides
- `viewer/vitest.config.ts` — Vitest config extending existing vite.config.ts
- `viewer/package.json` — added test/lint scripts and devDependencies
- `viewer/src/__tests__/viewport.test.ts` — 14 viewport unit tests
- `viewer/src/__tests__/hit-test.test.ts` — 7 hit-test unit tests
- `viewer/src/__tests__/undo.test.ts` — 10 undo stack unit tests
- `viewer/src/__tests__/url-state.test.ts` — 9 url-state unit tests
- `viewer/src/editor/lsp-bridge.ts` — removed unused `import type * as monaco` 
- `viewer/src/main.ts` — fixed prefer-const, unused catch var, removed stale @ts-ignore
